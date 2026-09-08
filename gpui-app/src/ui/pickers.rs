//! Picker tabs — port of `EmojiPicker.tsx`, `KaomojiPicker.tsx`, `SymbolPicker.tsx`
//! + `PickerLayout` / `CategoryStrip` / `CategoryPill` / `SectionHeader`.
//!
//! Layout notes: grid columns derive from the window width (deterministic, matches
//! React at the same width). Category pills wrap instead of horizontal-scroll
//! (documented delta — all categories visible without scrolling).

use gpui::{Context, Window, div, prelude::*, px};

use super::icons::{self, icon};
use super::popup::{Popup, SearchWhich, Tab};
use crate::pickers::{Emoji, KAOMOJI_CATEGORIES, SYMBOL_CATEGORIES, SymbolItem};
use crate::theme;

pub fn render_picker_tab(state: &Popup, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
    match state.tab {
        Tab::Emoji => render_emoji(state, window, cx),
        Tab::Kaomoji => render_kaomoji(state, window, cx),
        Tab::Symbols => render_symbol(state, window, cx),
        Tab::Clipboard => div().into_any_element(),
    }
}

fn win_w(window: &Window) -> f32 {
    f32::from(window.bounds().size.width)
}

/// Emoji/symbol columns: floor((width − 48) / 40) — 7 columns at 360px, as React.
pub(crate) fn grid_columns(window: &Window) -> usize {
    (((win_w(window) - 48.0) / 40.0).floor() as usize).max(1)
}

/// Kaomoji breakpoints: ≥768 → 4, ≥640 → 3, else 2 (React `columnCount` memo).
pub(crate) fn kaomoji_columns(window: &Window) -> usize {
    let w = win_w(window);
    if w >= 768.0 {
        4
    } else if w >= 640.0 {
        3
    } else {
        2
    }
}

fn secondary_text(is_dark: bool) -> gpui::Rgba {
    if is_dark {
        theme::dark::text_secondary()
    } else {
        theme::light::text_secondary()
    }
}

fn tertiary_text(is_dark: bool) -> gpui::Rgba {
    if is_dark {
        theme::dark::text_tertiary()
    } else {
        theme::light::text_secondary()
    }
}

fn hover_bg(is_dark: bool) -> gpui::Rgba {
    if is_dark {
        theme::dark::bg_card_hover()
    } else {
        theme::light::bg_card_hover()
    }
}

// --- Shared chrome ---

fn chrome(
    state: &Popup,
    header: gpui::AnyElement,
    sub: Option<gpui::AnyElement>,
    grid: gpui::AnyElement,
    footer: gpui::AnyElement,
) -> gpui::AnyElement {
    let is_dark = state.is_dark;
    div()
        .flex()
        .flex_col()
        .size_full()
        .overflow_hidden()
        .child(div().px(px(12.)).pt(px(12.)).pb(px(8.)).flex_shrink_0().child(header))
        .children(sub.map(|s| div().px(px(12.)).pb(px(8.)).flex_shrink_0().child(s)))
        .child(
            div()
                .flex_1()
                .min_h(px(0.))
                .overflow_hidden()
                .relative()
                .child(grid),
        )
        .child(
            div()
                .px(px(12.))
                .py(px(8.))
                .h(px(40.))
                .flex_shrink_0()
                .border_t_1()
                .border_color(if is_dark {
                    theme::dark::border_subtle()
                } else {
                    theme::light::border()
                })
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(footer),
        )
        .into_any_element()
}

fn footer_preview(
    is_dark: bool,
    hovered: Option<(String, String)>,
    hint: &str,
) -> gpui::AnyElement {
    match hovered {
        Some((glyph, name)) => div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(div().text_size(px(20.)).child(glyph))
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(secondary_text(is_dark))
                    .overflow_hidden()
                    .child(name),
            )
            .into_any_element(),
        None => div()
            .text_size(px(10.5))
            .text_color(tertiary_text(is_dark))
            .child(hint.to_string())
            .into_any_element(),
    }
}

