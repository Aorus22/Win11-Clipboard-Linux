//! The clipboard popup: state, pure logic and the clipboard tab's view.
//!
//! Rows of the history list have fixed heights so the list can be virtualised
//! (only rows near the viewport are built) and the focused row scrolled into
//! view with plain arithmetic.

use std::collections::HashMap;

use app_shared::history::{
    detect_smart_actions, filter_history, relative_time, ClipboardContent, ClipboardItem,
    SmartAction,
};
use app_shared::settings::config_dir;
use cosmic::iced::widget::scrollable;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::widget::{self, image::Handle};
use cosmic::{theme, Element};
use serde::{Deserialize, Serialize};

use crate::pickers_ui::{self, PickerState};
use crate::scale;
use crate::widgets::{self, Ic};
use crate::win11::{self, flat, Flat, Pal, CLEAR};

pub const POPUP_W: f32 = 360.0;
pub const POPUP_H: f32 = 480.0;

const ITEM_ONE_LINE_H: f32 = 61.0;
const ITEM_TWO_LINES_H: f32 = 78.0;
const ITEM_IMAGE_H: f32 = 92.0;
const ITEM_COMPACT_H: f32 = 40.0;
const SECTION_H: f32 = 30.0;
const GAP: f32 = 6.0;

/// Height of a history card in layout px (before the UI scale): text cards take one or
/// two lines, so the list stays virtualisable with known row heights.
fn item_layout_h(item: &ClipboardItem, compact: bool) -> f32 {
    if compact {
        return ITEM_COMPACT_H;
    }
    match &item.content {
        ClipboardContent::Image { .. } => ITEM_IMAGE_H,
        _ => {
            if preview_lines(&item.preview) >= 2 { ITEM_TWO_LINES_H } else { ITEM_ONE_LINE_H }
        }
    }
}

fn item_h(item: &ClipboardItem, compact: bool) -> f32 {
    scale::px(item_layout_h(item, compact))
}

/// One visual line holds about this many characters of a card's text.
const CHARS_PER_LINE: usize = 40;

