//! GNOME/GTK theme probe — reads the *active* GTK theme's named colors so the
//! UI can follow the desktop appearance (`theme_mode == "gtk"`).
//!
//! Two sources are merged, in this order of authority:
//!
//! 1. **The user's own GTK config** ([`UserCss`]) — `colors.css` / `gtk.css`
//!    under `gtk-4.0` (then `gtk-3.0`). On GNOME the `gtk-theme` setting is only
//!    half the story: libadwaita 1.6+ apps take their palette from `:root`
//!    custom properties in `~/.config/gtk-4.0/gtk.css` (what palette tools such
//!    as Rewaita write), with `@define-color` entries in the `colors.css` next
//!    to it. Those files are what the desktop *actually* looks like — a user
//!    whose `gtk-theme` is still `WhiteSur-Dark` but whose GTK4 config is Tokyo
//!    Night sees Tokyo Night everywhere else.
//! 2. **The GTK3 style engine** — GTK is already initialized for the
//!    AppIndicator tray (`gtk::init()` runs on the main thread in `main`), so an
//!    unrealized `GtkWindow`'s style context resolves the theme's named colors
//!    (`theme_bg_color`, `theme_selected_bg_color`, …) without hand-rolling a
//!    theme parse. This is the fallback for users with no user CSS at all.
//!
//! Everything GTK-touching is confined to [`probe`] and only runs when GTK was
//! initialized on the current thread; the CSS parser and
//! [`palette_from_colors`] are pure and unit-tested so the mapping stays
//! testable without a display.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::Rgba;
use parking_lot::Mutex;

use crate::theme;

/// Raw named colors read from the active GTK theme (`None` = not defined).
/// Plain `Send` data — no GTK types escape this module.
#[derive(Clone, Debug, PartialEq)]
pub struct GtkColors {
    pub theme_name: String,
    /// `gtk-application-prefer-dark-theme`.
    pub prefer_dark: bool,
    pub bg: Rgba,
    pub fg: Rgba,
    pub base: Rgba,
    /// Raised surface (libadwaita `--card-bg-color`), when the user set one.
    pub card: Option<Rgba>,
    pub selected_bg: Option<Rgba>,
    pub selected_fg: Option<Rgba>,
    pub borders: Option<Rgba>,
    pub insensitive_fg: Option<Rgba>,
    pub error: Option<Rgba>,
    pub warning: Option<Rgba>,
    pub success: Option<Rgba>,
    pub accent_color: Option<Rgba>,
    pub accent_bg_color: Option<Rgba>,
}

/// Cache key: re-probe only when the GTK theme, its dark preference, or one of
/// the user's CSS files changes (editing `gtk.css` live-updates the UI).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeKey {
    pub name: String,
    pub prefer_dark: bool,
    /// Newest mtime among the user CSS files, in seconds (0 = none present).
    pub css_mtime: u64,
}

/// The user's own GTK color tables (`colors.css` / `gtk.css`).
///
/// Names are normalized (`--window-bg-color`, `window_bg_color` and
/// `@theme_bg_color` all collapse to `theme_bg_color`), values keep their raw
/// text so `var()`/`@name` indirection can be followed at lookup time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserCss {
    vars: HashMap<String, String>,
    defines: HashMap<String, String>,
}

/// Files that make up the user's palette, **lowest priority first** (later
/// inserts overwrite earlier ones): GTK3 below GTK4, `colors.css` below the
/// `gtk.css` that carries the live libadwaita palette.
const USER_CSS_FILES: [(&str, &str); 4] = [
    ("gtk-3.0", "colors.css"),
    ("gtk-3.0", "gtk.css"),
    ("gtk-4.0", "colors.css"),
    ("gtk-4.0", "gtk.css"),
];

impl UserCss {
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty() && self.defines.is_empty()
    }

    fn raw(&self, name: &str) -> Option<&str> {
        let key = normalize_css_name(name);
        self.vars
            .get(&key)
            .or_else(|| self.defines.get(&key))
            .map(|value| value.as_str())
    }

    /// Resolve one color by name, following `var(--x)` / `@x` indirection.
    pub fn get(&self, name: &str) -> Option<Rgba> {
        let mut value = self.raw(name)?.to_string();
        for _ in 0..4 {
            let reference = value
                .strip_prefix("var(")
                .and_then(|inner| inner.strip_suffix(')'))
                .map(|inner| inner.trim().to_string())
                .or_else(|| value.strip_prefix('@').map(|inner| inner.trim().to_string()));
            match reference {
                Some(reference) => value = self.raw(&reference)?.to_string(),
                None => return parse_css_color(&value),
            }
        }
        None
    }

    /// First of `names` that resolves — the priority lists in [`probe`] use it.
    pub fn get_any(&self, names: &[&str]) -> Option<Rgba> {
        names.iter().find_map(|name| self.get(name))
    }
}