fn empty_state(is_dark: bool, message: &str) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .h_full()
        .py(px(32.))
        .child(
            div()
                .text_size(px(12.25))
                .text_color(secondary_text(is_dark))
                .child(message.to_string()),
        )
        .into_any_element()
}

fn section_label(is_dark: bool, icon_name: &str, label: &str) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .mb(px(6.))
        .text_size(px(10.5))
        .text_color(tertiary_text(is_dark))
        .child(icon(icon_name, px(12.)).flex_shrink_0())
        .child(label.to_string())
        .into_any_element()
}

// --- Category pills ---

fn category_pills(
    state: &Popup,
    group: &'static str,
    categories: &[&str],
    selected: Option<&str>,
    has_custom: bool,
    on_select: impl Fn(&mut Popup, Option<String>, &mut Context<Popup>) + 'static + Clone,
    cx: &mut Context<Popup>,
) -> gpui::AnyElement {
    let is_dark = state.is_dark;
    let tertiary = state.settings.tertiary_opacity(is_dark);
    let mut pills: Vec<gpui::AnyElement> = Vec::new();
    // "All" pill (index 0).
    pills.push(pill(
        group,
        0,
        "All".to_string(),
        selected.is_none(),
        is_dark,
        tertiary,
        {
            let on_select = on_select.clone();
            move |this, _, _, cx| {
                on_select(this, None, cx);
            }
        },
        cx,
    ));
    // "Custom" pill (kaomoji only, index 1).
    if has_custom {
        pills.push(pill(group, 1, "Custom".to_string(), selected == Some("Custom"), is_dark, tertiary, {
            let on_select = on_select.clone();
            move |this, _, _, cx| {
                on_select(this, Some("Custom".to_string()), cx);
            }
        }, cx));
    }
    let base = if has_custom { 2 } else { 1 };
    for (i, cat) in categories.iter().enumerate() {
        let cat = cat.to_string();
        let active = selected == Some(cat.as_str());
        let label = cat.clone();
        pills.push(pill(
            group,
            base + i,
            label,
            active,
            is_dark,
            tertiary,
            {
                let on_select = on_select.clone();
                move |this, _, _, cx| {
                    on_select(this, Some(cat.clone()), cx);
                }
            },
            cx,
        ));
    }
    div().flex().flex_row().flex_wrap().gap(px(6.)).children(pills).into_any_element()
}

