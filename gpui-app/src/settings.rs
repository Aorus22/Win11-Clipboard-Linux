//! App settings for the GPUI frontend.
//!
//! Same schema and defaults as the Tauri app's `user_settings.json`
//! (see `src-tauri/src/user_settings.rs`), but stored in a SEPARATE config
//! directory so both builds coexist without fighting (SYS-06):
//! `~/.config/win11-clipboard-history-gpui/user_settings.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const CONFIG_DIR_NAME: &str = "win11-clipboard-history-gpui";
const SETTINGS_FILE: &str = "user_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomKaomoji {
    pub text: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

fn default_category() -> String {
    "Custom".to_string()
}

/// Mirrors `UserSettings` from the backend (stringly theme to stay file-compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    #[serde(default = "default_opacity")]
    pub dark_background_opacity: f32,
    #[serde(default = "default_opacity")]
    pub light_background_opacity: f32,
    #[serde(default = "default_true")]
    pub enable_dynamic_tray_icon: bool,
    #[serde(default = "default_true")]
    pub enable_smart_actions: bool,
    #[serde(default = "default_true")]
    pub enable_ui_polish: bool,
    #[serde(default = "default_max_history")]
    pub max_history_size: usize,
    #[serde(default)]
    pub auto_delete_interval: u64,
    #[serde(default = "default_unit")]
    pub auto_delete_unit: String,
    #[serde(default)]
    pub custom_kaomojis: Vec<CustomKaomoji>,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
}

fn default_theme_mode() -> String {
    "system".to_string()
}
fn default_opacity() -> f32 {
    0.70
}
fn default_true() -> bool {
    true
}
fn default_max_history() -> usize {
    50
}
fn default_unit() -> String {
    "hours".to_string()
}
fn default_ui_scale() -> f32 {
    1.0
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: default_theme_mode(),
            dark_background_opacity: default_opacity(),
            light_background_opacity: default_opacity(),
            enable_dynamic_tray_icon: true,
            enable_smart_actions: true,
            enable_ui_polish: true,
            max_history_size: default_max_history(),
            auto_delete_interval: 0,
            auto_delete_unit: default_unit(),
            custom_kaomojis: Vec::new(),
            ui_scale: default_ui_scale(),
        }
    }
}

impl AppSettings {
    pub fn validate(&mut self) {
        self.dark_background_opacity = self.dark_background_opacity.clamp(0.0, 1.0);
        self.light_background_opacity = self.light_background_opacity.clamp(0.0, 1.0);
        if !["system", "dark", "light"].contains(&self.theme_mode.as_str()) {
            self.theme_mode = "system".to_string();
        }
        self.max_history_size = self.max_history_size.clamp(1, 100_000);
        self.ui_scale = self.ui_scale.clamp(0.5, 2.0);
        if !["minutes", "hours", "days", "weeks"].contains(&self.auto_delete_unit.as_str()) {
            self.auto_delete_unit = "hours".to_string();
        }
    }

    /// Secondary opacity = base + 0.3 clamped (port of themeUtils.ts).
    pub fn secondary_opacity(&self, is_dark: bool) -> f32 {
        let base = if is_dark {
            self.dark_background_opacity
        } else {
            self.light_background_opacity
        };
        (base + 0.3).min(1.0)
    }

    /// Tertiary opacity = secondary + 0.3 clamped (port of themeUtils.ts).
    pub fn tertiary_opacity(&self, is_dark: bool) -> f32 {
        (self.secondary_opacity(is_dark) + 0.3).min(1.0)
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR_NAME)
}

/// GNOME `color-scheme` probe for `theme_mode == "system"`.
/// The full XDG portal listener lands in Phase 5 (SYS-04); this covers day-one parity.
pub fn system_prefers_dark() -> bool {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args([
            "get",
            "org.gnome.desktop.interface",
            "color-scheme",
        ])
        .output()
    {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout);
            return value.contains("prefer-dark");
        }
    }
    // Unknown desktop: match Tauri fallback behavior (light) — documented delta if it bites.
    false
}

pub fn resolve_dark(settings: &AppSettings) -> bool {
    match settings.theme_mode.as_str() {
        "dark" => true,
        "light" => false,
        _ => system_prefers_dark(),
    }
}

pub fn load() -> AppSettings {
    let path = config_dir().join(SETTINGS_FILE);
    if !path.exists() {
        return AppSettings::default();
    }
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<AppSettings>(&content) {
            Ok(mut settings) => {
                settings.validate();
                settings
            }
            Err(e) => {
                eprintln!("[gpui settings] parse failed ({e}); using defaults");
                AppSettings::default()
            }
        },
        Err(e) => {
            eprintln!("[gpui settings] read failed ({e}); using defaults");
            AppSettings::default()
        }
    }
}

#[allow(dead_code)]
pub fn save(settings: &AppSettings) -> Result<(), String> {
    let dir = config_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("create config dir: {e}"))?;
    }
    let mut validated = settings.clone();
    validated.validate();
    let content =
        serde_json::to_string_pretty(&validated).map_err(|e| format!("serialize: {e}"))?;
    fs::write(dir.join(SETTINGS_FILE), content).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_tauri_app() {
        let s = AppSettings::default();
        assert_eq!(s.theme_mode, "system");
        assert!((s.dark_background_opacity - 0.70).abs() < f32::EPSILON);
        assert!((s.light_background_opacity - 0.70).abs() < f32::EPSILON);
        assert!((s.ui_scale - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.max_history_size, 50);
    }

    #[test]
    fn opacity_math_matches_theme_utils() {
        let s = AppSettings::default();
        // base 0.70 → secondary 1.0 (0.70+0.30), tertiary 1.0
        assert!((s.secondary_opacity(true) - 1.0).abs() < f32::EPSILON);
        assert!((s.tertiary_opacity(true) - 1.0).abs() < f32::EPSILON);
        let mut low = AppSettings::default();
        low.dark_background_opacity = 0.2;
        assert!((low.secondary_opacity(true) - 0.5).abs() < 1e-6);
        assert!((low.tertiary_opacity(true) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn validate_clamps() {
        let mut s = AppSettings {
            theme_mode: "neon".to_string(),
            dark_background_opacity: 2.0,
            ui_scale: 9.0,
            ..Default::default()
        };
        s.validate();
        assert_eq!(s.theme_mode, "system");
        assert!((s.dark_background_opacity - 1.0).abs() < f32::EPSILON);
        assert!((s.ui_scale - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn config_dir_is_separate_from_tauri() {
        assert!(config_dir().ends_with(CONFIG_DIR_NAME));
        assert_ne!(CONFIG_DIR_NAME, "win11-clipboard-history");
    }
}
