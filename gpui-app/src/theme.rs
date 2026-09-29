//! Windows 11 design tokens — port of `tailwind.config.js` + `themeUtils.ts`
//! — plus an optional desktop (GNOME/GTK) palette override.
//!
//! All colors are sRGB 0-255; alpha comes from the settings opacity values so the
//! "same token, same pixel" contract with the React build holds by construction.
//!
//! Note: `gpui::rgb`/`rgba` are not `const fn`, so tokens are plain functions.
//!
//! # GTK mode (`theme_mode == "gtk"`)
//!
//! The Win11 literals live in [`hex`] and are returned verbatim unless a desktop
//! palette is installed ([`install_gtk`]). While one is installed *both* the
//! `dark::*` and `light::*` halves return the GTK value, so the existing
//! `if is_dark { dark::x() } else { light::x() }` call sites keep working: only
//! the branch matching the GTK palette's own scheme is ever rendered (in GTK
//! mode `is_dark` *is* the GTK palette's darkness). [`raw`] exposes the
//! unconditional Win11 values for the Appearance preview swatches and tests.

use gpui::{Rgba, rgb, rgba};
use parking_lot::RwLock;

use crate::settings::AppSettings;

pub const RADIUS_CARD: f32 = 8.0;
pub const RADIUS_WINDOW: f32 = 12.0;

/// Transparent frame reserved around the Settings/Setup windows for their shadow.
///
/// X11 gives a client no shadow of its own and mutter's own shadow is painted
/// around the *rectangle* of the window — which is exactly the dark wedge the
/// user saw through the transparent rounded corners. So the windows do what GTK
/// CSD does: keep [`WINDOW_SHADOW_MARGIN`] transparent pixels all around, paint
/// the shadow into them ourselves ([`window_shadow`]), and advertise the same
/// margin as `_GTK_FRAME_EXTENTS` so mutter treats the inset frame as the window
/// and stops drawing its square shadow (see `window_drag::set_frame_extents`).
///
/// The window is therefore `2 * WINDOW_SHADOW_MARGIN` larger than the card the
/// user sees, which is why `centered_options` adds it back.
pub const WINDOW_SHADOW_MARGIN: f32 = 16.0;

/// The window shadow, painted by gpui into [`WINDOW_SHADOW_MARGIN`].
///
/// Roughly GTK's `0 4px 12px rgba(0, 0, 0, .45)`: big enough to read as depth,
/// small enough that the blur stays inside the reserved margin (a clipped
/// shadow edge is what would give the margin away — see the test below).
pub fn window_shadow() -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: rgba(0x00000073).into(),
        offset: gpui::point(gpui::px(0.), gpui::px(4.)),
        blur_radius: gpui::px(12.),
        spread_radius: gpui::px(0.),
    }]
}

/// Win11 literal palette — the single source of truth for the pre-GTK design.
/// Values are `0xRRGGBB` (opaque) or `0xRRGGBBAA` (with alpha).
pub mod hex {
    // Top-level
    pub const ACCENT: u32 = 0x0078d4;
    pub const ACCENT_HOVER: u32 = 0x1a86d9;
    pub const ERROR: u32 = 0xff5f5f;
    pub const WARNING: u32 = 0xfcb900;
    pub const SUCCESS: u32 = 0x6ccb5f;
    pub const ON_ACCENT: u32 = 0xffffff;
    pub const PLACEHOLDER: u32 = 0x888888;
    pub const CLOSE_HOVER: u32 = 0xc42b1c;

    // Dark
    pub const DARK_BG_PRIMARY: u32 = 0x202020;
    pub const DARK_BG_SECONDARY: u32 = 0x2d2d2d;
    pub const DARK_BG_TERTIARY: u32 = 0x383838;
    pub const DARK_BG_CARD: u32 = 0x2d2d2d;
    pub const DARK_BG_CARD_HOVER: u32 = 0x3d3d3d;
    pub const DARK_TEXT_PRIMARY: u32 = 0xffffff;
    pub const DARK_TEXT_SECONDARY: u32 = 0xc5c5c5;
    pub const DARK_TEXT_TERTIARY: u32 = 0x9e9e9e;
    pub const DARK_TEXT_DISABLED: u32 = 0x6e6e6e;
    pub const DARK_BORDER: u32 = 0x454545;
    pub const DARK_BORDER_SUBTLE: u32 = 0x3a3a3a;
    pub const DARK_ACRYLIC: u32 = 0x202020;
    pub const DARK_CARD: u32 = 0x2d2d2d;
    pub const DARK_TERTIARY: u32 = 0x383838;

