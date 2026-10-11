//! Emoji, Kaomoji and Symbol tabs: category strip, virtualised grids, hover
//! preview and keyboard navigation. Data comes from `app_shared::pickers`.

use app_shared::pickers::{
    emoji_categories, get_kaomojis, get_symbols, load_emojis, load_recent_symbols, search_emojis,
    Emoji, Kaomoji, SymbolItem, KAOMOJI_CATEGORIES, RECENT_CATEGORY, SYMBOL_CATEGORIES,
};
use cosmic::iced::widget::scrollable;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self};
use cosmic::Element;

use crate::popup::{Popup, Tab, ViewCtx};
use crate::scale;
use crate::widgets;
use crate::win11::{self, flat, Flat, Pal};

pub const COLS: usize = 7;
pub const KAOMOJI_COLS: usize = 2;
const CELL: f32 = 40.0;
const KAOMOJI_H: f32 = 44.0;
const GRID_H: f32 = 258.0;
const OVERSCAN: usize = 2;

#[derive(Debug, Clone)]
pub enum Msg {
    Search(Tab, String),
    Category(Tab, Option<String>),
    Hover(Option<(String, String)>),
    PickEmoji(String),
    PickKaomoji(String),
    PickSymbol(usize),
    Scrolled(Tab, scrollable::Viewport),
    Noop,
}

pub struct Tabs {
    pub search: String,
    pub focused: usize,
    pub category: Option<String>,
    pub scroll: f32,
    pub search_id: widget::Id,
    pub grid_id: widget::Id,
}

impl Tabs {
    fn new() -> Self {
        Self {
            search: String::new(),
            focused: 0,
            category: None,
            scroll: 0.0,
            search_id: widget::Id::unique(),
            grid_id: widget::Id::unique(),
        }
    }

    fn reset(&mut self) {
        self.search.clear();
        self.focused = 0;
        self.category = None;
        self.scroll = 0.0;
    }
}

pub struct PickerState {
    pub emoji: Tabs,
    pub kaomoji: Tabs,
    pub symbol: Tabs,
    pub hovered: Option<(String, String)>,
    pub emoji_list: Vec<Emoji>,
    pub kaomoji_list: Vec<Kaomoji>,
    pub symbol_list: Vec<SymbolItem>,
    symbol_recents: Vec<SymbolItem>,
}

impl PickerState {
    pub fn new() -> Self {
        Self {
            emoji: Tabs::new(),
            kaomoji: Tabs::new(),
            symbol: Tabs::new(),
            hovered: None,
            emoji_list: Vec::new(),
            kaomoji_list: Vec::new(),
            symbol_list: Vec::new(),
            symbol_recents: Vec::new(),
        }
    }

    pub fn reset_for_show(&mut self) {
        self.emoji.reset();
        self.kaomoji.reset();
        self.symbol.reset();
        self.hovered = None;
    }

    pub fn tabs(&self, tab: Tab) -> Option<&Tabs> {
        match tab {
            Tab::Emoji => Some(&self.emoji),
            Tab::Kaomoji => Some(&self.kaomoji),
            Tab::Symbols => Some(&self.symbol),
            Tab::Clipboard => None,
        }
    }

    pub fn tabs_mut(&mut self, tab: Tab) -> Option<&mut Tabs> {
        match tab {
            Tab::Emoji => Some(&mut self.emoji),
            Tab::Kaomoji => Some(&mut self.kaomoji),
            Tab::Symbols => Some(&mut self.symbol),
            Tab::Clipboard => None,
        }
    }

    pub fn len(&self, tab: Tab) -> usize {
        match tab {
            Tab::Emoji => self.emoji_list.len(),
            Tab::Kaomoji => self.kaomoji_list.len(),
            Tab::Symbols => self.symbol_list.len(),
            Tab::Clipboard => 0,
        }
    }

    pub fn cols(tab: Tab) -> usize {
        if tab == Tab::Kaomoji { KAOMOJI_COLS } else { COLS }
    }

