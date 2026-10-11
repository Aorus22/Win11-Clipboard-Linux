//! Shared app state + coexistence-safe system shims.
//!
//! - `SharedConfig`: settings + version counter shared between the popup and the
//!   settings window (live-apply without an event bus: the popup re-reads on bump).
//! First-run marker, autostart and launcher path live in `app_shared::launcher`.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::backend::BackendService;
use crate::settings::{self, AppSettings};
use crate::theme::ResolvedTheme;

pub struct SharedConfig {
    pub settings: AppSettings,
    /// Last resolved theme. `theme::resolve` installs the palette globally;
    /// caching the flags here keeps render paths away from GTK entirely.
    pub theme: ResolvedTheme,
    pub version: u64,
    /// Set by the settings window Reset action; the main poll loop opens the
    /// wizard window and clears it (parity: `show-setup-wizard` event).
    pub open_wizard_requested: bool,
    /// Set by the wizard on completion when no popup is open; the main poll
    /// loop opens the popup window and clears it.
    pub open_popup_requested: bool,
    /// Set by the popup view instead of destroying its window (the window is
    /// persistent and parked offscreen); the main poll loop hides it and
    /// clears the flag.
    pub hide_popup_requested: bool,
    /// Mirrors the main loop's popup visibility so background threads (the
    /// evdev outside-click watcher) can gate their work on it.
    pub popup_visible: bool,
    /// Monotonic count of the pointer presses the popup window itself handled,
    /// plus the time of the last one.
    ///
    /// The evdev watcher must never ask X where the pointer is: on a Wayland
    /// session the X pointer is frozen while the real pointer sits over a
    /// Wayland-native window, so it keeps pointing at the last X surface it
    /// was on — the popup — and every outside click would look like an inside
    /// one. Whether a press landed inside is therefore answered by whether the
    /// popup received that press (see `click_watch`).
    pub popup_pointer_down_seq: u64,
    pub popup_last_pointer_down: Option<std::time::Instant>,
}

pub type Shared = Arc<Mutex<SharedConfig>>;

/// Lets the shared outside-click watchers read and dismiss the popup state.
pub struct SharedHooks(pub Shared);

impl app_shared::platform::PopupHooks for SharedHooks {
    fn popup_visible(&self) -> bool {
        self.0.lock().popup_visible
    }

    fn pointer_down_seq(&self) -> u64 {
        self.0.lock().popup_pointer_down_seq
    }

    fn last_pointer_down(&self) -> Option<std::time::Instant> {
        self.0.lock().popup_last_pointer_down
    }

    fn request_hide(&self) {
        self.0.lock().hide_popup_requested = true;
    }
}

pub fn shared(settings: AppSettings, theme: ResolvedTheme) -> Shared {
    Arc::new(Mutex::new(SharedConfig {
        settings,
        theme,
        version: 1,
        open_wizard_requested: false,
        open_popup_requested: false,
        hide_popup_requested: false,
        popup_visible: false,
        popup_pointer_down_seq: 0,
        popup_last_pointer_down: None,
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

pub use app_shared::launcher::*;

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
        let _serial = app_shared::test_util::serial_lock();
        let backup = backup_settings();
        let backend = BackendService::new(&AppSettings::default());
        let shared = shared(AppSettings::default(), ResolvedTheme::win11(false));
        assert_eq!(shared.lock().version, 1);
        shared.lock().settings.dark_background_opacity = 0.42;
        save_now(&shared, &backend).expect("save");
        assert_eq!(shared.lock().version, 2);
        let reloaded = settings::load();
        assert!((reloaded.dark_background_opacity - 0.42).abs() < 1e-6);
        restore_settings(backup);
    }
}
