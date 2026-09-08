//! History card — port of `HistoryItem/` (index, _HistoryItemContent, _HistoryItemUtils).
//!
//! Layout: `flex gap-3` row — 32px icon box (24px compact), flexible text column,
//! hover-reveal action buttons. Pinned cards get the accent ring + corner dot.

use std::sync::Arc;

use gpui::{Context, InteractiveElement, Window, div, img, prelude::*, px};

use super::icons::{self, icon};
use super::popup::Popup;
use crate::history::{ClipboardContent, ClipboardItem, SmartAction, detect_smart_actions, relative_time};
use crate::theme;
use chrono::Utc;

pub struct CardParams {
    pub index: usize,
    pub focused: bool,
}

pub fn render_history_item(
    item: &ClipboardItem,
    params: &CardParams,
    state: &Popup,
    window: &Window,
    cx: &mut Context<Popup>,
) -> impl IntoElement {
    let is_dark = state.is_dark;
    let secondary = state.settings.secondary_opacity(is_dark);
    let compact = state.compact && state.settings.enable_ui_polish;
    let group = format!("card-{}", item.id);
    let item_id = item.id.clone();
    let highlighted = params.focused || item.pinned;

    let (border, card_bg, hover_bg) = if is_dark {
        (
            theme::dark::border_subtle(),
            theme::dark::card(secondary),
            theme::dark::bg_card_hover(),
        )
    } else {
        (
            theme::light::border(),
            theme::light::card(secondary),
            theme::light::bg_card_hover(),
        )
    };

    div()
        .id(("history-item", params.index))
        .group(group.clone())
        .relative()
        .p(if compact { px(8.) } else { px(12.) })
        .rounded(px(8.))
        .cursor_pointer()
        .bg(card_bg)
        .border_1()
        .border_color(if highlighted {
            theme::accent()
        } else {
            border
        })
        .hover(move |s| s.bg(hover_bg))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.paste_item_by_id(&item_id, window, cx);
        }))
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(12.))
                .child(render_icon_box(item, state, compact))
                .child(render_content(item, state, compact, window))
                .child(render_actions(item, params.index, state, &group, cx)),
        )
        .children(item.pinned.then(|| {
            div()
                .absolute()
                .top(px(-4.))
                .right(px(-4.))
                .w(px(8.))
                .h(px(8.))
                .rounded_full()
                .bg(theme::accent())
        }))
}

fn render_icon_box(item: &ClipboardItem, state: &Popup, compact: bool) -> impl IntoElement {
    let is_dark = state.is_dark;
    let tertiary = state.settings.tertiary_opacity(is_dark);
    let actions = if state.settings.enable_smart_actions {
        detect_smart_actions(item)
    } else {
        Vec::new()
    };
    let color_preview = actions.iter().find(|a| a.id == "color-preview");
    let box_px = if compact { 24. } else { 32. };
    let icon_px = if compact { 12. } else { 16. };
    let icon_color = if is_dark {
        theme::dark::text_secondary()
    } else {
        theme::light::text_secondary()
    };
    let is_text = matches!(
        item.content,
        ClipboardContent::Text(_) | ClipboardContent::RichText { .. }
    );
    div()
        .flex_shrink_0()
        .w(px(box_px))
        .h(px(box_px))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .bg(color_preview
            .map(|a| parse_color(&a.data))
            .unwrap_or_else(|| theme::tertiary_bg(is_dark, tertiary)))
        .children(color_preview.is_none().then(|| {
            icon(
                if is_text { icons::TYPE_TEXT } else { icons::IMAGE },
                px(icon_px),
            )
            .text_color(icon_color)
        }))
}

/// Parse `#rgb` / `#rrggbb` / `rgb(r, g, b)` preview colors (same inputs the
/// detector accepts, so parsing cannot fail — defensive fallback included).
fn parse_color(data: &str) -> gpui::Rgba {
    let t = data.trim();
    if let Some(hex) = t.strip_prefix('#') {
        let expanded = if hex.len() == 3 {
            hex.chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            hex.to_string()
        };
        if expanded.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&expanded[0..2], 16),
                u8::from_str_radix(&expanded[2..4], 16),
                u8::from_str_radix(&expanded[4..6], 16),
            ) {
                return gpui::rgb((r as u32) << 16 | (g as u32) << 8 | b as u32);
            }
        }
    }
    if t.to_lowercase().starts_with("rgb(") && t.ends_with(')') {
        let nums: Vec<u8> = t[4..t.len() - 1]
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        if nums.len() == 3 {
            return gpui::rgb((nums[0] as u32) << 16 | (nums[1] as u32) << 8 | nums[2] as u32);
        }
    }
    gpui::rgb(0x888888)
}

fn render_content(
    item: &ClipboardItem,
    state: &Popup,
    compact: bool,
    _window: &Window,
) -> impl IntoElement {
    let is_dark = state.is_dark;
    let text_color = if is_dark {
        theme::dark::text_primary()
    } else {
        theme::light::text_primary()
    };
    let time_color = if is_dark {
        theme::dark::text_tertiary()
    } else {
        theme::light::text_secondary()
    };
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .children(match &item.content {
            ClipboardContent::Text(text) => Some(render_text(text, text_color, compact)),
            ClipboardContent::RichText { plain, .. } => {
                Some(render_text(plain, text_color, compact))
            }
            ClipboardContent::Image { .. } => None,
        })
        .children(match &item.content {
            ClipboardContent::Image { base64, width, height } => {
                Some(render_image(base64, *width, *height, compact, time_color))
            }
            _ => None,
        })
        .children((!compact).then(|| {
            div()
                .mt(px(4.))
                .text_size(px(10.5))
                .text_color(time_color)
                .child(relative_time(&item.timestamp, &Utc::now()))
        }))
}

