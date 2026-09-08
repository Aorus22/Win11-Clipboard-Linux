//! Settings controls — Switch, Slider (click + drag + arrows), TextField.
//!
//! Slider drag mapping couples to the settings layout (documented): the window is a
//! fixed 480px (parity: Tauri `resizable: false`), main content `px-8` (32px) and
//! section `p-6` (24px), so every slider track starts at x=56 with width 368.
//! Only the X axis matters, so vertical scroll doesn't affect the mapping.

use gpui::{
    ClickEvent, Context, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Window, div, prelude::*, px,
};

use super::search::edit_text;
use super::settings::SettingsState;
use crate::theme;

/// Slider identifiers (dragging + focus routing). Defined here (not settings.rs)
// to avoid a module cycle: settings.rs imports controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderId {
    DarkOpacity,
    LightOpacity,
    UiScale,
}

pub const SLIDER_X0: f32 = 56.0;

/// Track width derives from the live window width (content px-8 + section p-6 on
/// each side). Resize-safe; the Tauri window is fixed 480px but GPUI can't lock that.
pub fn slider_track_w(win_w: f32) -> f32 {
    (win_w - 2.0 * SLIDER_X0).max(50.0)
}

pub fn slider_value_from_x(x: f32, win_w: f32, min: f32, max: f32, step: f32) -> f32 {
    let frac = ((x - SLIDER_X0) / slider_track_w(win_w)).clamp(0.0, 1.0);
    let raw = min + frac * (max - min);
    ((raw / step).round() * step).clamp(min, max)
}

// --- Switch (port of Switch.tsx: w-11 h-6 pill + 16px knob) ---

