//! Popup root — port of `ClipboardApp.tsx` (clipboard tab) + `DragHandle.tsx`.
//!
//! Owns all UI state. Backend mutations go through `BackendService`; the 500ms
//! watcher thread bumps a version counter and the poll task refreshes the snapshot.

use std::sync::Arc;

use gpui::{
    Context, FocusHandle, Focusable, KeyDownEvent, Render, Window, div, prelude::*, px,
};
use serde::{Deserialize, Serialize};

use super::header::render_header;
use super::history_item::{CardParams, render_history_item};
use super::icons::{self, icon};
use super::search::SearchState;
use super::tabbar::render_tabbar;
use crate::backend::BackendService;
use crate::history::{ClipboardItem, filter_history};
use crate::pickers::{Emoji, Kaomoji, SymbolItem};
use crate::settings::{AppSettings, config_dir, resolve_dark};
use crate::theme;
use win11_clipboard_history_lib::focus_manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Clipboard,
    Symbols,
    Emoji,
    Kaomoji,
}

/// Which search field — clipboard + one per picker (parity: independent state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchWhich {
    Clipboard,
    Emoji,
    Kaomoji,
    Symbol,
}

/// Per-picker UI state (mirrors the React picker hooks' local state).
pub struct PickerTabState {
    pub search: SearchState,
    pub focused_main: usize,
    pub focused_recent: usize,
    pub focused_category: usize,
    pub category: Option<String>,
    /// (glyph/text, name/category) for the footer preview.
    pub hovered: Option<(String, String)>,
}

const UI_STATE_FILE: &str = "ui_state.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UiState {
    #[serde(default)]
    compact: bool,
    #[serde(default = "default_expanded")]
    pinned_expanded: bool,
}

fn default_expanded() -> bool {
    true
}

pub struct Popup {
    pub backend: Arc<BackendService>,
    pub settings: AppSettings,
    pub is_dark: bool,
    pub items: Vec<ClipboardItem>,
    pub tab: Tab,
    pub search: SearchState,
    pub search_visible: bool,
    pub focused: usize,
    pub compact: bool,
    pub pinned_expanded: bool,
    pub focus: FocusHandle,
    pub emoji: PickerTabState,
    pub kaomoji: PickerTabState,
    pub symbol: PickerTabState,
    pub symbol_recents: Vec<SymbolItem>,
    last_version: u64,
}

impl PickerTabState {
    fn new(search_focus: FocusHandle) -> Self {
        Self {
            search: SearchState::new(search_focus),
            focused_main: 0,
            focused_recent: 0,
            focused_category: 0,
            category: None,
            hovered: None,
        }
    }
}

impl Popup {
    pub fn new(
        backend: Arc<BackendService>,
        settings: AppSettings,
        focus: FocusHandle,
        search_focus: FocusHandle,
        emoji_focus: FocusHandle,
        kaomoji_focus: FocusHandle,
        symbol_focus: FocusHandle,
        initial_tab: Tab,
        cx: &mut Context<Self>,
    ) -> Self {
        let is_dark = resolve_dark(&settings);
        let items = backend.snapshot();
        let version = backend.version();
        let ui = load_ui_state();
        let symbol_recents = crate::pickers::load_recent_symbols();
        // Initial keyboard focus is set by main.rs after the window opens.
        Self {
            backend,
            settings,
            is_dark,
            items,
            tab: initial_tab,
            search: SearchState::new(search_focus),
            search_visible: false,
            focused: 0,
            compact: ui.compact,
            pinned_expanded: ui.pinned_expanded,
            focus,
            emoji: PickerTabState::new(emoji_focus),
            kaomoji: PickerTabState::new(kaomoji_focus),
            symbol: PickerTabState::new(symbol_focus),
            symbol_recents,
            last_version: version,
        }
    }

    pub fn search_mut(&mut self, which: SearchWhich) -> &mut SearchState {
        match which {
            SearchWhich::Clipboard => &mut self.search,
            SearchWhich::Emoji => &mut self.emoji.search,
            SearchWhich::Kaomoji => &mut self.kaomoji.search,
            SearchWhich::Symbol => &mut self.symbol.search,
        }
    }

    fn picker_mut(&mut self, tab: Tab) -> Option<&mut PickerTabState> {
        match tab {
            Tab::Emoji => Some(&mut self.emoji),
            Tab::Kaomoji => Some(&mut self.kaomoji),
            Tab::Symbols => Some(&mut self.symbol),
            Tab::Clipboard => None,
        }
    }

