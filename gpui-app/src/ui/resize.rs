//! Window resize handles for the client-side frame of the Settings window.
//!
//! The window is a transparent margin ([`theme::WINDOW_SHADOW_MARGIN`]) around
//! the visible card, and gpui paints no frame of its own, so the app has to
//! start the compositor resize itself. The handles are driven by window-level
//! pointer events (one canvas layer) instead of per-edge elements, so no widget
//! painted above the margin can swallow them: the cursor follows the pointer and
//! a press starts a compositor resize.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    CursorStyle, Decorations, DispatchPhase, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, ResizeEdge, Tiling, Window, canvas, prelude::*,
};

use crate::theme;

/// Width of the grab area along each edge, from the window's outer edge: the
/// transparent shadow margin plus a sliver of the visible frame.
const RESIZE_EDGE: f32 = theme::WINDOW_SHADOW_MARGIN + 3.0;
/// Corners are easier to hit with a larger square.
const RESIZE_CORNER: f32 = theme::WINDOW_SHADOW_MARGIN + 12.0;

/// Edge of the window the pointer is over, shared with the render so the layer
/// can tell when the hover changed.
pub type ResizeHover = Rc<Cell<Option<ResizeEdge>>>;

/// True when the window floats with its rounded frame and shadow margin. A
/// maximized, fullscreen or tiled window is square and full-bleed instead.
pub fn is_framed(window: &Window) -> bool {
    !window.is_maximized()
        && !window.is_fullscreen()
        && !matches!(
            window.window_decorations(),
            Decorations::Client { tiling } if tiling.is_tiled()
        )
}

/// The transparent margin currently around the card: the full shadow margin
/// while framed, nothing otherwise. Anything that maps window coordinates to
/// card coordinates (the sliders) has to use this and not the constant.
pub fn frame_margin(window: &Window) -> f32 {
    if is_framed(window) {
        theme::WINDOW_SHADOW_MARGIN
    } else {
        0.0
    }
}

/// Which resize handle (if any) the point `(x, y)` falls on in a window of
/// `width` x `height`. Edges the compositor reports as tiled cannot be resized.
fn resize_edge_at(x: f32, y: f32, width: f32, height: f32, tiling: Tiling) -> Option<ResizeEdge> {
    let near = |distance: f32, limit: f32| distance >= 0.0 && distance < limit;
    let left = !tiling.left && near(x, RESIZE_EDGE);
    let right = !tiling.right && near(width - x, RESIZE_EDGE);
    let top = !tiling.top && near(y, RESIZE_EDGE);
    let bottom = !tiling.bottom && near(height - y, RESIZE_EDGE);
    let corner_x = |from_left: bool| {
        if from_left {
            near(x, RESIZE_CORNER)
        } else {
            near(width - x, RESIZE_CORNER)
        }
    };
    let corner_y = |from_top: bool| {
        if from_top {
            near(y, RESIZE_CORNER)
        } else {
            near(height - y, RESIZE_CORNER)
        }
    };
    if !tiling.top && !tiling.left && corner_x(true) && corner_y(true) && (left || top) {
        return Some(ResizeEdge::TopLeft);
    }
    if !tiling.top && !tiling.right && corner_x(false) && corner_y(true) && (right || top) {
        return Some(ResizeEdge::TopRight);
    }
    if !tiling.bottom && !tiling.left && corner_x(true) && corner_y(false) && (left || bottom) {
        return Some(ResizeEdge::BottomLeft);
    }
    if !tiling.bottom && !tiling.right && corner_x(false) && corner_y(false) && (right || bottom) {
        return Some(ResizeEdge::BottomRight);
    }
    if top {
        Some(ResizeEdge::Top)
    } else if bottom {
        Some(ResizeEdge::Bottom)
    } else if left {
        Some(ResizeEdge::Left)
    } else if right {
        Some(ResizeEdge::Right)
    } else {
        None
    }
}

fn cursor_for_edge(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

/// The handle layer. Place it last in the outer window box so it covers the
/// true window edges; it paints nothing and only listens to the pointer.
pub fn resize_layer(tiling: Tiling, hover: ResizeHover) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let size = bounds.size;
            let edge_at = move |position: Point<Pixels>| {
                let local = position - bounds.origin;
                resize_edge_at(
                    f32::from(local.x),
                    f32::from(local.y),
                    f32::from(size.width),
                    f32::from(size.height),
                    tiling,
                )
            };
            if let Some(edge) = edge_at(window.mouse_position()) {
                window.set_window_cursor_style(cursor_for_edge(edge));
            }
            window.on_mouse_event({
                let hover = hover.clone();
                move |event: &MouseMoveEvent, _, window, _| {
                    let edge = edge_at(event.position);
                    if hover.replace(edge) != edge {
                        window.refresh();
                    }
                }
            });
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    if let Some(edge) = edge_at(event.position) {
                        window.start_window_resize(edge);
                        cx.stop_propagation();
                    }
                }
            });
        },
    )
    // Pinned to the top-left: an absolute child without insets would sit at
    // its static position, i.e. below the window content.
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;
    // `use gpui::*`-style imports can shadow the std `test` attribute.
    use core::prelude::v1::test;

    const W: f32 = 512.0;
    const H: f32 = 552.0;

    fn edge(x: f32, y: f32) -> Option<ResizeEdge> {
        resize_edge_at(x, y, W, H, Tiling::default())
    }

    #[test]
    fn shadow_margin_resizes_from_every_side() {
        assert_eq!(edge(4.0, H / 2.0), Some(ResizeEdge::Left));
        assert_eq!(edge(W - 2.0, 300.0), Some(ResizeEdge::Right));
        assert_eq!(edge(250.0, 5.0), Some(ResizeEdge::Top));
        assert_eq!(edge(250.0, H - 3.0), Some(ResizeEdge::Bottom));
    }

    #[test]
    fn corners_win_over_edges() {
        assert_eq!(edge(3.0, 3.0), Some(ResizeEdge::TopLeft));
        assert_eq!(edge(W - 3.0, 20.0), Some(ResizeEdge::TopRight));
        assert_eq!(edge(10.0, H - 3.0), Some(ResizeEdge::BottomLeft));
        assert_eq!(edge(W - 25.0, H - 4.0), Some(ResizeEdge::BottomRight));
    }

    #[test]
    fn the_window_body_is_not_a_handle() {
        assert_eq!(edge(W / 2.0, H / 2.0), None);
        assert_eq!(edge(60.0, 400.0), None);
        assert_eq!(edge(250.0, 50.0), None);
    }

    #[test]
    fn tiled_edges_cannot_be_resized() {
        let mut tiled = Tiling::default();
        tiled.left = true;
        assert_eq!(resize_edge_at(4.0, 300.0, W, H, tiled), None);
        assert_eq!(resize_edge_at(W - 4.0, 300.0, W, H, tiled), Some(ResizeEdge::Right));
    }
}
