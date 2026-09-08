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
    last_version: u64,
}

impl Popup {
    pub fn new(
        backend: Arc<BackendService>,
        settings: AppSettings,
        focus: FocusHandle,
        search_focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let is_dark = resolve_dark(&settings);
        let items = backend.snapshot();
        let version = backend.version();
        let ui = load_ui_state();
        // Initial keyboard focus is set by main.rs after the window opens.
        Self {
            backend,
            settings,
            is_dark,
            items,
            tab: Tab::Clipboard,
            search: SearchState::new(search_focus),
            search_visible: false,
            focused: 0,
            compact: ui.compact,
            pinned_expanded: ui.pinned_expanded,
            focus,
            last_version: version,
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
            if self.search_visible {
                self.close_search(window, cx);
            } else {
                cx.hide();
            }
            cx.stop_propagation();
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
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.25))
                .text_color(if self.is_dark {
                    theme::dark::text_secondary()
                } else {
                    theme::light::text_secondary()
                })
                .child(format!("{} — coming in Phase 3", self.tab.label()))
                .into_any_element();
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