    fn search_which(tab: Tab) -> SearchWhich {
        match tab {
            Tab::Clipboard => SearchWhich::Clipboard,
            Tab::Emoji => SearchWhich::Emoji,
            Tab::Kaomoji => SearchWhich::Kaomoji,
            Tab::Symbols => SearchWhich::Symbol,
        }
    }

    /// Reset per-picker grid focus when its filter changes (parity: index reset effects).
    pub fn after_picker_filter_change(&mut self, which: SearchWhich) {
        match which {
            SearchWhich::Clipboard => self.after_filter_change(),
            SearchWhich::Emoji => self.emoji.focused_main = 0,
            SearchWhich::Kaomoji => self.kaomoji.focused_main = 0,
            SearchWhich::Symbol => self.symbol.focused_main = 0,
        }
    }

    /// Pull a fresh snapshot when the watcher bumped the version. Returns true on change.
    pub fn poll_backend(&mut self, cx: &mut Context<Self>) -> bool {
        let version = self.backend.version();
        if version == self.last_version {
            return false;
        }
        self.last_version = version;
        self.refresh_items();
        self.after_filter_change();
        cx.notify();
        true
    }

    pub fn refresh_items(&mut self) {
        self.items = self.backend.snapshot();
    }

    /// Mirror the React effect: reset focused index when filtered results change.
    pub fn after_filter_change(&mut self) {
        self.focused = 0;
    }

    pub fn save_ui_state(&self) {
        let ui = UiState {
            compact: self.compact,
            pinned_expanded: self.pinned_expanded,
        };
        if let Ok(content) = serde_json::to_string_pretty(&ui) {
            let _ = std::fs::write(config_dir().join(UI_STATE_FILE), content);
        }
    }

    /// Flat visible list for keyboard nav + paste (mirrors `visibleItems`).
    fn visible_items(&self) -> Vec<&ClipboardItem> {
        let filtered = filter_history(&self.items, &self.search.text, self.search.regex_mode);
        let show_sections = self.search.text.is_empty() && filtered.iter().any(|i| i.pinned);
        if show_sections && !self.pinned_expanded {
            filtered.into_iter().filter(|i| !i.pinned).collect()
        } else {
            filtered
        }
    }

    fn filtered_count(&self) -> usize {
        filter_history(&self.items, &self.search.text, self.search.regex_mode).len()
    }

    pub fn open_smart_url(&self, url: &str) {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }

    // --- Picker flows (parity with `paste_text` command + picker hooks) ---

    fn paste_picker_text(&mut self, text: &str, record_emoji: bool, cx: &mut Context<Self>) {
        cx.hide();
        let _ = focus_manager::restore_focused_window();
        let _ = self.backend.paste_text(text, record_emoji);
        cx.notify();
    }

    pub fn paste_emoji(&mut self, ch: &str, cx: &mut Context<Self>) {
        self.paste_picker_text(ch, true, cx);
    }

    pub fn paste_kaomoji(&mut self, text: &str, cx: &mut Context<Self>) {
        self.paste_picker_text(text, false, cx);
    }

    pub fn paste_symbol(&mut self, symbol: &SymbolItem, cx: &mut Context<Self>) {
        let ch = symbol.char.clone();
        crate::pickers::record_symbol_usage(symbol);
        self.symbol_recents = crate::pickers::load_recent_symbols();
        self.paste_picker_text(&ch, false, cx);
    }

    pub fn emoji_filtered(&self) -> Vec<Emoji> {
        let q = self.emoji.search.text.trim().to_string();
        if !q.is_empty() {
            return crate::pickers::search_emojis(&q, 100);
        }
        let all = crate::pickers::load_emojis();
        if let Some(cat) = &self.emoji.category {
            return all.into_iter().filter(|e| &e.category == cat).collect();
        }
        let recent = self.backend.recent_emojis();
        if recent.is_empty() {
            return all;
        }
        let map: std::collections::HashMap<&str, &Emoji> =
            all.iter().map(|e| (e.char.as_str(), e)).collect();
        let mut out: Vec<Emoji> = recent
            .iter()
            .filter_map(|r| map.get(r.char.as_str()).map(|e| (*e).clone()))
            .collect();
        let recent_chars: std::collections::HashSet<String> =
            out.iter().map(|e| e.char.clone()).collect();
        out.extend(all.into_iter().filter(|e| !recent_chars.contains(&e.char)));
        out
    }