fn pill(
    group: &'static str,
    index: usize,
    label: String,
    active: bool,
    is_dark: bool,
    tertiary: f32,
    on_click: impl Fn(&mut Popup, &gpui::ClickEvent, &mut Window, &mut Context<Popup>) + 'static,
    cx: &mut Context<Popup>,
) -> gpui::AnyElement {
    div()
        .id((group, index))
        .px(px(12.))
        .py(px(4.))
        .text_size(px(10.5))
        .rounded_full()
        .cursor_pointer()
        .bg(if active {
            theme::accent()
        } else {
            theme::tertiary_bg(is_dark, tertiary)
        })
        .text_color(if active {
            gpui::rgb(0xffffff)
        } else {
            secondary_text(is_dark)
        })
        .hover(|s| {
            if active {
                s
            } else {
                s.bg(hover_bg(is_dark))
            }
        })
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

// --- Emoji tab ---

fn render_emoji(state: &Popup, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
    let is_dark = state.is_dark;
    let secondary = state.settings.secondary_opacity(is_dark);
    let query = state.emoji.search.text.clone();
    let searching = !query.trim().is_empty();

    let header = state
        .emoji
        .search
        .render_for(SearchWhich::Emoji, "Search emojis...", is_dark, secondary, window, cx)
        .into_any_element();

    // Recent strip (hidden while searching) + categories.
    let recent = state.backend.recent_emojis();
    let all = crate::pickers::load_emojis();
    let map: std::collections::HashMap<&str, &Emoji> =
        all.iter().map(|e| (e.char.as_str(), e)).collect();
    let recent_full: Vec<Emoji> = recent
        .iter()
        .filter_map(|r| map.get(r.char.as_str()).map(|e| (*e).clone()))
        .take(16)
        .collect();

    let mut sub_children: Vec<gpui::AnyElement> = Vec::new();
    if !searching && !recent_full.is_empty() {
        sub_children.push(section_label(is_dark, icons::CLOCK, "Recently used"));
        sub_children.push(render_glyph_rows(
            state,
            "recent-emoji",
            &recent_full.iter().collect::<Vec<_>>(),
            8,
            32.0,
            24.0,
            state.emoji.focused_recent,
            {
                move |this, emoji: &Emoji, window, cx| {
                    let ch = emoji.char.clone();
                    this.paste_emoji(&ch, window, cx);
                }
            },
            {
                move |this, emoji: Option<&Emoji>, _cx| {
                    this.emoji.hovered =
                        emoji.map(|e| (e.char.clone(), e.name.clone()));
                }
            },
            cx,
        ));
    }
    if !searching {
        sub_children.push(render_emoji_categories(state, cx));
    }
    let sub = if sub_children.is_empty() {
        None
    } else {
        Some(
            div().flex().flex_col().children(sub_children).into_any_element(),
        )
    };

    let items = state.emoji_filtered();
    let grid = if items.is_empty() {
        empty_state(is_dark, "No emojis found")
    } else {
        render_glyph_grid(state, window, "emoji-cell", &items, state.emoji.focused_main, 24.0, cx)
    };

    let footer = footer_preview(is_dark, state.emoji.hovered.clone(), "Click to paste emoji");
    chrome(state, header, sub, grid, footer)
}

fn render_emoji_categories(state: &Popup, cx: &mut Context<Popup>) -> gpui::AnyElement {
    // Categories computed fresh (cheap: sorts ≤10 names).
    let cats = crate::pickers::emoji_categories();
    // Leak-free: pass owned Strings via a 'static slice trick — instead rebuild pills inline.
    let is_dark = state.is_dark;
    let tertiary = state.settings.tertiary_opacity(is_dark);
    let selected = state.emoji.category.clone();
    let mut pills: Vec<gpui::AnyElement> = vec![pill(
        "emoji-cat",
        0,
        "All".to_string(),
        selected.is_none(),
        is_dark,
        tertiary,
        |this, _, _, cx| {
            this.emoji.category = None;
            this.emoji.focused_main = 0;
            this.emoji.focused_category = 0;
            cx.notify();
        },
        cx,
    )];
    for (i, cat) in cats.iter().enumerate() {
        let cat = cat.clone();
        let active = selected.as_deref() == Some(cat.as_str());
        let label = cat.clone();
        pills.push(pill(
            "emoji-cat",
            i + 1,
            label,
            active,
            is_dark,
            tertiary,
            move |this, _, _, cx| {
                this.emoji.category = Some(cat.clone());
                this.emoji.focused_main = 0;
                cx.notify();
            },
            cx,
        ));
    }
    div().flex().flex_row().flex_wrap().gap(px(6.)).children(pills).into_any_element()
}

/// Glyph grid with fixed 40px rows, chunked deterministically.
fn render_glyph_rows<F, H>(
    state: &Popup,
    group: &'static str,
    items: &[&Emoji],
    columns: usize,
    cell: f32,
    glyph: f32,
    focused: usize,
    on_select: F,
    on_hover: H,
    cx: &mut Context<Popup>,
) -> gpui::AnyElement
where
    F: Fn(&mut Popup, &Emoji, &mut Window, &mut Context<Popup>) + 'static + Clone,
    H: Fn(&mut Popup, Option<&Emoji>, &mut Context<Popup>) + 'static + Clone,
{
    let is_dark = state.is_dark;
    let mut rows: Vec<gpui::AnyElement> = Vec::new();
    for (row, chunk) in items.chunks(columns.max(1)).enumerate() {
        let mut cells: Vec<gpui::AnyElement> = Vec::new();
        for (c, item) in chunk.iter().enumerate() {
            let idx = row * columns.max(1) + c;
            let item = (*item).clone();
            let is_focused = idx == focused;
            let on_select = on_select.clone();
            let on_hover = on_hover.clone();
            let enter = item.clone();
            let leave = item.clone();
            cells.push(
                div()
                    .id((group, idx))
                    .w(px(cell))
                    .h(px(cell))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(glyph))
                    .cursor_pointer()
                    .border_2()
                    .border_color(if is_focused {
                        theme::accent()
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .hover(|s| s.bg(hover_bg(is_dark)))
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        if *hovering {
                            on_hover(this, Some(&enter), cx);
                        } else {
                            on_hover(this, None, cx);
                        }
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        on_select(this, &leave, window, cx);
                    }))
                    .child(item.char.clone())
                    .into_any_element(),
            );
        }
        rows.push(div().flex().flex_row().children(cells).into_any_element());
    }
    div()
        .flex()
        .flex_col()
        .flex_wrap()
        .gap(px(4.))
        .children(rows)
        .into_any_element()
}

