//! Installing the GNOME Shell extension that places and dismisses the popup.
//!
//! The extension is embedded in the binary, so an AppImage or a bare `cargo
//! install` carries it. Installing writes it to the user's extension directory
//! and adds it to `org.gnome.shell enabled-extensions`.
//!
//! On Wayland, GNOME Shell cannot be restarted and only scans extensions at
//! login, so a freshly installed extension is active after the next log-in.
//! Until then `platform::select()` keeps the `x11` backend.

use std::path::PathBuf;
use std::process::Command;

use super::client;

pub const UUID: &str = "clipboard-history-gpui@win11-clipboard.dev";
const METADATA: &str = include_str!("extension/metadata.json");
const EXTENSION_JS: &str = include_str!("extension/extension.js");

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Installed and already running in this session.
    Active,
    /// Installed and enabled; takes effect after the next log-in.
    NeedsRelogin,
}

/// One-line, user-facing result of [`install`].
pub fn describe(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Active => "GNOME extension installed and active.",
        Outcome::NeedsRelogin => {
            "GNOME extension installed. Log out and back in to activate it \
             (GNOME Shell on Wayland only loads new extensions at login)."
        }
    }
}

pub fn extension_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|d| d.join("gnome-shell/extensions").join(UUID))
        .ok_or_else(|| "no XDG data dir".to_string())
}

/// Whether the installed files match the embedded ones.
pub fn is_installed() -> bool {
    let Ok(dir) = extension_dir() else {
        return false;
    };
    std::fs::read_to_string(dir.join("extension.js")).is_ok_and(|s| s == EXTENSION_JS)
        && std::fs::read_to_string(dir.join("metadata.json")).is_ok_and(|s| s == METADATA)
}

/// Whether the extension is listed in `enabled-extensions` (the shell loads
/// it at startup, so it is about to appear on the bus).
pub fn is_enabled() -> bool {
    gsettings(&["get", "org.gnome.shell", "enabled-extensions"])
        .map(|raw| {
            super::super::shortcuts::gnome::parse_path_list(&raw)
                .iter()
                .any(|u| u == UUID)
        })
        .unwrap_or(false)
}

/// Copy the extension into place and enable it.
pub fn install() -> Result<Outcome, String> {
    let dir = extension_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    for (name, content) in [("metadata.json", METADATA), ("extension.js", EXTENSION_JS)] {
        std::fs::write(dir.join(name), content)
            .map_err(|e| format!("write {name}: {e}"))?;
    }
    enable()?;
    Ok(if client::extension_ready() {
        Outcome::Active
    } else {
        Outcome::NeedsRelogin
    })
}

/// Add the extension to `enabled-extensions` (what `gnome-extensions enable`
/// does, minus the requirement that the running shell already knows it).
fn enable() -> Result<(), String> {
    let raw = gsettings(&["get", "org.gnome.shell", "enabled-extensions"])?;
    let mut list = super::super::shortcuts::gnome::parse_path_list(&raw);
    if !list.iter().any(|u| u == UUID) {
        list.push(UUID.to_string());
        let value = format!(
            "[{}]",
            list.iter()
                .map(|u| format!("'{u}'"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        gsettings(&["set", "org.gnome.shell", "enabled-extensions", &value])?;
    }
    // Live-enable when the running shell already knows the extension (an
    // update of one installed earlier); a no-op error otherwise.
    let _ = Command::new("gnome-extensions").args(["enable", UUID]).output();
    Ok(())
}

fn gsettings(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| format!("gsettings missing: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "gsettings failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_metadata_matches_the_installer() {
        let meta: serde_json::Value = serde_json::from_str(METADATA).expect("valid metadata.json");
        assert_eq!(meta["uuid"], UUID);
        assert!(meta["shell-version"].as_array().is_some_and(|v| !v.is_empty()));
    }

    #[test]
    fn extension_api_version_is_in_sync_with_the_client() {
        let needle = format!("get Version() {{ return {}; }}", client::API_VERSION);
        assert!(EXTENSION_JS.contains(&needle), "bump extension.js and client::API_VERSION together");
    }

    #[test]
    fn extension_uses_the_clients_bus_names() {
        for name in [client::BUS_NAME, client::OBJECT_PATH] {
            assert!(EXTENSION_JS.contains(name), "{name} missing from extension.js");
        }
    }

    #[test]
    fn enabled_extensions_list_round_trips() {
        let list = super::super::super::shortcuts::gnome::parse_path_list(
            "['a@b', 'clipboard-history-gpui@win11-clipboard.dev']",
        );
        assert_eq!(list, ["a@b", UUID]);
        assert!(super::super::super::shortcuts::gnome::parse_path_list("@as []").is_empty());
    }
}
