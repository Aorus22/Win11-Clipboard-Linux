//! Windows 11 Clipboard History — GPUI frontend entry point.
//!
//! Milestone v0.8.0: pixel-identical port of the Tauri/React frontend.
//! - Default: clipboard popup (360×480, ui_scale multiplies).
//! - `--settings`: also open the settings window (480×520, decorated).
//! - `--setup` or first run (own marker): wizard window (550×650) instead of popup.
//! - `--background`: tray-only start (no popup until toggled).
//! - `--toggle` / `--settings-open` / `--quit`: signal a running instance, exit fast.
//! - `--version` / `-v`: print version.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Application, AssetSource, Bounds, Context, Render, SharedString, Size, Window,
    WindowBounds, WindowDecorations, WindowHandle, WindowKind, WindowOptions, div, point,
    prelude::*, px,
};

mod app_state;
mod backend;
mod click_watch;
mod drag_log;
mod geometry;
mod gnome_shortcut;
mod history;
mod hotkey;
mod instance;
mod pickers;
mod settings;
#[cfg(test)]
mod test_util;
mod theme;
mod theme_watch;
mod tray;
mod ui;
mod window_drag;

use app_state::Shared;
use backend::BackendService;
use geometry::{MonitorRect, bottom_center, clamp_to_monitor, cursor_position};
use instance::{AppSignal, InstanceRole};
use ui::popup::{Popup, Tab};
use ui::settings::SettingsState;
use ui::wizard::WizardState;
use win11_clipboard_history_lib::{focus_manager, session};

/// Separate app-id from the Tauri build so both can coexist (SYS-06).
const APP_ID: &str = "dev.gustavosett.clipboard-history-gpui";

/// Main popup dimensions mirror `tauri.conf.json` (`main` window).
/// ui_scale multiplies both (documented delta: React zooms content instead).
const POPUP_W: f32 = 360.0;
const POPUP_H: f32 = 480.0;
const SETTINGS_W: f32 = 480.0;
const SETTINGS_H: f32 = 520.0;
const SETUP_W: f32 = 550.0;
const SETUP_H: f32 = 650.0;

/// Offscreen origin where the persistent popup is created while "hidden".
/// Best effort only: the WM may clamp it onscreen, which is why hiding also
/// unmaps the window (see `window_drag::set_popup_mapped`).
const PARKED_POS: (f32, f32) = (-10000.0, -10000.0);

/// Physical X-window size of the popup (bounds x scale factor — see the drag
/// path for why `viewport_size()` can't be used on HiDPI).
fn popup_physical_size(
    handle: &WindowHandle<Popup>,
    cx: &mut gpui::AsyncApp,
) -> Option<(f32, f32)> {
    handle
        .update(cx, |_, window, _| {
            let scale = window.scale_factor();
            let bounds = window.bounds().size;
            (
                bounds.width.to_f64() as f32 * scale,
                bounds.height.to_f64() as f32 * scale,
            )
        })
        .ok()
}

/// Display scale factor of the popup's window — the factor between gpui's
/// logical coordinates and the physical frame the cursor and `move_popup_to`
/// use (2.0 on a 3072x1728 screen reporting `Xft.dpi: 192`).
fn popup_scale_factor(handle: &WindowHandle<Popup>, cx: &mut gpui::AsyncApp) -> Option<f32> {
    handle
        .update(cx, |_, window, _| window.scale_factor())
        .ok()
}

/// Move the persistent popup's X11 window (show → cursor-follow position,
/// hide → [`PARKED_POS`]) without touching the GPUI window itself.
///
/// gpui 0.2.2 exposes no per-window move/hide, so this goes straight to the
/// server like the client-side drag does. Logs and continues on failure
/// (native Wayland has no X window to configure).
fn place_popup(handle: &WindowHandle<Popup>, cx: &mut gpui::AsyncApp, x: i32, y: i32) {
    match popup_physical_size(handle, cx) {
        Some(size) => {
            if let Err(e) = window_drag::move_popup_to(APP_ID, size, x, y) {
                eprintln!("[gpui-app] move popup failed: {e}");
            }
        }
        None => eprintln!("[gpui-app] popup gone while moving"),
    }
}

