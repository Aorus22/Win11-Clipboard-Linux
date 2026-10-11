//! Legacy backend: the app runs as an X11 client, under XWayland on Wayland
//! sessions. Default for every desktop without a backend of its own.

use win11_clipboard_history_lib::session;

use super::{Capabilities, PlatformBackend, PopupDrag};
use crate::app_state::Shared;
use crate::APP_ID;

pub mod click_watch;
pub mod hotkey;
pub mod pointer;
pub mod window;

pub struct X11Backend;

/// Pin the display backend to X11/XWayland on Wayland sessions.
///
/// xdg-shell has no equivalent of `_NET_WM_WINDOW_TYPE_NOTIFICATION`, so a
/// native-Wayland popup is an ordinary toplevel and GNOME lists it in
/// Dash-to-Dock while it is open (the Tauri build hits the same wall:
/// tauri-apps/tauri#9829). gpui's X11 backend tags `WindowKind::PopUp` as
/// NOTIFICATION — hidden from docks/taskbars — and cursor-follow positioning
/// for the popup comes back as well.
///
/// gpui resolves the backend with `guess_compositor()`, which prefers Wayland
/// whenever `WAYLAND_DISPLAY` is non-empty, so drop it here — before the
/// platform initializes. `XDG_SESSION_TYPE` is aligned at the same time so the
/// shared backend's `session::is_wayland()` (cached on first use) matches the
/// backend actually in use and takes the X11 focus/positioning paths.
///
/// Opt out with `WIN11_CLIPBOARD_ALLOW_WAYLAND=1`.
pub(in crate::platform) fn pin_display_backend_to_x11() {
    if std::env::var_os("WIN11_CLIPBOARD_ALLOW_WAYLAND").is_some() {
        return;
    }
    if !std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty()) {
        return; // Already X11, or not a Wayland session at all.
    }
    // Without XWayland there is nothing to fall back to — stay on Wayland
    // rather than starting headless.
    if !std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) {
        eprintln!("[gpui-app] no DISPLAY (XWayland) available; keeping the Wayland backend");
        return;
    }
    std::env::remove_var("WAYLAND_DISPLAY");
    std::env::remove_var("WAYLAND_SOCKET");
    std::env::set_var("XDG_SESSION_TYPE", "x11");
    eprintln!("[gpui-app] X11/XWayland backend pinned (popup stays out of the dock)");
}

impl PlatformBackend for X11Backend {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn prepare_process(&self) {
        pin_display_backend_to_x11();
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        // Still on native Wayland (no XWayland to pin to): there is no global
        // cursor to ask for, so the popup goes bottom-center.
        if session::is_wayland() {
            return None;
        }
        pointer::cursor_position()
    }

    fn place_popup(&self, size: (f32, f32), x: i32, y: i32) -> Result<(), String> {
        window::move_popup_to(APP_ID, size, x, y)
    }

    fn set_popup_visible(&self, size: (f32, f32), visible: bool) -> Result<(), String> {
        window::set_popup_mapped(APP_ID, size, visible)
    }

    fn popup_window_id(&self) -> Option<u32> {
        window::popup_window_id(APP_ID)
    }

    fn begin_drag(&self, size: (f32, f32)) -> Result<Box<dyn PopupDrag>, String> {
        window::WindowDrag::begin_locating(APP_ID, size)
            .map(|drag| Box::new(drag) as Box<dyn PopupDrag>)
    }

    fn unmanage_holder(&self) -> Result<(), String> {
        // Window managers that honor the NOTIFICATION type already keep the
        // holder out of docks.
        Ok(())
    }

    fn set_frame_extents(&self, title: &str, margin: u32) -> Result<(), String> {
        window::set_frame_extents(title, margin)
    }

    fn spawn_outside_click_watcher(&self, shared: Shared) {
        click_watch::spawn_click_watcher(shared);
    }

    fn register_global_hotkeys(&self) -> Option<hotkey::Hotkeys> {
        hotkey::register_hotkeys()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            cursor_follow: !session::is_wayland(),
            client_drag: !session::is_wayland(),
            outside_click: true,
            global_hotkeys: !session::is_wayland(),
        }
    }
}
