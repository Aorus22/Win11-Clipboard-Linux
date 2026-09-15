//! Window chrome for sessions that hand us none.
//!
//! The settings and wizard windows are ordinary app windows, but GPUI asks for
//! client-side decorations: on a compositor that answers with
//! `Decorations::Client`, nobody paints a title bar, so the window could not be
//! moved, minimised or closed at all. When [`needs_client_chrome`] says the WM
//! is not decorating us, this strip stands in for it — drag anywhere, buttons on
//! the right — and when the WM does decorate us (`Decorations::Server`) nothing
//! here is rendered, so the two can never both appear.

use gpui::{ClickEvent, Decorations, IntoElement, MouseButton, App, Window, div, prelude::*, px};

use super::icons::{self, icon};
use crate::theme;

/// True when the app has to draw its own title bar.
pub fn needs_client_chrome(window: &Window) -> bool {
    matches!(window.window_decorations(), Decorations::Client { .. })
}

/// 36px title bar: draggable strip, minimise / maximise / close on the right.
pub fn render_titlebar(is_dark: bool) -> gpui::AnyElement {
    let glyph = if is_dark {
        theme::gray::g400()
    } else {
        theme::gray::g500()
    };
    div()
        .id("titlebar")
        .w_full()
        .h(px(36.))
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(px(2.))
        .px(px(6.))
        .flex_shrink_0()
        .bg(if is_dark {
            theme::dark::bg_primary()
        } else {
            theme::tint::settings_light_bg()
        })
        .cursor_grab()
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.start_window_move()
        })
        .child(control(
            "titlebar-minimize",
            is_dark,
            false,
            |_, window, _| window.minimize_window(),
            div().w(px(10.)).h(px(1.)).rounded_full().bg(glyph),
        ))
        .child(control(
            "titlebar-maximize",
            is_dark,
            false,
            |_, window, _| window.zoom_window(),
            div()
                .w(px(10.))
                .h(px(10.))
                .rounded(px(2.))
                .border_1()
                .border_color(glyph),
        ))
        .child(control(
            "titlebar-close",
            is_dark,
            true,
            |_, window, _| window.remove_window(),
            icon(icons::X, px(14.)).flex_shrink_0(),
        ))
        .into_any_element()
}

/// One window control. The press stops here: a bubble to the strip would be read
/// by the compositor as the start of a move and swallow the click.
fn control(
    id: &'static str,
    is_dark: bool,
    close: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    glyph: impl IntoElement,
) -> gpui::AnyElement {
    let button = div()
        .id(id)
        .w(px(36.))
        .h(px(28.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(if is_dark {
            theme::gray::g400()
        } else {
            theme::gray::g500()
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(glyph);
    let button = if close {
        // Windows 11 close-hover red.
        button.hover(|s| s.bg(gpui::rgb(0xc42b1c)).text_color(gpui::rgb(0xffffff)))
    } else if is_dark {
        button.hover(|s| s.bg(theme::white_pct(0.10)))
    } else {
        button.hover(|s| s.bg(theme::black_pct(0.06)))
    };
    button.into_any_element()
}