    pub fn row_height(tab: Tab) -> f32 {
        scale::px(if tab == Tab::Kaomoji { KAOMOJI_H } else { CELL })
    }

    /// Recompute the lists a tab shows from its search / category (call after
    /// either changes, and on show).
    pub fn refresh(&mut self, tab: Tab, recent_emojis: &[String], custom: &[Kaomoji]) {
        match tab {
            Tab::Emoji => self.emoji_list = emoji_list(&self.emoji, recent_emojis),
            Tab::Kaomoji => {
                self.kaomoji_list =
                    get_kaomojis(self.kaomoji.category.as_deref(), &self.kaomoji.search, custom)
            }
            Tab::Symbols => {
                self.symbol_recents = load_recent_symbols();
                self.symbol_list = symbol_list(&self.symbol, &self.symbol_recents);
            }
            Tab::Clipboard => {}
        }
    }

    pub fn refresh_all(&mut self, recent_emojis: &[String], custom: &[Kaomoji]) {
        for tab in [Tab::Emoji, Tab::Kaomoji, Tab::Symbols] {
            self.refresh(tab, recent_emojis, custom);
        }
    }

    /// Grid navigation (port of `useKeyboardNavigation`). Returns the new focus.
    pub fn nav(&mut self, tab: Tab, key: &str, ctrl: bool) -> bool {
        let cols = Self::cols(tab);
        let len = self.len(tab);
        let Some(tabs) = self.tabs_mut(tab) else { return false };
        match grid_move(tabs.focused, key, ctrl, cols, len) {
            Some(n) => {
                tabs.focused = n;
                true
            }
            None => false,
        }
    }

    /// Scroll offset that centres the focused row.
    pub fn scroll_target(&self, tab: Tab) -> Option<f32> {
        let tabs = self.tabs(tab)?;
        let row = tabs.focused / Self::cols(tab).max(1);
        let h = Self::row_height(tab);
        let top = row as f32 * h;
        let view_h = scale::px(GRID_H);
        if top < tabs.scroll || top + h > tabs.scroll + view_h {
            Some((top - (view_h - h) / 2.0).max(0.0))
        } else {
            None
        }
    }

    pub fn emoji_at_focus(&self) -> Option<String> {
        self.emoji_list.get(self.emoji.focused).map(|e| e.char.clone())
    }

    pub fn kaomoji_at_focus(&self) -> Option<String> {
        self.kaomoji_list.get(self.kaomoji.focused).map(|k| k.text.clone())
    }

    pub fn symbol_at_focus(&self) -> Option<SymbolItem> {
        self.symbol_list.get(self.symbol.focused).cloned()
    }
}

fn emoji_list(tabs: &Tabs, recent: &[String]) -> Vec<Emoji> {
    let q = tabs.search.trim();
    if !q.is_empty() {
        return search_emojis(q, 100);
    }
    let all = load_emojis();
    let map: std::collections::HashMap<&str, &Emoji> = all.iter().map(|e| (e.char.as_str(), e)).collect();
    let recents: Vec<Emoji> = recent.iter().filter_map(|r| map.get(r.as_str()).map(|e| (*e).clone())).collect();
    if tabs.category.as_deref() == Some(RECENT_CATEGORY) {
        return recents;
    }
    if let Some(cat) = &tabs.category {
        return all.into_iter().filter(|e| &e.category == cat).collect();
    }
    if recents.is_empty() {
        return all;
    }
    let seen: std::collections::HashSet<String> = recents.iter().map(|e| e.char.clone()).collect();
    let mut out = recents;
    out.extend(all.into_iter().filter(|e| !seen.contains(&e.char)));
    out
}

fn symbol_list(tabs: &Tabs, recents: &[SymbolItem]) -> Vec<SymbolItem> {
    let base = get_symbols(tabs.category.as_deref(), &tabs.search);
    if !tabs.search.is_empty() || tabs.category.is_some() || recents.is_empty() {
        return base;
    }
    let mut out: Vec<SymbolItem> = recents.iter().take(16).cloned().collect();
    out.extend(base.into_iter().filter(|s| !recents.iter().take(16).any(|r| r.char == s.char)));
    out
}

