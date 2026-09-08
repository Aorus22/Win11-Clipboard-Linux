//! Picker data — port of `emojiService.ts`, `emojiSearch.ts` (ranking subset),
//! `kaomojiService.ts`, `symbolService.ts`, and the symbol-recents hook logic.
//!
//! Datasets: `assets/emojis.json` (vendored from `emojilib@4.0.3` — the exact
//! dataset the React build uses), `src/data/kaomojis.json` + `src/data/symbols.json`
//! via `include_str!` (zero duplication).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

// --- Emoji ---

#[derive(Debug, Clone, PartialEq)]
pub struct Emoji {
    pub char: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub category: String,
}

const CATEGORY_KEYWORDS: &[(&str, &[&str])] = &[
    (
        "Smileys & People",
        &[
            "face", "smile", "laugh", "cry", "person", "people", "hand", "body", "gesture",
            "emotion", "heart", "love", "family", "couple", "woman", "man", "girl", "boy",
        ],
    ),
    (
        "Animals & Nature",
        &[
            "animal", "bird", "cat", "dog", "nature", "plant", "flower", "tree", "weather",
            "sun", "moon", "fish", "insect", "bug",
        ],
    ),
    (
        "Food & Drink",
        &[
            "food", "fruit", "vegetable", "drink", "meal", "eat", "beverage", "coffee",
            "alcohol", "meat", "dessert",
        ],
    ),
    (
        "Activities",
        &[
            "sport", "game", "activity", "ball", "music", "art", "hobby", "play", "exercise",
        ],
    ),
    (
        "Travel & Places",
        &[
            "travel", "place", "vehicle", "car", "plane", "building", "city", "country",
            "flag", "transport",
        ],
    ),
    (
        "Objects",
        &[
            "object", "tool", "phone", "computer", "office", "book", "money", "mail",
            "clothing", "fashion",
        ],
    ),
    (
        "Symbols",
        &[
            "symbol", "arrow", "sign", "number", "letter", "zodiac", "warning", "mark",
            "button",
        ],
    ),
    ("Flags", &["flag"]),
];

fn detect_category(keywords: &[String]) -> String {
    let lowered: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
    for (category, cat_keywords) in CATEGORY_KEYWORDS {
        for kw in *cat_keywords {
            if lowered.iter().any(|k| k == kw || k.contains(kw)) {
                return category.to_string();
            }
        }
    }
    "Other".to_string()
}

fn emoji_map() -> &'static HashMap<String, Vec<String>> {
    static MAP: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    MAP.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/emojis.json")).expect("emojis.json parses")
    })
}

fn all_emojis() -> &'static Vec<Emoji> {
    static ALL: OnceLock<Vec<Emoji>> = OnceLock::new();
    ALL.get_or_init(|| {
        emoji_map()
            .iter()
            .filter(|(_, kw)| !kw.is_empty())
            .map(|(char, keywords)| {
                let name = keywords[0].split('_').collect::<Vec<_>>().join(" ");
                let category = detect_category(keywords);
                Emoji {
                    char: char.clone(),
                    name,
                    keywords: keywords.clone(),
                    category,
                }
            })
            .collect()
    })
}

pub fn load_emojis() -> Vec<Emoji> {
    all_emojis().clone()
}

pub fn emoji_categories() -> Vec<String> {
    let mut cats: Vec<String> = all_emojis()
        .iter()
        .map(|e| e.category.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    cats.sort();
    cats
}

/// Ranked-contains search (limit 100). Documented delta vs Fuse.js 0.3: no typo
/// fuzziness; exact/prefix/contains queries rank identically.
pub fn search_emojis(query: &str, limit: usize) -> Vec<Emoji> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(u8, usize)> = Vec::new();
    for (idx, emoji) in all_emojis().iter().enumerate() {
        let name = emoji.name.to_lowercase();
        let score = if name == q {
            0
        } else if name.starts_with(&q) {
            1
        } else if emoji.keywords.iter().any(|k| k.to_lowercase().starts_with(&q)) {
            2
        } else if name.contains(&q) {
            3
        } else if emoji.keywords.iter().any(|k| k.to_lowercase().contains(&q)) {
            4
        } else {
            continue;
        };
        scored.push((score, idx));
    }
    scored.sort();
    scored
        .into_iter()
        .take(limit)
        .map(|(_, idx)| all_emojis()[idx].clone())
        .collect()
}

// --- Kaomoji ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Kaomoji {
    pub id: String,
    pub text: String,
    pub category: String,
    pub keywords: Vec<String>,
}

pub const KAOMOJI_CATEGORIES: &[&str] = &[
    "Happy", "Sad", "Angry", "Love", "Confused", "Surprised", "Action", "Greetings",
    "Sleeping", "Apology", "Magic", "Dancing", "Food", "Music", "Study", "Cats", "Bears",
    "Dogs", "Rabbits", "Friends", "Skeptical", "Cute", "Shy", "Sympathy",
];

fn static_kaomojis() -> &'static Vec<Kaomoji> {
    static LIST: OnceLock<Vec<Kaomoji>> = OnceLock::new();
    LIST.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/data/kaomojis.json"))
            .expect("kaomojis.json parses")
    })
}

pub fn get_kaomojis(
    category: Option<&str>,
    search: &str,
    custom: &[Kaomoji],
) -> Vec<Kaomoji> {
    let mut list: Vec<Kaomoji> = custom.to_vec();
    list.extend(static_kaomojis().iter().cloned());
    if let Some(cat) = category {
        list.retain(|k| k.category == cat);
    }
    if !search.is_empty() {
        let term = search.to_lowercase();
        list.retain(|k| {
            k.text.to_lowercase().contains(&term)
                || k.keywords.iter().any(|kw| kw.contains(&term))
                || k.category.to_lowercase().contains(&term)
        });
    }
    list
}

