//! Popup root — port of `ClipboardApp.tsx` (clipboard tab) + `DragHandle.tsx`.
//!
//! Owns all UI state. Backend mutations go through `BackendService`; the 500ms
//! watcher thread bumps a version counter and the poll task refreshes the snapshot.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, Context, FocusHandle, Focusable, KeyDownEvent, ListAlignment,
    ListOffset, ListState, Pixels, Point, Render, RenderImage, ScrollStrategy,
    UniformListScrollHandle, Window, div, list, point, prelude::*, px,
};
use serde::{Deserialize, Serialize};

use super::header::render_header;
use super::history_item::{CardParams, decode_thumb, render_history_item};
use super::icons::{self, icon};
use super::search::SearchState;
use super::tabbar::render_tabbar;
use crate::app_state::Shared;
use crate::backend::BackendService;
use crate::history::{ClipboardContent, ClipboardItem, filter_history};
use crate::pickers::{Emoji, Kaomoji, SymbolItem};
use crate::settings::{AppSettings, config_dir};
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
    /// Horizontal scroll of the category strip (React's `CategoryStrip` keeps
    /// its own `scrollContainerRef` the same way).
    pub category_scroll: gpui::ScrollHandle,
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

/// Overlay mode (`PopupSurface::Overlay`): the window is a transparent surface
/// covering the whole output and the popup is a card drawn inside it.
pub struct OverlayCard {
    /// Top-left of the card in window px; `None` until the pointer's first
    /// position tells where the popup should open.
    pos: Option<Point<Pixels>>,
    /// Pointer offset inside the card while its header is being dragged.
    grab: Option<Point<Pixels>>,
    opened: Instant,
}

/// Compositor blur behind the overlay card (`ext-background-effect-v1`).
/// Opt-in with `WIN11_CLIPBOARD_BLUR=1` until it has been checked on a real
/// compositor; the card is acrylic-tinted either way.
pub fn overlay_blur_enabled() -> bool {
    std::env::var("WIN11_CLIPBOARD_BLUR").as_deref() == Ok("1")
}

/// How long the overlay waits for the pointer's first position before opening
/// bottom-center instead.
const OVERLAY_POINTER_WAIT: Duration = Duration::from_millis(150);

pub struct Popup {
    pub backend: Arc<BackendService>,
    pub shared: Shared,
    pub settings: AppSettings,
    pub is_dark: bool,
    pub items: Vec<ClipboardItem>,
    /// Decoded preview textures by item id. The app owns these `RenderImage`s
    /// so their GPU atlas tiles can be freed via `drop_image` when the item
    /// leaves history — GPUI itself never evicts atlas entries.
    thumbs: HashMap<String, Arc<RenderImage>>,
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
    /// Virtualized clipboard list: state + row model + its signature (the
    /// pickers carry their own handles inside `UniformListScrollHandle`).
    pub list_state: ListState,
    /// The current row model (section headers + cards) of the virtualized
    /// list, rebuilt each frame from the filtered history.
    list_rows: Vec<ListRow>,
    /// Signature of [`Self::list_rows`] — the list state is reset (which
    /// re-measures and returns to the top) only when this changes.
    rows_signature: u64,
    pub emoji_scroll: UniformListScrollHandle,
    pub kaomoji_scroll: UniformListScrollHandle,
    pub symbol_scroll: UniformListScrollHandle,
    last_version: u64,
    last_settings_version: u64,
    last_settings_mtime: Option<std::time::SystemTime>,
    /// Bumped on every show so the appear animation gets a fresh element id
    /// and visibly restarts (the window — and this view — is persistent now).
    appear_gen: u64,
    /// Armed focus-loss dismissal. Dropping the subscription would silently
    /// stop the popup from ever closing itself, so it lives as long as the view.
    activation_watch: Option<gpui::Subscription>,
    /// Timestamp of the last focus loss with the pointer outside the popup.
    /// Dismissal fires only if focus stays away past [`FOCUS_LOSS_GRACE`]
    /// (see `check_focus_loss_dismiss`): on focus-follows-mouse desktops a
    /// brief excursion must not close the popup, only a real move-away/click.
    focus_loss_armed_at: Option<Instant>,
    /// `Some` when the popup is an overlay card rather than a window.
    pub overlay: Option<OverlayCard>,
}

/// One row of the virtualized history list: a section header or a card.
#[derive(Clone)]
enum ListRow {
    Header {
        pinned: bool,
        count: usize,
        collapsed: bool,
    },
    Item {
        /// Index into `Popup::items` for O(1) lookup at render time.
        item_ix: usize,
        /// Item id — part of the signature so a same-count replacement still
        /// invalidates the cached row measurements.
        id: String,
        /// Card index used for focus highlighting (the `visible_items` index).
        card_index: usize,
    },
}

/// FNV fold over the row model — any change here means the list state must be
/// reset (rows re-measured).
fn rows_signature(rows: &[ListRow]) -> u64 {
    use win11_clipboard_history_lib::clipboard_manager::calculate_hash;
    let mut combined = rows.len() as u64;
    for row in rows {
        let row_hash = match row {
            ListRow::Header {
                pinned,
                count,
                collapsed,
            } => calculate_hash(&("header", *pinned, *count, *collapsed)),
            ListRow::Item {
                item_ix,
                id,
                card_index,
            } => calculate_hash(&("item", *item_ix, id.as_str(), *card_index)),
        };
        combined = combined.rotate_left(9) ^ row_hash;
    }
    combined
}

