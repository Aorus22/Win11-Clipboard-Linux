//! Hand-rolled vertical scrollbar for the popup's scroll areas.
//!
//! gpui 0.2.2 ships no scrollbar element, and the popup has to stay scrollable
//! without a wheel, so the bar is built from the scroll handle itself:
//! `ScrollHandle` exposes the viewport bounds, the current offset and the
//! maximum offset, and takes `set_offset` back — which is everything a bar
//! needs.
//!
//! The bar is an overlay *sibling* of the scroll area, never a child (a child
//! would scroll away with the content), so it is placed by the `relative`
//! wrapper [`with_scrollbar`] creates around the scroll area.
//!
//! Drag state lives in a thread-local: GPUI dispatches pointer events on the UI
//! thread and only one bar can be dragged at a time.

use std::cell::RefCell;

use gpui::{
    App, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollHandle, Window, div,
    point, prelude::*, px,
};

/// Visual width of the thumb and its corner radius — React's `.scrollbar-win11`
/// uses a 6 px bar with `border-radius: 3px` in both themes.
const THUMB_W: f32 = 6.0;
const THUMB_RADIUS: f32 = 3.0;
/// Grabbable strip: wider than the thumb so a vertical drag does not slip off
/// the bar sideways. Kept narrow so it only clips the content's own margin —
/// the history cards' hover actions sit further in.
const HIT_W: f32 = 12.0;
/// Thumb floor, so a very long list still leaves something to grab.
const MIN_THUMB_H: f32 = 28.0;

#[derive(Clone, Copy)]
struct Drag {
    id: &'static str,
    /// Where inside the thumb the pointer grabbed it.
    grab: f32,
}

thread_local! {
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
}

/// The thumb's geometry for one frame, derived from a scroll handle.
#[derive(Clone, Copy)]
struct Metrics {
    /// Top of the scroll area in window coordinates (pointer math).
    track_top: f32,
    viewport_h: f32,
    thumb_h: f32,
    thumb_top: f32,
    /// Pixels the thumb can travel (viewport minus thumb).
    travel: f32,
    /// Maximum scroll offset, always ≥ 0; 0 means "nothing to scroll".
    max: f32,
}

impl Metrics {
    fn of(handle: &ScrollHandle) -> Self {
        let viewport_h = f32::from(handle.bounds().size.height);
        let max = f32::from(handle.max_offset().height).max(0.0);
        // The thumb is the viewport's share of the whole content, so the bar
        // reads as a map of the list.
        let thumb_h = if viewport_h > 0.0 && max > 0.0 {
            (viewport_h * viewport_h / (viewport_h + max)).clamp(MIN_THUMB_H, viewport_h)
        } else {
            viewport_h
        };
        let travel = (viewport_h - thumb_h).max(0.0);
        let scrolled = (-f32::from(handle.offset().y)).clamp(0.0, max);
        Self {
            track_top: f32::from(handle.bounds().top()),
            viewport_h,
            thumb_h,
            travel,
            max,
            thumb_top: if max > 0.0 {
                scrolled / max * travel
            } else {
                0.0
            },
        }
    }

    fn scrollable(&self) -> bool {
        self.max > 1.0 && self.travel > 0.0
    }

    /// Move the scroll offset so the thumb's grab point sits at `pointer_y`.
    fn scrub(&self, handle: &ScrollHandle, pointer_y: f32, grab: f32) {
        if !self.scrollable() {
            return;
        }
        let frac = ((pointer_y - self.track_top - grab) / self.travel).clamp(0.0, 1.0);
        handle.set_offset(point(px(0.), px(-frac * self.max)));
    }
}

/// Wrap a scroll area so it gets a vertical scrollbar on its right edge.
///
/// `repaint` re-renders the owning view after a scroll: the thumb is a plain
/// element, so it only moves when the view renders again.
pub fn with_scrollbar(
    id: &'static str,
    handle: &ScrollHandle,
    is_dark: bool,
    content: impl IntoElement,
    repaint: impl Fn(&mut Window, &mut App) + Clone + 'static,
) -> impl IntoElement {
    let m = Metrics::of(handle);
    let dragging = DRAG.with(|drag| drag.borrow().is_some_and(|drag| drag.id == id));
    // Same alphas as `.scrollbar-win11`: 0.2 at rest, 0.35 while hovered or
    // dragged (the React build lifts the thumb on `:hover` only, but a grabbed
    // bar should read as hovered too).
    let (thumb_idle, thumb_active) = if is_dark {
        (gpui::rgba(0xffffff33), gpui::rgba(0xffffff59))
    } else {
        (gpui::rgba(0x00000033), gpui::rgba(0x00000059))
    };
    let thumb_color = if dragging { thumb_active } else { thumb_idle };

    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .child(content)
        .children(m.scrollable().then(|| {
            let bar = handle.clone();
            let on_down = {
                let bar = bar.clone();
                let repaint = repaint.clone();
                move |event: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                    let y = f32::from(event.position.y);
                    let in_thumb =
                        y >= m.track_top + m.thumb_top && y <= m.track_top + m.thumb_top + m.thumb_h;
                    // Grabbing the thumb keeps the pointer where it was inside
                    // it; clicking the track centers the thumb on the pointer.
                    let grab = if in_thumb {
                        y - (m.track_top + m.thumb_top)
                    } else {
                        m.thumb_h * 0.5
                    };
                    DRAG.with(|drag| *drag.borrow_mut() = Some(Drag { id, grab }));
                    m.scrub(&bar, y, grab);
                    repaint(window, cx);
                }
            };
            let on_move = {
                let bar = bar.clone();
                let repaint = repaint.clone();
                move |event: &MouseMoveEvent, window: &mut Window, cx: &mut App| {
                    let grab = DRAG.with(|drag| {
                        drag.borrow().filter(|drag| drag.id == id).map(|drag| drag.grab)
                    });
                    let Some(grab) = grab else {
                        return;
                    };
                    m.scrub(&bar, f32::from(event.position.y), grab);
                    repaint(window, cx);
                }
            };
            let on_up = {
                let repaint = repaint.clone();
                move |_: &MouseUpEvent, window: &mut Window, cx: &mut App| {
                    if DRAG.with(|drag| drag.borrow_mut().take().is_some()) {
                        repaint(window, cx);
                    }
                }
            };
            div()
                .id(id)
                .absolute()
                .top(px(0.))
                .bottom(px(0.))
                .right(px(1.))
                .w(px(HIT_W))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, on_down)
                .on_mouse_move(on_move)
                .on_mouse_up(MouseButton::Left, on_up.clone())
                .on_mouse_up_out(MouseButton::Left, on_up)
                .child(
                    div()
                        .absolute()
                        .top(px(m.thumb_top))
                        .right(px(0.))
                        .h(px(m.thumb_h))
                        .w(px(THUMB_W))
                        .rounded(px(THUMB_RADIUS))
                        .bg(thumb_color)
                        .hover(move |style| style.bg(thumb_active)),
                )
                .into_any_element()
        }))
}
