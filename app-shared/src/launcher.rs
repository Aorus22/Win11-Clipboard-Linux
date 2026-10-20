//! First-run marker, autostart (systemd user unit or XDG entry) and launcher path
//! — coexistence-safe system shims shared by the frontends. Every file is the
//! frontend's OWN (named after [`crate::identity::AppIdentity`]), never the Tauri
//! build's.

use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
//
// Two mechanisms, preferred first:
// - a systemd user unit with `Restart=on-failure`: the daemon dying mid-session
//   (an AppImage mount torn down under it, an OOM kill, a panic) is recovered,
//   instead of taking clipboard capture — and Super+V, which only *signals* a
//   running instance — down with it until the next login;
// - a plain XDG autostart entry for sessions with no user systemd manager.

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

/// systemd user unit for the resident daemon.
///
/// `Restart=on-failure`, not `always`: `--quit` is a clean exit and must stay
/// exited. `PartOf=graphical-session.target` ties it to the session, so logging
/// out stops it instead of restarting it forever.
pub fn systemd_unit(exe_path: &str) -> String {
    systemd_unit_with(exe_path, launch_prefix())
}

fn systemd_unit_with(exe_path: &str, prefix: &str) -> String {
    let id = crate::identity::get();
    // As an `Environment=` line rather than an `env` prefix in ExecStart:
    // systemd requires an absolute executable path there, so the prefix form
    // would have to be split anyway.
    let environment = if prefix.is_empty() {
        String::new()
    } else {
        format!("Environment={EXTRACT_AND_RUN_VAR}\n")
    };
    format!(
        "[Unit]\n\
         Description={} (resident daemon)\n\
         PartOf=graphical-session.target\n\
         After=graphical-session.target\n\
         # A session whose user manager has no Wayland/X11 in its environment\n\
         # cannot open a window: give up instead of looping on every restart.\n\
         StartLimitIntervalSec=60\n\
         StartLimitBurst=5\n\
         \n\
         [Service]\n\
         Type=simple\n\
         {environment}\
         ExecStart=\"{exe_path}\" --background --no-tray\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=graphical-session.target\n",
        id.display_name
    )
}

fn autostart_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart"))
}

fn autostart_path() -> Option<std::path::PathBuf> {
    autostart_dir().map(|d| d.join(format!("{}.desktop", crate::identity::get().binary_name)))
}

fn unit_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("systemd/user"))
}

fn unit_path() -> Option<std::path::PathBuf> {
    unit_dir().map(|d| d.join(unit_name()))
}

fn unit_name() -> String {
    format!("{}.service", crate::identity::get().binary_name)
}

/// Is a user systemd manager there to hand the daemon to? Probed once per call
/// (cheap, and it keeps the test/CI override meaningful).
///
/// `WIN11_CLIPBOARD_AUTOSTART=desktop|systemd` forces the mechanism, which the
/// live tests need: they exercise the `.desktop` writer without touching the
/// real user manager.
fn systemd_available() -> bool {
    match std::env::var(AUTOSTART_MODE_ENV).as_deref() {
        Ok("desktop") => false,
        Ok("systemd") => true,
        _ => std::process::Command::new("systemctl")
            .args(["--user", "show-environment"])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false),
    }
}

const AUTOSTART_MODE_ENV: &str = "WIN11_CLIPBOARD_AUTOSTART";

fn systemctl(args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|e| format!("systemctl missing: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "systemctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// `$APPIMAGE` when this process really lives inside that mount, else `None`.
///
/// The check matters: a *parent* AppImage (VS Code, a terminal, any
/// AppImage-packaged tool) exports `$APPIMAGE`/`$APPDIR` too, and blindly
/// trusting them would register someone else's bundle in our autostart entries,
/// our shortcuts and our launch prefix.
fn own_appimage(
    appimage: Option<String>,
    appdir: Option<String>,
    exe: Option<PathBuf>,
) -> Option<String> {
    let (appimage, appdir) = (appimage?, appdir?);
    let exe = exe?;
    exe.starts_with(PathBuf::from(&appdir)).then_some(appimage)
}

fn non_empty(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.is_empty())
}

pub fn appimage_path() -> Option<String> {
    own_appimage(non_empty("APPIMAGE"), non_empty("APPDIR"), std::env::current_exe().ok())
}

/// Prefix that keeps an AppImage launch away from its FUSE mount.
///
/// quick-sharun's RunImage runtime mounts the image through FUSE (dwarfs) at a
/// *fixed* path derived from the file. That layer is fragile: the dwarfs block
/// cache (1.5G by default) can fail to allocate under memory pressure, after
/// which dwarfs aborts, the mount dies under the running app (SIGBUS for the
/// app) and every later `<AppImage> --toggle` is refused outright ("refusing
/// reuse") — so Super+V stops working with no visible reason. Extract-and-run
/// skips FUSE: one cached extraction, and it is immune to a dead mount.
pub fn launch_prefix() -> &'static str {
    if appimage_path().is_some() {
        EXTRACT_AND_RUN
    } else {
        ""
    }
}

