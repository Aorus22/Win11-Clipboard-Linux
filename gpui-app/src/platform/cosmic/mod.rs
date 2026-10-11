//! COSMIC backend: native Wayland, the popup is a `wlr-layer-shell` overlay.
//!
//! A layer surface needs no help from the compositor to stay out of the dock,
//! to sit on top, or to take the keyboard, so there is no XWayland, no
//! override-redirect window and no evdev watcher. What it cannot do is sit at
//! an arbitrary position, so the surface covers the whole output (transparent)
//! and the popup *card* is drawn inside it where the pointer is — see
//! [`layer`] and the overlay branch of `ui::popup`.

use super::{Capabilities, PlatformBackend, PopupDrag, PopupSurface};
use crate::app_state::Shared;

pub mod layer;

pub struct CosmicBackend;

/// COSMIC on a Wayland session.
pub(super) fn auto_selected() -> bool {
    super::shortcuts::current_desktop() == super::shortcuts::Desktop::Cosmic
        && super::wayland_session()
}

impl PlatformBackend for CosmicBackend {
    fn name(&self) -> &'static str {
        "cosmic"
    }

    fn window_kind(&self) -> gpui::WindowKind {
        layer::popup_kind()
    }

    fn popup_surface(&self) -> PopupSurface {
        PopupSurface::Overlay
    }

    fn native_wayland_windows(&self) -> bool {
        true
    }

    /// Unknown before the surface exists; the view reads it off the first
    /// pointer event of the overlay.
    fn cursor_position(&self) -> Option<(i32, i32)> {
        None
    }

    fn place_popup(&self, _size: (f32, f32), _x: i32, _y: i32) -> Result<(), String> {
        Ok(())
    }

    fn set_popup_visible(&self, _size: (f32, f32), _visible: bool) -> Result<(), String> {
        Ok(())
    }

    fn popup_window_id(&self) -> Option<u32> {
        None
    }

    fn begin_drag(&self, _size: (f32, f32)) -> Result<Box<dyn PopupDrag>, String> {
        Err("overlay card is dragged inside its own surface".to_string())
    }

    fn needs_holder_window(&self) -> bool {
        false
    }

    fn unmanage_holder(&self) -> Result<(), String> {
        Ok(())
    }

    fn set_frame_extents(&self, _title: &str, _margin: u32) -> Result<(), String> {
        Ok(())
    }

    fn spawn_outside_click_watcher(&self, _shared: Shared) {
        // The overlay receives every click, so the view tells inside from outside.
    }

    fn register_global_hotkeys(&self) -> Option<super::Hotkeys> {
        // No key grabs on Wayland: the desktop shortcut runs `--toggle`.
        None
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            cursor_follow: true,
            client_drag: true,
            outside_click: true,
            global_hotkeys: false,
        }
    }
}
