//! Shared app state + coexistence-safe system shims.
//!
//! - `SharedConfig`: settings + version counter shared between the popup and the
//!   settings window (live-apply without an event bus: the popup re-reads on bump).
//! - First-run marker: OWN file (`first_run_done`) — never the Tauri `setup.json`.
//! - Autostart: OWN desktop entry (`win11-clipboard-history-gpui.desktop`) with the
//!   GPUI binary path — the backend's entry hardcodes Tauri paths and must NOT be reused.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::backend::BackendService;
use crate::settings::{self, AppSettings};

pub struct SharedConfig {
    pub settings: AppSettings,
    pub version: u64,
    /// Set by the settings window Reset action; the main poll loop opens the
    /// wizard window and clears it (parity: `show-setup-wizard` event).
    pub open_wizard_requested: bool,
    /// Set by the wizard on completion when no popup is open; the main poll
    /// loop opens the popup window and clears it.
    pub open_popup_requested: bool,
}

pub type Shared = Arc<Mutex<SharedConfig>>;

pub fn shared(settings: AppSettings) -> Shared {
    Arc::new(Mutex::new(SharedConfig {
        settings,
        version: 1,
        open_wizard_requested: false,
        open_popup_requested: false,
    }))
}

/// Persist settings + sync the backend + bump (parity with `set_user_settings`).
pub fn save_now(shared: &Shared, backend: &BackendService) -> Result<(), String> {
    let mut guard = shared.lock();
    settings::save(&guard.settings)?;
    backend.set_max_history_size(guard.settings.max_history_size);
    guard.version += 1;
    Ok(())
}

// --- First run (own marker) ---

const FIRST_RUN_FILE: &str = "first_run_done";

fn first_run_path() -> std::path::PathBuf {
    settings::config_dir().join(FIRST_RUN_FILE)
}

pub fn is_first_run() -> bool {
    !first_run_path().exists()
}

pub fn mark_first_run_complete() -> Result<(), String> {
    let dir = settings::config_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create config dir: {e}"))?;
    }
    std::fs::write(first_run_path(), "done\n").map_err(|e| format!("write marker: {e}"))?;
    Ok(())
}

#[allow(dead_code)]
pub fn reset_first_run() -> Result<(), String> {
    let path = first_run_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("remove marker: {e}"))?;
    }
    Ok(())
}

// --- Autostart (own entry) ---

const AUTOSTART_FILE: &str = "win11-clipboard-history-gpui.desktop";

pub fn autostart_desktop_entry(exe_path: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Win11 Clipboard History (GPUI)\n\
         Exec={exe_path}\nHidden=false\nNoDisplay=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

fn autostart_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart"))
}

fn autostart_path() -> Option<std::path::PathBuf> {
    autostart_dir().map(|d| d.join(AUTOSTART_FILE))
}

fn current_exe_string() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "win11-clipboard-history-gpui".to_string())
}

pub fn autostart_enable() -> Result<(), String> {
    let dir = autostart_dir().ok_or("no config dir")?;
    let path = autostart_path().ok_or("no autostart path")?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create autostart dir: {e}"))?;
    }
    // NOTE (Phase 5): hidden-at-login start lands with tray integration; until then
    // the entry launches the popup visibly — same as a manual launch.
    std::fs::write(&path, autostart_desktop_entry(&current_exe_string()))
        .map_err(|e| format!("write autostart entry: {e}"))?;
    Ok(())
}

pub fn autostart_disable() -> Result<(), String> {
    if let Some(path) = autostart_path() {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("remove autostart entry: {e}"))?;
        }
    }
    Ok(())
}

pub fn autostart_is_enabled() -> bool {
    autostart_path().map(|p| p.exists()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backup_settings() -> Option<String> {
        let path = settings::config_dir().join("user_settings.json");
        std::fs::read_to_string(&path).ok()
    }

    fn restore_settings(backup: Option<String>) {
        let path = settings::config_dir().join("user_settings.json");
        match backup {
            Some(content) => {
                let _ = std::fs::write(&path, content);
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    #[test]
    fn save_bumps_version_and_persists() {
        let _serial = crate::test_util::serial_lock();
        let backup = backup_settings();
        let backend = BackendService::new(&AppSettings::default());
        let shared = shared(AppSettings::default());
        assert_eq!(shared.lock().version, 1);
        shared.lock().settings.dark_background_opacity = 0.42;
        save_now(&shared, &backend).expect("save");
        assert_eq!(shared.lock().version, 2);
        let reloaded = settings::load();
        assert!((reloaded.dark_background_opacity - 0.42).abs() < 1e-6);
        restore_settings(backup);
    }

    #[test]
    fn first_run_marker_lifecycle() {
        let _serial = crate::test_util::serial_lock();
        let path = first_run_path();
        let existed = path.exists();
        let _ = std::fs::remove_file(&path);
        assert!(is_first_run());
        mark_first_run_complete().expect("mark");
        assert!(!is_first_run());
        reset_first_run().expect("reset");
        assert!(is_first_run());
        if existed {
            mark_first_run_complete().expect("restore");
        }
    }

    #[test]
    fn autostart_entry_names_gpui_binary() {
        let entry = autostart_desktop_entry("/tmp/fake-gpui-bin");
        assert!(entry.contains("/tmp/fake-gpui-bin"));
        assert!(!entry.contains("win11-clipboard-history-bin"));
        assert!(!entry.contains("/usr/bin/win11-clipboard-history\""));
    }

    #[test]
    fn own_paths_never_touch_tauri_locations() {
        assert!(first_run_path()
            .to_string_lossy()
            .contains("win11-clipboard-history-gpui"));
        assert!(autostart_path()
            .expect("path")
            .to_string_lossy()
            .contains("win11-clipboard-history-gpui.desktop"));
    }

    /// Live FS cycle against an isolated XDG_CONFIG_HOME (never the real one):
    /// enable → entry exists with our exe → disable → gone.
    #[test]
    fn autostart_live_realfs_isolated() {
        let _serial = crate::test_util::serial_lock();
        let tmp = std::env::temp_dir().join(format!("gpui-xdg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let prior = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

        let result = (|| -> Result<(), String> {
            assert!(!autostart_is_enabled());
            autostart_enable()?;
            assert!(autostart_is_enabled());
            let path = autostart_path().ok_or("no path")?;
            let content =
                std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
            // In tests current_exe is the harness binary; assert it round-trips
            // into Exec= and that no Tauri paths leak in.
            let exe = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            assert!(content.contains(&format!("Exec={exe}")), "Exec points at this binary");
            assert!(!content.contains("win11-clipboard-history-bin"));
            assert!(!content.contains("/usr/bin/win11-clipboard-history\""));
            autostart_disable()?;
            assert!(!autostart_is_enabled());
            Ok(())
        })();

        match prior {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        let _ = std::fs::remove_dir_all(&tmp);
        result.expect("autostart live cycle");
    }
}