/// Grace between focus loss (pointer outside) and auto-dismissal. Long
/// enough to survive focus flicker and a quick mouse pass-through, short
/// enough that an outside click still feels like it closes the popup.
const FOCUS_LOSS_GRACE: Duration = Duration::from_millis(350);

impl PickerTabState {
    fn new(search_focus: FocusHandle) -> Self {
        Self {
            search: SearchState::new(search_focus),
            focused_main: 0,
            focused_recent: 0,
            focused_category: 0,
            category: None,
            hovered: None,
            category_scroll: gpui::ScrollHandle::new(),
        }
    }
}

impl Popup {
    pub fn new(
        backend: Arc<BackendService>,
        shared: Shared,
        settings: AppSettings,
        focus: FocusHandle,
        search_focus: FocusHandle,
        emoji_focus: FocusHandle,
        kaomoji_focus: FocusHandle,
        symbol_focus: FocusHandle,
        initial_tab: Tab,
        cx: &mut Context<Self>,
    ) -> Self {
        // Resolve + install the palette (GTK mode may read the desktop theme).
        let resolved = theme::resolve(&settings);
        shared.lock().theme = resolved;
        let is_dark = resolved.is_dark;
        let items = backend.snapshot();
        let version = backend.version();
        let ui = load_ui_state();
        let symbol_recents = crate::pickers::load_recent_symbols();
        // Initial keyboard focus is set by main.rs after the window opens.
        let mut this = Self {
            backend,
            shared,
            settings,
            is_dark,
            items,
            thumbs: HashMap::new(),
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
            list_state: ListState::new(0, ListAlignment::Top, px(600.)),
            list_rows: Vec::new(),
            rows_signature: 0,
            emoji_scroll: UniformListScrollHandle::default(),
            kaomoji_scroll: UniformListScrollHandle::default(),
            symbol_scroll: UniformListScrollHandle::default(),
            last_version: version,
            last_settings_version: 1,
            last_settings_mtime: crate::settings::settings_mtime(),
            appear_gen: 0,
            activation_watch: None,
            focus_loss_armed_at: None,
            overlay: (crate::platform::backend().popup_surface()
                == crate::platform::PopupSurface::Overlay)
                .then(|| OverlayCard {
                    pos: None,
                    grab: None,
                    opened: Instant::now(),
                }),
        };
        this.sync_thumbs(cx);
        // The scrollbar thumb is a plain sibling element: re-render the view
        // on wheel scroll so it follows (drags repaint through their own path).
        // Weak handle: during `cx.new` the entity is not inserted yet, so a
        // strong `cx.entity()` here would panic.
        let scroll_entity = cx.weak_entity();
        this.list_state
            .set_scroll_handler(move |_, _, cx| {
                let _ = scroll_entity.update(cx, |_, cx| cx.notify());
            });
        this
    }

    /// Ask the main loop to park the persistent window offscreen instead of
    /// destroying it (called from every place that used to `remove_window`).
    pub fn request_hide(&self) {
        self.shared.lock().hide_popup_requested = true;
    }

    /// Record that this window handled a pointer press.
    ///
    /// The evdev outside-click watcher cannot ask X where the pointer is
    /// (frozen over Wayland-native windows), so it decides inside/outside by
    /// whether the popup received the press it just read off the device.
    pub fn note_pointer_down(&self) {
        let mut shared = self.shared.lock();
        shared.popup_pointer_down_seq = shared.popup_pointer_down_seq.wrapping_add(1);
        shared.popup_last_pointer_down = Some(std::time::Instant::now());
    }

    /// Reset per-open state for a fresh show: latest items, cleared search and
    /// selection, requested tab, and a new appear-animation generation.
    pub fn prepare_for_show(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.refresh_items(cx);
        self.search.clear();
        self.search_visible = false;
        self.focused = 0;
        self.tab = tab;
        self.after_filter_change();
        self.appear_gen += 1;
        // A stale arm from before the hide must never fire right after a show.
        self.focus_loss_armed_at = None;
        if let Some(overlay) = self.overlay.as_mut() {
            *overlay = OverlayCard {
                pos: None,
                grab: None,
                opened: Instant::now(),
            };
        }
        cx.notify();
    }

    /// Card size in window px (the popup's logical size times `ui_scale`).
    fn overlay_card_size(&self) -> (f32, f32) {
        (
            crate::POPUP_W * self.settings.ui_scale,
            crate::POPUP_H * self.settings.ui_scale,
        )
    }

    fn overlay_card_rect(&self, window: &Window) -> Option<(Point<Pixels>, (f32, f32))> {
        let overlay = self.overlay.as_ref()?;
        let size = self.overlay_card_size();
        let pos = overlay.pos.unwrap_or_else(|| {
            let view = window.viewport_size();
            let mon = crate::geometry::MonitorRect {
                x: 0,
                y: 0,
                w: f32::from(view.width) as i32,
                h: f32::from(view.height) as i32,
            };
            let (x, y) = crate::geometry::bottom_center(&mon, size.0 as i32, size.1 as i32);
            point(px(x as f32), px(y as f32))
        });
        Some((pos, size))
    }

