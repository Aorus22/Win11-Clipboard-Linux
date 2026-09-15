//! Client-side window move for the X11 backend.
//!
//! `gpui::Window::start_window_move()` asks the window manager to run an
//! interactive move (`_NET_WM_MOVERESIZE`). Mutter services that with a WM-side
//! grab, and the clipboard popup is a `_NET_WM_WINDOW_TYPE_NOTIFICATION` window
//! — a type it does not hand a move grab to, so dragging the header pill did
//! nothing. Repositioning our own X window with `ConfigureWindow` needs no WM
//! cooperation and was verified to move this exact window, which is why the X11
//! backend drags itself and only Wayland defers to the compositor.
//!
//! The move is driven by polling the pointer rather than by gpui's mouse-move
//! events: the window follows the pointer, so the pointer barely moves *within*
//! the window and its local motion deltas are useless for positioning.

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConfigureWindowAux, ConnectionExt, KeyButMask, MapState,
};
use x11rb::rust_connection::RustConnection;

/// How often the drag loop repositions the window (≈60 Hz).
pub const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Ceiling for one drag. A released button is detected from the pointer state,
/// so this only guards against the loop running on after a lost release.
const MAX_DRAG: Duration = Duration::from_secs(30);

/// Physical-pixel slack when matching the popup by size.
const SIZE_TOLERANCE: i32 = 32;

/// Result of one drag step.
pub enum Tick {
    /// Window was repositioned to this root position (physical px).
    Moved(i32, i32),
    /// Pointer has not moved; nothing to do.
    Still,
    /// Drag over, with the reason (for the trace).
    Finished(String),
}

/// A client-side window move in progress.
pub struct WindowDrag {
    conn: RustConnection,
    root: u32,
    window: u32,
    /// Pointer position minus window origin (physical px), fixed for the whole
    /// drag so the window tracks the pointer without jumping.
    grab_offset: (i32, i32),
    last: (i32, i32),
    started: Instant,
    ticks: u32,
    moves: u32,
}

impl WindowDrag {
    /// Start moving this process's popup window, located on the X server.
    ///
    /// `physical_size` is the popup's X window geometry in physical pixels —
    /// pass `bounds() x scale_factor()` (NOT `viewport_size()`, which reports
    /// logical size on HiDPI). The X window id itself is found server-side because
    /// gpui's X11 `window_handle()` is a hard `unimplemented!()` — calling it
    /// panics and, with `panic = "abort"` in the release profile, kills the
    /// whole app.
    ///
    /// The `Err` text is the reason the drag fell back to the compositor path;
    /// it goes straight into the drag trace.
    pub fn begin_locating(app_id: &str, physical_size: (f32, f32)) -> Result<Self, String> {
        let (conn, root) = connect_root()?;
        let expected = (physical_size.0.round() as i32, physical_size.1.round() as i32);
        let window = locate_popup_window(
            &conn,
            root,
            app_id,
            std::process::id(),
            expected,
            true,
        )?;
        Self::attach(conn, root, window)
    }

    fn attach(conn: RustConnection, root: u32, window: u32) -> Result<Self, String> {
        let pointer = conn
            .query_pointer(root)
            .map_err(|error| format!("query_pointer error: {error}"))?
            .reply()
            .map_err(|error| format!("query_pointer reply: {error}"))?;
        let origin = conn
            .translate_coordinates(window, root, 0, 0)
            .map_err(|error| format!("translate_coordinates error: {error}"))?
            .reply()
            .map_err(|error| format!("translate_coordinates reply: {error}"))?;
        let pointer_pos = (i32::from(pointer.root_x), i32::from(pointer.root_y));
        let window_pos = (i32::from(origin.dst_x), i32::from(origin.dst_y));
        Ok(Self {
            grab_offset: (pointer_pos.0 - window_pos.0, pointer_pos.1 - window_pos.1),
            conn,
            root,
            window,
            last: window_pos,
            started: Instant::now(),
            ticks: 0,
            moves: 0,
        })
    }

    pub fn window(&self) -> u32 {
        self.window
    }

    pub fn root(&self) -> u32 {
        self.root
    }