fn normalize_css_name(name: &str) -> String {
    name.trim()
        .trim_start_matches("--")
        .trim_start_matches('@')
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb(...)`/`rgba(...)` and
/// `transparent`. Everything else (`currentColor`, `alpha(...)`, `color-mix`…)
/// returns `None` and leaves the token to the next source in the list.
fn parse_css_color(value: &str) -> Option<Rgba> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        return match hex.len() {
            3 | 4 => {
                let nibble = |i: usize| {
                    u8::from_str_radix(&hex[i..i + 1], 16)
                        .ok()
                        .map(|v| v * 17)
                };
                let alpha = match hex.len() {
                    4 => nibble(3)?,
                    _ => 0xff,
                };
                Some(gpui::rgba(
                    ((nibble(0)? as u32) << 24)
                        | ((nibble(1)? as u32) << 16)
                        | ((nibble(2)? as u32) << 8)
                        | alpha as u32,
                ))
            }
            6 => Some(gpui::rgb(
                ((byte(0)? as u32) << 16) | ((byte(2)? as u32) << 8) | byte(4)? as u32,
            )),
            8 => Some(gpui::rgba(
                ((byte(0)? as u32) << 24)
                    | ((byte(2)? as u32) << 16)
                    | ((byte(4)? as u32) << 8)
                    | byte(6)? as u32,
            )),
            _ => None,
        };
    }
    if value.eq_ignore_ascii_case("transparent") {
        return Some(gpui::rgba(0x00000000));
    }
    let open = value.find('(')?;
    if !value[..open].trim().eq_ignore_ascii_case("rgb")
        && !value[..open].trim().eq_ignore_ascii_case("rgba")
    {
        return None;
    }
    let inner = value[open + 1..].trim_end_matches(')').trim();
    let parts: Vec<&str> = inner.split(',').map(|part| part.trim()).collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = |part: &str| part.split('%').next()?.trim().parse::<f32>().ok();
    let (r, g, b) = (channel(parts[0])?, channel(parts[1])?, channel(parts[2])?);
    let a = match parts.get(3) {
        Some(part) => channel(part)?,
        None => 1.0,
    };
    let to_byte = |v: f32| (v.clamp(0.0, 255.0).round() as u32) & 0xff;
    Some(gpui::rgba(
        (to_byte(r) << 24) | (to_byte(g) << 16) | (to_byte(b) << 8) | (a.clamp(0.0, 1.0) * 255.0).round() as u32,
    ))
}

/// Drop `/* … */` spans (which may wrap lines) before line-oriented parsing.
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn clean_css_value(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or(value)
        .trim()
        .to_string()
}

/// Parse `--name: value;` custom properties and `@define-color name value;`
/// declarations out of one stylesheet.
pub fn parse_css_into(css: &str, out: &mut UserCss) {
    for line in strip_css_comments(css).lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("@define-color") {
            let mut parts = rest.trim().splitn(2, char::is_whitespace);
            if let (Some(name), Some(value)) = (parts.next(), parts.next()) {
                out.defines
                    .insert(normalize_css_name(name), clean_css_value(value));
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("--") {
            if let Some((name, value)) = rest.split_once(':') {
                out.vars
                    .insert(normalize_css_name(name), clean_css_value(value));
            }
        }
    }
}

fn user_css_paths() -> Vec<PathBuf> {
    let Some(config) = dirs::config_dir() else {
        return Vec::new();
    };
    USER_CSS_FILES
        .iter()
        .map(|(dir, file)| config.join(dir).join(file))
        .collect()
}

/// Read every user CSS file, in increasing priority order.
fn load_user_css() -> UserCss {
    let mut out = UserCss::default();
    for path in user_css_paths() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            parse_css_into(&text, &mut out);
        }
    }
    out
}

/// Newest mtime among the user CSS files (cache key for live theme edits).
pub fn user_css_mtime() -> u64 {
    user_css_paths()
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok())
        .filter_map(|meta| meta.modified().ok())
        .filter_map(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_secs())
        .max()
        .unwrap_or(0)
}

static GTK_READY: AtomicBool = AtomicBool::new(false);
static MAIN_THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);
static CACHE: Mutex<Option<(ThemeKey, theme::GtkPalette)>> = Mutex::new(None);
/// Whether the last probe found user CSS — drives the Settings caption (read
/// without touching the filesystem again).
static USER_CSS_SEEN: AtomicBool = AtomicBool::new(false);

/// GNOME's fallback accent (Adwaita blue) when a theme defines no accent at all.
const GNOME_BLUE: u32 = 0x3584e4;

/// Initialize GTK on this (main) thread. Replaces the bare `gtk::init()` call in
/// `main`: the tray needs GTK anyway, and the theme probe reuses it.
///
/// Returns whether GTK is usable; the caller skips the tray when it is not.
pub fn init() -> bool {
    match gtk::init() {
        Ok(()) => {
            GTK_READY.store(true, Ordering::SeqCst);
            *MAIN_THREAD.lock() = Some(std::thread::current().id());
            true
        }
        Err(e) => {
            eprintln!("[gpui-app] GTK unavailable, running without tray: {e:?}");
            false
        }
    }
}

pub fn is_available() -> bool {
    GTK_READY.load(Ordering::SeqCst)
}

/// GTK widgets and `GtkSettings` may only be touched from the thread that ran
/// `gtk::init()`. Every entry point bails out elsewhere instead of tripping
/// gtk-rs's `assert_initialized_main_thread!` panic.
fn on_main_thread() -> bool {
    match *MAIN_THREAD.lock() {
        Some(id) => id == std::thread::current().id(),
        None => false,
    }
}

/// Cheap change detector: theme name + dark preference (two property reads).
pub fn current_key() -> Option<ThemeKey> {
    if !is_available() || !on_main_thread() {
        return None;
    }
    let settings = gtk::Settings::default()?;
    use gtk::prelude::*;
    Some(ThemeKey {
        name: settings
            .gtk_theme_name()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        prefer_dark: settings.is_gtk_application_prefer_dark_theme(),
        css_mtime: user_css_mtime(),
    })
}

/// Name of the theme the cached palette came from (UI hint / diagnostics).
pub fn cached_name() -> Option<String> {
    current_key().map(|key| key.name).filter(|n| !n.is_empty())
}

/// Whether the active palette comes from the user's own `colors.css`/`gtk.css`
/// rather than from the theme named by `gtk-theme` (Settings caption).
pub fn user_css_active() -> bool {
    USER_CSS_SEEN.load(Ordering::SeqCst)
}

/// Resolved GTK palette, re-probed only when [`current_key`] changes.
pub fn palette() -> Option<theme::GtkPalette> {
    let key = current_key()?;
    {
        let cache = CACHE.lock();
        if let Some((cached_key, palette)) = cache.as_ref() {
            if *cached_key == key {
                return Some(*palette);
            }
        }
    }
    let colors = probe()?;
    let palette = palette_from_colors(&colors);
    *CACHE.lock() = Some((key, palette));
    Some(palette)
}

/// Drop the cache (tests / forced re-probe).
#[allow(dead_code)]
pub fn invalidate_cache() {
    *CACHE.lock() = None;
}

/// Read the active theme's named colors: the user's own CSS first, then the
/// GTK3 style context of a throwaway popup window.
///
/// The window is never shown: creating it is what attaches the theme's style
/// provider to the style context, which is also how `GtkStyleContext::lookup_color`
/// is used from C.
fn probe() -> Option<GtkColors> {
    if !is_available() || !on_main_thread() {
        return None;
    }
    use gtk::prelude::*;

    let settings = gtk::Settings::default();
    let prefer_dark = settings
        .as_ref()
        .map(|s| s.is_gtk_application_prefer_dark_theme())
        .unwrap_or(false);
    let theme_name = settings
        .as_ref()
        .and_then(|s| s.gtk_theme_name())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let user = load_user_css();
    USER_CSS_SEEN.store(!user.is_empty(), Ordering::SeqCst);
    let window = gtk::Window::new(gtk::WindowType::Popup);
    let context = window.style_context();
    let lookup = |name: &str| {
        context.lookup_color(name).map(|c| Rgba {
            r: c.red() as f32,
            g: c.green() as f32,
            b: c.blue() as f32,
            a: c.alpha() as f32,
        })
    };
    // User CSS wins; the GTK3 theme is the fallback. Each list runs from the
    // most specific name to the most generic one.
    let from_user = |names: &[&str]| user.get_any(names);
    let pick = |names: &[&str], theme_names: &[&str]| {
        from_user(names).or_else(|| theme_names.iter().find_map(|name| lookup(name)))
    };

    // Every GTK theme must define a background and foreground; without them the
    // theme is unusable for us and the caller falls back to the Win11 palette.
    let bg = pick(
        &["window_bg_color", "theme_bg_color_breeze", "theme_bg_color"],
        &["theme_bg_color", "bg_color"],
    )?;
    let fg = pick(
        &[
            "window_fg_color",
            "theme_fg_color_breeze",
            "theme_fg_color",
            "theme_text_color_breeze",
            "theme_text_color",
        ],
        &["theme_fg_color", "fg_color"],
    )?;
    let base = pick(
        &[
            "theme_base_color_breeze",
            "theme_base_color",
            "view_bg_color",
            "content_view_bg_breeze",
        ],
        &["theme_base_color", "base_color"],
    )
    .unwrap_or(bg);
    // Raised surfaces: libadwaita cards/popovers, else the base color.
    let card = from_user(&["card_bg_color", "popover_bg_color", "theme_card_bg_color"]);

    Some(GtkColors {
        theme_name,
        prefer_dark,
        bg,
        fg,
        base,
        card,
        selected_bg: pick(
            &[
                "accent_bg_color",
                "accent_color",
                "theme_selected_bg_color_breeze",
                "theme_selected_bg_color",
                "theme_hovering_selected_bg_color_breeze",
                // Palette-only configs (Rewaita-style) name the hues directly.
                "blue_1",
            ],
            &["theme_selected_bg_color", "selected_bg_color"],
        ),
        selected_fg: pick(
            &[
                "accent_fg_color",
                "theme_selected_fg_color_breeze",
                "theme_selected_fg_color",
            ],
            &["theme_selected_fg_color", "selected_fg_color"],
        ),
        borders: pick(
            &[
                "borders_breeze",
                "unfocused_borders_breeze",
                "borders",
                "border_color",
                "sidebar_border_color",
            ],
            &["borders", "unfocused_borders"],
        ),
        insensitive_fg: pick(
            &[
                "insensitive_fg_color_breeze",
                "insensitive_fg_color",
                "theme_unfocused_fg_color_breeze",
                "theme_unfocused_fg_color",
            ],
            &["insensitive_fg_color"],
        ),
        error: pick(
            &["error_color_breeze", "error_color", "red_1", "red_2"],
            &["error_color"],
        ),
        warning: pick(
            &["warning_color_breeze", "warning_color", "yellow_1", "orange_1"],
            &["warning_color"],
        ),
        success: pick(
            &["success_color_breeze", "success_color", "green_1"],
            &["success_color"],
        ),
        accent_color: pick(&["accent_color", "blue_1"], &["accent_color"]),
        accent_bg_color: pick(&["accent_bg_color"], &["accent_bg_color"]),
    })
}

/// Map the theme's named colors onto the UI token table (pure — unit-tested).
pub fn palette_from_colors(c: &GtkColors) -> theme::GtkPalette {
    let bg = c.bg;
    let fg = c.fg;
    let base = c.base;

    // GNOME 47+ accents live in `accent_bg_color` (libadwaita) or the portal;
    // GTK3 themes embed their own in `theme_selected_bg_color`.
    let accent = c
        .selected_bg
        .or(c.accent_bg_color)
        .or(c.accent_color)
        .or_else(theme::portal_accent)
        .unwrap_or_else(|| gpui::rgb(GNOME_BLUE));

    let is_dark = c.prefer_dark || theme::relative_luminance(bg) < 0.5;

    let bg_primary = bg;
    let bg_secondary = base;
    let bg_tertiary = theme::mix(base, fg, 0.06);
    // Cards sit on top of the window: libadwaita's own card color when the user
    // configured one, else the base surface.
    let bg_card = c.card.unwrap_or(base);
    let bg_card_hover = theme::mix(bg_card, fg, 0.08);

    let text_primary = fg;
    let text_secondary = theme::mix(bg_primary, fg, 0.75);
    let text_tertiary = theme::mix(bg_primary, fg, 0.55);
    // `insensitive_fg_color` is usually translucent; composite it so tokens stay
    // opaque and no unexpected layering shows up.
    let text_disabled = match c.insensitive_fg {
        Some(insensitive) => theme::mix(bg_primary, insensitive, insensitive.a),
        None => theme::mix(bg_primary, fg, 0.35),
    };

    let border = c
        .borders
        .unwrap_or_else(|| theme::mix(base, fg, 0.18));
    let border_subtle = theme::mix(bg_primary, border, 0.6);

    let accent_hover = if is_dark {
        theme::lighten(accent, 0.10)
    } else {
        theme::darken(accent, 0.10)
    };
    let on_accent = c
        .selected_fg
        .unwrap_or_else(|| theme::contrast_text(accent));

    let error = c.error.unwrap_or_else(|| gpui::rgb(theme::hex::ERROR));
    let warning = c.warning.unwrap_or_else(|| gpui::rgb(theme::hex::WARNING));
    let success = c.success.unwrap_or_else(|| gpui::rgb(theme::hex::SUCCESS));

    // Neutral ramp between the palette's own background and foreground: only the
    // branch matching this scheme is rendered, so each step keeps its role
    // (`g400` = muted text on dark, `g100` = hover surface on light, …).
    let ramp = |t: f32| theme::mix(bg_primary, fg, t);
    let gray = [
        ramp(0.03),
        ramp(0.06),
        ramp(0.12),
        ramp(0.28),
        ramp(0.45),
        ramp(0.58),
        ramp(0.70),
        ramp(0.80),
        ramp(0.90),
        fg,
    ];

    theme::GtkPalette {
        is_dark,
        bg_primary,
        bg_secondary,
        bg_tertiary,
        bg_card,
        bg_card_hover,
        text_primary,
        text_secondary,
        text_tertiary,
        text_disabled,
        border,
        border_subtle,
        accent,
        accent_hover,
        on_accent,
        error,
        warning,
        success,
        placeholder: theme::mix(bg_primary, fg, 0.55),
        close_hover: theme::darken(error, 0.10),
        settings_light_bg: bg_primary,
        gray,
        green50: theme::mix(bg_primary, success, 0.10),
        green200: theme::mix(bg_primary, success, 0.30),
        amber50: theme::mix(bg_primary, warning, 0.10),
        amber200: theme::mix(bg_primary, warning, 0.30),
        red50: theme::mix(bg_primary, error, 0.10),
        success_bg: theme::with_alpha_byte(success, 0x26),
        warning_bg: theme::with_alpha_byte(warning, 0x26),
        error_bg: theme::with_alpha_byte(error, 0x26),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::serial_lock;

    fn color(hex: u32) -> Rgba {
        gpui::rgb(hex)
    }

    /// WhiteSur-Dark-esque fixture (values probed from the real theme).
    fn fixture() -> GtkColors {
        GtkColors {
            theme_name: "WhiteSur-Dark".to_string(),
            prefer_dark: false,
            bg: color(0x333333),
            fg: color(0xdedede),
            base: color(0x242424),
            card: None,
            selected_bg: Some(color(0x0860f2)),
            selected_fg: Some(color(0xffffff)),
            borders: Some(color(0x2f3140)),
            insensitive_fg: Some(Rgba {
                a: 0.35,
                ..color(0xdedede)
            }),
            error: Some(color(0xed5f5d)),
            warning: Some(color(0xe9873a)),
            success: Some(color(0x79b757)),
            accent_color: None,
            accent_bg_color: None,
        }
    }

    #[test]
    fn maps_theme_colors_onto_tokens() {
        let _serial = serial_lock();
        theme::set_portal_accent(None);
        theme::clear_gtk();
        let palette = palette_from_colors(&fixture());

        // #333333 background: dark despite `prefer_dark == false`.
        assert!(palette.is_dark);
        assert_eq!(palette.bg_primary, color(0x333333));
        assert_eq!(palette.bg_card, color(0x242424));
        assert_eq!(palette.text_primary, color(0xdedede));
        assert_eq!(palette.accent, color(0x0860f2));
        assert_eq!(palette.on_accent, color(0xffffff));
        assert_eq!(palette.border, color(0x2f3140));
        assert_eq!(palette.error, color(0xed5f5d));
        assert_eq!(palette.warning, color(0xe9873a));
        assert_eq!(palette.success, color(0x79b757));
        assert_eq!(palette.settings_light_bg, color(0x333333));
        theme::clear_gtk();
    }

    #[test]
    fn accent_falls_back_through_portal_to_gnome_blue() {
        let _serial = serial_lock();
        theme::clear_gtk();
        let mut colors = fixture();
        colors.selected_bg = None;
        colors.accent_bg_color = None;
        colors.accent_color = None;

        theme::set_portal_accent(None);
        assert_eq!(palette_from_colors(&colors).accent, color(GNOME_BLUE));

        theme::set_portal_accent(Some(color(0xe62d42)));
        assert_eq!(palette_from_colors(&colors).accent, color(0xe62d42));

        colors.accent_bg_color = Some(color(0x123456));
        assert_eq!(palette_from_colors(&colors).accent, color(0x123456));
        theme::set_portal_accent(None);
    }

    #[test]
    fn light_theme_stays_light_and_prefers_fg() {
        let _serial = serial_lock();
        theme::clear_gtk();
        let colors = GtkColors {
            theme_name: "Adwaita".to_string(),
            prefer_dark: false,
            bg: color(0xf6f5f4),
            fg: color(0x2e3436),
            base: color(0xffffff),
            card: None,
            selected_bg: Some(color(0x3584e4)),
            selected_fg: None,
            borders: None,
            insensitive_fg: None,
            error: None,
            warning: None,
            success: None,
            accent_color: None,
            accent_bg_color: None,
        };
        let palette = palette_from_colors(&colors);
        assert!(!palette.is_dark);
        assert_eq!(palette.border, theme::mix(color(0xffffff), color(0x2e3436), 0.18));
        assert_eq!(palette.on_accent, color(0xffffff));
        // Darker accent on hover for a light scheme.
        assert_eq!(palette.accent_hover, theme::darken(color(0x3584e4), 0.10));
        theme::clear_gtk();
    }

    #[test]
    fn gray_ramp_runs_from_background_to_foreground() {
        let _serial = serial_lock();
        let palette = palette_from_colors(&fixture());
        assert_eq!(palette.gray[0], theme::mix(palette.bg_primary, palette.text_primary, 0.03));
        assert_eq!(palette.gray[9], palette.text_primary);
        // Monotonic towards the foreground.
        for pair in palette.gray.windows(2) {
            assert!(
                theme::relative_luminance(pair[1]) > theme::relative_luminance(pair[0]),
                "ramp must climb towards the light foreground"
            );
        }
    }

    /// Tokyo Night as it sits on this project's dev machine: `gtk-theme` is
    /// still `WhiteSur-Dark`, but the GTK4 user config (Rewaita output) is what
    /// every libadwaita app on the desktop actually renders.
    const TOKYO_NIGHT_GTK_CSS: &str = r#"
:root {
  --window-bg-color: #1a1b26;
  --window-fg-color: #a9b1d6;
  --card-bg-color: #282a38;
  --headerbar-bg-color: #16161e;
  --sidebar-bg-color: var(--window-bg-color);
  --blue-1: #7aa2f7;
  --green-1: #9ece6a;
  --yellow-1: #e0af68;
  --red-1: #f7768e;
  color: var(--window-fg-color);
}
"#;

    const TOKYO_NIGHT_COLORS_CSS: &str = r#"
/* exported from the desktop color scheme */
@define-color theme_bg_color_breeze #1a1b26;
@define-color theme_fg_color_breeze #a9b1d6;
@define-color theme_base_color_breeze #1a1b26;
@define-color theme_selected_bg_color_breeze #7aa2f7;
@define-color theme_selected_fg_color_breeze #1a1b26;
@define-color borders_breeze #373949;
@define-color insensitive_fg_color_breeze #484c5f;
@define-color error_color_breeze #f7768e;
@define-color warning_color_breeze #e0af68;
@define-color success_color_breeze #9ece6a;
@define-color borders alpha(currentColor, 0.12);
"#;

    fn user_css() -> UserCss {
        let mut user = UserCss::default();
        parse_css_into(TOKYO_NIGHT_GTK_CSS, &mut user);
        parse_css_into(TOKYO_NIGHT_COLORS_CSS, &mut user);
        user
    }

    #[test]
    fn css_parser_reads_vars_and_defines() {
        let user = user_css();
        assert!(!user.is_empty());
        // `--var` and `@define-color` land in the same namespace.
        assert_eq!(user.get("--window-bg-color"), Some(color(0x1a1b26)));
        assert_eq!(user.get("window_bg_color"), Some(color(0x1a1b26)));
        assert_eq!(user.get("theme_bg_color_breeze"), Some(color(0x1a1b26)));
        assert_eq!(user.get("borders_breeze"), Some(color(0x373949)));
        // var()/colour references are followed.
        assert_eq!(user.get("sidebar_bg_color"), Some(color(0x1a1b26)));
        // Unsupported values are skipped, not guessed.
        assert_eq!(user.get("borders"), None);
        assert_eq!(user.get("does_not_exist"), None);
        // Breeze-style palettes are preferred over the plain theme names.
        assert_eq!(
            user.get_any(&["borders", "borders_breeze"]),
            Some(color(0x373949))
        );
    }

    #[test]
    fn css_colors_override_the_gtk3_theme() {
        let _serial = serial_lock();
        theme::set_portal_accent(None);
        theme::clear_gtk();
        let user = user_css();
        // A WhiteSur-Dark GTK3 context is what the desktop reports by name; the
        // user CSS must win over it token by token.
        let colors = GtkColors {
            theme_name: "WhiteSur-Dark".to_string(),
            prefer_dark: true,
            bg: user.get_any(&["window_bg_color", "theme_bg_color"]).unwrap(),
            fg: user
                .get_any(&["window_fg_color", "theme_fg_color_breeze"])
                .unwrap(),
            base: user
                .get_any(&["theme_base_color_breeze", "view_bg_color"])
                .unwrap(),
            card: user.get_any(&["card_bg_color", "popover_bg_color"]),
            selected_bg: user.get_any(&["accent_bg_color", "theme_selected_bg_color_breeze"]),
            selected_fg: user.get_any(&["accent_fg_color", "theme_selected_fg_color_breeze"]),
            borders: user.get_any(&["borders_breeze", "borders"]),
            insensitive_fg: user.get_any(&["insensitive_fg_color_breeze", "insensitive_fg_color"]),
            error: user.get_any(&["error_color_breeze", "red_1"]),
            warning: user.get_any(&["warning_color_breeze", "yellow_1"]),
            success: user.get_any(&["success_color_breeze", "green_1"]),
            accent_color: None,
            accent_bg_color: None,
        };
        let palette = palette_from_colors(&colors);

        assert!(palette.is_dark);
        assert_eq!(palette.bg_primary, color(0x1a1b26));
        assert_eq!(palette.bg_card, color(0x282a38));
        assert_eq!(palette.text_primary, color(0xa9b1d6));
        assert_eq!(palette.text_disabled, color(0x484c5f));
        assert_eq!(palette.accent, color(0x7aa2f7));
        assert_eq!(palette.on_accent, color(0x1a1b26));
        assert_eq!(palette.border, color(0x373949));
        assert_eq!(palette.error, color(0xf7768e));
        assert_eq!(palette.warning, color(0xe0af68));
        assert_eq!(palette.success, color(0x9ece6a));
        theme::clear_gtk();
    }

    /// The user CSS accent declarations win over the theme's own selection
    /// color — on the dev machine the desktop accent is Tokyo Night pink
    /// (`accent_bg_color`) while `theme_selected_bg_color` is a different blue.
    #[test]
    fn user_accent_declarations_beat_the_theme_selection_color() {
        let mut user = user_css();
        parse_css_into(
            "@define-color accent_bg_color #f7768e;\n@define-color accent_fg_color #222222;",
            &mut user,
        );
        assert_eq!(
            user.get_any(&["accent_bg_color", "theme_selected_bg_color_breeze"]),
            Some(color(0xf7768e))
        );
        assert_eq!(
            user.get_any(&["accent_fg_color", "theme_selected_fg_color_breeze"]),
            Some(color(0x222222))
        );
    }

    #[test]
    fn no_gtk_returns_none_instead_of_panicking() {
        // Tests never initialize GTK: the guard must return None, not assert.
        assert!(!is_available() || current_key().is_none());
        if !is_available() {
            assert!(current_key().is_none());
            assert!(palette().is_none());
        }
    }
}