/// Pure index math for arrow / home / end / page keys.
pub fn grid_move(current: usize, key: &str, ctrl: bool, cols: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let cols = cols.max(1);
    match key {
        "right" => current.checked_add(1).filter(|&n| n < len),
        "left" => current.checked_sub(1),
        "down" => {
            let n = current + cols;
            (n < len).then_some(n)
        }
        "up" => current.checked_sub(cols),
        "home" => Some(if ctrl { 0 } else { current / cols * cols }),
        "end" => Some(if ctrl {
            len - 1
        } else {
            ((current / cols + 1) * cols).saturating_sub(1).min(len - 1)
        }),
        "pagedown" => Some((current + cols * 3).min(len - 1)),
        "pageup" => Some(current.saturating_sub(cols * 3)),
        _ => None,
    }
}

// --- View ---

pub fn view<'a>(popup: &'a Popup, ctx: &ViewCtx) -> Element<'a, Msg> {
    let p = ctx.pal;
    let tab = popup.tab;
    let Some(tabs) = popup.pickers.tabs(tab) else {
        return widget::space().into();
    };
    let placeholder = match tab {
        Tab::Emoji => "Search emojis...",
        Tab::Kaomoji => "Search kaomoji...",
        _ => "Search symbols...",
    };
    let search = widget::container(
        win11::search_input(p, placeholder, &tabs.search)
            .id(tabs.search_id.clone())
            .on_input(move |s| Msg::Search(tab, s))
            .on_submit(|_| Msg::Noop)
            .width(Length::Fill),
    )
    .padding(scale::pad4(12, 12, 8, 12));

    let mut col = widget::column::with_capacity(5).push(search);
    if tabs.search.trim().is_empty() {
        col = col.push(categories(p, tab, tabs, !ctx.recent_emojis.is_empty()));
    }
    col = col.push(grid(popup, p, tab, tabs));
    col = col.push(win11::rule(p.border_subtle));
    col = col.push(footer(popup, p));
    col.into()
}

fn categories<'a>(p: Pal, tab: Tab, tabs: &'a Tabs, has_recents: bool) -> Element<'a, Msg> {
    let mut names: Vec<String> = Vec::new();
    match tab {
        Tab::Emoji => {
            if has_recents {
                names.push(RECENT_CATEGORY.to_string());
            }
            names.extend(emoji_categories());
        }
        Tab::Kaomoji => names.extend(KAOMOJI_CATEGORIES.iter().map(|s| s.to_string())),
        _ => names.extend(SYMBOL_CATEGORIES.iter().map(|s| s.to_string())),
    }
    let mut row = widget::row::with_capacity(names.len() + 1).spacing(scale::u(6)).padding(scale::pad4(0, 12, 8, 12));
    row = row.push(pill(p, "All".to_string(), tabs.category.is_none(), Msg::Category(tab, None)));
    for name in names {
        let selected = tabs.category.as_deref() == Some(name.as_str());
        let target = if selected { None } else { Some(name.clone()) };
        row = row.push(pill(p, name, selected, Msg::Category(tab, target)));
    }
    scrollable(row)
        .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(0).scroller_width(0)))
        .width(Length::Fill)
        .into()
}

/// A category pill: tertiary fill, accent when selected.
fn pill<'a>(p: Pal, label: String, selected: bool, message: Msg) -> Element<'a, Msg> {
    let style = if selected {
        Flat::new(win11::rgb(win11::ON_ACCENT), win11::rgb(win11::ACCENT)).bg(win11::rgb(win11::ACCENT)).radius(100.0)
    } else {
        Flat::new(p.text2, p.card_hover).bg(p.tertiary).radius(100.0)
    };
    let color = if selected { win11::rgb(win11::ON_ACCENT) } else { p.text2 };
    flat(win11::small(label, color), (4, 12), style, Some(message))
}