// --- Symbols ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolItem {
    pub char: String,
    pub name: String,
    pub category: String,
    pub keywords: Vec<String>,
}

pub const SYMBOL_CATEGORIES: &[&str] = &[
    "General Punctuation",
    "Technical Symbols",
    "Currency Symbols",
    "Latin Symbols",
    "Letterlike Symbols",
    "Greek Symbols",
    "Math Symbols",
    "Geometric Symbols",
    "Dingbats",
    "Arrows",
    "Box Drawing",
    "Block Elements",
    "Miscellaneous Symbols",
    "Musical Symbols",
];

fn all_symbols() -> &'static Vec<SymbolItem> {
    static LIST: OnceLock<Vec<SymbolItem>> = OnceLock::new();
    LIST.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/data/symbols.json"))
            .expect("symbols.json parses")
    })
}

pub fn get_symbols(category: Option<&str>, query: &str) -> Vec<SymbolItem> {
    let mut list: Vec<SymbolItem> = all_symbols().clone();
    if let Some(cat) = category {
        list.retain(|s| s.category == cat);
    }
    if !query.is_empty() {
        let q = query.to_lowercase();
        list.retain(|s| {
            s.name.to_lowercase().contains(&q)
                || s.char.contains(query)
                || s.keywords.iter().any(|k| k.to_lowercase().contains(&q))
        });
    }
    list
}

// --- Symbol recents (port of the localStorage logic, max 24, most-recent-first) ---

pub const MAX_RECENT_SYMBOLS: usize = 24;
const RECENT_SYMBOLS_FILE: &str = "recent_symbols.json";

fn recent_symbols_path() -> std::path::PathBuf {
    crate::settings::config_dir().join(RECENT_SYMBOLS_FILE)
}

pub fn load_recent_symbols() -> Vec<SymbolItem> {
    std::fs::read_to_string(recent_symbols_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn record_symbol_usage(symbol: &SymbolItem) {
    let mut recent = load_recent_symbols();
    recent.retain(|s| s.char != symbol.char);
    recent.insert(0, symbol.clone());
    recent.truncate(MAX_RECENT_SYMBOLS);
    if let Some(parent) = recent_symbols_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&recent) {
        let _ = std::fs::write(recent_symbols_path(), content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_dataset_matches_react_build() {
        let emojis = load_emojis();
        assert_eq!(emojis.len(), 1914);
        let grin = emojis.iter().find(|e| e.char == "😀").expect("grinning face");
        assert_eq!(grin.name, "grinning face");
        assert!(grin.keywords.contains(&"smile".to_string()));
        let heart = emojis.iter().find(|e| e.char == "❤️").expect("red heart");
        assert_eq!(heart.category, "Smileys & People");
    }

    #[test]
    fn emoji_categories_sane() {
        let cats = emoji_categories();
        assert!(cats.contains(&"Smileys & People".to_string()));
        assert!(cats.len() >= 8);
        // Quirk parity: "Flags" never wins ("Travel & Places" also matches), and
        // 🇺🇸 lands in "Animals & Nature" because "indicator".contains("cat") —
        // identical substring semantics as the TS original.
        let us = load_emojis()
            .into_iter()
            .find(|e| e.char == "🇺🇸")
            .expect("us flag");
        assert_eq!(us.category, "Animals & Nature");
        assert!(!cats.contains(&"Flags".to_string()));
    }

    #[test]
    fn emoji_search_ranks_exact_first() {
        let hits = search_emojis("grinning face", 100);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].char, "😀");
        let hearts = search_emojis("heart", 100);
        assert!(hearts.iter().any(|e| e.char == "❤️"));
        assert!(search_emojis("   ", 100).is_empty());
    }

    #[test]
    fn kaomoji_dataset_and_filter() {
        assert_eq!(static_kaomojis().len(), 972);
        let happy = get_kaomojis(Some("Happy"), "", &[]);
        assert!(!happy.is_empty());
        assert!(happy.iter().all(|k| k.category == "Happy"));
        let searched = get_kaomojis(None, "uwu", &[]);
        assert!(searched.iter().any(|k| k.text.contains("uwu")));
        let custom = vec![Kaomoji {
            id: "custom-0".to_string(),
            text: "(custom!)".to_string(),
            category: "Custom".to_string(),
            keywords: vec![],
        }];
        let merged = get_kaomojis(None, "", &custom);
        assert_eq!(merged[0].text, "(custom!)");
        assert_eq!(merged.len(), 973);
    }

    #[test]
    fn symbol_dataset_and_filter() {
        assert_eq!(all_symbols().len(), 2484);
        let arrows = get_symbols(Some("Arrows"), "");
        assert!(!arrows.is_empty());
        assert!(arrows.iter().all(|s| s.category == "Arrows"));
        let dash = get_symbols(None, "em dash");
        assert!(dash.iter().any(|s| s.char == "—"));
    }

    #[test]
    fn symbol_recents_lru_round_trip() {
        let _serial = crate::test_util::serial_lock();
        // Uses the real config dir; restores prior content afterwards.
        let path = recent_symbols_path();
        let backup = std::fs::read_to_string(&path).ok();
        let _ = std::fs::remove_file(&path);
        let sym = SymbolItem {
            char: "→".to_string(),
            name: "Rightwards Arrow".to_string(),
            category: "Arrows".to_string(),
            keywords: vec!["arrow".to_string()],
        };
        record_symbol_usage(&sym);
        record_symbol_usage(&sym);
        let recent = load_recent_symbols();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].char, "→");
        match backup {
            Some(content) => {
                let _ = std::fs::write(&path, content);
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}
