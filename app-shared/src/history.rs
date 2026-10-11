//! History store logic — port of `ClipboardTab.tsx` filtering/sections,
//! `_HistoryItemContent.tsx` timestamp, and `smartActionService.ts` detection.
//!
//! Pure functions over the reused backend types so behavior matches the React
//! build by construction. Covered by unit tests below.

pub use win11_clipboard_history_lib::{ClipboardContent, ClipboardItem};
use chrono::{DateTime, Utc};
use regex::Regex;
use std::sync::OnceLock;

pub fn searchable_text(item: &ClipboardItem) -> Option<&str> {
    match &item.content {
        ClipboardContent::Text(text) => Some(text),
        ClipboardContent::RichText { plain, .. } => Some(plain),
        ClipboardContent::Image { .. } => None,
    }
}

/// Mirrors the `filteredHistory` memo: images never match; invalid regex matches nothing.
pub fn matches_query(item: &ClipboardItem, query: &str, regex_mode: bool) -> bool {
    if query.is_empty() {
        return true;
    }
    let Some(text) = searchable_text(item) else {
        return false;
    };
    if regex_mode {
        match Regex::new(&format!("(?i){query}")) {
            Ok(re) => re.is_match(text),
            Err(_) => false,
        }
    } else {
        text.to_lowercase().contains(&query.to_lowercase())
    }
}

pub fn filter_history<'a>(
    history: &'a [ClipboardItem],
    query: &str,
    regex_mode: bool,
) -> Vec<&'a ClipboardItem> {
    history
        .iter()
        .filter(|item| matches_query(item, query, regex_mode))
        .collect()
}

/// Split into (pinned, unpinned), preserving order — mirrors the React memos.
pub fn partition_pinned(history: &[ClipboardItem]) -> (Vec<&ClipboardItem>, Vec<&ClipboardItem>) {
    let mut pinned = Vec::new();
    let mut unpinned = Vec::new();
    for item in history {
        if item.pinned {
            pinned.push(item);
        } else {
            unpinned.push(item);
        }
    }
    (pinned, unpinned)
}

/// Relative timestamp — port of `formatTime` in `_HistoryItemContent.tsx`.
pub fn relative_time(timestamp: &DateTime<Utc>, now: &DateTime<Utc>) -> String {
    let diff_secs = now.signed_duration_since(timestamp).num_seconds().max(0);
    let diff_mins = diff_secs / 60;
    let diff_hours = diff_secs / 3600;
    if diff_mins < 1 {
        "Just now".to_string()
    } else if diff_mins < 60 {
        format!("{diff_mins}m ago")
    } else if diff_hours < 24 {
        format!("{diff_hours}h ago")
    } else {
        // `toLocaleDateString()` (en-US) → M/D/YYYY.
        timestamp.format("%-m/%-d/%Y").to_string()
    }
}

// --- Smart actions (port of smartActionService.ts) ---

#[derive(Debug, Clone, PartialEq)]
pub struct SmartAction {
    pub id: &'static str,
    pub label: &'static str,
    pub data: String,
}

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // NOTE: the TS original starts with a `(?!mailto:)` lookahead, which the
    // `regex` crate cannot express — enforced in code instead (see below).
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^(?:(?:http|https|ftp)://)(?:\S+(?::\S*)?@)?(?:(?:(?:[1-9]\d?|1\d\d|2[01]\d|22[0-3])(?:\.(?:1?\d{1,2}|2[0-4]\d|25[0-5])){2}(?:\.(?:[0-9]\d?|1\d\d|2[0-4]\d|25[0-4]))|(?:(?:[a-z¡-퟿0-9]+-?)*[a-z¡-퟿0-9]+)(?:\.(?:[a-z¡-퟿0-9]+-?)*[a-z¡-퟿0-9]+)*(?:\.(?:[a-z¡-�]{2,})))|localhost)(?::\d{2,5})?(?:(/|\?|#)(?:[^\s]*[^.\s])?)?$",
        )
        .expect("url regex must compile")
    })
}

fn email_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").expect("email regex must compile")
    })
}

fn hex_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^#([0-9A-F]{3}){1,2}$").expect("hex regex must compile")
    })
}

