//! Platform backends: everything that depends on the window system or the
//! desktop lives under this module, the rest of the app talks to
//! [`PlatformBackend`] only.
//!
//! Two axes are kept apart on purpose:
//! - the **window backend** (`x11`, `cosmic`): how the popup is created,
//!   placed, dragged and dismissed — chosen once by [`init`];
//! - the **desktop** ([`shortcuts`]): where the Super+V shortcut is registered,
//!   which the x11 backend running on GNOME/COSMIC via XWayland still needs.
//!
//! One binary ships every backend, so the choice is a runtime trait object,
//! not a `cfg`.

use std::sync::OnceLock;
use std::time::Duration;

use crate::geometry::MonitorRect;

pub mod cosmic;
pub mod gnome;
pub mod pointer_nudge;
pub mod shortcuts;
pub mod x11;

pub use x11::hotkey::Hotkeys;

/// What the outside-click watchers need to know about, and how to dismiss the
/// popup. The frontend implements it over its own state.
pub trait PopupHooks: Send + Sync {
    fn popup_visible(&self) -> bool;
    /// Monotonic count of pointer presses the popup window itself handled.
    fn pointer_down_seq(&self) -> u64;
    fn last_pointer_down(&self) -> Option<std::time::Instant>;
    /// Ask the main loop to hide the popup.
    fn request_hide(&self);
}

pub type Hooks = std::sync::Arc<dyn PopupHooks>;

/// How often a client-side drag repositions the window (≈60 Hz).
pub const DRAG_POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Result of one drag step.
pub enum Tick {
    /// Window was repositioned to this root position (physical px).
    Moved(i32, i32),
    /// Pointer has not moved; nothing to do.
    Still,
    /// Drag over, with the reason (for the trace).
    Finished(String),
}

/// A client-side popup move in progress, driven from its own thread.
pub trait PopupDrag: Send {
    /// One-line trace of the drag's starting state.
    fn describe_start(&self) -> String;
    /// Advance the drag once; called every [`DRAG_POLL_INTERVAL`].
    fn tick(&mut self) -> Tick;
    /// One-line trace of how the drag ended.
    fn describe_finish(&self, reason: &str) -> String;
}

/// What the active backend can do, so callers don't probe the session type.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Consumed by UI gating as backends diverge (Phase 2+).
pub struct Capabilities {
    /// The popup can open at the cursor (otherwise bottom-center).
    pub cursor_follow: bool,
    /// The app moves the popup itself on a header drag; otherwise the
    /// compositor does.
    pub client_drag: bool,
    /// Clicks outside the popup are detected by the app (evdev).
    pub outside_click: bool,
    /// The app grabs Super+V itself instead of relying on the desktop.
    pub global_hotkeys: bool,
}

/// How the clipboard popup exists on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupSurface {
    /// A persistent toplevel window that is shown and hidden in place; the
    /// backend places it (`place_popup` / `set_popup_visible`).
    Window,
    /// A full-output transparent overlay (layer-shell) created on every show and
    /// destroyed on every hide. The view draws the card itself, follows the
    /// pointer, handles drag and dismisses on a click outside the card.
    Overlay,
}

pub trait PlatformBackend: Sync + Send {
    fn name(&self) -> &'static str;

    /// Runs before gpui initializes: environment, display backend pinning.
    fn prepare_process(&self) {}

    fn popup_surface(&self) -> PopupSurface {
        PopupSurface::Window
    }

    /// Settings/Setup are native Wayland toplevels (not X11 clients): the app
    /// must declare its client-side shadow inset itself.
    fn native_wayland_windows(&self) -> bool {
        false
    }

    /// Global cursor position in physical px; `None` means the popup opens at
    /// the bottom-center instead.
    fn cursor_position(&self) -> Option<(i32, i32)>;

    /// Work area of the monitor containing this point, when the backend knows
    /// monitor geometry better than gpui does.
    fn monitor_at(&self, _x: i32, _y: i32) -> Option<MonitorRect> {
        None
    }

    /// Factor from the toolkit's logical px to the coordinate frame of
    /// [`cursor_position`](Self::cursor_position) / [`place_popup`](Self::place_popup):
    /// the display scale where those are physical (X11), 1 where they are
    /// compositor-logical (native Wayland).
    fn coordinate_scale(&self, window_scale: f32) -> f32 {
        window_scale
    }

    /// Ask the compositor to report the pointer position to a just-created overlay
    /// (backends whose compositor does not send it until the pointer moves).
    fn prime_pointer(&self) {}

    /// Whether the permanent 1×1 holder window is needed to keep the run loop
    /// alive (not on native Wayland, where it would be a real, visible window).
    fn needs_holder_window(&self) -> bool {
        true
    }

    /// Move the (possibly hidden) popup to this physical origin.
    fn place_popup(&self, size: (f32, f32), x: i32, y: i32) -> Result<(), String>;

    /// Map (`true`) or unmap (`false`) the popup. `size` is its physical size.
    fn set_popup_visible(&self, size: (f32, f32), visible: bool) -> Result<(), String>;