/// Hide the persistent popup: unmap its X11 window and hand focus back.
/// Unmapping (not offscreen parking) is what actually removes a managed
/// window from the screen.
fn hide_popup_window(handle: &WindowHandle<Popup>, cx: &mut gpui::AsyncApp) {
    if let Some(size) = popup_physical_size(handle, cx) {
        if let Err(e) = window_drag::set_popup_mapped(APP_ID, size, false) {
            eprintln!("[gpui-app] unmap popup failed: {e}");
        }
    }
    let _ = focus_manager::restore_focused_window();
    focus_manager::note_popup_window(None);
}

struct GpuiAssets {
    base: PathBuf,
}

impl AssetSource for GpuiAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        match std::fs::read(self.base.join(path)) {
            Ok(data) => Ok(Some(std::borrow::Cow::Owned(data))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(std::fs::read_dir(self.base.join(path))?
            .filter_map(|entry| {
                Some(SharedString::from(
                    entry.ok()?.path().to_string_lossy().into_owned(),
                ))
            })
            .collect())
    }
}

/// Asset directory across the layouts we ship:
/// 1. `$APPDIR/usr/share/...` — AppImage payload.
/// 2. `<exe_dir>/../share/...` — installed PREFIX (AppImage `usr/bin` too).
/// 3. `CARGO_MANIFEST_DIR/assets` — dev checkout (`cargo run`).
/// 4. `/usr/share/...` — system install.
fn assets_dir() -> PathBuf {
    const ASSETS_SUFFIX: &str = "share/win11-clipboard-history-gpui/assets";
    if let Some(appdir) = std::env::var_os("APPDIR").filter(|d| !d.is_empty()) {
        let candidate = PathBuf::from(appdir).join("usr").join(ASSETS_SUFFIX);
        if candidate.is_dir() {
            return candidate;
        }
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        let candidate = dir.join("..").join(ASSETS_SUFFIX);
        if candidate.is_dir() {
            return candidate;
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    if bundled.is_dir() {
        return bundled;
    }
    PathBuf::from("/").join(ASSETS_SUFFIX)
}

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
fn pin_display_backend_to_x11() {
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

/// Initial popup origin in **physical** pixels: cursor-follow on X11,
/// bottom-center on Wayland — matching the Tauri `WindowController` behavior
/// exactly.
///
/// `win_w`/`win_h` are the popup's real physical size and `scale` its display
/// scale factor: gpui hands out display bounds in logical pixels, while the
/// cursor query and the X11 move are physical, so the monitor rect is scaled up
/// here before anything is compared. Passing `scale = 1.0` clamps in gpui's
/// logical frame instead — that is what the pre-window creation path needs,
/// since the scale factor is only readable from a live window.
fn initial_origin(cx: &App, win_w: i32, win_h: i32, scale: f32) -> (f32, f32) {
    let displays = cx.displays();
    let rect_of = |b: gpui::Bounds<gpui::Pixels>| MonitorRect {
        x: f32::from(b.origin.x) as i32,
        y: f32::from(b.origin.y) as i32,
        w: f32::from(b.size.width) as i32,
        h: f32::from(b.size.height) as i32,
    };
    let first = displays.first().map(|d| rect_of(d.bounds()).scaled(scale));
    let Some(mon) = first else {
        return (0.0, 0.0);
    };
    if session::is_wayland() {
        let (x, y) = bottom_center(&mon, win_w, win_h);
        return (x as f32, y as f32);
    }
    match cursor_position() {
        Some((cx_, cy)) => {
            // Prefer the monitor containing the cursor (multi-monitor parity).
            let mon = cx
                .displays()
                .iter()
                .map(|d| rect_of(d.bounds()).scaled(scale))
                .find(|m| m.contains(cx_, cy))
                .unwrap_or(mon);
            let (x, y) = clamp_to_monitor(&mon, win_w, win_h, cx_, cy);
            (x as f32, y as f32)
        }
        None => {
            let (x, y) = bottom_center(&mon, win_w, win_h);
            (x as f32, y as f32)
        }
    }
}

fn popup_options(origin: (f32, f32), w: f32, h: f32) -> WindowOptions {
    let bounds = Bounds::new(
        point(px(origin.0), px(origin.1)),
        Size {
            width: px(w),
            height: px(h),
        },
    );
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background: gpui::WindowBackgroundAppearance::Transparent,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        focus: true,
        app_id: Some(APP_ID.into()),
        ..Default::default()
    }
}

fn centered_options(w: f32, h: f32, title: &'static str, cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, gpui::size(px(w), px(h)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // Client-side decorations. Whatever the session answers with decides who
        // paints the title bar: `Decorations::Server` means the WM does, and the
        // views then draw `ui::titlebar` (min/max/close + drag) to fill the gap
        // when it answers `Decorations::Client` instead.
        window_decorations: Some(WindowDecorations::Client),
        // Window title: X11 takes it from here, Wayland via `set_window_title`.
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(title.into()),
            ..Default::default()
        }),
        show: true,
        focus: true,
        app_id: Some(APP_ID.into()),
        ..Default::default()
    }
}