    // Light
    pub const LIGHT_BG_PRIMARY: u32 = 0xf3f3f3;
    pub const LIGHT_BG_SECONDARY: u32 = 0xffffff;
    pub const LIGHT_BG_TERTIARY: u32 = 0xe5e5e5;
    pub const LIGHT_BG_CARD: u32 = 0xffffff;
    pub const LIGHT_BG_CARD_HOVER: u32 = 0xf5f5f5;
    pub const LIGHT_TEXT_PRIMARY: u32 = 0x1a1a1a;
    pub const LIGHT_TEXT_SECONDARY: u32 = 0x5c5c5c;
    pub const LIGHT_BORDER: u32 = 0xe5e5e5;
    pub const LIGHT_ACRYLIC: u32 = 0xf3f3f3;
    pub const LIGHT_CARD: u32 = 0xffffff;
    pub const LIGHT_TERTIARY: u32 = 0xe5e5e5;

    // Tailwind gray ramp
    pub const G50: u32 = 0xf9fafb;
    pub const G100: u32 = 0xf3f4f6;
    pub const G200: u32 = 0xe5e7eb;
    pub const G300: u32 = 0xd1d5db;
    pub const G400: u32 = 0x9ca3af;
    pub const G500: u32 = 0x6b7280;
    pub const G600: u32 = 0x4b5563;
    pub const G700: u32 = 0x374151;
    pub const G800: u32 = 0x1f2937;
    pub const G900: u32 = 0x111827;

    // Status tints
    pub const SUCCESS_BG_DARK: u32 = 0x6ccb5f26;
    pub const WARNING_BG_DARK: u32 = 0xfcb90026;
    pub const ERROR_BG_DARK: u32 = 0xff5f5f26;
    pub const GREEN50: u32 = 0xf0fdf4;
    pub const GREEN200: u32 = 0xbbf7d0;
    pub const GREEN500: u32 = 0x22c55e;
    pub const GREEN600: u32 = 0x16a34a;
    pub const GREEN700: u32 = 0x15803d;
    pub const AMBER50: u32 = 0xfffbeb;
    pub const AMBER200: u32 = 0xfde68a;
    pub const AMBER700: u32 = 0xb45309;
    pub const RED50: u32 = 0xfef2f2;
    pub const RED400: u32 = 0xf87171;
    pub const RED500: u32 = 0xef4444;
    pub const RED600: u32 = 0xdc2626;
    pub const YELLOW300: u32 = 0xfcd34d;
    pub const YELLOW400: u32 = 0xfbbf24;
    pub const YELLOW700: u32 = 0xa16207;
    pub const YELLOW800: u32 = 0x92400e;
    pub const SETTINGS_LIGHT_BG: u32 = 0xf0f3f9;
}

// --- Color helpers (pure; unit-tested) ---

fn alpha_byte(opacity: f32) -> u32 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u32
}

/// Same color with the given float alpha.
pub fn with_alpha(color: Rgba, alpha: f32) -> Rgba {
    Rgba {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}

/// Same color with the given 0-255 alpha byte (keeps `rgba(0xRRGGBBAA)` parity).
pub fn with_alpha_byte(color: Rgba, alpha: u8) -> Rgba {
    with_alpha(color, alpha as f32 / 255.0)
}

/// Linear interpolation between two colors (`t = 0` → `a`, `t = 1` → `b`).
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub fn lighten(color: Rgba, amount: f32) -> Rgba {
    mix(color, rgb(0xffffff), amount)
}

pub fn darken(color: Rgba, amount: f32) -> Rgba {
    mix(color, rgb(0x000000), amount)
}

fn srgb_to_linear(component: f32) -> f32 {
    if component <= 0.04045 {
        component / 12.92
    } else {
        ((component + 0.055) / 1.055).powf(2.4)
    }
}

/// Rec. 709 relative luminance (0 = black, 1 = white).
pub fn relative_luminance(color: Rgba) -> f32 {
    0.2126 * srgb_to_linear(color.r)
        + 0.7152 * srgb_to_linear(color.g)
        + 0.0722 * srgb_to_linear(color.b)
}

/// Readable text color on top of `background` (black on light, white on dark).
pub fn contrast_text(background: Rgba) -> Rgba {
    if relative_luminance(background) > 0.45 {
        rgb(0x000000)
    } else {
        rgb(0xffffff)
    }
}

// --- Desktop (GTK) palette ---

/// Every token the GTK mode can override, resolved from the active GTK theme.
/// Plain data (`Copy` + `Send`) so render paths never touch GTK themselves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GtkPalette {
    pub is_dark: bool,
    pub bg_primary: Rgba,
    pub bg_secondary: Rgba,
    pub bg_tertiary: Rgba,
    pub bg_card: Rgba,
    pub bg_card_hover: Rgba,
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_tertiary: Rgba,
    pub text_disabled: Rgba,
    pub border: Rgba,
    pub border_subtle: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub on_accent: Rgba,
    pub error: Rgba,
    pub warning: Rgba,
    pub success: Rgba,
    pub placeholder: Rgba,
    pub close_hover: Rgba,
    pub settings_light_bg: Rgba,
    /// Neutral ramp: index 0 = `gray::g50` … index 9 = `gray::g900`.
    pub gray: [Rgba; 10],
    pub green50: Rgba,
    pub green200: Rgba,
    pub amber50: Rgba,
    pub amber200: Rgba,
    pub red50: Rgba,
    pub success_bg: Rgba,
    pub warning_bg: Rgba,
    pub error_bg: Rgba,
}