pub fn switch(
    id: (&'static str, usize),
    checked: bool,
    is_dark: bool,
    cx: &mut Context<SettingsState>,
    on_toggle: impl Fn(&mut SettingsState, &ClickEvent, &mut Window, &mut Context<SettingsState>)
    + 'static,
) -> gpui::AnyElement {
    div()
        .id(id)
        .w(px(44.))
        .h(px(24.))
        .rounded_full()
        .flex()
        .flex_row()
        .items_center()
        .cursor_pointer()
        .bg(if checked {
            theme::accent()
        } else if is_dark {
            gpui::rgba(0xffffff1a)
        } else {
            gpui::rgb(0xd1d5db)
        })
        .child(
            div()
                .w(px(16.))
                .h(px(16.))
                .rounded_full()
                .bg(gpui::rgb(0xffffff))
                .ml(px(if checked { 24. } else { 4. })),
        )
        .on_click(cx.listener(on_toggle))
        .into_any_element()
}

// --- Slider ---

pub fn slider(
    id: SliderId,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    disabled: bool,
    dragging: bool,
    focused: bool,
    focus: &FocusHandle,
    is_dark: bool,
    window: &Window,
    cx: &mut Context<SettingsState>,
    on_change: impl Fn(&mut SettingsState, f32, &mut Context<SettingsState>) + 'static,
    on_commit: impl Fn(&mut SettingsState, &mut Context<SettingsState>) + 'static,
) -> gpui::AnyElement {
    let frac = ((value - min) / (max - min)).clamp(0.0, 1.0);
    let win_w = f32::from(window.bounds().size.width);
    let knob_x = frac * slider_track_w(win_w);
    // Shared by the down + move handlers (each `move` closure needs its own handle).
    let on_change = std::sync::Arc::new(on_change);
    let track_bg = if is_dark {
        gpui::rgb(0x374151)
    } else {
        gpui::rgb(0xe5e7eb)
    };
    div()
        .id(("slider", id as usize))
        .h(px(16.))
        .w_full()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .track_focus(focus)
        .border_2()
        .border_color(if focused {
            theme::accent()
        } else {
            gpui::rgba(0x00000000)
        })
        .cursor_pointer()
        .opacity(if disabled { 0.5 } else { 1.0 })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener({
                let on_change = std::sync::Arc::clone(&on_change);
                move |this, event: &MouseDownEvent, window, cx| {
                    if disabled {
                        return;
                    }
                    this.dragging = Some(id);
                    let w = f32::from(window.bounds().size.width);
                    let v = slider_value_from_x(f32::from(event.position.x), w, min, max, step);
                    on_change(this, v, cx);
                    cx.notify();
                }
            }),
        )
        .on_mouse_move(cx.listener({
            let on_change = std::sync::Arc::clone(&on_change);
            move |this, event: &MouseMoveEvent, window, cx| {
                if this.dragging != Some(id) || disabled {
                    return;
                }
                let w = f32::from(window.bounds().size.width);
                let v = slider_value_from_x(f32::from(event.position.x), w, min, max, step);
                on_change(this, v, cx);
                cx.notify();
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                if this.dragging == Some(id) {
                    this.dragging = None;
                    on_commit(this, cx);
                    cx.notify();
                }
            }),
        )
        .child(
            div()
                .absolute()
                .left(px(0.))
                .right(px(0.))
                .h(px(6.))
                .rounded_full()
                .bg(track_bg),
        )
        .child(
            div()
                .absolute()
                .left(px(0.))
                .h(px(6.))
                .w(px(knob_x.max(6.)))
                .rounded_full()
                .bg(theme::accent()),
        )
        .child(
            div()
                .absolute()
                .left(px((knob_x - 8.).max(0.)))
                .w(px(16.))
                .h(px(16.))
                .rounded_full()
                .bg(gpui::rgb(0xffffff)),
        )
        .into_any_element()
}

// --- TextField (single-line editor, settings input styling) ---

pub struct TextField {
    pub text: String,
    pub cursor: usize,
    pub focus: FocusHandle,
}

impl TextField {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            focus,
        }
    }

    pub fn with(text: String, focus: FocusHandle) -> Self {
        let cursor = text.len();
        Self {
            text,
            cursor,
            focus,
        }
    }

    pub fn handle_key(&mut self, event: &KeyDownEvent) -> bool {
        edit_text(&mut self.text, &mut self.cursor, event)
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn render(
        &self,
        id: &'static str,
        placeholder: &str,
        is_dark: bool,
        window: &Window,
        cx: &mut Context<SettingsState>,
        select: fn(&mut SettingsState) -> &mut TextField,
    ) -> gpui::AnyElement {
        let (text_color, placeholder_color) = if is_dark {
            (theme::dark::text_primary(), theme::dark::text_disabled())
        } else {
            (theme::light::text_primary(), theme::dark::text_disabled())
        };
        let cursor = self.cursor.min(self.text.len());
        let (before, after) = self.text.split_at(cursor);
        div()
            .id(id)
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(6.))
            .border_1()
            .border_color(if is_dark {
                gpui::rgba(0xffffff1a)
            } else {
                gpui::rgb(0xe5e7eb)
            })
            .bg(if is_dark {
                gpui::rgba(0xffffff0d)
            } else {
                gpui::rgb(0xf9fafb)
            })
            .text_size(px(12.25))
            .text_color(text_color)
            .track_focus(&self.focus)
            .on_click(cx.listener(move |this, _, window: &mut Window, cx| {
                select(this).focus.focus(window);
                cx.notify();
            }))
            .children(self.text.is_empty().then(|| {
                div()
                    .flex_1()
                    .text_size(px(12.25))
                    .text_color(placeholder_color)
                    .child(placeholder.to_string())
            }))
            .children((!self.text.is_empty()).then(|| {
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .overflow_hidden()
                    .child(before.to_string())
                    .child(div().w(px(1.5)).h(px(15.)).bg(theme::accent()))
                    .child(after.to_string())
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_mapping_matches_layout_math() {
        // 480px window → track x0=56, width=368 (same numbers as before).
        assert!((slider_value_from_x(56.0, 480.0, 0.0, 1.0, 0.01) - 0.0).abs() < 1e-6);
        assert!((slider_value_from_x(56.0 + 368.0, 480.0, 0.0, 1.0, 0.01) - 1.0).abs() < 1e-6);
        assert!((slider_value_from_x(56.0 + 184.0, 480.0, 0.0, 1.0, 0.01) - 0.5).abs() < 1e-6);
        // Stepping + clamping.
        assert!((slider_value_from_x(0.0, 480.0, 0.5, 2.0, 0.1) - 0.5).abs() < 1e-6);
        assert!((slider_value_from_x(9999.0, 480.0, 0.5, 2.0, 0.1) - 2.0).abs() < 1e-6);
        // frac=1/3 → raw=1.0 → stepped 1.0 (chosen to avoid f32 half-way cases).
        assert!((slider_value_from_x(56.0 + 368.0 / 3.0, 480.0, 0.5, 2.0, 0.1) - 1.0).abs() < 1e-4);
        // Resize-safe: wider window widens the track proportionally.
        assert!((slider_track_w(600.0) - 488.0).abs() < 1e-6);
    }
}
