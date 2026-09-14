//! Theme Manager Module for Tauri
//! Handles tray icon dynamic updating and event emission for Tauri frontend,
//! delegating portal detection and caching to win11-clipboard-core.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::image::Image;
use win11_clipboard_core::user_settings::UserSettings;

pub use win11_clipboard_core::theme_manager::{
    clear_theme_cache, get_system_color_scheme, is_event_listener_running, ThemeInfo,
};

/// Cached setting for dynamic tray icon (avoids disk I/O in listener loop)
static DYNAMIC_ICON_ENABLED: AtomicBool = AtomicBool::new(false);

/// Update the cached dynamic tray icon setting
pub fn update_dynamic_tray_flag(enabled: bool) {
    DYNAMIC_ICON_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Helper to get the initial tray icon.
/// Uses a default icon initially to avoid blocking startup, then updates asynchronously.
pub fn initial_tray_icon(_settings: &UserSettings) -> (Image<'static>, bool) {
    eprintln!("[Tray] Initializing with default icon (non-blocking).");
    let icon =
        Image::from_bytes(include_bytes!("../icons/icon.png")).expect("Failed to load tray icon");
    (icon, false)
}

fn get_icon_bytes(enable_dynamic: bool, is_dark: bool) -> &'static [u8] {
    if enable_dynamic {
        if is_dark {
            include_bytes!("../icons/icon-light.png")
        } else {
            include_bytes!("../icons/icon-dark.png")
        }
    } else {
        include_bytes!("../icons/icon.png")
    }
}

fn apply_icon_to_tray(app: &tauri::AppHandle, icon_bytes: &[u8]) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(icon) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(icon));
            let _ = tray.set_icon_as_template(false);
        }
    }
}

/// Optimized update that takes the settings directly
pub fn update_tray_icon_with_settings(
    app: &tauri::AppHandle,
    is_dark: bool,
    settings: &UserSettings,
) {
    let icon_bytes = get_icon_bytes(settings.enable_dynamic_tray_icon, is_dark);
    apply_icon_to_tray(app, icon_bytes);
}

/// Refresh the tray icon manually (e.g. after settings change).
/// Accepts settings to avoid reloading them.
pub async fn refresh_tray_icon(
    app_handle: &tauri::AppHandle,
    settings: &UserSettings,
) {
    let theme_info = get_system_color_scheme().await;
    update_tray_icon_with_settings(app_handle, theme_info.prefers_dark, settings);
}

/// Start listening for theme changes via D-Bus signals
/// This is more efficient than polling as it reacts to actual system changes
pub async fn start_theme_listener(
    app_handle: tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = app_handle.clone();
    win11_clipboard_core::theme_manager::start_theme_listener(move |scheme| {
        use tauri::Emitter;
        let is_dark = scheme.is_dark();
        let enable_dynamic = DYNAMIC_ICON_ENABLED.load(Ordering::Relaxed);
        let icon_bytes = get_icon_bytes(enable_dynamic, is_dark);
        apply_icon_to_tray(&app, icon_bytes);
        let _ = app.emit(
            "theme-changed",
            serde_json::json!({
                "color_scheme": scheme,
                "prefers_dark": is_dark,
                "source": "xdg-portal-signal"
            }),
        );
    })
    .await
}