fn render_glyph_grid(
    state: &Popup,
    window: &Window,
    group: &'static str,
    items: &[Emoji],
    focused: usize,
    glyph: f32,
    cx: &mut Context<Popup>,
) -> gpui::AnyElement {
    let cols = grid_columns(window);
    let refs: Vec<&Emoji> = items.iter().collect();
    div()
        .id("emoji-grid-scroll")
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scroll()
        .p(px(12.))
        .child(render_glyph_rows(
            state,
            group,
            &refs,
            cols,
            40.0,
            glyph,
            focused,
            |this, emoji: &Emoji, window, cx| {
                let ch = emoji.char.clone();
                this.paste_emoji(&ch, window, cx);
            },
            |this, emoji: Option<&Emoji>, _cx| {
                this.emoji.hovered = emoji.map(|e| (e.char.clone(), e.name.clone()));
            },
            cx,
        ))
        .into_any_element()
}

// --- Kaomoji tab ---

fn render_kaomoji(state: &Popup, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
    let is_dark = state.is_dark;
    let secondary = state.settings.secondary_opacity(is_dark);

    let header = state
        .kaomoji
        .search
        .render_for(SearchWhich::Kaomoji, "Search kaomoji...", is_dark, secondary, window, cx)
        .into_any_element();

    let query = state.kaomoji.search.text.clone();
    let searching = !query.trim().is_empty();
    let sub = if searching {
        None
    } else {
        Some(category_pills(
            state,
            "kaomoji-cat",
            KAOMOJI_CATEGORIES,
            state.kaomoji.category.as_deref(),
            !state.settings.custom_kaomojis.is_empty(),
            |this, cat, cx| {
                this.kaomoji.category = cat;
                this.kaomoji.focused_main = 0;
                this.kaomoji.focused_category = 0;
                cx.notify();
            },
            cx,
        ))
    };

    let items = state.kaomoji_filtered();
    let grid = if items.is_empty() {
        empty_state(is_dark, "No kaomojis found")
    } else {
        let cols = kaomoji_columns(window);
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (row, chunk) in items.chunks(cols).enumerate() {
            let mut cells: Vec<gpui::AnyElement> = Vec::new();
            for (c, item) in chunk.iter().enumerate() {
                let idx = row * cols + c;
                let text = item.text.clone();
                let cat = item.category.clone();
                let is_focused = idx == state.kaomoji.focused_main;
                let enter = (text.clone(), cat.clone());
                cells.push(
                    div()
                        .id(("kaomoji-cell", idx))
                        .flex_1()
                        .h(px(48.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .text_size(px(12.25))
                        .text_color(if is_dark {
                            theme::dark::text_primary()
                        } else {
                            theme::light::text_primary()
                        })
                        .cursor_pointer()
                        .border_1()
                        .border_color(if is_focused {
                            theme::accent()
                        } else {
                            gpui::rgba(0x00000000)
                        })
                        .hover(|s| {
                            s.bg(hover_bg(is_dark)).border_color(if is_dark {
                                theme::dark::border_subtle()
                            } else {
                                theme::light::border()
                            })
                        })
                        .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                            this.kaomoji.hovered = if *hovering { Some(enter.clone()) } else { None };
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let t = text.clone();
                            this.paste_kaomoji(&t, window, cx);
                        }))
                        .child(item.text.clone())
                        .into_any_element(),
                );
            }
            rows.push(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .children(cells)
                    .into_any_element(),
            );
        }
        div()
            .id("kaomoji-grid-scroll")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(rows)
            .into_any_element()
    };

    let footer = footer_preview(is_dark, state.kaomoji.hovered.clone(), "Click to paste kaomoji");
    chrome(state, header, sub, grid, footer)
}