fn render_text(text: &str, color: gpui::Rgba, compact: bool) -> impl IntoElement {
    div()
        .text_size(px(12.25))
        .text_color(color)
        .max_h(px(if compact { 18. } else { 52. }))
        .overflow_hidden()
        .child(text.to_string())
}

fn render_image(
    base64_data: &str,
    width: u32,
    height: u32,
    compact: bool,
    dim: gpui::Rgba,
) -> impl IntoElement {
    if compact {
        return div()
            .text_size(px(12.25))
            .text_color(dim)
            .child(format!("Image ({width}×{height})"));
    }
    let thumb = decode_thumb(base64_data);
    let has_thumb = thumb.is_some();
    div().relative().child(
        div()
            .max_w_full()
            .max_h(px(96.))
            .rounded(px(4.))
            .overflow_hidden()
            .bg(gpui::rgba(0x0000001a))
            .children(thumb.map(|image| {
                img(gpui::ImageSource::Image(Arc::new(image)))
                    .max_w_full()
                    .max_h(px(96.))
                    .object_fit(gpui::ObjectFit::Contain)
            }))
            .children((!has_thumb).then(|| {
                div()
                    .text_size(px(12.25))
                    .text_color(dim)
                    .child(format!("Image ({width}×{height})"))
            })),
    )
    .child(
        div()
            .absolute()
            .bottom(px(4.))
            .right(px(4.))
            .px(px(6.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(gpui::rgba(0x00000099))
            .text_size(px(10.5))
            .text_color(gpui::rgb(0xffffff))
            .child(format!("{width}×{height}")),
    )
}

fn decode_thumb(base64_data: &str) -> Option<gpui::Image> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .ok()?;
    // Keep whatever PNG bytes the backend stored; the renderer decodes them.
    Some(gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes))
}

fn render_actions(
    item: &ClipboardItem,
    index: usize,
    state: &Popup,
    group: &str,
    cx: &mut Context<Popup>,
) -> impl IntoElement {
    let is_dark = state.is_dark;
    let tertiary = state.settings.tertiary_opacity(is_dark);
    let hover_bg = theme::tertiary_bg(is_dark, tertiary);
    let idle = if is_dark {
        theme::dark::text_tertiary()
    } else {
        theme::light::text_secondary()
    };
    let actions: Vec<SmartAction> = if state.settings.enable_smart_actions {
        detect_smart_actions(item)
    } else {
        Vec::new()
    };
    let link = actions.iter().find(|a| a.id == "open-link").cloned();
    let mail = actions.iter().find(|a| a.id == "compose-email").cloned();
    let pin_id = item.id.clone();
    let del_id = item.id.clone();
    let pinned = item.pinned;
    // Only the delete button turns red on hover (parity with HistoryItem).
    let danger = gpui::rgb(0xff5f5f);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .opacity(0.0)
        .group_hover(group.to_string(), |s| s.opacity(1.0))
        .children(link.map(|action| {
            action_button(("act-link", index), icons::LINK, idle, hover_bg, idle, cx, move |this, _, _, cx| {
                this.open_smart_url(&action.data);
                cx.notify();
            })
        }))
        .children(mail.map(|action| {
            action_button(("act-mail", index), icons::MAIL, idle, hover_bg, idle, cx, move |this, _, _, cx| {
                this.open_smart_url(&action.data);
                cx.notify();
            })
        }))
        .child(action_button(
            ("act-pin", index),
            if pinned { icons::PIN_FILLED } else { icons::PIN },
            if pinned { theme::accent() } else { idle },
            hover_bg,
            if pinned { theme::accent() } else { idle },
            cx,
            move |this, _, _, cx| {
                this.backend.toggle_pin(&pin_id);
                this.refresh_items();
                cx.notify();
            },
        ))
        .child(action_button(
            ("act-delete", index),
            icons::X,
            idle,
            hover_bg,
            danger,
            cx,
            move |this, _, _, cx| {
                this.backend.remove_item(&del_id);
                this.refresh_items();
                this.after_filter_change();
                cx.notify();
            },
        ))
}

fn action_button(
    id: (&'static str, usize),
    icon_name: &str,
    color: gpui::Rgba,
    hover_bg: gpui::Rgba,
    hover_color: gpui::Rgba,
    cx: &mut Context<Popup>,
    on_click: impl Fn(&mut Popup, &gpui::ClickEvent, &mut Window, &mut Context<Popup>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .p(px(6.))
        .rounded(px(6.))
        .cursor_pointer()
        .text_color(color)
        .hover(move |s| s.bg(hover_bg).text_color(hover_color))
        .on_click(cx.listener(on_click))
        .child(icon(icon_name, px(16.)).flex_shrink_0())
}