    pub fn kaomoji_filtered(&self) -> Vec<Kaomoji> {
        let custom: Vec<Kaomoji> = self
            .settings
            .custom_kaomojis
            .iter()
            .enumerate()
            .map(|(i, c)| Kaomoji {
                id: format!("custom-{i}"),
                text: c.text.clone(),
                category: c.category.clone(),
                keywords: c.keywords.clone(),
            })
            .collect();
        crate::pickers::get_kaomojis(
            self.kaomoji.category.as_deref(),
            &self.kaomoji.search.text,
            &custom,
        )
    }

    pub fn symbol_filtered(&self) -> Vec<SymbolItem> {
        crate::pickers::get_symbols(
            self.symbol.category.as_deref(),
            &self.symbol.search.text,
        )
    }

    /// Grid navigation port of `useKeyboardNavigation` (pure index math).
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
            "home" => Some(if ctrl {
                0
            } else {
                current / cols * cols
            }),
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

    fn search_ref(&self, which: SearchWhich) -> &SearchState {
        match which {
            SearchWhich::Clipboard => &self.search,
            SearchWhich::Emoji => &self.emoji.search,
            SearchWhich::Kaomoji => &self.kaomoji.search,
            SearchWhich::Symbol => &self.symbol.search,
        }
    }

    /// Picker-tab key routing: editor when focused, grid nav otherwise,
    /// Ctrl+Left/Right to switch tabs (parity outcomes, see Phase 3 CONTEXT).
    fn handle_picker_key(
        &mut self,
        tab: Tab,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;
        let ctrl = mods.control && !mods.alt;
        let which = Self::search_which(tab);

        if ctrl && (key == "f" || key == "F") {
            let h = self.search_ref(which).focus.clone();
            h.focus(window);
            cx.stop_propagation();
            return;
        }

        if self.search_ref(which).focus.is_focused(window) {
            if key == "enter" {
                // Parity: Enter inside search does nothing.
                cx.stop_propagation();
                return;
            }
            let consumed = self.search_mut(which).handle_key(event);
            if consumed {
                self.after_picker_filter_change(which);
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }

        if ctrl && (key == "left" || key == "right") {
            self.tab = if key == "left" {
                self.tab.prev()
            } else {
                self.tab.next()
            };
            self.focused = 0;
            cx.notify();
            cx.stop_propagation();
            return;
        }

        match key {
            "enter" | " " => {
                self.paste_picker_at(tab, window, cx);
                cx.stop_propagation();
            }
            "up" | "down" | "left" | "right" | "home" | "end" | "pagedown" | "pageup" => {
                let cols = match tab {
                    Tab::Kaomoji => super::pickers::kaomoji_columns(window),
                    _ => super::pickers::grid_columns(window),
                };
                let len = match tab {
                    Tab::Emoji => self.emoji_filtered().len(),
                    Tab::Kaomoji => self.kaomoji_filtered().len(),
                    Tab::Symbols => self.symbol_filtered().len(),
                    Tab::Clipboard => 0,
                };
                let focused = self.picker_mut(tab).map(|p| p.focused_main).unwrap_or(0);
                if let Some(n) = Self::grid_move(focused, key, ctrl, cols, len) {
                    if let Some(p) = self.picker_mut(tab) {
                        p.focused_main = n;
                    }
                    cx.notify();
                }
                cx.stop_propagation();
            }
            _ => {
                if !mods.control && !mods.alt && !mods.platform && key.chars().count() == 1 {
                    let h = self.search_ref(which).focus.clone();
                    h.focus(window);
                    let s = self.search_mut(which);
                    s.text.push_str(key);
                    s.cursor = s.text.len();
                    self.after_picker_filter_change(which);
                    cx.notify();
                    cx.stop_propagation();
                }
            }
        }
    }

    fn paste_picker_at(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        match tab {
            Tab::Emoji => {
                let idx = self.emoji.focused_main;
                if let Some(e) = self.emoji_filtered().get(idx).cloned() {
                    self.paste_emoji(&e.char, cx);
                }
            }
            Tab::Kaomoji => {
                let idx = self.kaomoji.focused_main;
                if let Some(k) = self.kaomoji_filtered().get(idx).cloned() {
                    self.paste_kaomoji(&k.text, cx);
                }
            }
            Tab::Symbols => {
                let idx = self.symbol.focused_main;
                if let Some(s) = self.symbol_filtered().get(idx).cloned() {
                    self.paste_symbol(&s, cx);
                }
            }
            Tab::Clipboard => {}
        }
    }

    pub fn paste_item_by_id(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.backend.snapshot().iter().find(|i| i.id == id).cloned() else {
            self.refresh_items();
            cx.notify();
            return;
        };
        // Parity with the Tauri `paste_item` command: hide → restore focus → paste.
        cx.hide();
        let _ = focus_manager::restore_focused_window();
        if self.backend.paste(&item).is_err() {
            self.refresh_items();
        } else {
            self.refresh_items();
            self.after_filter_change();
        }
        let _ = window;
        cx.notify();
    }

    fn paste_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self
            .visible_items()
            .get(self.focused)
            .map(|item| item.id.clone());
        if let Some(id) = id {
            self.paste_item_by_id(&id, window, cx);
        }
    }