    /// Pointer moved over the overlay: places the card on the first position,
    /// then follows the pointer while the header is dragged.
    fn overlay_pointer_moved(
        &mut self,
        position: Point<Pixels>,
        pressed: Option<gpui::MouseButton>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let size = self.overlay_card_size();
        let view = window.viewport_size();
        let clamp = |pos: Point<Pixels>| {
            let mon = crate::geometry::MonitorRect {
                x: 0,
                y: 0,
                w: f32::from(view.width) as i32,
                h: f32::from(view.height) as i32,
            };
            let (x, y) = crate::geometry::clamp_to_monitor(
                &mon,
                size.0 as i32,
                size.1 as i32,
                f32::from(pos.x) as i32,
                f32::from(pos.y) as i32,
            );
            point(px(x as f32), px(y as f32))
        };
        let Some(overlay) = self.overlay.as_mut() else {
            return;
        };
        if overlay.pos.is_none() {
            crate::drag_log::log(format!(
                "overlay: first pointer position ({:.0},{:.0}) in {:.0}x{:.0}",
                f32::from(position.x),
                f32::from(position.y),
                f32::from(view.width),
                f32::from(view.height)
            ));
            overlay.pos = Some(clamp(position));
            cx.notify();
            return;
        }
        if pressed != Some(gpui::MouseButton::Left) {
            overlay.grab = None;
        }
        if let Some(grab) = overlay.grab {
            overlay.pos = Some(clamp(point(position.x - grab.x, position.y - grab.y)));
            cx.notify();
        }
    }

    /// A press on the overlay: outside the card dismisses, on the header strip
    /// (see [`render_drag_strip`]) starts a drag.
    fn overlay_mouse_down(
        &mut self,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some((pos, size)) = self.overlay_card_rect(window) else {
            return;
        };
        let inside = position.x >= pos.x
            && position.x < pos.x + px(size.0)
            && position.y >= pos.y
            && position.y < pos.y + px(size.1);
        if !inside {
            crate::drag_log::log("overlay: press outside the card -> hide");
            self.request_hide();
            cx.notify();
        }
    }

    fn overlay_begin_drag(&mut self, position: Point<Pixels>, window: &Window) {
        let Some((pos, _)) = self.overlay_card_rect(window) else {
            return;
        };
        let grab = point(position.x - pos.x, position.y - pos.y);
        if let Some(overlay) = self.overlay.as_mut() {
            overlay.pos = Some(pos);
            overlay.grab = Some(grab);
        }
    }

    /// Whether a pending focus-loss dismissal has matured and should fire.
    /// Pure over the timestamp so it is unit-testable.
    fn focus_loss_due(armed_at: Option<Instant>, now: Instant) -> bool {
        armed_at.is_some_and(|t| now.saturating_duration_since(t) >= FOCUS_LOSS_GRACE)
    }

    /// Fire a matured focus-loss dismissal (called from the 50 ms poll loop
    /// via `check_focus_loss_dismiss`). Returns true when it hid the popup.
    pub fn check_focus_loss_dismiss(&mut self) -> bool {
        if Self::focus_loss_due(self.focus_loss_armed_at, Instant::now()) {
            self.focus_loss_armed_at = None;
            crate::drag_log::log("activation: dismissed on focus loss (grace elapsed)");
            self.request_hide();
            return true;
        }
        false
    }

    /// Close the popup when another window — or the desktop itself — takes
    /// focus (parity with the Tauri build's `WindowEvent::Focused(false)`
    /// handler, minus its settings-window carve-out).
    ///
    /// This is deliberately focus-loss based rather than an outside-click
    /// detector: under XWayland an `XGrabPointer` only sees clicks that land on
    /// XWayland-owned pixels, so clicks on the desktop, on window decorations or
    /// on Wayland-native apps never arrive — the same wall OpenJDK hit in
    /// JDK-8280993, whose shipped fix is also focus-loss dismissal.
    ///
    /// Dismissal is suppressed while the pointer sits inside the popup, which is
    /// exactly the Tauri build's `is_mouse_inside` guard. It matters because a
    /// window-move grab (the drag strip) is a WM-side grab that drops the
    /// popup's focus at the moment the drag starts; without the guard the popup
    /// destroys itself on the first pixel of a drag. A click on another window
    /// or on the desktop cannot happen with the pointer over the popup, so the
    /// outside-click behaviour is unaffected.
    ///
    /// Dismissal is delayed by [`FOCUS_LOSS_GRACE`], not instant: on
    /// focus-follows-mouse desktops focus also moves on hover, so a brief
    /// excursion (mouse passing over another window, focus flicker) arms the
    /// timer and regaining focus disarms it — only a sustained move-away,
    /// such as an outside click, actually hides the popup. The evdev click
    /// watcher still closes instantly on a physical outside click where it
    /// can see one.
    ///
    /// `observe_window_activation` invokes the callback once immediately at
    /// registration, which happens before the popup has been activated, so only
    /// a transition *away* from an activated window dismisses it; reacting to
    /// the first callback would close the popup the instant it opened.
    pub fn watch_activation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut was_active = false;
        self.activation_watch = Some(cx.observe_window_activation(window, move |this, window, _| {
            if window.is_window_active() {
                was_active = true;
                // Back home: a pending dismissal no longer applies.
                this.focus_loss_armed_at = None;
            } else if was_active && !window.is_window_hovered() {
                // Focus-follows-mouse desktops move focus on hover, so focus
                // loss alone must not close when the user disabled it — the
                // evdev click watcher still closes on a physical outside click.
                if this.shared.lock().settings.close_on_focus_loss {
                    if this.focus_loss_armed_at.is_none() {
                        crate::drag_log::log("activation: focus loss armed (grace running)");
                    }
                    this.focus_loss_armed_at = Some(Instant::now());
                } else {
                    crate::drag_log::log("activation: focus loss ignored (close_on_focus_loss=off)");
                }
            } else if was_active {
                crate::drag_log::log("activation: focus lost while pointer inside, kept open");
            }
        }));
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

