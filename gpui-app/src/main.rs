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
use std::sync::{Arc, mpsc};
use std::time::Duration;

use gpui::{
    App, Application, AssetSource, Bounds, Context, Render, SharedString, Size, Window,
    WindowBounds, WindowDecorations, WindowHandle, WindowKind, WindowOptions, div, point,
    prelude::*, px,
};

mod app_state;
mod backend;
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

fn assets_dir() -> PathBuf {
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    if bundled.is_dir() {
        return bundled;
    }
    // Installed layout (Phase 5 packaging).
    PathBuf::from("/usr/share/win11-clipboard-history-gpui/assets")
}

/// Initial popup origin: cursor-follow on X11, bottom-center on Wayland —
/// matching the Tauri `WindowController` behavior exactly.
fn initial_origin(cx: &App, win_w: i32, win_h: i32) -> (f32, f32) {
    let displays = cx.displays();
    let rect_of = |b: gpui::Bounds<gpui::Pixels>| MonitorRect {
        x: f32::from(b.origin.x) as i32,
        y: f32::from(b.origin.y) as i32,
        w: f32::from(b.size.width) as i32,
        h: f32::from(b.size.height) as i32,
    };
    let first = displays.first().map(|d| rect_of(d.bounds()));
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
                .map(|d| rect_of(d.bounds()))
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
        is_movable: false,
        focus: true,
        app_id: Some(APP_ID.into()),
        ..Default::default()
    }
}

fn centered_options(w: f32, h: f32, cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, gpui::size(px(w), px(h)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_decorations: Some(WindowDecorations::Client),
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
) -> WindowHandle<Popup> {
    // Fresh position on every show (follow-mouse parity) + fresh state.
    focus_manager::save_focused_window();
    let (w, h) = (POPUP_W * scale, POPUP_H * scale);
    let origin = initial_origin(cx, w as i32, h as i32);
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
    let _ = handle.update(cx, |popup, window, cx| {
        popup.focus.focus(window);
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
        .open_window(centered_options(SETTINGS_W, SETTINGS_H, cx), {
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
        .open_window(centered_options(SETUP_W, SETUP_H, cx), {
            let backend = Arc::clone(backend);
            let shared = Arc::clone(shared);
            move |_window, cx: &mut App| {
                let focus = cx.focus_handle();
                cx.new(|cx| WizardState::new(shared, backend, focus, cx))
            }
        })
        .expect("failed to open GPUI setup wizard window");
    let _ = handle.update(cx, |state, window, cx| {
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

/// Permanent 1×1 invisible window: keeps the platform run loop alive when the
/// popup is closed (tray-only resident). Opened once, never closed.
fn open_holder_window(cx: &mut App) {
    let bounds = Bounds::new(point(px(0.), px(0.)), gpui::size(px(1.), px(1.)));
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            show: true,
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
    let scale = settings.ui_scale;
    let backend = BackendService::new(&settings);
    let shared = app_state::shared(settings);

    eprintln!(
        "[gpui-app] loaded {} history items from {}",
        backend.snapshot().len(),
        backend::history_path().display()
    );

    // Single instance first: a duplicate normal start exits quietly.
    let (sig_tx, sig_rx) = mpsc::channel();
    match instance::acquire(sig_tx.clone()) {
        InstanceRole::Notified => {
            eprintln!("[gpui-app] another instance is running; exiting");
            return;
        }
        InstanceRole::Primary => {}
    }
    theme_watch::spawn_theme_watcher(sig_tx);

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
            let mut popup = if !want_setup && !background {
                Some(open_popup_window(cx, &backend, &shared, scale, initial_tab))
            } else {
                None
            };
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
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
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
                    // 3. Act.
                    for signal in signals {
                        match signal {
                            AppSignal::Toggle => {
                                if let Some(h) = popup.take() {
                                    let _ = h.update(cx, |_, window, _| {
                                        window.remove_window();
                                    });
                                } else {
                                    let st = shared.lock().settings.clone();
                                    match cx.update(|cx| {
                                        open_popup_window(
                                            cx,
                                            &backend,
                                            &shared,
                                            st.ui_scale,
                                            Tab::Clipboard,
                                        )
                                    }) {
                                        Ok(h) => popup = Some(h),
                                        Err(e) => {
                                            eprintln!("[gpui-app] reopen failed: {e:?}")
                                        }
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
                }
            })
            .detach();

            cx.activate(true);
        });
}
