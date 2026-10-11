//! GNOME backend: native Wayland, placed by a GNOME Shell extension.
//!
//! A Wayland client cannot position, raise or hide its own toplevel, and
//! xdg-shell has no "notification" window type to keep the popup out of the
//! dock. The extension in `extension/` is the compositor-side helper: this
//! backend asks it over D-Bus ([`client`]) to place / show / hide the popup,
//! to report the pointer and to say when the user clicked outside. Selected
//! only while the extension is actually running (see `platform::select`); the
//! `x11` backend remains the fallback.

use super::{Capabilities, PlatformBackend, PopupDrag};
use crate::app_state::Shared;
use crate::geometry::MonitorRect;

pub mod client;
pub mod install;

pub struct GnomeBackend;

/// GNOME on a Wayland session with the helper extension running.
pub(super) fn auto_selected() -> bool {
    super::shortcuts::current_desktop() == super::shortcuts::Desktop::Gnome
        && super::wayland_session()
        && client::extension_ready()
}

impl PlatformBackend for GnomeBackend {
    fn name(&self) -> &'static str {
        "gnome"
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        client::pointer().ok()
    }

    fn monitor_at(&self, x: i32, y: i32) -> Option<MonitorRect> {
        let (x, y, w, h) = client::monitor_at(x, y).ok()?;
        Some(MonitorRect { x, y, w, h })
    }

    /// Compositor-logical coordinates: no physical scaling anywhere.
    fn coordinate_scale(&self, _window_scale: f32) -> f32 {
        1.0
    }

    fn place_popup(&self, _size: (f32, f32), x: i32, y: i32) -> Result<(), String> {
        client::place(x, y)
    }

    fn set_popup_visible(&self, _size: (f32, f32), visible: bool) -> Result<(), String> {
        client::set_visible(visible)
    }

    fn popup_window_id(&self) -> Option<u32> {
        None
    }

    fn begin_drag(&self, _size: (f32, f32)) -> Result<Box<dyn PopupDrag>, String> {
        Err("compositor moves the window (xdg_toplevel.move)".to_string())
    }

    fn native_wayland_windows(&self) -> bool {
        true
    }

    fn needs_holder_window(&self) -> bool {
        false
    }

    fn unmanage_holder(&self) -> Result<(), String> {
        Ok(())
    }

    fn set_frame_extents(&self, _title: &str, _margin: u32) -> Result<(), String> {
        // Wayland: the visible frame is declared through xdg_surface geometry.
        Ok(())
    }

    fn spawn_outside_click_watcher(&self, shared: Shared) {
        std::thread::Builder::new()
            .name("gnome-outside-click".to_string())
            .spawn(move || {
                let result = client::watch_outside_clicks(|| {
                    let mut state = shared.lock();
                    if state.popup_visible {
                        state.hide_popup_requested = true;
                    }
                });
                if let Err(e) = result {
                    eprintln!("[gpui-app] outside-click watcher stopped: {e}");
                }
            })
            .expect("spawn outside-click thread");
    }

    fn register_global_hotkeys(&self) -> Option<super::Hotkeys> {
        // No key grabs on Wayland: the desktop shortcut runs `--toggle`.
        None
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            cursor_follow: true,
            client_drag: false,
            outside_click: true,
            global_hotkeys: false,
        }
    }
}