// --- Symbol tab ---

fn render_symbol(state: &Popup, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
    let is_dark = state.is_dark;
    let secondary = state.settings.secondary_opacity(is_dark);
    let query = state.symbol.search.text.clone();
    let searching = !query.trim().is_empty();
    let has_category = state.symbol.category.is_some();

    let header = state
        .symbol
        .search
        .render_for(SearchWhich::Symbol, "Search symbols...", is_dark, secondary, window, cx)
        .into_any_element();

    let mut sub_children: Vec<gpui::AnyElement> = Vec::new();
    if !searching && !has_category && !state.symbol_recents.is_empty() {
        sub_children.push(section_label(is_dark, icons::CLOCK, "Recently used"));
        let recents: Vec<SymbolItem> = state.symbol_recents.iter().take(16).cloned().collect();
        sub_children.push(render_symbol_rows(
            state,
            "recent-symbol",
            &recents,
            10,
            32.0,
            20.0,
            state.symbol.focused_recent,
            cx,
        ));
    }
    if !searching {
        sub_children.push(category_pills(
            state,
            "symbol-cat",
            SYMBOL_CATEGORIES,
            state.symbol.category.as_deref(),
            false,
            |this, cat, cx| {
                this.symbol.category = cat;
                this.symbol.focused_main = 0;
                this.symbol.focused_category = 0;
                cx.notify();
            },
            cx,
        ));
    }
    let sub = if sub_children.is_empty() {
        None
    } else {
        Some(div().flex().flex_col().children(sub_children).into_any_element())
    };

    let items = state.symbol_filtered();
    let grid = if items.is_empty() {
        empty_state(is_dark, "No symbols found")
    } else {
        let cols = grid_columns(window);
        let refs: Vec<&SymbolItem> = items.iter().collect();
        div()
            .id("symbol-grid-scroll")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(12.))
            .child(render_symbol_rows(
                state,
                "symbol-cell",
                &refs.iter().map(|s| (*s).clone()).collect::<Vec<_>>(),
                cols,
                40.0,
                20.0,
                state.symbol.focused_main,
                cx,
            ))
            .into_any_element()
    };

    let footer = footer_preview(is_dark, state.symbol.hovered.clone(), "Click to paste symbol");
    chrome(state, header, sub, grid, footer)
}

fn render_symbol_rows(
    state: &Popup,
    group: &'static str,
    items: &[SymbolItem],
    columns: usize,
    cell: f32,
    glyph: f32,
    focused: usize,
    cx: &mut Context<Popup>,
) -> gpui::AnyElement {
    let is_dark = state.is_dark;
    let mut rows: Vec<gpui::AnyElement> = Vec::new();
    for (row, chunk) in items.chunks(columns.max(1)).enumerate() {
        let mut cells: Vec<gpui::AnyElement> = Vec::new();
        for (c, item) in chunk.iter().enumerate() {
            let idx = row * columns.max(1) + c;
            let item = item.clone();
            let is_focused = idx == focused;
            let enter = item.clone();
            let leave = item.clone();
            let glyph_text = item.char.clone();
            cells.push(
                div()
                    .id((group, idx))
                    .w(px(cell))
                    .h(px(cell))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(glyph))
                    .cursor_pointer()
                    .border_2()
                    .border_color(if is_focused {
                        theme::accent()
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .hover(|s| s.bg(hover_bg(is_dark)))
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        this.symbol.hovered = if *hovering {
                            Some((enter.char.clone(), enter.name.clone()))
                        } else {
                            None
                        };
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.paste_symbol(&leave, window, cx);
                    }))
                    .child(glyph_text)
                    .into_any_element(),
            );
        }
        rows.push(div().flex().flex_row().children(cells).into_any_element());
    }
    div().flex().flex_col().gap(px(4.)).children(rows).into_any_element()
}