    /// Native id of the popup window, for focus bookkeeping.
    fn popup_window_id(&self) -> Option<u32>;

    /// Start a client-side header drag. `Err` carries the reason the
    /// compositor has to move the window instead.
    fn begin_drag(&self, size: (f32, f32)) -> Result<Box<dyn PopupDrag>, String>;

    /// Keep the permanent 1×1 holder window out of docks. Fails while the
    /// window does not exist yet, so callers retry.
    fn unmanage_holder(&self) -> Result<(), String>;

    /// Advertise the client-side shadow margin of a titled window.
    fn set_frame_extents(&self, title: &str, margin: u32) -> Result<(), String>;

    /// Watch for clicks outside the popup and request a hide.
    fn spawn_outside_click_watcher(&self, hooks: Hooks);

    /// App-level global hotkeys, where the backend can grab keys.
    fn register_global_hotkeys(&self) -> Option<Hotkeys>;

    #[allow(dead_code)] // Consumed by UI gating as backends diverge (Phase 2+).
    fn capabilities(&self) -> Capabilities;
}

static BACKEND: OnceLock<&'static dyn PlatformBackend> = OnceLock::new();
static WAYLAND_SESSION: OnceLock<bool> = OnceLock::new();

/// The active backend. [`init`] must have run (done first thing in `main`).
pub fn backend() -> &'static dyn PlatformBackend {
    *BACKEND.get().expect("platform::init() not called")
}

/// Pick the backend and let it prepare the process. Must run before gpui
/// initializes and before anything caches the session type.
pub fn init() {
    init_with_preference(None);
}

/// Pick the backend using a saved user preference after any explicit debug
/// override from the environment.
pub fn init_with_preference(preference: Option<&str>) {
    // Cache the real session before the XWayland backend rewrites the
    // environment for toolkit initialization.
    let _ = wayland_session();
    let backend = select(preference);
    eprintln!("[app] platform backend: {}", backend.name());
    backend.prepare_process();
    let _ = BACKEND.set(backend);
}

static X11_BACKEND: x11::X11Backend = x11::X11Backend;
static COSMIC_BACKEND: cosmic::CosmicBackend = cosmic::CosmicBackend;
static GNOME_BACKEND: gnome::GnomeBackend = gnome::GnomeBackend;

/// Backend choice, in order:
/// 1. `WIN11_CLIPBOARD_BACKEND=x11|cosmic|gnome` (debug/CI override);
/// 2. the saved `wayland` / `xwayland` setup choice;
/// 3. automatic COSMIC or GNOME detection;
/// 4. `x11` (X11 sessions, and Wayland desktops without a native backend).
fn select(preference: Option<&str>) -> &'static dyn PlatformBackend {
    if let Ok(name) = std::env::var("WIN11_CLIPBOARD_BACKEND") {
        match name.trim().to_ascii_lowercase().as_str() {
            "x11" => return &X11_BACKEND,
            "cosmic" => return &COSMIC_BACKEND,
            "gnome" => return &GNOME_BACKEND,
            other => eprintln!(
                "[app] unknown WIN11_CLIPBOARD_BACKEND={other:?}; choosing automatically"
            ),
        }
    }
    let preference = preference.map(|value| value.trim().to_ascii_lowercase());
    match preference.as_deref() {
        Some("xwayland") => return &X11_BACKEND,
        Some("wayland") => {
            if cosmic::auto_selected() {
                return &COSMIC_BACKEND;
            }
            if gnome::auto_selected() {
                return &GNOME_BACKEND;
            }
            eprintln!(
                "[app] Wayland was selected, but native desktop integration is not ready; \
                 using XWayland for this launch"
            );
        }
        Some("auto") | None => {}
        Some(other) => eprintln!(
            "[app] unknown saved platform backend {other:?}; choosing automatically"
        ),
    }
    if cosmic::auto_selected() {
        return &COSMIC_BACKEND;
    }
    if gnome::auto_selected() {
        return &GNOME_BACKEND;
    }
    &X11_BACKEND
}

/// Wayland session, read from the environment directly: this runs before
/// `session::is_wayland()` may be consulted (it caches on first use, and the
/// x11 backend rewrites the environment it reads).
pub fn wayland_session() -> bool {
    *WAYLAND_SESSION.get_or_init(|| match std::env::var("XDG_SESSION_TYPE") {
        Ok(kind) if kind.eq_ignore_ascii_case("wayland") => true,
        Ok(kind) if kind.eq_ignore_ascii_case("x11") => false,
        _ => std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty()),
    })
}

/// Desktop-specific first-run setup beyond the shortcut. On GNOME Wayland this
/// installs the helper extension that lets the popup open natively; `None` when
/// there is nothing to do on this desktop.
pub fn setup_desktop_integration() -> Option<Result<String, String>> {
    if shortcuts::current_desktop() != shortcuts::Desktop::Gnome || !wayland_session() {
        return None;
    }
    Some(
        gnome::install::install()
            .map(|outcome| gnome::install::describe(&outcome).to_string())
            .map_err(|e| format!("Could not install the GNOME extension: {e}")),
    )
}
