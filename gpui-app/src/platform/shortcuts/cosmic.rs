//! Own COSMIC shortcut registration, always pointing at THIS binary.
//!
//! COSMIC keeps user shortcuts in a RON map at
//! `~/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom`
//! (`{ (modifiers: [Super], key: "v", description: Some("..")): Spawn("cmd"), }`)
//! and cosmic-settings-daemon reloads it when the file changes. Like
//! `gnome`, entries use `<exe> --toggle` and are recognised by their
//! command, so Tauri-owned bindings are never touched (coexistence-safe).

use std::path::{Path, PathBuf};

const CONFIG_REL: &str = ".config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom";

/// (description, modifiers, key)
const BINDINGS: &[(&str, &[&str], &str)] = &[
    ("Clipboard History (GPUI)", &["Super"], "v"),
    ("Clipboard History (GPUI Alt)", &["Ctrl", "Alt"], "v"),
];

pub fn is_cosmic() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_lowercase().contains("cosmic"))
        .unwrap_or(false)
}

fn config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    Ok(PathBuf::from(home).join(CONFIG_REL))
}

fn escape_ron(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn build_entry(description: &str, mods: &[&str], key: &str, command: &str) -> String {
    format!(
        "(modifiers: [{}], key: \"{}\", description: Some(\"{}\")): Spawn(\"{}\")",
        mods.join(", "),
        escape_ron(key),
        escape_ron(description),
        escape_ron(command),
    )
}

/// Split the top-level RON map into its entries (without the trailing comma).
/// String literals are skipped so a command containing `,` `(` `)` cannot
/// confuse the depth tracking. `Err` means the file is not a RON map we can
/// safely rewrite.
fn split_entries(content: &str) -> Result<Vec<String>, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err("unexpected COSMIC shortcuts format (expected a RON map)".to_string());
    }
    let inner = &trimmed[1..trimmed.len() - 1];

    let mut entries = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for ch in inner.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                current.push(ch);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                let entry = current.trim();
                if !entry.is_empty() {
                    entries.push(entry.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if in_string || depth != 0 {
        return Err("unbalanced COSMIC shortcuts file".to_string());
    }
    let tail = current.trim();
    if !tail.is_empty() {
        entries.push(tail.to_string());
    }
    Ok(entries)
}

/// Our entries are recognised by the binary name in the Spawn command, plus
/// the exact launcher path so a renamed AppImage still matches its own entries.
fn is_ours(entry: &str) -> bool {
    entry.contains("win11-clipboard-history-gpui")
        || entry.contains(&crate::app_state::launcher_path())
}

fn render(entries: &[String]) -> String {
    if entries.is_empty() {
        return "{}\n".to_string();
    }
    let body: Vec<String> = entries.iter().map(|e| format!("    {e},")).collect();
    format!("{{\n{}\n}}\n", body.join("\n"))
}

/// Backup once, then write via temp file + rename so cosmic-settings-daemon
/// never observes a half-written file.
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {parent:?}: {e}"))?;
    }
    if path.exists() {
        let bak = path.with_extension("bak");
        if !bak.exists() {
            std::fs::copy(path, &bak).map_err(|e| format!("backup {bak:?}: {e}"))?;
        }
    }
    let tmp = path.with_extension("tmp-gpui");
    std::fs::write(&tmp, content).map_err(|e| format!("write {tmp:?}: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename {tmp:?}: {e}"))
}

fn read_entries(path: &Path) -> Result<Vec<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => split_entries(&content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("read {path:?}: {e}")),
    }
}

fn register_at(path: &Path, command: &str) -> Result<String, String> {
    let mut entries = read_entries(path)?;
    entries.retain(|e| !is_ours(e));
    for (description, mods, key) in BINDINGS {
        entries.push(build_entry(description, mods, key, command));
    }
    write_atomic(path, &render(&entries))?;
    Ok(format!("Registered {} shortcut(s) → {command}", BINDINGS.len()))
}

fn unregister_at(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok("No GPUI shortcut entries found".to_string());
    }
    let entries = read_entries(path)?;
    let remaining: Vec<String> = entries.iter().filter(|e| !is_ours(e)).cloned().collect();
    let removed = entries.len() - remaining.len();
    if removed == 0 {
        return Ok("No GPUI shortcut entries found".to_string());
    }
    write_atomic(path, &render(&remaining))?;
    Ok(format!("Removed {removed} shortcut entr(y/ies)"))
}

/// Register both shortcuts. Idempotent: replaces our own entries, never
/// touches entries owned by anyone else (including the Tauri build).
pub fn register() -> Result<String, String> {
    register_at(&config_path()?, &super::gnome::current_exe_toggle())
}

/// Remove only our own entries.
pub fn unregister() -> Result<String, String> {
    unregister_at(&config_path()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gpui-cosmic-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("custom")
    }

    #[test]
    fn splits_entries_ignoring_punctuation_in_strings() {
        let raw = r#"{
            (modifiers: [Super], key: "a", description: None): Spawn("sh -c 'x, (y)'"),
            (modifiers: [Alt], key: "b", description: Some("d")): Spawn("foo"),
        }"#;
        let entries = split_entries(raw).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].contains("sh -c 'x, (y)'"));
        assert!(entries[1].ends_with("Spawn(\"foo\")"));
    }

    #[test]
    fn rejects_non_map_content() {
        assert!(split_entries("garbage").is_err());
        assert!(split_entries("{ (unclosed ").is_err());
        assert!(split_entries("   ").unwrap().is_empty());
    }

    #[test]
    fn register_is_idempotent_and_preserves_foreign_entries() {
        let path = temp_file("register");
        let foreign = r#"(modifiers: [Super], key: "t", description: None): Spawn("alacritty")"#;
        std::fs::write(&path, format!("{{\n    {foreign},\n}}\n")).unwrap();

        let cmd = "/opt/win11-clipboard-history-gpui --toggle";
        register_at(&path, cmd).unwrap();
        register_at(&path, cmd).unwrap();

        let entries = read_entries(&path).unwrap();
        assert_eq!(entries.len(), 1 + BINDINGS.len());
        assert!(entries.iter().any(|e| e == foreign));
        assert_eq!(entries.iter().filter(|e| is_ours(e)).count(), BINDINGS.len());
        assert!(path.with_extension("bak").exists());
    }

    #[test]
    fn unregister_removes_only_ours() {
        let path = temp_file("unregister");
        let foreign = r#"(modifiers: [Super], key: "t", description: None): Spawn("alacritty")"#;
        std::fs::write(&path, format!("{{\n    {foreign},\n}}\n")).unwrap();
        register_at(&path, "/opt/win11-clipboard-history-gpui --toggle").unwrap();

        unregister_at(&path).unwrap();
        let entries = read_entries(&path).unwrap();
        assert_eq!(entries, vec![foreign.to_string()]);
    }

    #[test]
    fn register_creates_missing_file() {
        let path = temp_file("fresh");
        register_at(&path, "/opt/win11-clipboard-history-gpui --toggle").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("modifiers: [Super], key: \"v\""));
        assert!(content.contains("modifiers: [Ctrl, Alt], key: \"v\""));
    }
}