/// Visual lines a preview takes (capped at two).
fn preview_lines(preview: &str) -> usize {
    let logical = preview.lines().count().max(1);
    let longest = preview.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    if logical >= 2 || longest > CHARS_PER_LINE { 2 } else { 1 }
}
/// Rows built beyond the viewport on each side.
const OVERSCAN: usize = 3;
const UI_STATE_FILE: &str = "ui_state.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Clipboard,
    Symbols,
    Emoji,
    Kaomoji,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Clipboard, Tab::Symbols, Tab::Emoji, Tab::Kaomoji];

    pub fn next(self) -> Tab {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Tab {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn icon(self) -> Ic {
        match self {
            Tab::Clipboard => Ic::ClipboardList,
            Tab::Symbols => Ic::Omega,
            Tab::Emoji => Ic::Smile,
            Tab::Kaomoji => Ic::Type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UiState {
    #[serde(default)]
    compact: bool,
    #[serde(default = "yes")]
    pinned_expanded: bool,
}

fn yes() -> bool {
    true
}

fn load_ui_state() -> UiState {
    std::fs::read_to_string(config_dir().join(UI_STATE_FILE))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(UiState { compact: false, pinned_expanded: true })
}

/// Messages the popup view produces.
#[derive(Debug, Clone)]
pub enum Msg {
    Tab(Tab),
    /// The pointer was pressed somewhere in the popup.
    Pressed,
    Close,
    DragStart,
    Search(String),
    SearchOpen(bool),
    ToggleRegex,
    Paste(String),
    Pin(String),
    Delete(String),
    ClearAll,
    ToggleCompact,
    TogglePinned,
    OpenUrl(String),
    Hover(Option<String>),
    Scrolled(scrollable::Viewport),
    Noop,
    Picker(pickers_ui::Msg),
}

/// What a row of the list is.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Header(bool), // true = Pinned, false = Recent
    Item(usize),  // index into the visible list
}

fn row_height(row: Row, compact: bool, visible: &[&ClipboardItem]) -> f32 {
    match row {
        Row::Header(_) => scale::px(SECTION_H),
        Row::Item(i) => visible.get(i).map(|item| item_h(item, compact)).unwrap_or(0.0) + scale::px(GAP),
    }
}

pub struct Popup {
    pub tab: Tab,
    pub items: Vec<ClipboardItem>,
    pub search: String,
    pub regex: bool,
    pub search_open: bool,
    pub focused: usize,
    pub compact: bool,
    pub pinned_expanded: bool,
    pub scroll: f32,
    pub viewport_h: f32,
    pub search_id: widget::Id,
    pub list_id: widget::Id,
    pub pickers: PickerState,
    thumbs: HashMap<String, Option<Handle>>,
    /// The history card under the pointer (its action buttons are shown).
    pub hovered: Option<String>,
}

impl Popup {
    pub fn new() -> Self {
        let ui = load_ui_state();
        Self {
            tab: Tab::Clipboard,
            items: Vec::new(),
            search: String::new(),
            regex: false,
            search_open: false,
            focused: 0,
            compact: ui.compact,
            pinned_expanded: ui.pinned_expanded,
            scroll: 0.0,
            viewport_h: scale::px(POPUP_H - 150.0),
            search_id: widget::Id::unique(),
            list_id: widget::Id::unique(),
            pickers: PickerState::new(),
            thumbs: HashMap::new(),
            hovered: None,
        }
    }

    /// Reset per-open state for a fresh show.
    pub fn prepare_for_show(&mut self, tab: Tab, items: Vec<ClipboardItem>) {
        self.items = items;
        self.search.clear();
        self.search_open = false;
        self.focused = 0;
        self.scroll = 0.0;
        self.tab = tab;
        self.pickers.reset_for_show();
        let live: std::collections::HashSet<&str> = self.items.iter().map(|i| i.id.as_str()).collect();
        self.thumbs.retain(|id, _| live.contains(id.as_str()));
    }

    pub fn save_ui_state(&self) {
        let ui = UiState { compact: self.compact, pinned_expanded: self.pinned_expanded };
        if let Ok(content) = serde_json::to_string_pretty(&ui) {
            let _ = std::fs::write(config_dir().join(UI_STATE_FILE), content);
        }
    }

    // --- Logic (pure over state) ---

    fn filtered(&self) -> Vec<&ClipboardItem> {
        filter_history(&self.items, &self.search, self.regex)
    }

    fn show_sections(&self, filtered: &[&ClipboardItem]) -> bool {
        self.search.is_empty() && filtered.iter().any(|i| i.pinned)
    }

    /// Flat visible list for keyboard nav + paste.
    pub fn visible_items(&self) -> Vec<&ClipboardItem> {
        let filtered = self.filtered();
        if self.show_sections(&filtered) && !self.pinned_expanded {
            filtered.into_iter().filter(|i| !i.pinned).collect()
        } else {
            filtered
        }
    }

    pub fn filtered_count(&self) -> usize {
        self.filtered().len()
    }

    fn rows(&self) -> Vec<Row> {
        let filtered = self.filtered();
        let show_sections = self.show_sections(&filtered);
        let visible = self.visible_items();
        let mut rows = Vec::with_capacity(visible.len() + 2);
        if show_sections {
            rows.push(Row::Header(true));
            let pinned = visible.iter().filter(|i| i.pinned).count();
            for i in 0..pinned {
                rows.push(Row::Item(i));
            }
            rows.push(Row::Header(false));
            for i in pinned..visible.len() {
                rows.push(Row::Item(i));
            }
        } else {
            rows.extend((0..visible.len()).map(Row::Item));
        }
        rows
    }

    /// Scroll offset that brings the focused row fully into view, if it is not.
    pub fn scroll_target(&self) -> Option<f32> {
        let rows = self.rows();
        let visible = self.visible_items();
        let mut top = 0.0;
        for row in &rows {
            let h = row_height(*row, self.compact, &visible);
            if *row == Row::Item(self.focused) {
                // Keep the section header above the first item visible too.
                if top < self.scroll {
                    return Some(top.max(0.0));
                }
                if top + h > self.scroll + self.viewport_h {
                    return Some(top + h - self.viewport_h);
                }
                return None;
            }
            top += h;
        }
        None
    }

    pub fn move_focus(&mut self, delta: isize) {
        let len = self.visible_items().len();
        if len == 0 {
            return;
        }
        self.focused = (self.focused as isize + delta).clamp(0, len as isize - 1) as usize;
    }

    pub fn focused_item_id(&self) -> Option<String> {
        self.visible_items().get(self.focused).map(|i| i.id.clone())
    }

    /// Up from the first recent item re-opens a collapsed pinned section.
    pub fn up(&mut self) {
        let filtered = self.filtered();
        let sections = self.show_sections(&filtered);
        let pinned = filtered.iter().filter(|i| i.pinned).count();
        drop(filtered);
        if sections && !self.pinned_expanded && self.focused == 0 {
            self.pinned_expanded = true;
            self.save_ui_state();
            self.focused = pinned.saturating_sub(1);
        } else {
            self.move_focus(-1);
        }
    }

    /// Left collapses the pinned section while focus is in it; otherwise it changes tab.
    pub fn horizontal(&mut self, left: bool) {
        let filtered = self.filtered();
        let sections = self.show_sections(&filtered);
        let pinned = filtered.iter().filter(|i| i.pinned).count();
        drop(filtered);
        if left && sections && self.pinned_expanded && self.focused < pinned {
            self.pinned_expanded = false;
            self.save_ui_state();
            self.focused = 0;
        } else {
            self.tab = if left { self.tab.prev() } else { self.tab.next() };
            self.focused = 0;
        }
    }

    pub fn after_filter_change(&mut self) {
        self.focused = 0;
        self.scroll = 0.0;
    }

    fn thumb(&mut self, item: &ClipboardItem) -> Option<Handle> {
        let ClipboardContent::Image { base64, .. } = &item.content else {
            return None;
        };
        self.thumbs
            .entry(item.id.clone())
            .or_insert_with(|| {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD.decode(base64).ok()?;
                Some(Handle::from_bytes(bytes))
            })
            .clone()
    }

    /// Decode the thumbnails of the rows about to be built (needs `&mut`).
    pub fn warm_thumbs(&mut self) {
        let wanted: Vec<ClipboardItem> = self
            .items
            .iter()
            .filter(|i| matches!(i.content, ClipboardContent::Image { .. }))
            .filter(|i| !self.thumbs.contains_key(&i.id))
            .take(40)
            .cloned()
            .collect();
        for item in wanted {
            self.thumb(&item);
        }
    }

    // --- View ---

    pub fn view<'a>(&'a self, ctx: &ViewCtx) -> Element<'a, Msg> {
        let p = ctx.pal;
        let tab_buttons: Vec<Element<'a, Msg>> = Tab::ALL
            .iter()
            .map(|tab| {
                let active = *tab == self.tab;
                let style = Flat::new(if active { p.text } else { p.text2 }, p.tertiary).bg(if active { p.tertiary } else { CLEAR });
                flat(widgets::ic(tab.icon(), 16), (8, 16), style, Some(Msg::Tab(*tab)))
            })
            .collect();
        let tabs = widget::container(widget::row::with_children(tab_buttons).spacing(scale::u(4)).align_y(Alignment::Center))
            .padding(scale::pad2(8, 16));

        // Drag strip: the pill, with the close button at the right.
        let pill = widget::container(widget::space().width(scale::u(40)).height(scale::u(4)))
            .class(win11::fill(p.wash(0.20), 100.0, CLEAR));
        let drag = widget::mouse_area(
            widget::container(pill)
                .width(Length::Fill)
                .padding(scale::pad4(14, 0, 4, 0))
                .align_x(Alignment::Center),
        )
        .on_press(Msg::DragStart);
        let close = flat(
            widgets::ic(Ic::X, 12),
            (8, 8),
            Flat::new(p.text2, win11::rgb(win11::CLOSE_HOVER)).hover_fg(win11::rgb(0xffffff)).radius(100.0),
            Some(Msg::Close),
        );
        // A spacer as wide as the close button keeps the pill centred on the card.
        let top = widget::row::with_capacity(3)
            .align_y(Alignment::Start)
            .push(widget::space().width(Length::Fixed(scale::px(12.0 + 16.0 + 16.0))))
            .push(drag)
            .push(widget::container(close).padding(scale::pad4(8, 16, 0, 0)));

        let body: Element<'a, Msg> = if self.tab == Tab::Clipboard {
            self.clipboard_body(ctx)
        } else {
            pickers_ui::view(self, ctx).map(Msg::Picker)
        };

        let card = widget::container(
            widget::column::with_capacity(5)
                .push(top)
                .push(tabs)
                .push(win11::rule(p.border_subtle))
                .push(body),
        )
        .width(Length::Fixed(scale::px(POPUP_W)))
        .height(Length::Fixed(scale::px(POPUP_H)))
        .clip(true)
        .class(win11::surface(p));

        widget::mouse_area(card).on_press(Msg::Pressed).into()
    }

    fn clipboard_body<'a>(&'a self, ctx: &ViewCtx) -> Element<'a, Msg> {
        let p = ctx.pal;
        if self.items.is_empty() {
            return self.empty_state(p);
        }
        let count = self.filtered_count();
        let mut left = widget::row::with_capacity(2).spacing(scale::u(8)).align_y(Alignment::Center).push(win11::strong("Clipboard", p.text));
        if count > 0 {
            left = left.push(
                widget::container(win11::small(count.to_string(), p.text2))
                    .padding(scale::pad2(2, 8))
                    .class(win11::fill(p.tertiary, 100.0, p.text2)),
            );
        }
        let faint = Color { a: 0.5, ..p.text2 };
        let header = widget::row::with_capacity(4)
            .align_y(Alignment::Center)
            .padding(scale::pad2(12, 16))
            .push(widget::container(left).width(Length::Fill))
            .push(flat(
                widgets::ic(Ic::LayoutList, 16),
                (8, 8),
                Flat::new(if self.compact { p.text2 } else { faint }, p.tertiary),
                Some(Msg::ToggleCompact),
            ))
            .push(flat(
                win11::small("Clear All", if count == 0 { faint } else { p.text2 }),
                (8, 8),
                Flat::new(p.text2, p.tertiary),
                (count > 0).then_some(Msg::ClearAll),
            ));

        let mut col = widget::column::with_capacity(3).push(header);
        if self.search_open {
            col = col.push(
                widget::container(
                    widget::row::with_capacity(2)
                        .align_y(Alignment::Center)
                        .spacing(scale::u(4))
                        .push(
                            win11::search_input(p, "Search history...", &self.search)
                                .id(self.search_id.clone())
                                .on_input(Msg::Search)
                                .on_submit(|_| Msg::Noop)
                                .width(Length::Fill),
                        )
                        .push(flat(
                            widgets::ic(Ic::Regex, 16),
                            (8, 8),
                            Flat::new(if self.regex { win11::rgb(win11::ACCENT) } else { p.text2 }, p.tertiary),
                            Some(Msg::ToggleRegex),
                        )),
                )
                .padding(scale::pad4(4, 12, 8, 12)),
            );
        }
        col = col.push(self.list(ctx));
        col.into()
    }

    fn empty_state<'a>(&'a self, p: Pal) -> Element<'a, Msg> {
        widget::container(
            widget::column::with_capacity(3)
                .align_x(Alignment::Center)
                .push(
                    widget::container(win11::icon(Ic::ClipboardList, 32, p.text3))
                        .width(Length::Fixed(scale::px(64.0)))
                        .height(Length::Fixed(scale::px(64.0)))
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .class(win11::fill(p.bg_tertiary, 100.0, p.text3)),
                )
                .push(widget::space().height(scale::u(16)))
                .push(win11::title("No clipboard history yet", p.text))
                .push(widget::space().height(scale::u(8)))
                .push(
                    widget::container(
                        win11::body("Copy something to see it appear here. Press Super+V to open anytime.", p.text2)
                            .align_x(cosmic::iced::alignment::Horizontal::Center)
                            .width(Length::Fill),
                    )
                    .width(Length::Fixed(scale::px(200.0))),
                ),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    }

    fn list<'a>(&'a self, ctx: &ViewCtx) -> Element<'a, Msg> {
        let p = ctx.pal;
        let filtered = self.filtered();
        if filtered.is_empty() {
            return widget::container(win11::body("No items found", Color { a: 0.6, ..p.text2 }))
                .width(Length::Fill)
                .padding(scale::u(32))
                .align_x(Alignment::Center)
                .into();
        }
        drop(filtered);

        let rows = self.rows();
        let visible = self.visible_items();
        // Virtualise: find the rows intersecting the viewport.
        let mut offsets = Vec::with_capacity(rows.len());
        let mut y = 0.0;
        for row in &rows {
            offsets.push(y);
            y += row_height(*row, self.compact, &visible);
        }
        let total = y;
        let first = offsets
            .iter()
            .rposition(|top| *top <= self.scroll)
            .unwrap_or(0)
            .saturating_sub(OVERSCAN);
        let last = offsets
            .iter()
            .position(|top| *top >= self.scroll + self.viewport_h)
            .unwrap_or(rows.len())
            .saturating_add(OVERSCAN)
            .min(rows.len());

        let mut col = widget::column::with_capacity(last - first + 2).padding(scale::pad2(0, 12));
        col = col.push(widgets::spacer_h(offsets.get(first).copied().unwrap_or(0.0)).map(|()| Msg::Noop));
        let now = chrono::Utc::now();
        for idx in first..last {
            match rows[idx] {
                Row::Header(pinned) => col = col.push(self.section_header(p, pinned, visible.len())),
                Row::Item(i) => {
                    if let Some(item) = visible.get(i) {
                        col = col.push(self.item_card(p, item, i, &now, ctx));
                    }
                }
            }
        }
        let below = total - offsets.get(last).copied().unwrap_or(total);
        col = col.push(widgets::spacer_h(below.max(0.0)).map(|()| Msg::Noop));

        scrollable(col)
            .id(self.list_id.clone())
            .height(Length::Fill)
            .on_scroll(Msg::Scrolled)
            .into()
    }

    fn section_header<'a>(&'a self, p: Pal, pinned: bool, _total: usize) -> Element<'a, Msg> {
        let label = if pinned { "Pinned" } else { "Recent" };
        let count = self
            .filtered()
            .iter()
            .filter(|i| i.pinned == pinned)
            .count();
        let mut row = widget::row::with_capacity(4).align_y(Alignment::Center).spacing(scale::u(6));
        row = row.push(widgets::ic(if pinned { Ic::Pin } else { Ic::History }, 12));
        row = row.push(win11::small(label, p.text3));
        row = row.push(widget::container(win11::small(count.to_string(), Color { a: 0.6, ..p.text3 })).width(Length::Fill));
        if pinned {
            row = row.push(widgets::ic(Ic::ChevronDown, 12));
        }
        let style = Flat::new(p.text3, p.tertiary).radius(4.0);
        let content = widget::container(row).width(Length::Fill).height(Length::Fixed(scale::px(SECTION_H) - scale::px(8.0))).align_y(Alignment::Center);
        let button = flat(content, (4, 4), style, pinned.then_some(Msg::TogglePinned));
        widget::container(button).height(Length::Fixed(scale::px(SECTION_H))).align_y(Alignment::Center).into()
    }

    fn item_card<'a>(
        &'a self,
        p: Pal,
        item: &'a ClipboardItem,
        index: usize,
        now: &chrono::DateTime<chrono::Utc>,
        ctx: &ViewCtx,
    ) -> Element<'a, Msg> {
        let focused = index == self.focused;
        let hovered = self.hovered.as_deref() == Some(item.id.as_str());
        let compact = self.compact;
        let actions: Vec<SmartAction> = if ctx.smart_actions { detect_smart_actions(item) } else { Vec::new() };
        let color = actions.iter().find(|a| a.id == "color-preview").and_then(|a| parse_color(&a.data));
        let is_image = matches!(item.content, ClipboardContent::Image { .. });

        // Icon box: the colour itself for colour codes, else the content-type glyph.
        let box_px = if compact { 24.0 } else { 32.0 };
        let icon_box: Element<'a, Msg> = widget::container(match color {
            Some(_) => Element::from(widget::space()),
            None => win11::icon(if is_image { Ic::Image } else { Ic::Type }, if compact { 12 } else { 16 }, p.text2),
        })
        .width(Length::Fixed(scale::px(box_px)))
        .height(Length::Fixed(scale::px(box_px)))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(win11::fill(color.unwrap_or(p.tertiary), 6.0, p.text2))
        .into();

        let content: Element<'a, Msg> = match &item.content {
            ClipboardContent::Image { width, height, .. } => {
                if compact {
                    win11::body(format!("Image ({width}×{height})"), p.text3).into()
                } else {
                    let mut col = widget::column::with_capacity(2).spacing(scale::u(2));
                    if let Some(Some(handle)) = self.thumbs.get(&item.id) {
                        col = col.push(
                            widget::image(handle.clone())
                                .height(Length::Fixed(scale::px(40.0)))
                                .content_fit(cosmic::iced::ContentFit::Contain),
                        );
                    }
                    col.push(win11::small(format!("{width}×{height}"), p.text3)).into()
                }
            }
            _ => {
                let mut col = widget::column::with_capacity(2).push(win11::body(first_lines(&item.preview, if compact { 1 } else { 2 }), p.text));
                if !compact {
                    col = col.push(widget::space().height(scale::u(4))).push(win11::small(relative_time(&item.timestamp, now), p.text3));
                }
                col.into()
            }
        };

        let mut row = widget::row::with_capacity(3).spacing(scale::u(12)).align_y(Alignment::Start);
        row = row.push(icon_box);
        row = row.push(widget::container(content).width(Length::Fill));
        if hovered {
            let idle = p.text3;
            let hover_bg = p.tertiary;
            let mut buttons = widget::row::with_capacity(4).spacing(scale::u(4)).align_y(Alignment::Center);
            for action in &actions {
                let (ic_, id) = match action.id {
                    "open-link" => (Ic::Link, true),
                    "compose-email" => (Ic::Mail, true),
                    _ => (Ic::Link, false),
                };
                if id {
                    buttons = buttons.push(flat(widgets::ic(ic_, 16), (6, 6), Flat::new(idle, hover_bg), Some(Msg::OpenUrl(action.data.clone()))));
                }
            }
            let pin_color = if item.pinned { win11::rgb(win11::ACCENT) } else { idle };
            buttons = buttons
                .push(flat(
                    widgets::ic(if item.pinned { Ic::PinFilled } else { Ic::Pin }, 16),
                    (6, 6),
                    Flat::new(pin_color, hover_bg),
                    Some(Msg::Pin(item.id.clone())),
                ))
                .push(flat(
                    widgets::ic(Ic::X, 16),
                    (6, 6),
                    Flat::new(idle, hover_bg).hover_fg(win11::rgb(win11::ERROR)),
                    Some(Msg::Delete(item.id.clone())),
                ));
            row = row.push(buttons);
        }

        let height = item_h(item, compact);
        let card = widget::container(row)
            .padding(scale::u(if compact { 8 } else { 12 }))
            .width(Length::Fill)
            .height(Length::Fixed(height))
            .clip(true)
            .class(win11::card(p, hovered, focused || item.pinned));
        let area = widget::mouse_area(card)
            .on_press(Msg::Paste(item.id.clone()))
            .on_enter(Msg::Hover(Some(item.id.clone())))
            .on_exit(Msg::Hover(None));
        widget::container(area).padding(scale::pad4(0, 0, GAP as u16, 0)).into()
    }
}

