//! Header — port of `Header.tsx`: title + count pill + compact toggle + Clear All.

use gpui::{Context, Window, div, prelude::*, px};

use super::icons::{self, icon};
use super::popup::Popup;
use crate::theme;

pub fn render_header(state: &Popup, count: usize, _window: &Window, cx: &mut Context<Popup>) -> impl IntoElement {
    let is_dark = state.is_dark;
    let tertiary = state.settings.tertiary_opacity(is_dark);
    let (title_color, btn_color) = if is_dark {
        (
            theme::dark::text_primary(),
            theme::dark::text_secondary(),
        )
    } else {
        (
            theme::light::text_primary(),
            theme::light::text_secondary(),
        )
    };
    div()
        .id("header")
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(px(16.))
        .py(px(12.))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.25))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(title_color)
                        .child("Clipboard"),
                )
                .children((count > 0).then(|| {
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded_full()
                        .bg(theme::tertiary_bg(is_dark, tertiary))
                        .text_size(px(10.5))
                        .text_color(btn_color)
                        .child(format!("{count}"))
                })),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(
                    div()
                        .id("compact-toggle")
                        .p(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_color(btn_color)
                        .hover(|s| s.bg(theme::tertiary_bg(is_dark, tertiary)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.compact = !this.compact;
                            this.save_ui_state();
                            cx.notify();
                        }))
                        .child(
                            icon(icons::LAYOUT_LIST, px(16.))
                                .flex_shrink_0()
                                .opacity(if state.compact { 1.0 } else { 0.5 }),
                        ),
                )
                .child(
                    div()
                        .id("clear-all")
                        .p(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_color(btn_color)
                        .text_size(px(10.5))
                        .hover(|s| s.bg(theme::tertiary_bg(is_dark, tertiary)))
                        .opacity(if count == 0 { 0.5 } else { 1.0 })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.backend.clear();
                            this.refresh_items();
                            this.focused = 0;
                            cx.notify();
                        }))
                        .child("Clear All"),
                ),
        )
}