fn rgb_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^rgb\(\s*(25[0-5]|2[0-4]\d|1?\d?\d)\s*,\s*(25[0-5]|2[0-4]\d|1?\d?\d)\s*,\s*(25[0-5]|2[0-4]\d|1?\d?\d)\s*\)$",
        )
        .expect("rgb regex must compile")
    })
}

/// Mirrors `detectActions`: only plain `Text` content, whole-string matches.
pub fn detect_smart_actions(item: &ClipboardItem) -> Vec<SmartAction> {
    let ClipboardContent::Text(text) = &item.content else {
        return Vec::new();
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut actions = Vec::new();
    // Stand-in for the TS `(?!mailto:)` lookahead (unsupported by the regex crate).
    let is_mailto = trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("mailto:");
    if !is_mailto && url_regex().is_match(trimmed) {
        let normalized = if trimmed.starts_with("http") {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        };
        actions.push(SmartAction {
            id: "open-link",
            label: "Open Link",
            data: normalized,
        });
    }
    if email_regex().is_match(trimmed) {
        actions.push(SmartAction {
            id: "compose-email",
            label: "Compose Email",
            data: format!("mailto:{trimmed}"),
        });
    }
    if hex_regex().is_match(trimmed) || rgb_regex().is_match(trimmed) {
        actions.push(SmartAction {
            id: "color-preview",
            label: "Color",
            data: trimmed.to_string(),
        });
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use win11_clipboard_history_lib::ClipboardItem as BackendItem;

    fn text_item(text: &str) -> BackendItem {
        BackendItem::new_text(text.to_string())
    }

    #[test]
    fn filter_substring_case_insensitive() {
        let items = vec![text_item("Hello World"), text_item("goodbye")];
        let hit = filter_history(&items, "hello", false);
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].preview, "Hello World");
    }

    #[test]
    fn filter_regex_and_invalid_regex() {
        let items = vec![text_item("abc123"), text_item("xyz")];
        assert_eq!(filter_history(&items, r"^\w+\d+$", true).len(), 1);
        // Invalid regex matches nothing (mirrors `return []`).
        assert!(filter_history(&items, "([", true).is_empty());
    }

    #[test]
    fn filter_excludes_images() {
        let img = BackendItem::new_image("AAAA".to_string(), 10, 10, 42);
        let items = vec![img, text_item("AAAA")];
        let hit = filter_history(&items, "AAAA", false);
        assert_eq!(hit.len(), 1);
        assert!(matches!(hit[0].content, ClipboardContent::Text(_)));
    }

    #[test]
    fn partition_keeps_order() {
        let mut a = text_item("a");
        let b = text_item("b");
        let mut c = text_item("c");
        a.pinned = true;
        c.pinned = true;
        let items = vec![a, b, c];
        let (pinned, unpinned) = partition_pinned(&items);
        assert_eq!(pinned.len(), 2);
        assert_eq!(unpinned.len(), 1);
        assert_eq!(unpinned[0].preview, "b");
    }

    #[test]
    fn relative_time_boundaries() {
        let now = Utc::now();
        assert_eq!(relative_time(&now, &now), "Just now");
        let m = now - chrono::Duration::minutes(5);
        assert_eq!(relative_time(&m, &now), "5m ago");
        let h = now - chrono::Duration::hours(3);
        assert_eq!(relative_time(&h, &now), "3h ago");
        let d = now - chrono::Duration::days(2);
        assert_eq!(relative_time(&d, &now), d.format("%-m/%-d/%Y").to_string());
    }

    #[test]
    fn smart_actions_detect_all_kinds() {
        let url = text_item("https://example.com/x");
        let actions = detect_smart_actions(&url);
        assert!(actions.iter().any(|a| a.id == "open-link"));

        let email = text_item("a@b.com");
        let actions = detect_smart_actions(&email);
        assert!(actions.iter().any(|a| a.id == "compose-email"));

        let hex = text_item("#ff5f5f");
        let actions = detect_smart_actions(&hex);
        assert!(actions.iter().any(|a| a.id == "color-preview"));

        let rgb = text_item("rgb(255, 95, 95)");
        let actions = detect_smart_actions(&rgb);
        assert!(actions.iter().any(|a| a.id == "color-preview"));

        let plain = text_item("just some words");
        assert!(detect_smart_actions(&plain).is_empty());
    }
}