static GTK: RwLock<Option<GtkPalette>> = RwLock::new(None);
/// Accent pushed by `theme_watch` from the XDG portal (`accent-color`), used as
/// the last resort when the GTK theme defines no accent color of its own.
static PORTAL_ACCENT: RwLock<Option<Rgba>> = RwLock::new(None);

/// Install the desktop palette; every override-aware token switches over.
pub fn install_gtk(palette: GtkPalette) {
    *GTK.write() = Some(palette);
}

/// Drop the desktop palette; tokens fall back to the Win11 literals.
pub fn clear_gtk() {
    *GTK.write() = None;
}

pub fn gtk_active() -> bool {
    GTK.read().is_some()
}

pub fn portal_accent() -> Option<Rgba> {
    *PORTAL_ACCENT.read()
}

pub fn set_portal_accent(accent: Option<Rgba>) {
    *PORTAL_ACCENT.write() = accent;
}

/// Win11 literal unless a desktop palette overrides the token.
fn ovr(default: Rgba, pick: impl FnOnce(&GtkPalette) -> Rgba) -> Rgba {
    match *GTK.read() {
        Some(palette) => pick(&palette),
        None => default,
    }
}

/// Opaque color from 0-255 components (portal `accent-color` tuples).
pub fn rgb8(r: u8, g: u8, b: u8) -> Rgba {
    rgb((r as u32) << 16 | (g as u32) << 8 | b as u32)
}

/// How the current theme was resolved (stored in `SharedConfig`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ResolvedTheme {
    pub is_dark: bool,
    /// True when the GTK palette is installed and driving the colors.
    pub is_gtk: bool,
}

impl ResolvedTheme {
    pub fn win11(is_dark: bool) -> Self {
        Self {
            is_dark,
            is_gtk: false,
        }
    }
}

/// Single theme resolver — installs/removes the GTK palette and reports the
/// resulting darkness. Must be called on the GTK main thread (the GTK probe
/// bails out everywhere else, so the call itself is always safe).
pub fn resolve(settings: &AppSettings) -> ResolvedTheme {
    match settings.theme_mode.as_str() {
        "dark" => {
            clear_gtk();
            ResolvedTheme::win11(true)
        }
        "light" => {
            clear_gtk();
            ResolvedTheme::win11(false)
        }
        "gtk" => match crate::gtk_theme::palette() {
            Some(palette) => {
                let is_dark = palette.is_dark;
                install_gtk(palette);
                ResolvedTheme {
                    is_dark,
                    is_gtk: true,
                }
            }
            // No palette this time: either GTK is unavailable, or this call came
            // from a non-main thread where the probe refuses to run. Keep an
            // already-installed palette instead of tearing it down mid-frame.
            None => match *GTK.read() {
                Some(existing) => ResolvedTheme {
                    is_dark: existing.is_dark,
                    is_gtk: true,
                },
                None => ResolvedTheme::win11(crate::settings::system_prefers_dark()),
            },
        },
        _ => {
            clear_gtk();
            ResolvedTheme::win11(crate::settings::system_prefers_dark())
        }
    }
}

/// Unconditional Win11 colors — Appearance preview swatches and tests must not
/// follow the active GTK palette.
pub mod raw {
    use super::hex;
    use gpui::{Rgba, rgb};

    pub mod light {
        use super::*;
        pub fn bg_primary() -> Rgba {
            rgb(hex::LIGHT_BG_PRIMARY)
        }
    }