fn open_popup_window(
    cx: &mut App,
    backend: &Arc<BackendService>,
    shared: &Shared,
    scale: f32,
    initial_tab: Tab,
    start_hidden: bool,
) -> WindowHandle<Popup> {
    // Persistent-window model: the popup is created once and parked offscreen
    // ("hidden"); showing only moves it to the cursor, refreshes content and
    // activates it — no window construction on the toggle path, so Super+V
    // feels instant. `start_hidden` skips activation/focus so a background
    // start never steals focus.
    let (w, h) = (POPUP_W * scale, POPUP_H * scale);
    let origin = if start_hidden {
        PARKED_POS
    } else {
        // Fresh position on every show (follow-mouse parity) + fresh state.
        // No window exists yet, so the display scale factor is unknown: clamp in
        // gpui's logical frame here (`scale = 1.0`) and let the first show —
        // which does know it — re-place the window physically.
        focus_manager::save_focused_window();
        initial_origin(cx, w as i32, h as i32, 1.0)
    };
    let handle = cx
        .open_window(popup_options(origin, w, h), {
            let backend = Arc::clone(backend);
            let shared = Arc::clone(shared);
            move |_window, cx: &mut App| {
                let focus = cx.focus_handle();
                let search_focus = cx.focus_handle();
                let emoji_focus = cx.focus_handle();
                let kaomoji_focus = cx.focus_handle();
                let symbol_focus = cx.focus_handle();
                let settings = shared.lock().settings.clone();
                cx.new(|cx| {
                    Popup::new(
                        backend,
                        shared,
                        settings,
                        focus,
                        search_focus,
                        emoji_focus,
                        kaomoji_focus,
                        symbol_focus,
                        initial_tab,
                        cx,
                    )
                })
            }
        })
        .expect("failed to open GPUI popup window");
    if start_hidden {
        // Parked: only arm focus-loss dismissal, take neither focus nor
        // activation until the first real show — then withdraw immediately:
        // creation maps the window and the WM may clamp the parked origin
        // onscreen. Retry briefly; the X window may not be visible to our
        // own connection yet.
        let _ = handle.update(cx, |popup, window, cx| {
            popup.watch_activation(window, cx);
            cx.notify();
        });
        let size = handle.update(cx, |_, window, _| {
            let scale = window.scale_factor();
            let bounds = window.bounds().size;
            (
                bounds.width.to_f64() as f32 * scale,
                bounds.height.to_f64() as f32 * scale,
            )
        });
        if let Ok(size) = size {
            let mut last_err = String::new();
            for _ in 0..20 {
                match window_drag::set_popup_mapped(APP_ID, size, false) {
                    Ok(()) => {
                        last_err.clear();
                        break;
                    }
                    Err(e) => {
                        last_err = e;
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
            }
            if !last_err.is_empty() {
                eprintln!("[gpui-app] initial unmap failed: {last_err}");
            }
        }
        return handle;
    }
    let _ = handle.update(cx, |popup, window, cx| {
        // gpui's X11 backend ignores `WindowOptions.focus`, so ask the window
        // manager directly: an unfocused popup receives no keystrokes (search
        // text, arrows, Enter) and never reports losing focus, so clicking
        // outside would leave it up.
        window.activate_window();
        popup.focus.focus(window);
        popup.watch_activation(window, cx);
        cx.notify();
    });
    handle
}

fn open_settings_window(
    cx: &mut App,
    backend: &Arc<BackendService>,
    shared: &Shared,
) -> WindowHandle<SettingsState> {
    let handle = cx
        .open_window(
            centered_options(SETTINGS_W, SETTINGS_H, "Settings — Clipboard History", cx),
            {
            let backend = Arc::clone(backend);
            let shared = Arc::clone(shared);
            move |_window, cx: &mut App| {
                let focus = cx.focus_handle();
                let kaomoji = cx.focus_handle();
                let autodelete = cx.focus_handle();
                let maxhistory = cx.focus_handle();
                let dark_op = cx.focus_handle();
                let light_op = cx.focus_handle();
                let uiscale = cx.focus_handle();
                cx.new(|cx| {
                    SettingsState::new(
                        shared, backend, focus, kaomoji, autodelete, maxhistory, dark_op,
                        light_op, uiscale, cx,
                    )
                })
            }
        })
        .expect("failed to open GPUI settings window");
    let _ = handle.update(cx, |state, window, cx| {
        window.set_window_title("Settings — Clipboard History");
        state.focus.focus(window);
        cx.notify();
    });
    handle
}

fn open_wizard_window(
    cx: &mut App,
    backend: &Arc<BackendService>,
    shared: &Shared,
) -> WindowHandle<WizardState> {
    let handle = cx
        .open_window(
            centered_options(SETUP_W, SETUP_H, "Setup — Clipboard History", cx),
            {
            let backend = Arc::clone(backend);
            let shared = Arc::clone(shared);
            move |_window, cx: &mut App| {
                let focus = cx.focus_handle();
                cx.new(|cx| WizardState::new(shared, backend, focus, cx))
            }
        })
        .expect("failed to open GPUI setup wizard window");
    let _ = handle.update(cx, |state, window, cx| {
        window.set_window_title("Setup — Clipboard History");
        state.focus.focus(window);
        cx.notify();
    });
    handle
}

struct HolderView;

impl Render for HolderView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.))
    }
}