    /// Pull a fresh snapshot when the watcher bumped the version, and reload
    /// settings when the settings window saved. Returns true on change.
    pub fn poll_backend(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        let version = self.backend.version();
        if version != self.last_version {
            self.last_version = version;
            changed = true;
        }
        let sversion = self.shared.lock().version;
        if sversion != self.last_settings_version {
            self.last_settings_version = sversion;
            let settings = self.shared.lock().settings.clone();
            // Re-resolve so a live theme switch (including GTK swaps pushed by
            // the main loop) is reflected in this persistent window.
            let resolved = theme::resolve(&settings);
            self.shared.lock().theme = resolved;
            self.is_dark = resolved.is_dark;
            self.settings = settings;
            self.last_settings_mtime = crate::settings::settings_mtime();
            changed = true;
        } else if crate::settings::settings_mtime() != self.last_settings_mtime {
            // Another process saved (e.g. a separately launched settings window):
            // reload from disk and publish into shared state.
            let settings = crate::settings::load();
            self.backend
                .set_max_history_size(settings.max_history_size);
            let resolved = theme::resolve(&settings);
            self.shared.lock().theme = resolved;
            self.is_dark = resolved.is_dark;
            self.settings = settings.clone();
            let mut guard = self.shared.lock();
            guard.settings = settings;
            guard.version += 1;
            self.last_settings_version = guard.version;
            self.last_settings_mtime = crate::settings::settings_mtime();
            changed = true;
        }
        if !changed {
            return false;
        }
        self.refresh_items(cx);
        self.after_filter_change();
        cx.notify();
        true
    }

    pub fn refresh_items(&mut self, cx: &mut Context<Self>) {
        self.items = self.backend.snapshot();
        self.sync_thumbs(cx);
    }

    /// Keep the preview-texture cache in step with the history snapshot:
    /// decode thumbs for new items and free the GPU tiles of items that left
    /// history (delete, clear, trimmed by the size limit, auto-delete).
    fn sync_thumbs(&mut self, cx: &mut Context<Self>) {
        let live: std::collections::HashSet<&str> = self
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        let stale_ids: Vec<String> = self
            .thumbs
            .keys()
            .filter(|id| !live.contains(id.as_str()))
            .cloned()
            .collect();
        if !stale_ids.is_empty() {
            let stale: Vec<Arc<RenderImage>> = stale_ids
                .iter()
                .map(|id| self.thumbs.remove(id).expect("key just collected"))
                .collect();
            // drop_image only visits windows currently in `App.windows`; a
            // window mid-update is temporarily taken out of that map, so free
            // the tiles from a spawned task instead of the calling context.
            // The upgrade guard keeps a shutdown race from panicking on the
            // released app.
            cx.spawn(async move |this, cx| {
                if this.upgrade().is_none() {
                    return;
                }
                cx.update(|cx| {
                    for image in stale {
                        cx.drop_image(image, None);
                    }
                });
            })
            .detach();
        }
        for item in &self.items {
            if self.thumbs.contains_key(&item.id) {
                continue;
            }
            let ClipboardContent::Image {
                thumb: Some(thumb), ..
            } = &item.content
            else {
                continue;
            };
            if let Some(render) = decode_thumb(thumb) {
                self.thumbs.insert(item.id.clone(), render);
            }
        }
    }

    /// The decoded preview texture for a history card, if any.
    pub fn thumb_for(&self, id: &str) -> Option<&Arc<RenderImage>> {
        self.thumbs.get(id)
    }

    /// Mirror the React effect: reset focused index when filtered results change.
    pub fn after_filter_change(&mut self) {
        self.focused = 0;
        self.reset_list_scroll_to_top();
    }

    /// Map a `focused` index (into `visible_items()`) to the row index of the
    /// virtualized history list in `render_list`.
    ///
    /// The scroll container's children include the section headers, so the
    /// focused item is offset by them. Returns `None` when there is nothing
    /// to scroll to. Pure (no `self`) so it is unit-testable.
    pub fn clipboard_scroll_index(
        focused: usize,
        visible_len: usize,
        pinned_len: usize,
        unpinned_len: usize,
        show_sections: bool,
        pinned_expanded: bool,
    ) -> Option<usize> {
        if visible_len == 0 || focused >= visible_len {
            return None;
        }
        if !show_sections {
            return Some(focused);
        }
        if pinned_expanded {
            if unpinned_len == 0 {
                // Children: [PinnedHeader, pinned...]
                return Some(focused + 1);
            }
            if focused < pinned_len {
                // Pinned item, skipping the Pinned header.
                Some(focused + 1)
            } else {
                // Unpinned item, skipping both headers.
                Some(focused + 2)
            }
        } else {
            // Collapsed: visible items are unpinned only.
            // Children: [PinnedHeader, RecentHeader, unpinned...]
            if focused >= unpinned_len {
                return None;
            }
            Some(focused + 2)
        }
    }

    /// Compute the scroll child index for the current `focused` value.
    fn focused_scroll_index(&self) -> Option<usize> {
        let filtered = filter_history(&self.items, &self.search.text, self.search.regex_mode);
        let show_sections = self.search.text.is_empty() && filtered.iter().any(|i| i.pinned);
        let pinned_len = filtered.iter().filter(|i| i.pinned).count();
        let unpinned_len = filtered.len() - pinned_len;
        let visible_len = self.visible_items().len();
        Self::clipboard_scroll_index(
            self.focused,
            visible_len,
            pinned_len,
            unpinned_len,
            show_sections,
            self.pinned_expanded,
        )
    }