    pub mod dark {
        use super::*;
        pub fn bg_primary() -> Rgba {
            rgb(hex::DARK_BG_PRIMARY)
        }
    }
}

// --- Top-level tokens ---

pub fn accent() -> Rgba {
    ovr(rgb(hex::ACCENT), |g| g.accent)
}
pub fn accent_hover() -> Rgba {
    ovr(rgb(hex::ACCENT_HOVER), |g| g.accent_hover)
}
/// Accent at a specific 0-255 alpha byte (selected-row highlight, card tint).
pub fn accent_alpha(alpha: u8) -> Rgba {
    ovr(with_alpha_byte(rgb(hex::ACCENT), alpha), |g| {
        with_alpha_byte(g.accent, alpha)
    })
}
pub fn error() -> Rgba {
    ovr(rgb(hex::ERROR), |g| g.error)
}
pub fn warning() -> Rgba {
    ovr(rgb(hex::WARNING), |g| g.warning)
}
pub fn success() -> Rgba {
    ovr(rgb(hex::SUCCESS), |g| g.success)
}
pub fn error_alpha(alpha: u8) -> Rgba {
    ovr(with_alpha_byte(rgb(hex::ERROR), alpha), |g| {
        with_alpha_byte(g.error, alpha)
    })
}
pub fn warning_alpha(alpha: u8) -> Rgba {
    ovr(with_alpha_byte(rgb(hex::WARNING), alpha), |g| {
        with_alpha_byte(g.warning, alpha)
    })
}
pub fn success_alpha(alpha: u8) -> Rgba {
    ovr(with_alpha_byte(rgb(hex::SUCCESS), alpha), |g| {
        with_alpha_byte(g.success, alpha)
    })
}
/// Text/icon color painted on top of an accent-filled surface.
pub fn on_accent() -> Rgba {
    ovr(rgb(hex::ON_ACCENT), |g| g.on_accent)
}
/// Fallback color for content without an explicit color (e.g. a text preview).
pub fn placeholder() -> Rgba {
    ovr(rgb(hex::PLACEHOLDER), |g| g.placeholder)
}
/// Windows 11 title-bar close button hover red.
pub fn close_hover() -> Rgba {
    ovr(rgb(hex::CLOSE_HOVER), |g| g.close_hover)
}

/// White/black with fractional alpha (Tailwind `white/5`, `black/20`, …).
///
/// In GTK mode "white" becomes the palette's foreground (so a light overlay
/// stays light on a dark GTK theme); `black_pct` deliberately stays black — it
/// is only used for shadows/overlays painted over content.
pub fn white_pct(pct: f32) -> Rgba {
    ovr(rgba(0xffffff00 | alpha_byte(pct)), |g| {
        with_alpha(g.text_primary, pct)
    })
}
pub fn black_pct(pct: f32) -> Rgba {
    rgba(0x00000000 | alpha_byte(pct))
}

/// Tertiary background dispatch (port of `getTertiaryBackgroundStyle`).
pub fn tertiary_bg(is_dark: bool, tertiary_opacity: f32) -> Rgba {
    if is_dark {
        dark::tertiary(tertiary_opacity)
    } else {
        light::tertiary(tertiary_opacity)
    }
}

/// Tailwind gray palette (used verbatim by the settings + wizard UI).
/// In GTK mode this becomes a neutral ramp between the palette background and
/// foreground: only the branch matching the palette's own scheme is rendered,
/// so each step keeps its intended contrast role.
pub mod gray {
    use super::*;
    macro_rules! gray_token {
        ($name:ident, $hex:ident, $index:expr) => {
            pub fn $name() -> Rgba {
                ovr(rgb(hex::$hex), |g| g.gray[$index])
            }
        };
    }
    gray_token!(g50, G50, 0);
    gray_token!(g100, G100, 1);
    gray_token!(g200, G200, 2);
    gray_token!(g300, G300, 3);
    gray_token!(g400, G400, 4);
    gray_token!(g500, G500, 5);
    gray_token!(g600, G600, 6);
    gray_token!(g700, G700, 7);
    gray_token!(g800, G800, 8);
    gray_token!(g900, G900, 9);
}