const EXTRACT_AND_RUN: &str = "/usr/bin/env APPIMAGE_EXTRACT_AND_RUN=1 ";
const EXTRACT_AND_RUN_VAR: &str = "APPIMAGE_EXTRACT_AND_RUN=1";

/// Path used for autostart entries and DE shortcuts.
///
/// Inside an AppImage `current_exe()` points at the transient mount
/// (`/tmp/.mount_*/usr/bin/...`), which is gone after exit, so prefer the
/// `$APPIMAGE` path the runtime exported.
pub fn launcher_path() -> String {
    appimage_path().unwrap_or_else(|| {
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| crate::identity::get().binary_name.to_string())
    })
}

/// [`launcher_path`] plus [`launch_prefix`]: what our own entries and shortcuts
/// run.
pub fn launch_command() -> String {
    format!("{}{}", launch_prefix(), launcher_path())
}

fn write_desktop_entry(exe_path: &str) -> Result<(), String> {    let dir = autostart_dir().ok_or("no config dir")?;
    let path = autostart_path().ok_or("no autostart path")?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create autostart dir: {e}"))?;
    }
    // Tray + `--background` are in place, so the login start stays hidden.
    std::fs::write(&path, autostart_desktop_entry(exe_path))
        .map_err(|e| format!("write autostart entry: {e}"))?;
    // systemd's xdg-autostart generator warns about an executable entry, and the
    // bit means nothing here (the file is data, the AppImage is what runs).
    #[cfg(unix)]
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .map_err(|e| format!("chmod autostart entry: {e}"))?;
    Ok(())
}

fn remove_desktop_entry() -> Result<(), String> {
    if let Some(path) = autostart_path() {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("remove autostart entry: {e}"))?;
        }
    }
    Ok(())
}

fn remove_unit_file() -> Result<(), String> {
    if let Some(path) = unit_path() {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("remove unit file: {e}"))?;
        }
    }
    Ok(())
}

pub fn autostart_enable() -> Result<(), String> {
    // Prefixed so an AppImage start skips FUSE (see `launch_prefix`).
    let exe = launch_command();
    if systemd_available() {
        let path = unit_path().ok_or("no config dir")?;
        if let Some(dir) = path.parent() {
            if !dir.exists() {
                std::fs::create_dir_all(dir).map_err(|e| format!("create unit dir: {e}"))?;
            }
        }
        std::fs::write(&path, systemd_unit(&exe)).map_err(|e| format!("write unit: {e}"))?;        systemctl(&["daemon-reload"])?;
        // Plain `enable`, no `--now`: whoever calls this (the wizard, the
        // Settings toggle) is already a running instance, so starting the unit
        // here would only race it for the instance socket. It takes over at the
        // next login.
        systemctl(&["enable", &unit_name()])?;
        // One mechanism only — two would race for the same socket.
        let _ = remove_desktop_entry();
        return Ok(());
    }
    write_desktop_entry(&exe)
}

pub fn autostart_disable() -> Result<(), String> {
    let had_unit = unit_path().map(|p| p.exists()).unwrap_or(false);
    if had_unit {
        // Best-effort: the file is what we own, and disabling a unit the manager
        // no longer knows about must not block removing it.
        let _ = systemctl(&["disable", &unit_name()]);
    }
    remove_unit_file()?;
    if had_unit {
        let _ = systemctl(&["daemon-reload"]);
    }
    remove_desktop_entry()
}