    fn move_focus(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.visible_items().len();
        if len == 0 {
            return;
        }
        let next = (self.focused as isize + delta).clamp(0, len as isize - 1) as usize;
        self.focused = next;
        cx.notify();
    }

    fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_visible = !self.search_visible;
        if !self.search_visible {
            self.search.clear();
            self.after_filter_change();
            self.focus.focus(window);
        } else {
            self.search.focus.focus(window);
        }
        cx.notify();
    }

    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_visible = false;
        self.search.clear();
        self.after_filter_change();
        self.focus.focus(window);
        cx.notify();
    }

    fn type_to_filter(&mut self, ch: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_visible {
            self.search_visible = true;
            self.search.clear();
            self.search.focus.focus(window);
        }
        self.search.text.push_str(ch);
        self.search.cursor = self.search.text.len();
        self.after_filter_change();
        cx.notify();
    }

    /// Global key handling — mirrors `ClipboardTab.handleKeyDown` + item Enter/Space.
    /// Returns nothing; consumes via `cx.stop_propagation()` where appropriate.
    pub fn handle_root_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;

        // Ctrl+F toggles search from anywhere.
        if mods.control && !mods.alt && (key == "f" || key == "F") {
            self.toggle_search(window, cx);
            cx.stop_propagation();
            return;
        }

        // Escape closes search first, otherwise hides the popup (CORE-06).
        if key == "escape" {
            if self.tab == Tab::Clipboard && self.search_visible {
                self.close_search(window, cx);
            } else {
                cx.hide();
            }
            cx.stop_propagation();
            return;
        }

        // Picker tabs: dedicated routing (grid nav owns the arrows here).
        if self.tab != Tab::Clipboard {
            let tab = self.tab;
            self.handle_picker_key(tab, event, window, cx);
            return;
        }

        // Route to the editor while it is focused.
        if self.search_visible && self.search.focus.is_focused(window) {
            if key == "enter" {
                // Parity: Enter inside search does nothing.
                cx.stop_propagation();
                return;
            }
            if self.search.handle_key(event) {
                self.after_filter_change();
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }

        match key {
            "enter" | " " => {
                self.paste_focused(window, cx);
                cx.stop_propagation();
            }
            "up" => {
                // Collapse-aware: Up from the first recent item re-opens pinned.
                let filtered =
                    filter_history(&self.items, &self.search.text, self.search.regex_mode);
                let show_sections =
                    self.search.text.is_empty() && filtered.iter().any(|i| i.pinned);
                if show_sections && !self.pinned_expanded && self.focused == 0 {
                    self.pinned_expanded = true;
                    self.save_ui_state();
                    let pinned = filtered.iter().filter(|i| i.pinned).count();
                    self.focused = pinned.saturating_sub(1);
                    cx.notify();
                } else {
                    self.move_focus(-1, cx);
                }
                cx.stop_propagation();
            }
            "down" => {
                self.move_focus(1, cx);
                cx.stop_propagation();
            }
            "home" => {
                self.focused = 0;
                cx.notify();
                cx.stop_propagation();
            }
            "end" => {
                let len = self.visible_items().len();
                if len > 0 {
                    self.focused = len - 1;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "left" | "right" => {
                // Collapse pinned with Left (parity); otherwise switch tabs.
                let filtered =
                    filter_history(&self.items, &self.search.text, self.search.regex_mode);
                let show_sections =
                    self.search.text.is_empty() && filtered.iter().any(|i| i.pinned);
                let pinned = filtered.iter().filter(|i| i.pinned).count();
                if key == "left" && show_sections && self.pinned_expanded && self.focused < pinned
                {
                    self.pinned_expanded = false;
                    self.save_ui_state();
                    self.focused = 0;
                    cx.notify();
                } else {
                    self.tab = if key == "left" {
                        self.tab.prev()
                    } else {
                        self.tab.next()
                    };
                    self.focused = 0;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            _ => {
                if !mods.control && !mods.alt && !mods.platform && key.chars().count() == 1 {
                    self.type_to_filter(key, window, cx);
                    cx.stop_propagation();
                }
            }
        }
    }
}

impl Tab {
    fn prev(self) -> Self {
        match self {
            Tab::Clipboard => Tab::Kaomoji,
            Tab::Symbols => Tab::Clipboard,
            Tab::Emoji => Tab::Symbols,
            Tab::Kaomoji => Tab::Emoji,
        }
    }

    fn next(self) -> Self {
        match self {
            Tab::Clipboard => Tab::Symbols,
            Tab::Symbols => Tab::Emoji,
            Tab::Emoji => Tab::Kaomoji,
            Tab::Kaomoji => Tab::Clipboard,
        }
    }
}

impl Focusable for Popup {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Popup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_dark = self.is_dark;
        let opacity = if is_dark {
            self.settings.dark_background_opacity
        } else {
            self.settings.light_background_opacity
        };
        div()
            .id("popup-root")
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .rounded(px(12.))
            .bg(if is_dark {
                theme::dark::acrylic(opacity)
            } else {
                theme::light::acrylic(opacity)
            })
            .text_color(if is_dark {
                theme::dark::text_primary()
            } else {
                theme::light::text_primary()
            })
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_root_key(event, window, cx);
            }))
            .child(render_drag_strip(is_dark, cx))
            .child(render_tabbar(self, window, cx))
            .child(self.render_body(window, cx))
    }
}

