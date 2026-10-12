//! First-run marker, autostart entry and launcher path — coexistence-safe system
//! shims shared by the frontends. Every file is the frontend's OWN (named after
//! [`crate::identity::AppIdentity`]), never the Tauri build's.

use std::path::PathBuf;

// --- First run (own marker) ---

const FIRST_RUN_FILE: &str = "first_run_done";

fn first_run_path() -> std::path::PathBuf {
    crate::settings::config_dir().join(FIRST_RUN_FILE)
}

pub fn is_first_run() -> bool {
    !first_run_path().exists()
}

pub fn mark_first_run_complete() -> Result<(), String> {
    let dir = crate::settings::config_dir();
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

/// `--background --no-tray` keeps the login start quiet: no popup window or
/// status icon is mapped until the user toggles it.
pub fn autostart_desktop_entry(exe_path: &str) -> String {
    let id = crate::identity::get();
    format!(
        "[Desktop Entry]\nType=Application\nName={}\n\
         Comment=Windows 11-style clipboard history daemon\n\
         Exec={exe_path} --background --no-tray\nIcon={}\n\
         Terminal=false\nHidden=false\nNoDisplay=false\n\
         X-GNOME-Autostart-enabled=true\nX-GNOME-Autostart-Delay=5\nCategories=Utility;\n",
        id.display_name, id.binary_name
    )
}

fn autostart_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart"))
}

fn autostart_path() -> Option<std::path::PathBuf> {
    autostart_dir().map(|d| d.join(format!("{}.desktop", crate::identity::get().binary_name)))
}

/// Path used for autostart entries and DE shortcuts.
///
/// Inside an AppImage `current_exe()` points at the transient mount
/// (`/tmp/.mount_*/usr/bin/...`), which is gone after exit, so prefer the
/// `$APPIMAGE` path the runtime exported.
///
/// That only holds when this process really lives inside that mount: a *parent*
/// AppImage (VS Code, a terminal, any AppImage-packaged tool) exports the same
/// variables, and blindly trusting them would register someone else's bundle in
/// our autostart/shortcut entries.
pub fn launcher_path() -> String {
    if let (Some(appimage), Some(appdir)) = (
        std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()),
        std::env::var_os("APPDIR").filter(|p| !p.is_empty()),
    ) {
        let inside_own_mount = std::env::current_exe()
            .map(|exe| exe.starts_with(PathBuf::from(&appdir)))
            .unwrap_or(false);
        if inside_own_mount {
            return PathBuf::from(appimage).display().to_string();
        }
    }
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| crate::identity::get().binary_name.to_string())
}

pub fn autostart_enable() -> Result<(), String> {
    let dir = autostart_dir().ok_or("no config dir")?;
    let path = autostart_path().ok_or("no autostart path")?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create autostart dir: {e}"))?;
    }
    // Tray + `--background` are in place, so the login start stays hidden.
    std::fs::write(&path, autostart_desktop_entry(&launcher_path()))
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