    /// Minimal scroll so the focused history card is fully visible.
    /// Mirrors the web build's `scrollIntoView({ block: 'nearest' })`.
    pub(crate) fn scroll_to_focused(&self) {
        if let Some(row) = self.focused_scroll_index() {
            self.list_state.scroll_to_reveal_item(row);
        }
    }

    /// Jump back to the very top (used on filter reset / show / Home).
    pub(crate) fn reset_list_scroll_to_top(&self) {
        self.list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.),
        });
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

    fn paste_picker_text(
        &mut self,
        text: &str,
        record_emoji: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Hide (don't destroy): the poll loop parks the persistent window
        // offscreen on toggle. Per-window hide keeps the settings window
        // alive, unlike cx.hide().
        self.request_hide();
        let _ = focus_manager::restore_focused_window();
        let _ = self.backend.paste_text(text, record_emoji);
        cx.notify();
    }

    pub fn paste_emoji(&mut self, ch: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_picker_text(ch, true, window, cx);
    }

    pub fn paste_kaomoji(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_picker_text(text, false, window, cx);
    }

    pub fn paste_symbol(
        &mut self,
        symbol: &SymbolItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ch = symbol.char.clone();
        crate::pickers::record_symbol_usage(symbol);
        self.symbol_recents = crate::pickers::load_recent_symbols();
        self.paste_picker_text(&ch, false, window, cx);
    }

    pub fn emoji_filtered(&self) -> Vec<Emoji> {
        let q = self.emoji.search.text.trim().to_string();
        if !q.is_empty() {
            return crate::pickers::search_emojis(&q, 100);
        }
        let all = crate::pickers::load_emojis();
        let recent = self.backend.recent_emojis();
        let map: std::collections::HashMap<&str, &Emoji> =
            all.iter().map(|e| (e.char.as_str(), e)).collect();
        let recents: Vec<Emoji> = recent
            .iter()
            .filter_map(|r| map.get(r.char.as_str()).map(|e| (*e).clone()))
            .collect();
        if self.emoji.category.as_deref() == Some(crate::pickers::RECENT_CATEGORY) {
            return recents;
        }
        if let Some(cat) = &self.emoji.category {
            return all.into_iter().filter(|e| &e.category == cat).collect();
        }
        if recents.is_empty() {
            return all;
        }
        let recent_chars: std::collections::HashSet<String> =
            recents.iter().map(|e| e.char.clone()).collect();
        let mut out = recents;
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
            h.focus(window, cx);
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
                    // Keep the focused row visible in the virtualized grid.
                    let row = n / cols.max(1);
                    match tab {
                        Tab::Emoji => self.emoji_scroll.scroll_to_item(row, ScrollStrategy::Center),
                        Tab::Kaomoji => self.kaomoji_scroll.scroll_to_item(row, ScrollStrategy::Center),
                        Tab::Symbols => self.symbol_scroll.scroll_to_item(row, ScrollStrategy::Center),
                        Tab::Clipboard => {}
                    }
                    cx.notify();
                }
                cx.stop_propagation();
            }
            _ => {
                if !mods.control && !mods.alt && !mods.platform && key.chars().count() == 1 {
                    let h = self.search_ref(which).focus.clone();
                    h.focus(window, cx);
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
        match tab {
            Tab::Emoji => {
                let idx = self.emoji.focused_main;
                if let Some(e) = self.emoji_filtered().get(idx).cloned() {
                    self.paste_emoji(&e.char, window, cx);
                }
            }
            Tab::Kaomoji => {
                let idx = self.kaomoji.focused_main;
                if let Some(k) = self.kaomoji_filtered().get(idx).cloned() {
                    self.paste_kaomoji(&k.text, window, cx);
                }
            }
            Tab::Symbols => {
                let idx = self.symbol.focused_main;
                if let Some(s) = self.symbol_filtered().get(idx).cloned() {
                    self.paste_symbol(&s, window, cx);
                }
            }
            Tab::Clipboard => {}
        }
    }

    pub fn paste_item_by_id(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.items.iter().find(|i| i.id == id).cloned() else {
            self.refresh_items(cx);
            cx.notify();
            return;
        };
        // Parity with the Tauri `paste_item` command: hide → restore focus → paste.
        // Persistent window: ask the poll loop to park it offscreen.
        self.request_hide();
        let _ = focus_manager::restore_focused_window();
        if self.backend.paste(&item).is_err() {
            self.refresh_items(cx);
        } else {
            self.refresh_items(cx);
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
        self.scroll_to_focused();
        cx.notify();
    }

    fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_visible = !self.search_visible;
        if !self.search_visible {
            self.search.clear();
            self.after_filter_change();
            self.focus.focus(window, cx);
        } else {
            self.search.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_visible = false;
        self.search.clear();
        self.after_filter_change();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn type_to_filter(&mut self, ch: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_visible {
            self.search_visible = true;
            self.search.clear();
            self.search.focus.focus(window, cx);
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
                self.request_hide();
                let _ = focus_manager::restore_focused_window();
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
                    self.scroll_to_focused();
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
                self.reset_list_scroll_to_top();
                cx.notify();
                cx.stop_propagation();
            }
            "end" => {
                let len = self.visible_items().len();
                if len > 0 {
                    self.focused = len - 1;
                    self.scroll_to_focused();
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
                    self.scroll_to_focused();
                    cx.notify();
                } else {
                    self.tab = if key == "left" {
                        self.tab.prev()
                    } else {
                        self.tab.next()
                    };
                    self.focused = 0;
                    if self.tab == Tab::Clipboard {
                        self.reset_list_scroll_to_top();
                    }
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
        let overlay_rect = self.overlay_card_rect(window);
        let card = div()
            .id("popup-root")
            .relative()
            .flex()
            .flex_col()
            .map(|card| match overlay_rect {
                Some((_, (w, h))) => card.w(px(w)).h(px(h)),
                None => card.size_full(),
            })
            .overflow_hidden()
            .rounded(px(theme::RADIUS_WINDOW))
            // Same 1px outline as the Settings/Setup windows: with no
            // compositor shadow on this surface it is what separates the
            // popup from whatever is behind it.
            .border_1()
            .border_color(if is_dark {
                theme::dark::border()
            } else {
                theme::light::border()
            })
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
            // Every press that reaches this window is ours — record it for the
            // evdev outside-click watcher before the child handlers run (none
            // of them stops propagation except the close button, which hides
            // the popup anyway). Registered per button so a press with any
            // mouse button counts as "inside".
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseDownEvent, _, _| this.note_pointer_down()),
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, _: &gpui::MouseDownEvent, _, _| this.note_pointer_down()),
            )
            .on_mouse_down(
                gpui::MouseButton::Middle,
                cx.listener(|this, _: &gpui::MouseDownEvent, _, _| this.note_pointer_down()),
            )
            .child(render_drag_strip(is_dark, cx))
            .child(render_tabbar(self, window, cx))
            .child(self.render_body(window, cx))
            // Scrollbar drag capture: transparent overlay, only present while
            // a scrollbar thumb is held, so the drag survives drifting off
            // the track anywhere inside the window.
            .child(super::scrollbar::drag_capture_layer())
            // Appear animation (fade-in): the id carries a generation counter
            // bumped on every show, so the one-shot animation visibly restarts
            // even though the window — and this view — is persistent now.
            // Linear 250 ms so the fade is actually perceptible (ease-out
            // curves finish ~70% in the first frames and look instant).
            .with_animation(
                ("popup-appear", self.appear_gen as usize),
                Animation::new(Duration::from_millis(250)),
                |el, delta| el.opacity(delta),
            );
        let Some((pos, _)) = overlay_rect else {
            return card.into_any_element();
        };
        // Overlay: a transparent full-output surface. Until the pointer's first
        // position is known the card stays invisible; past the wait it opens
        // bottom-center rather than never.
        if overlay_blur_enabled() {
            // Blur only behind the card. Before the card has a position the
            // region is one pixel, never the empty slice (= whole surface).
            let region = if self.overlay.as_ref().is_some_and(|o| o.pos.is_some()) {
                let (w, h) = self.overlay_card_size();
                gpui::Bounds::new(pos, gpui::size(px(w), px(h)))
            } else {
                gpui::Bounds::new(point(px(0.), px(0.)), gpui::size(px(1.), px(1.)))
            };
            window.set_background_blur_regions(&[region]);
        }
        let waiting = self.overlay.as_ref().is_some_and(|o| {
            o.pos.is_none() && o.opened.elapsed() < OVERLAY_POINTER_WAIT
        });
        // The pointer never reports a position until it moves (COSMIC sends no
        // enter on map): once the wait is over, settle on bottom-center for good
        // instead of jumping when the pointer finally moves.
        if let Some(overlay) = self.overlay.as_mut() {
            if overlay.pos.is_none() {
                if waiting {
                    window.request_animation_frame();
                } else {
                    overlay.pos = Some(pos);
                }
            }
        }

        div()
            .id("popup-overlay")
            .relative()
            .size_full()
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                this.overlay_pointer_moved(event.position, event.pressed_button, window, cx);
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.overlay_mouse_down(event.position, window, cx);
                }),
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.overlay_mouse_down(event.position, window, cx);
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, _| {
                    if let Some(overlay) = this.overlay.as_mut() {
                        overlay.grab = None;
                    }
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(pos.x)
                    .top(pos.y)
                    .when(waiting, |slot| slot.invisible())
                    .child(card),
            )
            .into_any_element()
    }
}

