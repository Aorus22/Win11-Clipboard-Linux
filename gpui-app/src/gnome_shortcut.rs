//! Own GNOME shortcut registration, always pointing at THIS binary.
//!
//! The backend's `register_global_shortcut()` prefers the installed Tauri
//! wrapper and must NOT be reused (it would point Super+V at the Tauri app).
//! These entries use `<exe> --toggle` and are matched by command substring, so
//! Tauri-owned bindings are never touched (coexistence-safe).

use std::process::Command;

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM_FMT: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";

const ENTRY_PREFIX: &str = "win11-clipboard-history-gpui";

const BINDINGS: &[(&str, &str, &str)] = &[
    ("Clipboard History (GPUI)", "clipboard", "<Super>v"),
    ("Clipboard History (GPUI Alt)", "clipboard-alt", "<Ctrl><Alt>v"),
];

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

fn current_exe_toggle() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "win11-clipboard-history-gpui".to_string());
    format!("{exe} --toggle")
}

pub fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_lowercase().contains("gnome"))
        .unwrap_or(false)
}

/// Parse a gsettings `@as [...]` list of quoted paths into Vec<String>.
fn parse_path_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in raw.chars() {
        match ch {
            '\'' => {
                if in_quotes {
                    if !current.is_empty() {
                        out.push(current.clone());
                    }
                    current.clear();
                }
                in_quotes = !in_quotes;
            }
            _ if in_quotes => current.push(ch),
            _ => {}
        }
    }
    out
}

fn custom_list() -> Result<Vec<String>, String> {
    Ok(parse_path_list(&gsettings(&[MEDIA_KEYS, "custom-keybindings"])?))
}

fn set_custom_list(paths: &[String]) -> Result<(), String> {
    let joined = format!(
        "[{}]",
        paths
            .iter()
            .map(|p| format!("'{p}'"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    gsettings(&["set", MEDIA_KEYS, "custom-keybindings", &joined])?;
    Ok(())
}

fn entry_get(path: &str, key: &str) -> String {
    gsettings(&["get", &format!("{CUSTOM_FMT}:{path}"), key]).unwrap_or_default()
}

fn entry_set(path: &str, key: &str, value: &str) -> Result<(), String> {
    gsettings(&["set", &format!("{CUSTOM_FMT}:{path}"), key, value])?;
    Ok(())
}

fn is_ours(path: &str) -> bool {
    entry_get(path, "command").contains("win11-clipboard-history-gpui")
}

/// Register both shortcuts. Idempotent: reuses our own entries, never touches
/// entries owned by anyone else (including the Tauri build).
pub fn register() -> Result<String, String> {
    let command = current_exe_toggle();
    let mut paths = custom_list()?;
    let mut done = 0;
    for (name, slug, binding) in BINDINGS {
        // Reuse an existing own entry with the same binding.
        let mut target: Option<String> = None;
        for path in &paths {
            if is_ours(path) && entry_get(path, "binding").contains(binding) {
                target = Some(path.clone());
                break;
            }
        }
        let path = match target {
            Some(p) => p,
            None => {
                let fresh = alloc_path(&paths, slug);
                paths.push(fresh.clone());
                fresh
            }
        };
        entry_set(&path, "name", name)?;
        entry_set(&path, "command", &command)?;
        entry_set(&path, "binding", binding)?;
        done += 1;
    }
    set_custom_list(&paths)?;
    Ok(format!("Registered {done} shortcut(s) → {command}"))
}

/// Remove only our own entries (matched by command substring).
pub fn unregister() -> Result<String, String> {
    let paths = custom_list()?;
    let ours: Vec<String> = paths.iter().filter(|p| is_ours(p)).cloned().collect();
    if ours.is_empty() {
        return Ok("No GPUI shortcut entries found".to_string());
    }
    let remaining: Vec<String> = paths.into_iter().filter(|p| !is_ours(p)).collect();
    set_custom_list(&remaining)?;
    Ok(format!("Removed {} shortcut entr(y/ies)", ours.len()))
}

fn alloc_path(paths: &[String], slug: &str) -> String {
    for i in 0..100 {
        let candidate = format!("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/{ENTRY_PREFIX}-{slug}-{i}/");
        if !paths.contains(&candidate) {
            return candidate;
        }
    }
    format!("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/{ENTRY_PREFIX}-{slug}-x/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_list_parsing() {
        let raw = "['/org/a/custom0/', '/org/b/custom1/']";
        assert_eq!(
            parse_path_list(raw),
            vec!["/org/a/custom0/".to_string(), "/org/b/custom1/".to_string()]
        );
        assert!(parse_path_list("@as []").is_empty());
    }

    #[test]
    fn alloc_avoids_collisions() {
        let used = vec!["/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/win11-clipboard-history-gpui-clipboard-0/".to_string()];
        let fresh = alloc_path(&used, "clipboard");
        assert!(fresh.ends_with("clipboard-1/"));
        assert!(!used.contains(&fresh));
    }

    #[test]
    fn toggle_command_points_at_current_exe() {
        let cmd = current_exe_toggle();
        assert!(cmd.ends_with(" --toggle"));
        assert!(!cmd.contains("win11-clipboard-history-bin"));
    }
}