impl Popup {
    fn render_body(&self, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
        if self.tab != Tab::Clipboard {
            return super::pickers::render_picker_tab(self, window, cx);
        }
        if self.items.is_empty() {
            return self.render_empty(cx).into_any_element();
        }
        let count = self.filtered_count();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h(px(0.))
            .child(render_header(self, count, window, cx))
            .children(self.search_visible.then(|| {
                div()
                    .px(px(12.))
                    .pb(px(8.))
                    .pt(px(4.))
                    .child(self.search.render(self.is_dark, self.settings.secondary_opacity(self.is_dark), window, cx))
            }))
            .child(self.render_list(window, cx))
            .into_any_element()
    }

    fn render_list(&self, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
        let filtered = filter_history(&self.items, &self.search.text, self.search.regex_mode);
        if filtered.is_empty() {
            let secondary = if self.is_dark {
                theme::dark::text_secondary()
            } else {
                theme::light::text_secondary()
            };
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .p(px(32.))
                .text_size(px(12.25))
                .text_color(secondary)
                .opacity(0.6)
                .child(if self.search.text.is_empty() {
                    "No clipboard history yet"
                } else {
                    "No items found"
                })
                .into_any_element();
        }
        let show_sections = self.search.text.is_empty() && filtered.iter().any(|i| i.pinned);
        div()
            .id("history-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .children(if show_sections {
                self.render_sections(&filtered, window, cx)
            } else {
                filtered
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| {
                        render_history_item(
                            item,
                            &CardParams {
                                index: idx,
                                focused: idx == self.focused,
                            },
                            self,
                            window,
                            cx,
                        )
                        .into_any_element()
                    })
                    .collect::<Vec<_>>()
            })
            .into_any_element()
    }

    fn render_sections(
        &self,
        filtered: &[&ClipboardItem],
        window: &Window,
        cx: &mut Context<Popup>,
    ) -> Vec<gpui::AnyElement> {
        let (mut pinned, mut unpinned): (Vec<&ClipboardItem>, Vec<&ClipboardItem>) =
            (Vec::new(), Vec::new());
        for item in filtered {
            if item.pinned {
                pinned.push(*item);
            } else {
                unpinned.push(*item);
            }
        }
        let mut out = Vec::new();
        out.push(render_section_header(
            self,
            0,
            icons::PIN,
            "Pinned",
            pinned.len(),
            Some(!self.pinned_expanded),
            cx,
        ));
        if self.pinned_expanded {
            for (offset, item) in pinned.iter().enumerate() {
                out.push(
                    render_history_item(
                        item,
                        &CardParams {
                            index: offset,
                            focused: offset == self.focused,
                        },
                        self,
                        window,
                        cx,
                    )
                    .into_any_element(),
                );
            }
        }
        if !unpinned.is_empty() {
            out.push(render_section_header(
                self,
                1,
                icons::HISTORY,
                "Recent",
                unpinned.len(),
                None,
                cx,
            ));
            for (offset, item) in unpinned.iter().enumerate() {
                let idx = pinned.len() + offset;
                out.push(
                    render_history_item(
                        item,
                        &CardParams {
                            index: idx,
                            focused: idx == self.focused,
                        },
                        self,
                        window,
                        cx,
                    )
                    .into_any_element(),
                );
            }
        }
        out
    }

    fn render_empty(&self, cx: &mut Context<Popup>) -> impl IntoElement {
        let is_dark = self.is_dark;
        let (circle_bg, icon_color, title, body) = if is_dark {
            (
                theme::dark::bg_tertiary(),
                theme::dark::text_tertiary(),
                theme::dark::text_primary(),
                theme::dark::text_secondary(),
            )
        } else {
            (
                theme::light::bg_tertiary(),
                theme::light::text_secondary(),
                theme::light::text_primary(),
                theme::light::text_secondary(),
            )
        };
        let _ = cx;
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py(px(48.))
            .px(px(16.))
            .child(
                div()
                    .w(px(64.))
                    .h(px(64.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .mb(px(16.))
                    .bg(circle_bg)
                    .child(icon(icons::CLIPBOARD_LIST, px(32.)).text_color(icon_color)),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .mb(px(8.))
                    .text_color(title)
                    .child("No clipboard history yet"),
            )
            .child(
                div()
                    .text_size(px(12.25))
                    .text_color(body)
                    .max_w(px(200.))
                    .text_center()
                    .child("Copy something to see it appear here. Press Super+V to open anytime."),
            )
    }
}

fn render_section_header(
    state: &Popup,
    idx: usize,
    icon_name: &str,
    label: &str,
    count: usize,
    collapsed: Option<bool>,
    cx: &mut Context<Popup>,
) -> gpui::AnyElement {
    let is_dark = state.is_dark;
    let tertiary = state.settings.tertiary_opacity(is_dark);
    let idle = if is_dark {
        theme::dark::text_tertiary()
    } else {
        theme::light::text_secondary()
    };
    let hover_c = if is_dark {
        theme::dark::text_secondary()
    } else {
        theme::light::text_secondary()
    };
    let is_pinned_section = label == "Pinned";
    div()
        .id(("section", idx))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .px(px(4.))
        .py(px(4.))
        .rounded(px(4.))
        .text_size(px(10.5))
        .text_color(idle)
        .hover(move |s| s.text_color(hover_c).bg(theme::tertiary_bg(is_dark, tertiary)))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            if is_pinned_section {
                this.pinned_expanded = !this.pinned_expanded;
                this.save_ui_state();
                this.focused = 0;
                cx.notify();
            }
        }))
        .child(icon(icon_name, px(12.)).flex_shrink_0())
        .child(label.to_string())
        .child(div().flex_1().opacity(0.6).child(format!("{count}")))
        // Micro-delta (documented): React rotates the chevron -90° when collapsed;
        // GPUI Svg has no cheap rotate here, so the chevron stays static.
        .children(collapsed.map(|_| {
            icon(icons::CHEVRON_DOWN, px(12.)).flex_shrink_0()
        }))
        .into_any_element()
}