impl Popup {
    fn render_body(&mut self, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
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

    fn render_list(&mut self, window: &Window, cx: &mut Context<Popup>) -> gpui::AnyElement {
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
        let rows = self.build_list_rows(&filtered);
        let signature = rows_signature(&rows);
        if signature != self.rows_signature {
            // reset() returns the list to the top, so only reset when the row
            // model actually changed (new/removed/reordered rows, section
            // toggles) — not on unrelated re-renders.
            self.rows_signature = signature;
            self.list_state.reset(rows.len());
        }
        self.list_rows = rows;
        let repaint = {
            let entity = cx.entity();
            move |_window: &mut Window, cx: &mut gpui::App| {
                entity.update(cx, |_, cx| cx.notify());
            }
        };
        let this = cx.entity();
        let virtualized = list(self.list_state.clone(), move |row_ix, window, cx| {
            this.update(cx, |popup, cx| popup.render_row(row_ix, window, cx))
        })
        .flex_1()
        .min_h(px(0.))
        .p(px(12.));
        super::scrollbar::with_list_scrollbar(
            "list-scrollbar",
            &self.list_state,
            self.is_dark,
            virtualized,
            repaint,
        )
        .into_any_element()
    }

    /// One row of the virtualized list: a section header or a history card.
    ///
    /// The row sequence mirrors the child order the old non-virtualized
    /// container produced, so [`Self::clipboard_scroll_index`] still maps a
    /// focused card to its row.
    fn render_row(
        &self,
        row_ix: usize,
        window: &Window,
        cx: &mut Context<Popup>,
    ) -> gpui::AnyElement {
        // Each row carries its own bottom gap — the old container was a
        // flex column with `.gap(8.)`.
        let row = match self.list_rows.get(row_ix) {
            Some(row) => row,
            None => return div().pb(px(8.)).into_any_element(),
        };
        let content = match row {
            ListRow::Header {
                pinned,
                count,
                collapsed,
            } => {
                if *pinned {
                    render_section_header(self, 0, icons::PIN, "Pinned", *count, Some(*collapsed), cx)
                } else {
                    render_section_header(self, 1, icons::HISTORY, "Recent", *count, None, cx)
                }
            }
            ListRow::Item { item_ix, card_index, .. } => {
                let Some(item) = self.items.get(*item_ix) else {
                    return div().pb(px(8.)).into_any_element();
                };
                render_history_item(
                    item,
                    &CardParams {
                        index: *card_index,
                        focused: *card_index == self.focused,
                    },
                    self,
                    window,
                    cx,
                )
                .into_any_element()
            }
        };
        div().pb(px(8.)).child(content).into_any_element()
    }

    /// Build the row model: mirrors `render_sections` — pinned header (+ cards
    /// when expanded) then recent header + unpinned cards, or a flat list
    /// while searching.
    fn build_list_rows(&self, filtered: &[&ClipboardItem]) -> Vec<ListRow> {
        let show_sections = self.search.text.is_empty() && filtered.iter().any(|i| i.pinned);
        let mut rows = Vec::new();
        if !show_sections {
            for (card_index, item) in filtered.iter().enumerate() {
                rows.push(ListRow::Item {
                    item_ix: self.item_index(item),
                    id: item.id.clone(),
                    card_index,
                });
            }
            return rows;
        }
        let pinned_len = filtered.iter().filter(|i| i.pinned).count();
        let unpinned_len = filtered.len() - pinned_len;
        rows.push(ListRow::Header {
            pinned: true,
            count: pinned_len,
            collapsed: !self.pinned_expanded,
        });
        if self.pinned_expanded {
            for (offset, item) in filtered.iter().take(pinned_len).enumerate() {
                rows.push(ListRow::Item {
                    item_ix: self.item_index(item),
                    id: item.id.clone(),
                    card_index: offset,
                });
            }
        }
        if unpinned_len > 0 {
            rows.push(ListRow::Header {
                pinned: false,
                count: unpinned_len,
                collapsed: false,
            });
            for (offset, item) in filtered.iter().skip(pinned_len).enumerate() {
                rows.push(ListRow::Item {
                    item_ix: self.item_index(item),
                    id: item.id.clone(),
                    card_index: if self.pinned_expanded {
                        pinned_len + offset
                    } else {
                        offset
                    },
                });
            }
        }
        rows
    }

    /// Position of a filtered item within `self.items` (filtered items are
    /// references into it).
    fn item_index(&self, item: &&ClipboardItem) -> usize {
        self.items
            .iter()
            .position(|candidate| std::ptr::eq(candidate, *item))
            .unwrap_or(0)
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
                    // The glyph inherits this (see `icons::icon`).
                    .text_color(icon_color)
                    .child(icon(icons::CLIPBOARD_LIST, px(32.))),
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
                this.reset_list_scroll_to_top();
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

/// Begin an interactive window move from the drag strip.
///
/// Backends that can move the window themselves do so (`platform::x11::window`
/// explains why the WM path mutter offers cannot work for this window type).
/// Everywhere else the compositor owns the move, and gpui already holds the
/// pointer serial it needs.
fn start_window_drag(window: &mut Window, _cx: &mut Context<Popup>) {
    use crate::platform::{self, Tick, DRAG_POLL_INTERVAL};

    // The native window id is located by the backend (pid + class + size match);
    // gpui 0.2.2's X11 `window_handle()` is a hard `unimplemented!()` and taking
    // that path aborts the whole app. Only safe state reads go through gpui here.
    // NB: `viewport_size()` proved unreliable for this (it reported the logical
    // 360x480 on a 720x960 physical window), so the physical size is
    // reconstructed from `bounds()` x `scale_factor()` — bounds are logical on
    // the X11 backend (set_bounds divides ConfigureNotify values by scale).
    let scale = window.scale_factor();
    let bounds = window.bounds().size;
    let attempt = platform::backend().begin_drag((
        bounds.width.to_f64() as f32 * scale,
        bounds.height.to_f64() as f32 * scale,
    ));
    let mut drag = match attempt {
        Ok(drag) => drag,
        Err(reason) => {
            // Trace why the client-side move is unavailable, then let the
            // compositor try (the only legal move on Wayland).
            crate::drag_log::log(format!("client-side drag unavailable: {reason}"));
            crate::drag_log::log("falling back to window.start_window_move()");
            window.start_window_move();
            return;
        }
    };
    crate::drag_log::log(drag.describe_start());
    // Own thread, not the gpui foreground executor: the tick must keep its
    // 60 Hz cadence while the main thread renders (the executor loop stretched
    // ticks to ~100 ms under load, which read as a juddering window). The
    // drag owns its server connection, and ends on button release (or when the
    // server rejects a move because the popup is gone).
    std::thread::spawn(move || {
        loop {
            match drag.tick() {
                Tick::Moved(x, y) => {
                    crate::drag_log::trace(format!("moved to ({x},{y})"));
                }
                Tick::Still => {}
                Tick::Finished(reason) => {
                    crate::drag_log::log(drag.describe_finish(&reason));
                    break;
                }
            }
            std::thread::sleep(DRAG_POLL_INTERVAL);
        }
    });
}

/// Drag strip — port of `DragHandle.tsx`: centered pill + close button.
/// The whole strip starts a native window move (matches the Tauri drag region);
/// the close button opts out so its click still lands.
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
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                // Close button owns the top-right corner (~28×44px at right-16
                // top-8, plus margin): never start a move there, or the click
                // is swallowed by the compositor grab.
                let win_w = f32::from(window.bounds().size.width);
                let x = f32::from(event.position.x);
                let y = f32::from(event.position.y);
                crate::drag_log::log(format!(
                    "strip mousedown at ({x:.1},{y:.1}) win_w={win_w:.1} close_corner={}",
                    x >= win_w - 48.0 && y <= 60.0
                ));
                if this.overlay.is_some() {
                    // The card lives inside an overlay surface: it moves
                    // client-side, so the strip is relative to the card.
                    if let Some((pos, size)) = this.overlay_card_rect(window) {
                        let rel_x = x - f32::from(pos.x);
                        let rel_y = y - f32::from(pos.y);
                        if !(rel_x >= size.0 - 48.0 && rel_y <= 60.0) {
                            this.overlay_begin_drag(event.position, window);
                        }
                    }
                } else if !(x >= win_w - 48.0 && y <= 60.0) {
                    start_window_drag(window, cx);
                }
            }),
        )
        .child(
            div()
                .id("drag-pill")
                .w(px(40.))
                .h(px(4.))
                .rounded_full()
                .cursor_grab()
                .bg(if is_dark {
                    theme::white_pct(0.20)
                } else {
                    theme::black_pct(0.20)
                }),
        )
        .child(
            div()
                .id("drag-close")
                .absolute()
                .right(px(16.))
                .top(px(8.))
                .p(px(4.))
                // Keeps the glyph where it has always sat while the visible
                // circle below is centred on it.
                .pt(px(16.))
                .cursor_pointer()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.request_hide();
                        let _ = focus_manager::restore_focused_window();
                        cx.notify();
                    }),
                )
                .child(
                    // Round hit-friendly bubble: same close-hover red as the
                    // Settings window's control, so hovering it reads as the
                    // same "active" state.
                    div()
                        .id("drag-close-bubble")
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(28.))
                        .h(px(28.))
                        .rounded_full()
                        .text_color(if is_dark {
                            theme::white_pct(0.50)
                        } else {
                            theme::black_pct(0.50)
                        })
                        .hover(|s| s.bg(theme::close_hover()).text_color(theme::on_accent()))
                        .child(icon(icons::X, px(16.)).flex_shrink_0()),
                ),
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

