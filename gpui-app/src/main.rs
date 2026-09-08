//! Windows 11 Clipboard History — GPUI frontend entry point.
//!
//! Milestone v0.8.0: pixel-identical port of the Tauri/React frontend.
//! Phase 2: live clipboard popup (history + search + paste + positioning).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Application, AssetSource, Bounds, Context, SharedString, Size, WindowBounds, WindowKind,
    WindowOptions, point, prelude::*, px,
};

mod app_state;
mod backend;
mod geometry;
mod history;
mod pickers;
mod settings;
mod theme;
mod ui;

use backend::BackendService;
use geometry::{MonitorRect, bottom_center, clamp_to_monitor, cursor_position};
use ui::popup::{Popup, Tab};
use win11_clipboard_history_lib::{focus_manager, session};

/// Separate app-id from the Tauri build so both can coexist (SYS-06).
const APP_ID: &str = "dev.gustavosett.clipboard-history-gpui";

/// Main popup dimensions mirror `tauri.conf.json` (`main` window).
const POPUP_W: f32 = 360.0;
const POPUP_H: f32 = 480.0;

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

/// Initial window origin: cursor-follow on X11, bottom-center on Wayland —
/// matching the Tauri `WindowController` behavior exactly.
fn initial_origin(cx: &App) -> (f32, f32) {
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
        let (x, y) = bottom_center(&mon, geometry::POPUP_W, geometry::POPUP_H);
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
            let (x, y) = clamp_to_monitor(&mon, geometry::POPUP_W, geometry::POPUP_H, cx_, cy);
            (x as f32, y as f32)
        }
        None => {
            let (x, y) = bottom_center(&mon, geometry::POPUP_W, geometry::POPUP_H);
            (x as f32, y as f32)
        }
    }
}

fn popup_options(origin: (f32, f32)) -> WindowOptions {
    let bounds = Bounds::new(
        point(px(origin.0), px(origin.1)),
        Size {
            width: px(POPUP_W),
            height: px(POPUP_H),
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

fn main() {
    let settings = settings::load();
    let backend = BackendService::new(&settings);
    let shared = app_state::shared(settings.clone());
    eprintln!(
        "[gpui-app] loaded {} history items from {}",
        backend.snapshot().len(),
        backend::history_path().display()
    );
    // Remember the currently focused window (X11) so paste can restore it —
    // mirrors the Tauri show path (`save_focused_window` on toggle).
    focus_manager::save_focused_window();

    Application::new()
        .with_assets(GpuiAssets { base: assets_dir() })
        .run(move |cx: &mut App| {
            let origin = initial_origin(cx);
            let backend = Arc::clone(&backend);
            let shared = Arc::clone(&shared);
            let settings = settings.clone();
            // Verification aid (Phase 3+): open a specific tab to exercise its
            // render path headlessly, e.g. GPUI_SMOKE_TAB=emoji.
            let initial_tab = match std::env::var("GPUI_SMOKE_TAB").as_deref() {
                Ok("emoji") => Tab::Emoji,
                Ok("kaomoji") => Tab::Kaomoji,
                Ok("symbols") => Tab::Symbols,
                _ => Tab::Clipboard,
            };
            let handle = cx
                .open_window(popup_options(origin), |_window, cx: &mut App| {
                    let focus = cx.focus_handle();
                    let search_focus = cx.focus_handle();
                    let emoji_focus = cx.focus_handle();
                    let kaomoji_focus = cx.focus_handle();
                    let symbol_focus = cx.focus_handle();
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
                })
                .expect("failed to open GPUI popup window");

            // Initial keyboard focus on the popup root (parity: focus-first-item).
            let _ = handle.update(cx, |popup, window, cx| {
                popup.focus.focus(window);
                cx.notify();
            });

            // Watcher poll: refresh the snapshot when the backend version bumps.
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let alive = handle.update(cx, |popup, _window, cx| {
                        popup.poll_backend(cx);
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();

            cx.activate(true);
        });
}