fn grid<'a>(popup: &'a Popup, p: Pal, tab: Tab, tabs: &'a Tabs) -> Element<'a, Msg> {
    let cols = PickerState::cols(tab);
    let len = popup.pickers.len(tab);
    if len == 0 {
        let message = match tab {
            Tab::Emoji => "No emojis found",
            Tab::Kaomoji => "No kaomoji found",
            _ => "No symbols found",
        };
        return widget::container(win11::body(message, p.text2))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
    }
    let rows = len.div_ceil(cols);
    let row_h = PickerState::row_height(tab);
    let first = ((tabs.scroll / row_h).floor() as usize).saturating_sub(OVERSCAN);
    let last = (first + (scale::px(GRID_H) / row_h).ceil() as usize + OVERSCAN * 2 + 1).min(rows);

    let mut col = widget::column::with_capacity(last - first + 2).padding(scale::pad2(0, 12));
    col = col.push(widgets::spacer_h(first as f32 * row_h).map(|()| Msg::Noop));
    for r in first..last {
        let mut row = widget::row::with_capacity(cols).spacing(0);
        for c in 0..cols {
            let index = r * cols + c;
            if index >= len {
                break;
            }
            row = row.push(cell(popup, p, tab, index, index == tabs.focused));
        }
        col = col.push(row);
    }
    col = col.push(widgets::spacer_h((rows - last) as f32 * row_h).map(|()| Msg::Noop));

    scrollable(col)
        .id(tabs.grid_id.clone())
        .height(Length::Fill)
        .on_scroll(move |vp| Msg::Scrolled(tab, vp))
        .into()
}

/// A picker cell: 40 px, 6 px corners, accent ring when focused.
fn cell_style(p: Pal, focused: bool) -> Flat {
    let style = Flat::new(p.text, p.card_hover);
    if focused { style.ring(2.0 * scale::get(), win11::rgb(win11::ACCENT)) } else { style }
}

fn cell<'a>(popup: &'a Popup, p: Pal, tab: Tab, index: usize, focused: bool) -> Element<'a, Msg> {
    match tab {
        Tab::Emoji => {
            let e = &popup.pickers.emoji_list[index];
            let glyph: Element<'a, Msg> = match crate::emoji_image::handle(&e.char) {
                Some(handle) => widget::image(handle).width(Length::Fixed(scale::px(24.0))).height(Length::Fixed(scale::px(24.0))).into(),
                None => win11::body(e.char.clone(), p.text).into(),
            };
            let content = widget::container(glyph)
                .width(Length::Fixed(scale::px(CELL)))
                .height(Length::Fixed(scale::px(CELL)))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center);
            let button = flat(content, (0, 0), cell_style(p, focused), Some(Msg::PickEmoji(e.char.clone())));
            let hover = Msg::Hover(Some((e.char.clone(), e.name.clone())));
            widget::mouse_area(button).on_enter(hover).on_exit(Msg::Hover(None)).into()
        }
        Tab::Kaomoji => {
            let k = &popup.pickers.kaomoji_list[index];
            let content = widget::container(win11::body(k.text.clone(), p.text))
                .width(Length::Fill)
                .height(Length::Fixed(scale::px(KAOMOJI_H - 4.0)))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center);
            let button = flat(content, (0, 4), cell_style(p, focused), Some(Msg::PickKaomoji(k.text.clone())));
            widget::container(button).width(Length::FillPortion(1)).padding(scale::u(2)).into()
        }
        _ => {
            let s = &popup.pickers.symbol_list[index];
            let content = widget::container(win11::title(s.char.clone(), p.text))
                .width(Length::Fixed(scale::px(CELL)))
                .height(Length::Fixed(scale::px(CELL)))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center);
            let button = flat(content, (0, 0), cell_style(p, focused), Some(Msg::PickSymbol(index)));
            let hover = Msg::Hover(Some((s.char.clone(), s.name.clone())));
            widget::mouse_area(button).on_enter(hover).on_exit(Msg::Hover(None)).into()
        }
    }
}