#[cfg(test)]
mod tests {
    use super::Popup;

    #[test]
    fn scroll_index_flat_list_maps_directly() {
        // No sections: scroll child == focused.
        assert_eq!(
            Popup::clipboard_scroll_index(0, 5, 0, 5, false, true),
            Some(0)
        );
        assert_eq!(
            Popup::clipboard_scroll_index(4, 5, 0, 5, false, true),
            Some(4)
        );
        assert_eq!(Popup::clipboard_scroll_index(5, 5, 0, 5, false, true), None);
        assert_eq!(Popup::clipboard_scroll_index(0, 0, 0, 0, false, true), None);
    }

    #[test]
    fn scroll_index_expanded_sections_skips_headers() {
        // pinned=2, unpinned=3, expanded.
        // Children: [PinnedH, p0, p1, RecentH, u0, u1, u2]
        assert_eq!(
            Popup::clipboard_scroll_index(0, 5, 2, 3, true, true),
            Some(1)
        );
        assert_eq!(
            Popup::clipboard_scroll_index(1, 5, 2, 3, true, true),
            Some(2)
        );
        // First unpinned (focused=2) skips both headers.
        assert_eq!(
            Popup::clipboard_scroll_index(2, 5, 2, 3, true, true),
            Some(4)
        );
        assert_eq!(
            Popup::clipboard_scroll_index(4, 5, 2, 3, true, true),
            Some(6)
        );
    }