    /// Pointer-minus-window offset the drag grabbed at (physical px).
    pub fn grab_offset(&self) -> (i32, i32) {
        self.grab_offset
    }

    /// Where the window was before the drag started (physical px).
    pub fn start_position(&self) -> (i32, i32) {
        (self.last.0, self.last.1)
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn counters(&self) -> (u32, u32) {
        (self.ticks, self.moves)
    }

    /// Where the server says the window is right now (physical px).
    pub fn live_position(&self) -> Option<(i32, i32)> {
        let origin = self
            .conn
            .translate_coordinates(self.window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some((i32::from(origin.dst_x), i32::from(origin.dst_y)))
    }

    /// Advance the drag once.
    pub fn tick(&mut self) -> Tick {
        self.ticks += 1;
        if self.started.elapsed() > MAX_DRAG {
            return Tick::Finished("safety ceiling reached".to_string());
        }
        // The button state comes from the same round trip as the position, so a
        // release is noticed without depending on gpui's mouse-up delivery.
        let Ok(cookie) = self.conn.query_pointer(self.root) else {
            return Tick::Finished("query_pointer request failed".to_string());
        };
        let Ok(reply) = cookie.reply() else {
            return Tick::Finished("query_pointer reply failed".to_string());
        };
        if !reply.mask.contains(KeyButMask::BUTTON1) {
            return Tick::Finished("left button released".to_string());
        }
        let target = (
            i32::from(reply.root_x) - self.grab_offset.0,
            i32::from(reply.root_y) - self.grab_offset.1,
        );
        if target == self.last {
            return Tick::Still;
        }
        let aux = ConfigureWindowAux {
            x: Some(target.0),
            y: Some(target.1),
            ..Default::default()
        };
        // `check` waits for the server's verdict, so a refused move (BadWindow on
        // a window that just closed, BadMatch, …) is reported instead of silently
        // vanishing into the error queue.
        match self.conn.configure_window(self.window, &aux) {
            Ok(cookie) => {
                if let Err(error) = cookie.check() {
                    return Tick::Finished(format!("ConfigureWindow rejected: {error}"));
                }
            }
            Err(error) => {
                return Tick::Finished(format!("ConfigureWindow error: {error}"));
            }
        }
        let _ = self.conn.flush();
        self.last = target;
        self.moves += 1;
        Tick::Moved(target.0, target.1)
    }
}

/// Connect to the X server and resolve the default root window.
fn connect_root() -> Result<(RustConnection, u32), String> {
    let (conn, screen) =
        x11rb::connect(None).map_err(|error| format!("x11rb::connect failed: {error}"))?;
    let root = conn
        .setup()
        .roots
        .get(screen)
        .map(|screen| screen.root)
        .ok_or_else(|| format!("no X screen {screen}"))?;
    Ok((conn, root))
}

/// Reposition this process's popup window without a drag in progress.
///
/// Used to park the persistent popup offscreen ("hide") and to bring it back
/// to the cursor-follow position ("show") — both without destroying the GPUI
/// window, so toggling stays instant. Same server-side locate as the drag
/// path; fails (logged, non-fatal) on native Wayland where there is no X
/// window to configure.
///
/// `ConfigureWindow` is legal on unmapped windows and takes effect on map,
/// so — unlike the drag — this deliberately does NOT require the window to
/// be viewable: showing moves first, then maps.
pub fn move_popup_to(app_id: &str, physical_size: (f32, f32), x: i32, y: i32) -> Result<(), String> {
    let (conn, root) = connect_root()?;
    let expected = (physical_size.0.round() as i32, physical_size.1.round() as i32);
    let window = locate_popup_window(
        &conn,
        root,
        app_id,
        std::process::id(),
        expected,
        false,
    )?;
    let aux = ConfigureWindowAux {
        x: Some(x),
        y: Some(y),
        ..Default::default()
    };
    conn.configure_window(window, &aux)
        .map_err(|error| format!("ConfigureWindow error: {error}"))?
        .check()
        .map_err(|error| format!("ConfigureWindow rejected: {error}"))?;
    conn.flush()
        .map_err(|error| format!("flush error: {error}"))?;
    Ok(())
}

/// Map or unmap this process's popup window.
///
/// Hiding parks nothing: client `ConfigureWindow` moves on a *managed* X11
/// window go through the WM as `ConfigureRequest`s, and Mutter silently
/// clamps absurd positions (e.g. -10000) back onscreen — verified live: the
/// server accepted the move yet the window stayed put. Unmapping is honored
/// unconditionally, so hide = unmap, show = move + map.
pub fn set_popup_mapped(
    app_id: &str,
    physical_size: (f32, f32),
    mapped: bool,
) -> Result<(), String> {
    let (conn, root) = connect_root()?;
    let expected = (physical_size.0.round() as i32, physical_size.1.round() as i32);
    // No viewable requirement in either direction: mapping targets an
    // unmapped window by definition, and unmapping an already-unmapped one
    // is a harmless server-side no-op (keeps hide idempotent).
    let window = locate_popup_window(
        &conn,
        root,
        app_id,
        std::process::id(),
        expected,
        false,
    )?;
    if mapped {
        conn.map_window(window)
            .map_err(|error| format!("MapWindow error: {error}"))?
            .check()
            .map_err(|error| format!("MapWindow rejected: {error}"))?;
    } else {
        conn.unmap_window(window)
            .map_err(|error| format!("UnmapWindow error: {error}"))?
            .check()
            .map_err(|error| format!("UnmapWindow rejected: {error}"))?;
    }
    conn.flush()
        .map_err(|error| format!("flush error: {error}"))?;
    Ok(())
}

/// Live geometry (device px: x, y, w, h) of this process's popup X window.
///
/// Matches WM_CLASS + pid like the drag locate, but with NO size precondition
/// and NO viewable requirement beyond being mapped: it works at the window's
/// current position (it may have been dragged) and never matches the 2x2
/// holder — the largest match wins.
///
/// The outside-click test no longer uses X for this (the X pointer is frozen
/// over Wayland-native windows, so the popup reports the presses it handles
/// instead); kept for X-side debugging.
#[allow(dead_code)]
pub fn popup_geometry(app_id: &str) -> Option<(i32, i32, i32, i32)> {
    let (conn, root) = connect_root().ok()?;
    let tree = conn.query_tree(root).ok()?.reply().ok()?;
    let net_wm_pid = conn
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let pid = std::process::id();
    let mut best: Option<(i32, i32, i32, i32)> = None;
    for window in tree.children {
        let Some(class) = window_class(&conn, window) else {
            continue;
        };
        if !class
            .windows(app_id.len())
            .any(|chunk| chunk == app_id.as_bytes())
        {
            continue;
        }
        if let Some(owner) = window_pid(&conn, net_wm_pid, window) {
            if owner != pid {
                continue;
            }
        }
        let mapped = conn
            .get_window_attributes(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|attrs| attrs.map_state == MapState::VIEWABLE)
            .unwrap_or(false);
        if !mapped {
            continue;
        }
        let geo = conn.get_geometry(window).ok()?.reply().ok()?;
        let origin = conn
            .translate_coordinates(window, root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        let candidate = (
            i32::from(origin.dst_x),
            i32::from(origin.dst_y),
            i32::from(geo.width),
            i32::from(geo.height),
        );
        let area = candidate.2 as i64 * candidate.3 as i64;
        let best_area = best.map(|b| b.2 as i64 * b.3 as i64).unwrap_or(-1);
        if area > best_area {
            best = Some(candidate);
        }
    }
    best
}

/// The X id of this process's popup window — the same WM_CLASS/`_NET_WM_PID`
/// match as [`popup_geometry`], plus `_NET_WM_PID`-checked same-class
/// non-mapped windows, so it resolves even before the first show. Size filter
/// is skipped; among same-process windows the popup (720×960) is the largest
/// surface, which keeps the Settings/Wizard windows from matching.
pub fn popup_window_id(app_id: &str) -> Option<u32> {
    let (conn, root) = connect_root().ok()?;
    let tree = conn.query_tree(root).ok()?.reply().ok()?;
    let net_wm_pid = conn
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let pid = std::process::id();
    let mut best: Option<(u32, i64)> = None;
    for window in tree.children {
        let Some(class) = window_class(&conn, window) else {
            continue;
        };
        if !class
            .windows(app_id.len())
            .any(|chunk| chunk == app_id.as_bytes())
        {
            continue;
        }
        if let Some(owner) = window_pid(&conn, net_wm_pid, window) {
            if owner != pid {
                continue;
            }
        }
        let area = conn
            .get_geometry(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|geo| geo.width as i64 * geo.height as i64)
            .unwrap_or(1);
        let best_area = best.map(|b| b.1).unwrap_or(-1);
        if area > best_area {
            best = Some((window, area));
        }
    }
    best.map(|b| b.0)
}

/// Find this process's popup window among the root's top-level windows.
///
/// Matched by: WM_CLASS containing the app id,
/// `_NET_WM_PID` equal to this process when the property exists, and physical
/// size within [`SIZE_TOLERANCE`] of the expected popup size. The size check is
/// what keeps the Settings window (same pid, same class, different size) out.
/// `mapped_only` additionally requires `MapState::VIEWABLE` (used only by the
/// drag path — a drag on an invisible window is nonsense; move/map/unmap must
/// also work on unmapped windows).
fn locate_popup_window(
    conn: &RustConnection,
    root: u32,
    app_id: &str,
    pid: u32,
    expected: (i32, i32),
    mapped_only: bool,
) -> Result<u32, String> {
    let tree = conn
        .query_tree(root)
        .map_err(|error| format!("query_tree error: {error}"))?
        .reply()
        .map_err(|error| format!("query_tree reply: {error}"))?;
    let net_wm_pid = conn
        .intern_atom(false, b"_NET_WM_PID")
        .map_err(|error| format!("intern_atom error: {error}"))?
        .reply()
        .map_err(|error| format!("intern_atom reply: {error}"))?
        .atom;
    let mut candidates = Vec::new();
    for window in tree.children {
        let class = match window_class(conn, window) {
            Some(class) => class,
            None => continue,
        };
        if !class
            .windows(app_id.len())
            .any(|chunk| chunk == app_id.as_bytes())
        {
            continue;
        }
        // Record every same-class top-level with its distinguishing facts, so
        // the failure message explains exactly why each was rejected.
        let attrs = conn
            .get_window_attributes(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        let geo = conn
            .get_geometry(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        let owner = window_pid(conn, net_wm_pid, window)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        let size = geo
            .as_ref()
            .map(|geo| format!("{}x{}", geo.width, geo.height))
            .unwrap_or_else(|| "?".to_string());
        let mapped = attrs
            .as_ref()
            .map(|attrs| u8::from(attrs.map_state))
            .unwrap_or(0);
        candidates.push(format!(
            "0x{window:x} pid={owner} size={size} map_state={mapped}"
        ));
        if let Some(owner) = window_pid(conn, net_wm_pid, window) {
            if owner != pid {
                continue;
            }
        }
        let Some(attrs) = attrs else { continue };
        if mapped_only && attrs.map_state != MapState::VIEWABLE {
            continue;
        }
        let Some(geo) = geo else { continue };
        let (width, height) = (i32::from(geo.width), i32::from(geo.height));
        if (width - expected.0).abs() > SIZE_TOLERANCE
            || (height - expected.1).abs() > SIZE_TOLERANCE
        {
            continue;
        }
        return Ok(window);
    }
    Err(format!(
        "no viewable X window matches class={app_id} pid={pid} size≈{}x{}; \
         same-class windows: {}",
        expected.0,
        expected.1,
        if candidates.is_empty() {
            "(none)".to_string()
        } else {
            candidates.join(", ")
        }
    ))
}

/// WM_CLASS (instance\0class, both STRING) of a window, raw bytes.
fn window_class(conn: &RustConnection, window: u32) -> Option<Vec<u8>> {
    let reply = conn
        .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
        .ok()?
        .reply()
        .ok()?;
    if reply.type_ != u32::from(AtomEnum::STRING) {
        return None;
    }
    Some(reply.value)
}

/// `_NET_WM_PID` of a window, when present.
fn window_pid(conn: &RustConnection, net_wm_pid: u32, window: u32) -> Option<u32> {
    let reply = conn
        .get_property(false, window, net_wm_pid, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let first = reply.value32()?.next()?;
    Some(first)
}
