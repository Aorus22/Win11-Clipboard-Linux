//! Tab bar — port of `TabBar.tsx`.
//!
//! Four tabs (clipboard / symbols / emoji / kaomoji), arrow-key switching,
//! active + hover tertiary background, 1px bottom border.

use gpui::{Context, Window, div, prelude::*, px};

use super::icons::{self, icon};
use super::popup::{Popup, Tab};
use crate::theme;

const TABS: [(Tab, &str, &str); 4] = [
    (Tab::Clipboard, "Clipboard", icons::CLIPBOARD_LIST),
    (Tab::Symbols, "Symbols", icons::OMEGA),
    (Tab::Emoji, "Emoji", icons::SMILE),
    (Tab::Kaomoji, "Kaomoji", icons::TYPE_TEXT),
];

pub fn render_tabbar(state: &Popup, _window: &Window, cx: &mut Context<Popup>) -> impl IntoElement {
    let is_dark = state.is_dark;
    let tertiary = state.settings.tertiary_opacity(is_dark);
    let (text_active, text_idle, border) = if is_dark {
        (
            theme::dark::text_primary(),
            theme::dark::text_secondary(),
            theme::dark::border_subtle(),
        )
    } else {
        (
            theme::light::text_primary(),
            theme::light::text_secondary(),
            theme::light::border(),
        )
    };
    div()
        .id("tabbar")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .p(px(8.))
        .px(px(16.))
        .border_b_1()
        .border_color(border)
        .children(TABS.iter().enumerate().map(|(index, (tab, label, icon_name))| {
            let active = state.tab == *tab;
            let tab_id = *tab;
            div()
                .id(("tab", index))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .px(px(16.))
                .py(px(8.))
                .rounded(px(6.))
                .text_size(px(12.25))
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(if active { text_active } else { text_idle })
                .bg(if active {
                    theme::tertiary_bg(is_dark, tertiary)
                } else {
                    gpui::rgba(0x00000000)
                })
                .hover(|s| s.bg(theme::tertiary_bg(is_dark, tertiary)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tab = tab_id;
                    this.focused = 0;
                    cx.notify();
                }))
                .child(icon(icon_name, px(16.)).flex_shrink_0())
                .child(label.to_string())
        }))
}