/// What the view needs from settings.
pub struct ViewCtx {
    pub pal: Pal,
    pub smart_actions: bool,
    pub recent_emojis: Vec<String>,
    pub custom_kaomoji: Vec<app_shared::pickers::Kaomoji>,
}

fn first_lines(text: &str, lines: usize) -> String {
    // Two visual lines of text at most; the card clips whatever wraps beyond that.
    let mut out: Vec<&str> = text.lines().take(lines).collect();
    if out.is_empty() {
        out.push("");
    }
    let joined = out.join("\n");
    let limit = (CHARS_PER_LINE + 4) * lines;
    if joined.chars().count() > limit {
        format!("{}…", joined.chars().take(limit).collect::<String>())
    } else {
        joined
    }
}

fn parse_color(data: &str) -> Option<cosmic::iced::Color> {
    let s = data.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let expand = |c: char| c.to_digit(16).map(|d| (d * 17) as u8);
        let (r, g, b) = match hex.len() {
            3 => {
                let mut it = hex.chars();
                (expand(it.next()?)?, expand(it.next()?)?, expand(it.next()?)?)
            }
            6 => (
                u8::from_str_radix(&hex[0..2], 16).ok()?,
                u8::from_str_radix(&hex[2..4], 16).ok()?,
                u8::from_str_radix(&hex[4..6], 16).ok()?,
            ),
            _ => return None,
        };
        return Some(cosmic::iced::Color::from_rgb8(r, g, b));
    }
    let inner = s.strip_prefix("rgb(").or_else(|| s.strip_prefix("RGB("))?.strip_suffix(')')?;
    let mut parts = inner.split(',').map(|p| p.trim().parse::<u8>().ok());
    Some(cosmic::iced::Color::from_rgb8(parts.next()??, parts.next()??, parts.next()??))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests save UI state; keep that out of the real config directory.
    fn isolate() -> std::sync::MutexGuard<'static, ()> {
        let guard = app_shared::test_util::serial_lock();
        let dir = std::env::temp_dir().join(format!("iced-popup-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
        let _ = std::fs::create_dir_all(config_dir());
        guard
    }

    fn items(n: usize, pinned: usize) -> Vec<ClipboardItem> {
        (0..n)
            .map(|i| {
                let mut item = ClipboardItem::new_text(format!("item {i}"));
                item.pinned = i < pinned;
                item
            })
            .collect()
    }

    #[test]
    fn tabs_wrap_around() {
        assert_eq!(Tab::Kaomoji.next(), Tab::Clipboard);
        assert_eq!(Tab::Clipboard.prev(), Tab::Kaomoji);
    }

    #[test]
    fn collapsed_pinned_section_hides_pinned_items() {
        let _guard = isolate();
        let mut p = Popup::new();
        p.prepare_for_show(Tab::Clipboard, items(5, 2));
        p.pinned_expanded = true;
        assert_eq!(p.visible_items().len(), 5);
        p.pinned_expanded = false;
        assert_eq!(p.visible_items().len(), 3);
        // Up from the first recent item re-opens the section.
        p.focused = 0;
        p.up();
        assert!(p.pinned_expanded);
        assert_eq!(p.focused, 1);
    }

    #[test]
    fn left_collapses_pinned_then_changes_tab() {
        let _guard = isolate();
        let mut p = Popup::new();
        p.prepare_for_show(Tab::Clipboard, items(4, 1));
        p.pinned_expanded = true;
        p.focused = 0;
        p.horizontal(true);
        assert!(!p.pinned_expanded);
        p.horizontal(false);
        assert_eq!(p.tab, Tab::Symbols);
    }

    #[test]
    fn focus_scrolls_into_view() {
        let _guard = isolate();
        let mut p = Popup::new();
        p.prepare_for_show(Tab::Clipboard, items(30, 0));
        p.viewport_h = 300.0;
        p.focused = 0;
        assert_eq!(p.scroll_target(), None);
        p.focused = 20;
        let target = p.scroll_target().expect("must scroll");
        assert!(target > 0.0);
        p.scroll = target;
        assert_eq!(p.scroll_target(), None);
    }

    #[test]
    fn search_filters_and_resets_focus() {
        let _guard = isolate();
        let mut p = Popup::new();
        p.prepare_for_show(Tab::Clipboard, items(10, 0));
        p.focused = 5;
        p.search = "item 7".into();
        p.after_filter_change();
        assert_eq!(p.focused, 0);
        assert_eq!(p.visible_items().len(), 1);
    }

    #[test]
    fn colors_parse() {
        assert!(parse_color("#fff").is_some());
        assert!(parse_color("#1e90ff").is_some());
        assert!(parse_color("rgb(10, 20, 30)").is_some());
        assert!(parse_color("nope").is_none());
    }

    // --- Headless UI tests ---

    fn ui<'a>(popup: &'a Popup, ctx: &ViewCtx) -> iced_test::Simulator<'a, Msg, cosmic::Theme, cosmic::Renderer> {
        iced_test::Simulator::with_size(
            cosmic::iced::Settings::default(),
            cosmic::iced::Size::new(POPUP_W, POPUP_H),
            popup.view(ctx),
        )
    }

    fn ctx() -> ViewCtx {
        ViewCtx { pal: crate::win11::Pal::new(true, 0.8), smart_actions: true, recent_emojis: Vec::new(), custom_kaomoji: Vec::new() }
    }

    #[test]
    fn clear_all_and_item_clicks_send_messages() {
        let _headless = crate::testing::headless();
        let _guard = isolate();
        let mut p = Popup::new();
        p.prepare_for_show(Tab::Clipboard, items(3, 0));
        let ctx = ctx();
        let first_id = p.items[0].id.clone();
        let mut screen = ui(&p, &ctx);
        screen.click(crate::testing::text("Clear All")).unwrap();
        screen.click(crate::testing::text("item 0")).unwrap();
        let messages: Vec<_> = screen.into_messages().collect();
        assert!(matches!(messages.first(), Some(Msg::ClearAll)), "{messages:?}");
        assert!(messages.iter().any(|m| matches!(m, Msg::Paste(id) if *id == first_id)), "{messages:?}");
    }

    #[test]
    fn empty_history_shows_the_hint() {
        let _headless = crate::testing::headless();
        let _guard = isolate();
        let mut p = Popup::new();
        p.prepare_for_show(Tab::Clipboard, Vec::new());
        let ctx = ctx();
        let mut screen = ui(&p, &ctx);
        assert!(screen.find(crate::testing::text("No clipboard history yet")).is_ok());
    }
}