pub fn autostart_is_enabled() -> bool {
    unit_path().map(|p| p.exists()).unwrap_or(false)
        || autostart_path().map(|p| p.exists()).unwrap_or(false)
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
        assert!(unit_path()
            .expect("path")
            .to_string_lossy()
            .contains("win11-clipboard-history-gpui.service"));
    }

    /// The unit is what recovers a crash mid-session, so the restart policy and
    /// the session binding are the point of it — assert them, not the prose.
    #[test]
    fn systemd_unit_restarts_on_failure_only() {
        let unit = systemd_unit("/tmp/fake-gpui-bin");
        assert!(unit.contains("ExecStart=\"/tmp/fake-gpui-bin\" --background --no-tray"));
        assert!(unit.contains("Restart=on-failure"));
        // A clean `--quit` must stay exited, so no `Restart=always`.
        assert!(!unit.contains("Restart=always"));
        // Logging out stops the daemon instead of restarting it forever.
        assert!(unit.contains("PartOf=graphical-session.target"));
        assert!(unit.contains("WantedBy=graphical-session.target"));
        assert!(!unit.contains("win11-clipboard-history-bin"));
    }

    /// An AppImage start must skip the FUSE mount (the layer that goes away
    /// under the running daemon), and a non-AppImage install must not be given
    /// the variable at all.
    #[test]
    fn appimage_launches_skip_the_fuse_mount() {
        let appimage_unit = systemd_unit_with("/tmp/app.AppImage", EXTRACT_AND_RUN);
        assert!(appimage_unit.contains("Environment=APPIMAGE_EXTRACT_AND_RUN=1"));
        // ExecStart still gets an absolute, quotable executable path.
        assert!(appimage_unit.contains("ExecStart=\"/tmp/app.AppImage\" --background --no-tray"));

        let plain_unit = systemd_unit_with("/usr/bin/win11-clipboard-history-gpui", "");
        assert!(!plain_unit.contains("APPIMAGE_EXTRACT_AND_RUN"));

        assert!(autostart_desktop_entry("/usr/bin/env APPIMAGE_EXTRACT_AND_RUN=1 /tmp/a.AppImage")
            .contains("Exec=/usr/bin/env APPIMAGE_EXTRACT_AND_RUN=1 /tmp/a.AppImage --background"));
    }

    /// `$APPIMAGE` is only ours when this process really lives inside that mount:
    /// a parent AppImage exports the same variables.
    #[test]
    fn appimage_path_ignores_a_parent_appimage() {
        let mount = "/tmp/.mount_own/usr/bin/win11-clipboard-history-gpui";
        let own = own_appimage(
            Some("/home/u/own.AppImage".into()),
            Some("/tmp/.mount_own".into()),
            Some(PathBuf::from(mount)),
        );
        assert_eq!(own.as_deref(), Some("/home/u/own.AppImage"));

        // Running from a parent AppImage's mount (VS Code, a terminal).
        let foreign = own_appimage(
            Some("/home/u/vscode.AppImage".into()),
            Some("/tmp/.mount_vscode".into()),
            Some(PathBuf::from("/usr/bin/win11-clipboard-history-gpui")),
        );
        assert_eq!(foreign, None);

        // Not an AppImage at all.
        assert_eq!(own_appimage(None, None, Some(PathBuf::from(mount))), None);
        assert_eq!(
            own_appimage(Some("/home/u/own.AppImage".into()), None, Some(PathBuf::from(mount))),
            None
        );
    }

    /// Live FS cycle against an isolated XDG_CONFIG_HOME (never the real one):
    /// enable → entry exists with our exe → disable → gone.
    #[test]
    fn autostart_live_realfs_isolated() {
        let _serial = crate::test_util::serial_lock();
        let tmp = std::env::temp_dir().join(format!("gpui-xdg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let prior = std::env::var("XDG_CONFIG_HOME").ok();
        let prior_mode = std::env::var(AUTOSTART_MODE_ENV).ok();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };
        // Force the XDG path: this test owns the .desktop writer, and must never
        // touch the developer's real user manager.
        unsafe { std::env::set_var(AUTOSTART_MODE_ENV, "desktop") };

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
            // systemd's xdg-autostart generator warns about an executable entry.
            #[cfg(unix)]
            {
                let mode = std::fs::metadata(&path)
                    .map_err(|e| format!("stat: {e}"))?
                    .permissions()
                    .mode()
                    & 0o777;
                assert_eq!(mode, 0o644, "autostart entry must not be executable");
            }
            autostart_disable()?;
            assert!(!autostart_is_enabled());
            Ok(())
        })();

        match prior {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        match prior_mode {
            Some(v) => unsafe { std::env::set_var(AUTOSTART_MODE_ENV, v) },
            None => unsafe { std::env::remove_var(AUTOSTART_MODE_ENV) },
        }
        let _ = std::fs::remove_dir_all(&tmp);
        result.expect("autostart live cycle");
    }
}