    #[test]
    fn scroll_index_expanded_without_unpinned() {
        // Children: [PinnedH, p0, p1]
        assert_eq!(
            Popup::clipboard_scroll_index(0, 2, 2, 0, true, true),
            Some(1)
        );
        assert_eq!(
            Popup::clipboard_scroll_index(1, 2, 2, 0, true, true),
            Some(2)
        );
    }

    #[test]
    fn scroll_index_collapsed_shows_unpinned_only() {
        // pinned=2 hidden, unpinned=3 visible.
        // Children: [PinnedH, RecentH, u0, u1, u2]
        assert_eq!(
            Popup::clipboard_scroll_index(0, 3, 2, 3, true, false),
            Some(2)
        );
        assert_eq!(
            Popup::clipboard_scroll_index(2, 3, 2, 3, true, false),
            Some(4)
        );
        assert_eq!(Popup::clipboard_scroll_index(3, 3, 2, 3, true, false), None);
    }

    #[test]
    fn focus_loss_dismissal_needs_sustained_absence() {
        use super::FOCUS_LOSS_GRACE;
        use std::time::{Duration, Instant};
        let now = Instant::now();
        // Not armed: never due.
        assert!(!Popup::focus_loss_due(None, now));
        // Just armed: grace has not elapsed.
        assert!(!Popup::focus_loss_due(Some(now), now));
        assert!(!Popup::focus_loss_due(
            Some(now - FOCUS_LOSS_GRACE / 2),
            now
        ));
        // Focus stayed away past the grace: due.
        assert!(Popup::focus_loss_due(
            Some(now - FOCUS_LOSS_GRACE - Duration::from_millis(1)),
            now
        ));
    }
}
