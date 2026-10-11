//! Colour emoji as bitmaps.
//!
//! iced's text renderer draws neither CBDT nor COLRv1 colour glyphs, so each
//! emoji is read out of the bundled `NotoColorEmoji-CBDT.ttf` as the PNG it
//! stores and shown with the `image` widget. Glyphs are found by their `post`
//! name (`u1F600`, `u1F468_200D_1F469…`), which also covers ZWJ / keycap / flag
//! sequences without shaping.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use cosmic::widget::image::Handle;

const FONT: &[u8] = include_bytes!("../../gpui-app/assets/NotoColorEmoji-CBDT.ttf");

use ttf_parser::gsub::SubstitutionSubtable;
use ttf_parser::GlyphId;

fn face() -> Option<&'static ttf_parser::Face<'static>> {
    static FACE: OnceLock<Option<ttf_parser::Face<'static>>> = OnceLock::new();
    FACE.get_or_init(|| ttf_parser::Face::parse(FONT, 0).ok()).as_ref()
}

/// One ligature: first glyph + the following components → a single glyph.
struct Ligature {
    components: Vec<GlyphId>,
    glyph: GlyphId,
}

/// All GSUB ligature rules, per lookup, keyed by their first glyph.
type Lookup = HashMap<GlyphId, Vec<Ligature>>;

fn lookups() -> &'static Vec<Lookup> {
    static LOOKUPS: OnceLock<Vec<Lookup>> = OnceLock::new();
    LOOKUPS.get_or_init(|| {
        let mut out = Vec::new();
        let Some(gsub) = face().and_then(|f| f.tables().gsub) else {
            return out;
        };
        for index in 0..gsub.lookups.len() {
            let Some(lookup) = gsub.lookups.get(index) else { continue };
            let mut rules: Lookup = HashMap::new();
            for sub in 0..lookup.subtables.len() {
                let Some(SubstitutionSubtable::Ligature(table)) =
                    lookup.subtables.get::<SubstitutionSubtable>(sub)
                else {
                    continue;
                };
                for first in 0..face().map(|f| f.number_of_glyphs()).unwrap_or(0) {
                    let first = GlyphId(first);
                    let Some(set_index) = table.coverage.get(first) else { continue };
                    let Some(set) = table.ligature_sets.get(set_index) else { continue };
                    for i in 0..set.len() {
                        if let Some(lig) = set.get(i) {
                            rules.entry(first).or_default().push(Ligature {
                                components: lig.components.into_iter().collect(),
                                glyph: lig.glyph,
                            });
                        }
                    }
                }
            }
            if !rules.is_empty() {
                out.push(rules);
            }
        }
        out
    })
}

/// The glyph a character sequence shapes to, by cmap plus GSUB ligatures
/// (ZWJ sequences, flags, keycaps, skin tones); `None` unless it ends up as one glyph.
fn shape(chars: &[char]) -> Option<GlyphId> {
    let face = face()?;
    let mut glyphs: Vec<GlyphId> = chars
        .iter()
        .map(|c| face.glyph_index(*c))
        .collect::<Option<_>>()?;
    for _ in 0..3 {
        for lookup in lookups() {
            let mut i = 0;
            while i < glyphs.len() {
                let hit = lookup.get(&glyphs[i]).and_then(|rules| {
                    rules
                        .iter()
                        .filter(|l| {
                            glyphs.len() >= i + 1 + l.components.len()
                                && glyphs[i + 1..i + 1 + l.components.len()] == l.components[..]
                        })
                        .max_by_key(|l| l.components.len())
                });
                if let Some(lig) = hit {
                    let glyph = lig.glyph;
                    let len = lig.components.len();
                    glyphs.splice(i..=i + len, [glyph]);
                }
                i += 1;
            }
        }
        if glyphs.len() == 1 {
            return Some(glyphs[0]);
        }
    }
    (glyphs.len() == 1).then(|| glyphs[0])
}

/// The bitmap for an emoji, if the font has one. Cached.
pub fn handle(emoji: &str) -> Option<Handle> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Handle>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(emoji) {
        return hit.clone();
    }
    let found = lookup(emoji);
    cache.lock().unwrap().insert(emoji.to_string(), found.clone());
    found
}

fn lookup(emoji: &str) -> Option<Handle> {
    let face = face()?;
    let full: Vec<char> = emoji.chars().collect();
    let bare: Vec<char> = full.iter().copied().filter(|c| *c != '\u{FE0F}').collect();
    let glyph = shape(&bare).or_else(|| shape(&full))?;
    let raster = face.glyph_raster_image(glyph, u16::MAX)?;
    if raster.format != ttf_parser::RasterImageFormat::PNG {
        return None;
    }
    Some(Handle::from_bytes(raster.data.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_emoji_have_bitmaps() {
        for e in ["😀", "👍", "❤️", "🇮🇩", "👨\u{200D}👩\u{200D}👧", "1️⃣"] {
            assert!(handle(e).is_some(), "no bitmap for {e}");
        }
    }

    #[test]
    fn nearly_every_emoji_of_the_dataset_has_a_bitmap() {
        let all = app_shared::pickers::load_emojis();
        let missing: Vec<&str> = all.iter().filter(|e| handle(&e.char).is_none()).map(|e| e.char.as_str()).collect();
        eprintln!("{} of {} without bitmap: {:?}", missing.len(), all.len(), &missing[..missing.len().min(30)]);
        assert!(missing.len() * 50 < all.len(), "more than 2% of emoji have no bitmap");
    }
}