/// Settings-page surface tints.
pub mod tint {
    use super::*;
    macro_rules! tint_token {
        ($name:ident, $hex:ident, $field:ident) => {
            pub fn $name() -> Rgba {
                ovr(rgb(hex::$hex), |g| g.$field)
            }
        };
    }
    macro_rules! tint_alpha_token {
        ($name:ident, $hex:ident, $field:ident) => {
            pub fn $name() -> Rgba {
                ovr(rgba(hex::$hex), |g| g.$field)
            }
        };
    }
    tint_alpha_token!(success_bg_dark, SUCCESS_BG_DARK, success_bg);
    tint_alpha_token!(warning_bg_dark, WARNING_BG_DARK, warning_bg);
    tint_alpha_token!(error_bg_dark, ERROR_BG_DARK, error_bg);
    tint_token!(green50, GREEN50, green50);
    tint_token!(green200, GREEN200, green200);
    tint_token!(green500, GREEN500, success);
    tint_token!(green600, GREEN600, success);
    tint_token!(green700, GREEN700, success);
    tint_token!(amber50, AMBER50, amber50);
    tint_token!(amber200, AMBER200, amber200);
    tint_token!(amber700, AMBER700, warning);
    tint_token!(red50, RED50, red50);
    tint_token!(red400, RED400, error);
    tint_token!(red500, RED500, error);
    tint_token!(red600, RED600, error);
    /// Tailwind red-500 at a specific alpha byte (delete-hover washes).
    pub fn red500_alpha(alpha: u8) -> Rgba {
        ovr(with_alpha_byte(rgb(hex::RED500), alpha), |g| {
            with_alpha_byte(g.error, alpha)
        })
    }
    tint_token!(yellow300, YELLOW300, warning);
    tint_token!(yellow400, YELLOW400, warning);
    tint_token!(yellow700, YELLOW700, warning);
    tint_token!(yellow800, YELLOW800, warning);
    /// Settings light page background (custom `#f0f3f9` in React).
    pub fn settings_light_bg() -> Rgba {
        ovr(rgb(hex::SETTINGS_LIGHT_BG), |g| g.settings_light_bg)
    }
}

pub mod dark {
    use super::*;
    pub fn bg_primary() -> Rgba {
        ovr(rgb(hex::DARK_BG_PRIMARY), |g| g.bg_primary)
    }
    pub fn bg_secondary() -> Rgba {
        ovr(rgb(hex::DARK_BG_SECONDARY), |g| g.bg_secondary)
    }
    pub fn bg_tertiary() -> Rgba {
        ovr(rgb(hex::DARK_BG_TERTIARY), |g| g.bg_tertiary)
    }
    pub fn bg_card() -> Rgba {
        ovr(rgb(hex::DARK_BG_CARD), |g| g.bg_card)
    }
    pub fn bg_card_hover() -> Rgba {
        ovr(rgb(hex::DARK_BG_CARD_HOVER), |g| g.bg_card_hover)
    }
    pub fn text_primary() -> Rgba {
        ovr(rgb(hex::DARK_TEXT_PRIMARY), |g| g.text_primary)
    }
    pub fn text_secondary() -> Rgba {
        ovr(rgb(hex::DARK_TEXT_SECONDARY), |g| g.text_secondary)
    }
    pub fn text_tertiary() -> Rgba {
        ovr(rgb(hex::DARK_TEXT_TERTIARY), |g| g.text_tertiary)
    }
    pub fn text_disabled() -> Rgba {
        ovr(rgb(hex::DARK_TEXT_DISABLED), |g| g.text_disabled)
    }
    pub fn border() -> Rgba {
        ovr(rgb(hex::DARK_BORDER), |g| g.border)
    }
    pub fn border_subtle() -> Rgba {
        ovr(rgb(hex::DARK_BORDER_SUBTLE), |g| g.border_subtle)
    }
    /// Acrylic base at the configured window opacity (matches `--win11-*-bg-alpha`).
    pub fn acrylic(opacity: f32) -> Rgba {
        ovr(
            rgba(hex::DARK_ACRYLIC << 8 | alpha_byte(opacity)),
            |g| with_alpha(g.bg_primary, opacity),
        )
    }
    /// Card background (port of `getCardBackgroundStyle`, secondary opacity).
    pub fn card(secondary_opacity: f32) -> Rgba {
        ovr(
            rgba(hex::DARK_CARD << 8 | alpha_byte(secondary_opacity)),
            |g| with_alpha(g.bg_card, secondary_opacity),
        )
    }
    /// Tertiary background (port of `getTertiaryBackgroundStyle`).
    pub fn tertiary(tertiary_opacity: f32) -> Rgba {
        ovr(
            rgba(hex::DARK_TERTIARY << 8 | alpha_byte(tertiary_opacity)),
            |g| with_alpha(g.bg_tertiary, tertiary_opacity),
        )
    }
}