/// Permanent 1×1 window, created UNMAPPED so it never appears in taskbars,
/// docks, or pagers — yet keeps the platform run loop alive when the popup is
/// closed (tray-only resident). Opened once, never closed.
/// (If the run loop turns out to key off mapped surfaces, this reverts to mapped.)
fn open_holder_window(cx: &mut App) {
    let bounds = Bounds::new(point(px(0.), px(0.)), gpui::size(px(1.), px(1.)));
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            show: false,
            focus: false,
            kind: WindowKind::PopUp,
            is_movable: false,
            app_id: Some(APP_ID.into()),
            ..Default::default()
        },
        |_window, cx: &mut App| cx.new(|_| HolderView),
    );
}

fn has_arg(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn main() {
    // Must run before the platform is initialized (and before anything caches
    // the session type) so the whole process agrees on the X11 backend.
    pin_display_backend_to_x11();

    // The settings window reads the rendering environment (NVIDIA/AppImage
    // transparency note). The Tauri build initializes this at startup; without
    // it here the first open of Settings panicked and, with `panic = "abort"`,
    // took the whole resident app down.
    win11_clipboard_history_lib::rendering_env::init();

    let args: Vec<String> = std::env::args().collect();
    if has_arg(&args, "--version") || has_arg(&args, "-v") {
        println!("win11-clipboard-history-gpui {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    // Client modes: signal the running instance, then exit fast.
    for (flag, signal) in [
        ("--toggle", AppSignal::Toggle),
        ("--settings-open", AppSignal::OpenSettings),
        ("--quit", AppSignal::Quit),
    ] {
        if has_arg(&args, flag) {
            match instance::send_signal(signal) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("[gpui-app] no running instance ({e})");
                    std::process::exit(1);
                }
            }
        }
    }
    // Headless setup helpers (also used by packaging scripts).
    if has_arg(&args, "--register-shortcuts") {
        match gnome_shortcut::register() {
            Ok(msg) => {
                println!("{msg}");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("register failed: {e}");
                std::process::exit(1);
            }
        }
    }
    if has_arg(&args, "--unregister-shortcuts") {
        match gnome_shortcut::unregister() {
            Ok(msg) => {
                println!("{msg}");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("unregister failed: {e}");
                std::process::exit(1);
            }
        }
    }

    let want_settings = has_arg(&args, "--settings");
    let want_setup = has_arg(&args, "--setup") || app_state::is_first_run();
    let background = has_arg(&args, "--background");
    // Verification aid: open a specific popup tab to exercise its render path.
    let initial_tab = match std::env::var("GPUI_SMOKE_TAB").as_deref() {
        Ok("emoji") => Tab::Emoji,
        Ok("kaomoji") => Tab::Kaomoji,
        Ok("symbols") => Tab::Symbols,
        _ => Tab::Clipboard,
    };

    let settings = settings::load();
    let backend = BackendService::new(&settings);
    let shared = app_state::shared(settings);

    eprintln!(
        "[gpui-app] loaded {} history items from {}",
        backend.snapshot().len(),
        backend::history_path().display()
    );

    // Single instance first: a duplicate normal start exits quietly.
    let (sig_tx, mut sig_rx) = tokio::sync::mpsc::unbounded_channel();
    match instance::acquire(sig_tx.clone()) {
        InstanceRole::Notified => {
            eprintln!("[gpui-app] another instance is running; exiting");
            return;
        }
        InstanceRole::Primary => {}
    }
    theme_watch::spawn_theme_watcher(sig_tx);
    // Physical outside-click dismissal (evdev). Independent of focus, so it
    // keeps working on focus-follows-mouse desktops where focus-loss alone
    // cannot tell a hover from a click.
    click_watch::spawn_click_watcher(shared.clone());

    // GTK must initialize on the main thread before the AppIndicator tray backend.
    let mut tray = match gtk::init() {
        Ok(()) => {
            let s = shared.lock().settings.clone();
            match tray::Tray::build(s.enable_dynamic_tray_icon, settings::resolve_dark(&s)) {
                Ok(t) => Some(t),
                Err(e) => {
                    eprintln!("[gpui-app] tray unavailable: {e}");
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("[gpui-app] GTK unavailable, running without tray: {e:?}");
            None
        }
    };

    Application::new()
        .with_assets(GpuiAssets { base: assets_dir() })
        .run(move |cx: &mut App| {
            // Resident holder first: the app must survive with zero visible windows.
            open_holder_window(cx);
            // Persistent popup: created lazily on first toggle (or immediately
            // below when starting visible) and never destroyed afterwards —
            // show/hide only parks the X window offscreen.
            let mut popup: Option<WindowHandle<Popup>> = None;
            let mut popup_visible = false;
            let mut popup_scale: Option<f32> = None;
            let show_at_start = !want_setup && !background;
            if want_setup {
                open_wizard_window(cx, &backend, &shared);
            } else if want_settings {
                open_settings_window(cx, &backend, &shared);
            }
            let mut settings_win: Option<WindowHandle<SettingsState>> = None;

            // Poll loop: backend refresh + tray/menu/hotkey/IPC signals.
            // Tray + hotkeys live here (task-local): no cross-thread sharing.
            let hotkeys = hotkey::register_hotkeys();
            let mut last_theme_dark: Option<bool> = None;

            cx.spawn(async move |cx| {
                // Visible start (non-background launch) reuses the exact toggle
                // path once: create parked, then show (with the smoke-test tab
                // when GPUI_SMOKE_TAB requests one).
                let mut pending_show_tab: Option<Tab> = show_at_start.then_some(initial_tab);
                loop {
                    // Short fixed cadence (was 300 ms): keeps toggle latency
                    // low and tray clicks / X11 hotkeys / backend refresh
                    // responsive. Signals are drained below every tick.
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    // 0. GLib main context: libayatana-appindicator registers its
                    // StatusNotifierItem and delivers menu/click events from
                    // idle callbacks on this context. Nothing pumps it on its
                    // own because we never run a gtk::main loop, so without this
                    // the tray icon never appears in the GNOME top bar and its
                    // clicks are never delivered. Pumped every tick (50 ms or on
                    // signal wake) while costing almost nothing.
                    if tray.is_some() {
                        while gtk::events_pending() {
                            gtk::main_iteration_do(false);
                        }
                    }
                    // 1. Backend + settings refresh; drop dead popup handles.
                    if let Some(h) = &popup {
                        if h
                            .update(cx, |popup, _window, cx| {
                                popup.poll_backend(cx);
                            })
                            .is_err()
                        {
                            popup = None;
                        }
                    }
                    // 2. Collect signals: instance IPC, tray, hotkeys.
                    let mut signals: Vec<AppSignal> = Vec::new();
                    let mut show_tab = Tab::Clipboard;
                    if let Some(tab) = pending_show_tab.take() {
                        show_tab = tab;
                        signals.push(AppSignal::Toggle);
                    }
                    while let Ok(s) = sig_rx.try_recv() {
                        signals.push(s);
                    }
                    if let Some(t) = tray.as_ref() {
                        signals.extend(t.poll_menu());
                        signals.extend(t.poll_clicks());
                    }
                    if let Some(hk) = hotkeys.as_ref() {
                        signals.extend(hk.poll());
                    }
                    // 2b. Hide requests from the popup view (focus loss, Escape,
                    // paste, close button): park the persistent window instead
                    // of destroying it.
                    let hide_requested = std::mem::replace(
                        &mut shared.lock().hide_popup_requested,
                        false,
                    );
                    if hide_requested && popup_visible {
                        if let Some(h) = popup.as_ref() {
                            hide_popup_window(h, cx);
                        }
                        popup_visible = false;
                    }
                    // 3. Act.
                    for signal in signals {
                        match signal {
                            AppSignal::Toggle => {
                                let st = shared.lock().settings.clone();
                                // ui_scale changed since the window was built:
                                // rebuild once (rare), otherwise reuse.
                                if popup_scale != Some(st.ui_scale) {
                                    if let Some(h) = popup.take() {
                                        let _ = h.update(cx, |_, window, _| {
                                            window.remove_window();
                                        });
                                    }
                                    popup_visible = false;
                                    popup_scale = None;
                                }
                                if popup_visible {
                                    if let Some(h) = popup.as_ref() {
                                        hide_popup_window(h, cx);
                                    }
                                    popup_visible = false;
                                } else {
                                    if popup.is_none() {
                                        match cx.update(|cx| {
                                            open_popup_window(
                                                cx,
                                                &backend,
                                                &shared,
                                                st.ui_scale,
                                                Tab::Clipboard,
                                                true,
                                            )
                                        }) {
                                            Ok(h) => {
                                            popup = Some(h);
                                            popup_scale = Some(st.ui_scale);
                                            focus_manager::note_popup_window(
                                                window_drag::popup_window_id(APP_ID),
                                            );
                                        }
                                            Err(e) => {
                                                eprintln!("[gpui-app] popup open failed: {e:?}")
                                            }
                                        }
                                    }
                                    if let Some(h) = popup.as_ref() {
                                        // Show: fresh cursor-follow position,
                                        // fresh content + appear animation,
                                        // then take focus.
                                        focus_manager::note_popup_window(
                                            window_drag::popup_window_id(APP_ID),
                                        );
                                        focus_manager::save_focused_window();
                                        // The clamping frame is physical, so it
                                        // needs the window's *real* size and
                                        // display scale — not `ui_scale`.
                                        let (w, h_) = popup_physical_size(h, cx)
                                            .unwrap_or((
                                                POPUP_W * st.ui_scale,
                                                POPUP_H * st.ui_scale,
                                            ));
                                        let display_scale =
                                            popup_scale_factor(h, cx).unwrap_or(1.0);
                                        let origin = cx.update(|cx| {
                                            initial_origin(
                                                cx,
                                                w as i32,
                                                h_ as i32,
                                                display_scale,
                                            )
                                        });
                                        match origin {
                                            Ok((x, y)) => {
                                                // Move first (invisible), then
                                                // map: no onscreen jump.
                                                place_popup(h, cx, x as i32, y as i32);
                                                if let Some(size) =
                                                    popup_physical_size(h, cx)
                                                {
                                                    if let Err(e) =
                                                        window_drag::set_popup_mapped(
                                                            APP_ID, size, true,
                                                        )
                                                    {
                                                        eprintln!(
                                                            "[gpui-app] map popup failed: {e}"
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => eprintln!(
                                                "[gpui-app] origin failed: {e:?}"
                                            ),
                                        }
                                        let _ = h.update(cx, |popup, window, cx| {
                                            popup.prepare_for_show(show_tab, cx);
                                            window.activate_window();
                                            popup.focus.focus(window);
                                            cx.notify();
                                        });
                                        popup_visible = true;
                                    }
                                }
                            }
                            AppSignal::OpenSettings => {
                                let need_open = match settings_win.as_ref() {
                                    Some(h) => h
                                        .update(cx, |_, window, _| {
                                            window.activate_window();
                                        })
                                        .is_err(),
                                    None => true,
                                };
                                if need_open {
                                    match cx.update(|cx| {
                                        open_settings_window(cx, &backend, &shared)
                                    }) {
                                        Ok(h) => settings_win = Some(h),
                                        Err(e) => {
                                            eprintln!("[gpui-app] settings open failed: {e:?}")
                                        }
                                    }
                                }
                            }
                            AppSignal::Quit => std::process::exit(0),
                            AppSignal::ThemeChanged => {
                                let st = shared.lock().settings.clone();
                                let dark = match st.theme_mode.as_str() {
                                    "dark" => true,
                                    "light" => false,
                                    _ => settings::system_prefers_dark(),
                                };
                                if last_theme_dark != Some(dark) {
                                    last_theme_dark = Some(dark);
                                    if let Some(t) = tray.as_mut() {
                                        t.rebuild_icon(st.enable_dynamic_tray_icon, dark);
                                    }
                                    // Publish so the popup re-renders with the new theme.
                                    shared.lock().version += 1;
                                }
                            }
                        }
                    }
                    // Publish visibility for background threads (the evdev
                    // outside-click watcher gates on it).
                    shared.lock().popup_visible = popup_visible;
                }
            })
            .detach();

            cx.activate(true);
        });
}