fn footer<'a>(popup: &'a Popup, p: Pal) -> Element<'a, Msg> {
    let tab = popup.tab;
    let focused_preview: Option<(String, String)> = match tab {
        Tab::Emoji => popup.pickers.emoji_list.get(popup.pickers.emoji.focused).map(|e| (e.char.clone(), e.name.clone())),
        Tab::Symbols => popup.pickers.symbol_list.get(popup.pickers.symbol.focused).map(|s| (s.char.clone(), s.name.clone())),
        _ => None,
    };
    let shown = popup.pickers.hovered.clone().or(focused_preview);
    let hint = match tab {
        Tab::Emoji => "Click to paste emoji",
        Tab::Kaomoji => "Click to paste kaomoji",
        _ => "Click to paste symbol",
    };
    let content: Element<'a, Msg> = match shown {
        Some((glyph, name)) => {
            let big: Element<'a, Msg> = match (tab, crate::emoji_image::handle(&glyph)) {
                (Tab::Emoji, Some(h)) => widget::image(h).width(Length::Fixed(scale::px(22.0))).height(Length::Fixed(scale::px(22.0))).into(),
                _ => win11::title(glyph, p.text).into(),
            };
            widget::row::with_capacity(2).spacing(scale::u(8)).align_y(Alignment::Center).push(big).push(win11::small(name, p.text2)).into()
        }
        None => win11::small(hint, p.text3).into(),
    };
    widget::container(content)
        .padding(scale::pad2(8, 12))
        .width(Length::Fill)
        .height(Length::Fixed(scale::px(40.0)))
        .align_y(Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_move_by_cell_and_row() {
        assert_eq!(grid_move(0, "right", false, 7, 100), Some(1));
        assert_eq!(grid_move(0, "left", false, 7, 100), None);
        assert_eq!(grid_move(3, "down", false, 7, 100), Some(10));
        assert_eq!(grid_move(10, "up", false, 7, 100), Some(3));
        assert_eq!(grid_move(98, "down", false, 7, 100), None);
    }

    #[test]
    fn home_end_page_keys() {
        assert_eq!(grid_move(10, "home", false, 7, 100), Some(7));
        assert_eq!(grid_move(10, "home", true, 7, 100), Some(0));
        assert_eq!(grid_move(10, "end", false, 7, 100), Some(13));
        assert_eq!(grid_move(10, "end", true, 7, 100), Some(99));
        assert_eq!(grid_move(0, "pagedown", false, 7, 100), Some(21));
        assert_eq!(grid_move(5, "pageup", false, 7, 100), Some(0));
        assert_eq!(grid_move(0, "right", false, 7, 0), None);
    }

    #[test]
    fn lists_filter_by_category_and_search() {
        let mut p = PickerState::new();
        p.emoji.search = "grin".into();
        p.refresh(Tab::Emoji, &[], &[]);
        assert!(!p.emoji_list.is_empty() && p.emoji_list.len() <= 100);
        p.emoji.search.clear();
        p.emoji.category = Some("Animals & Nature".into());
        p.refresh(Tab::Emoji, &[], &[]);
        assert!(p.emoji_list.iter().all(|e| e.category == "Animals & Nature"));
        p.kaomoji.category = Some("Happy".into());
        p.refresh(Tab::Kaomoji, &[], &[]);
        assert!(p.kaomoji_list.iter().all(|k| k.category == "Happy"));
    }

    #[test]
    fn focus_scrolls_to_centre_when_off_screen() {
        let mut p = PickerState::new();
        p.emoji_list = load_emojis();
        p.emoji.focused = 0;
        assert_eq!(p.scroll_target(Tab::Emoji), None);
        p.emoji.focused = 700;
        assert!(p.scroll_target(Tab::Emoji).unwrap() > 0.0);
    }
}