/// Drag strip — port of `DragHandle.tsx`: centered pill + close button.
/// Window dragging wires to the platform in T3 (best-effort).
fn render_drag_strip(is_dark: bool, cx: &mut Context<Popup>) -> impl IntoElement {
    div()
        .id("drag-strip")
        .relative()
        .w_full()
        .flex()
        .justify_center()
        .pt(px(16.))
        .pb(px(4.))
        .cursor_grab()
        .child(
            div()
                .w(px(40.))
                .h(px(4.))
                .rounded_full()
                .bg(if is_dark {
                    gpui::rgba(0xffffff33)
                } else {
                    gpui::rgba(0x00000033)
                }),
        )
        .child(
            div()
                .id("drag-close")
                .absolute()
                .right(px(16.))
                .top(px(8.))
                .p(px(4.))
                .pt(px(20.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_color(if is_dark {
                    gpui::rgba(0xffffff80)
                } else {
                    gpui::rgba(0x00000080)
                })
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.hide();
                }))
                .child(icon(icons::X, px(20.)).flex_shrink_0()),
        )
}

fn load_ui_state() -> UiState {
    let path = config_dir().join(UI_STATE_FILE);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Clipboard => "Clipboard",
            Tab::Symbols => "Symbols",
            Tab::Emoji => "Emoji",
            Tab::Kaomoji => "Kaomoji",
        }
    }
}