pub mod light {
    use super::*;
    pub fn bg_primary() -> Rgba {
        ovr(rgb(hex::LIGHT_BG_PRIMARY), |g| g.bg_primary)
    }
    pub fn bg_secondary() -> Rgba {
        ovr(rgb(hex::LIGHT_BG_SECONDARY), |g| g.bg_secondary)
    }
    pub fn bg_tertiary() -> Rgba {
        ovr(rgb(hex::LIGHT_BG_TERTIARY), |g| g.bg_tertiary)
    }
    pub fn bg_card() -> Rgba {
        ovr(rgb(hex::LIGHT_BG_CARD), |g| g.bg_card)
    }
    pub fn bg_card_hover() -> Rgba {
        ovr(rgb(hex::LIGHT_BG_CARD_HOVER), |g| g.bg_card_hover)
    }
    pub fn text_primary() -> Rgba {
        ovr(rgb(hex::LIGHT_TEXT_PRIMARY), |g| g.text_primary)
    }
    pub fn text_secondary() -> Rgba {
        ovr(rgb(hex::LIGHT_TEXT_SECONDARY), |g| g.text_secondary)
    }
    pub fn border() -> Rgba {
        ovr(rgb(hex::LIGHT_BORDER), |g| g.border)
    }
    pub fn acrylic(opacity: f32) -> Rgba {
        ovr(
            rgba(hex::LIGHT_ACRYLIC << 8 | alpha_byte(opacity)),
            |g| with_alpha(g.bg_primary, opacity),
        )
    }
    pub fn card(secondary_opacity: f32) -> Rgba {
        ovr(
            rgba(hex::LIGHT_CARD << 8 | alpha_byte(secondary_opacity)),
            |g| with_alpha(g.bg_card, secondary_opacity),
        )
    }
    pub fn tertiary(tertiary_opacity: f32) -> Rgba {
        ovr(
            rgba(hex::LIGHT_TERTIARY << 8 | alpha_byte(tertiary_opacity)),
            |g| with_alpha(g.bg_tertiary, tertiary_opacity),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::serial_lock;

    /// Minimal palette used to prove the override plumbing without GTK.
    fn fixture(is_dark: bool) -> GtkPalette {
        let bg = rgb(0x333333);
        let fg = rgb(0xdedede);
        GtkPalette {
            is_dark,
            bg_primary: bg,
            bg_secondary: rgb(0x242424),
            bg_tertiary: mix(bg, fg, 0.06),
            bg_card: rgb(0x242424),
            bg_card_hover: mix(bg, fg, 0.08),
            text_primary: fg,
            text_secondary: mix(bg, fg, 0.75),
            text_tertiary: mix(bg, fg, 0.55),
            text_disabled: mix(bg, fg, 0.35),
            border: rgb(0x2f3140),
            border_subtle: rgb(0x2a2b36),
            accent: rgb(0x0860f2),
            accent_hover: lighten(rgb(0x0860f2), 0.10),
            on_accent: rgb(0xffffff),
            error: rgb(0xed5f5d),
            warning: rgb(0xe9873a),
            success: rgb(0x79b757),
            placeholder: mix(bg, fg, 0.55),
            close_hover: darken(rgb(0xed5f5d), 0.10),
            settings_light_bg: bg,
            gray: [
                mix(bg, fg, 0.03),
                mix(bg, fg, 0.06),
                mix(bg, fg, 0.12),
                mix(bg, fg, 0.28),
                mix(bg, fg, 0.45),
                mix(bg, fg, 0.58),
                mix(bg, fg, 0.70),
                mix(bg, fg, 0.80),
                mix(bg, fg, 0.90),
                fg,
            ],
            green50: mix(bg, rgb(0x79b757), 0.10),
            green200: mix(bg, rgb(0x79b757), 0.30),
            amber50: mix(bg, rgb(0xe9873a), 0.10),
            amber200: mix(bg, rgb(0xe9873a), 0.30),
            red50: mix(bg, rgb(0xed5f5d), 0.10),
            success_bg: with_alpha_byte(rgb(0x79b757), 0x26),
            warning_bg: with_alpha_byte(rgb(0xe9873a), 0x26),
            error_bg: with_alpha_byte(rgb(0xed5f5d), 0x26),
        }
    }

    #[test]
    fn token_values_match_tailwind_config() {
        let _serial = serial_lock();
        clear_gtk();
        assert_eq!(dark::bg_primary(), rgb(0x202020));
        assert_eq!(dark::bg_card_hover(), rgb(0x3d3d3d));
        assert_eq!(accent(), rgb(0x0078d4));
        assert_eq!(light::bg_primary(), rgb(0xf3f3f3));
        assert_eq!(light::text_primary(), rgb(0x1a1a1a));
        assert_eq!(RADIUS_CARD, 8.0);
        assert_eq!(RADIUS_WINDOW, 12.0);
        assert_eq!(WINDOW_SHADOW_MARGIN, 16.0);
    }

    /// The painted shadow has to fit in the reserved margin, or its clipped edge
    /// shows up as a hard rectangle around the rounded window.
    #[test]
    fn window_shadow_fits_inside_its_margin() {
        let shadows = window_shadow();
        assert_eq!(shadows.len(), 1);
        let shadow = &shadows[0];
        let reach = f32::from(shadow.offset.y).abs()
            + f32::from(shadow.blur_radius)
            + f32::from(shadow.spread_radius).abs();
        assert!(
            reach <= WINDOW_SHADOW_MARGIN,
            "shadow reach {reach} exceeds margin {WINDOW_SHADOW_MARGIN}"
        );
    }

    /// Every Win11 token still returns its historical literal — this is the
    /// pixel-parity guard for System/Light/Dark.
    #[test]
    fn win11_tokens_preserve_historical_literals() {
        let _serial = serial_lock();
        clear_gtk();
        assert_eq!(accent(), rgb(hex::ACCENT));
        assert_eq!(accent_hover(), rgb(hex::ACCENT_HOVER));
        assert_eq!(error(), rgb(hex::ERROR));
        assert_eq!(warning(), rgb(hex::WARNING));
        assert_eq!(success(), rgb(hex::SUCCESS));
        assert_eq!(on_accent(), rgb(hex::ON_ACCENT));
        assert_eq!(placeholder(), rgb(hex::PLACEHOLDER));
        assert_eq!(close_hover(), rgb(hex::CLOSE_HOVER));
        assert_eq!(accent_alpha(0x4d), rgba(0x0078d44d));
        assert_eq!(warning_alpha(0x1a), rgba(0xfcb9001a));
        assert_eq!(error_alpha(0x10), rgba(0xff5f5f10));
        assert_eq!(success_alpha(0x33), rgba(0x6ccb5f33));
        assert_eq!(white_pct(0.10), rgba(0xffffff1a));
        assert_eq!(black_pct(0.20), rgba(0x00000033));

        assert_eq!(dark::bg_secondary(), rgb(hex::DARK_BG_SECONDARY));
        assert_eq!(dark::bg_tertiary(), rgb(hex::DARK_BG_TERTIARY));
        assert_eq!(dark::bg_card(), rgb(hex::DARK_BG_CARD));
        assert_eq!(dark::text_secondary(), rgb(hex::DARK_TEXT_SECONDARY));
        assert_eq!(dark::text_tertiary(), rgb(hex::DARK_TEXT_TERTIARY));
        assert_eq!(dark::text_disabled(), rgb(hex::DARK_TEXT_DISABLED));
        assert_eq!(dark::border(), rgb(hex::DARK_BORDER));
        assert_eq!(dark::border_subtle(), rgb(hex::DARK_BORDER_SUBTLE));

        assert_eq!(light::bg_secondary(), rgb(hex::LIGHT_BG_SECONDARY));
        assert_eq!(light::bg_tertiary(), rgb(hex::LIGHT_BG_TERTIARY));
        assert_eq!(light::bg_card(), rgb(hex::LIGHT_BG_CARD));
        assert_eq!(light::bg_card_hover(), rgb(hex::LIGHT_BG_CARD_HOVER));
        assert_eq!(light::text_secondary(), rgb(hex::LIGHT_TEXT_SECONDARY));
        assert_eq!(light::border(), rgb(hex::LIGHT_BORDER));

        assert_eq!(gray::g50(), rgb(hex::G50));
        assert_eq!(gray::g100(), rgb(hex::G100));
        assert_eq!(gray::g200(), rgb(hex::G200));
        assert_eq!(gray::g300(), rgb(hex::G300));
        assert_eq!(gray::g400(), rgb(hex::G400));
        assert_eq!(gray::g500(), rgb(hex::G500));
        assert_eq!(gray::g600(), rgb(hex::G600));
        assert_eq!(gray::g700(), rgb(hex::G700));
        assert_eq!(gray::g800(), rgb(hex::G800));
        assert_eq!(gray::g900(), rgb(hex::G900));

        assert_eq!(tint::green500(), rgb(hex::GREEN500));
        assert_eq!(tint::red500(), rgb(hex::RED500));
        assert_eq!(tint::red500_alpha(0x1a), rgba(0xef44441a));
        assert_eq!(tint::settings_light_bg(), rgb(hex::SETTINGS_LIGHT_BG));
        assert_eq!(tint::success_bg_dark(), rgba(hex::SUCCESS_BG_DARK));
        assert_eq!(tint::warning_bg_dark(), rgba(hex::WARNING_BG_DARK));
        assert_eq!(tint::error_bg_dark(), rgba(hex::ERROR_BG_DARK));
    }

    #[test]
    fn gtk_palette_overrides_both_halves_and_raw_stays_win11() {
        let _serial = serial_lock();
        let palette = fixture(true);
        install_gtk(palette);
        // Both module halves answer with the GTK values …
        assert_eq!(dark::bg_primary(), palette.bg_primary);
        assert_eq!(light::bg_primary(), palette.bg_primary);
        assert_eq!(dark::text_primary(), palette.text_primary);
        assert_eq!(light::text_secondary(), palette.text_secondary);
        assert_eq!(accent(), palette.accent);
        assert_eq!(on_accent(), palette.on_accent);
        assert_eq!(gray::g400(), palette.gray[4]);
        assert_eq!(tint::green50(), palette.green50);
        assert_eq!(white_pct(0.5), with_alpha(palette.text_primary, 0.5));
        assert_eq!(tertiary_bg(true, 1.0), palette.bg_tertiary);
        assert_eq!(tertiary_bg(false, 1.0), palette.bg_tertiary);
        // … while the Win11 previews/tests stay literal.
        assert_eq!(raw::dark::bg_primary(), rgb(hex::DARK_BG_PRIMARY));
        assert_eq!(raw::light::bg_primary(), rgb(hex::LIGHT_BG_PRIMARY));
        clear_gtk();
        assert_eq!(dark::bg_primary(), rgb(hex::DARK_BG_PRIMARY));
        assert!(!gtk_active());
    }

    #[test]
    fn acrylic_carries_window_opacity() {
        let _serial = serial_lock();
        clear_gtk();
        // Default 0.70 → alpha ≈ 0.70.
        let c = dark::acrylic(0.70);
        assert!((c.r - 0x20 as f32 / 255.0).abs() < 0.01);
        assert!((c.g - 0x20 as f32 / 255.0).abs() < 0.01);
        assert!((c.b - 0x20 as f32 / 255.0).abs() < 0.01);
        assert!((c.a - 0.70).abs() < 0.01);
        // Fully opaque → a == 1.0 (solid branch parity).
        assert!((dark::card(1.0).a - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn gtk_acrylic_uses_gtk_background_with_settings_alpha() {
        let _serial = serial_lock();
        let palette = fixture(true);
        install_gtk(palette);
        let c = dark::acrylic(0.42);
        assert_eq!(c.r, palette.bg_primary.r);
        assert!((c.a - 0.42).abs() < 1e-6);
        let t = light::tertiary(0.85);
        assert_eq!(t.r, palette.bg_tertiary.r);
        assert!((t.a - 0.85).abs() < 1e-6);
        clear_gtk();
    }

    #[test]
    fn color_helpers_are_sane() {
        assert_eq!(with_alpha_byte(rgb(0x123456), 0x80).a, 128.0 / 255.0);
        let mid = mix(rgb(0x000000), rgb(0xffffff), 0.5);
        assert!((mid.r - 0.5).abs() < 1e-6 && (mid.b - 0.5).abs() < 1e-6);
        assert_eq!(mix(rgb(0x000000), rgb(0xffffff), 0.0), rgb(0x000000));
        assert_eq!(mix(rgb(0x000000), rgb(0xffffff), 1.0), rgb(0xffffff));
        assert_eq!(lighten(rgb(0x000000), 1.0), rgb(0xffffff));
        assert_eq!(darken(rgb(0xffffff), 1.0), rgb(0x000000));
        assert!(relative_luminance(rgb(0xffffff)) > 0.99);
        assert!(relative_luminance(rgb(0x000000)) < 0.01);
        assert_eq!(contrast_text(rgb(0xffffff)), rgb(0x000000));
        assert_eq!(contrast_text(rgb(0x0860f2)), rgb(0xffffff));
        assert_eq!(contrast_text(rgb(0xfcd34d)), rgb(0x000000));
    }

    #[test]
    fn resolved_theme_is_win11_by_default() {
        let resolved = ResolvedTheme::default();
        assert!(!resolved.is_dark);
        assert!(!resolved.is_gtk);
    }
}
