//! Windows 11 design tokens — port of `tailwind.config.js` + `themeUtils.ts`.
//!
//! All colors are sRGB 0-255; alpha comes from the settings opacity values so the
//! "same token, same pixel" contract with the React build holds by construction.
//!
//! Note: `gpui::rgb`/`rgba` are not `const fn`, so tokens are plain functions.

use gpui::{Rgba, rgb, rgba};

pub const RADIUS_CARD: f32 = 8.0;
pub const RADIUS_WINDOW: f32 = 12.0;
pub fn accent() -> Rgba {
    rgb(0x0078d4)
}
pub fn accent_hover() -> Rgba {
    rgb(0x1a86d9)
}
pub fn error() -> Rgba {
    rgb(0xff5f5f)
}

fn alpha_byte(opacity: f32) -> u32 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u32
}

/// White/black with fractional alpha (Tailwind `white/5`, `black/20`, …).
pub fn white_pct(pct: f32) -> Rgba {
    rgba(0xffffff00 | alpha_byte(pct))
}
pub fn black_pct(pct: f32) -> Rgba {
    rgba(0x00000000 | alpha_byte(pct))
}

/// Tailwind gray palette (used verbatim by the settings + wizard UI).
pub mod gray {
    use super::*;
    pub fn g50() -> Rgba {
        rgb(0xf9fafb)
    }
    pub fn g100() -> Rgba {
        rgb(0xf3f4f6)
    }
    pub fn g200() -> Rgba {
        rgb(0xe5e7eb)
    }
    pub fn g300() -> Rgba {
        rgb(0xd1d5db)
    }
    pub fn g400() -> Rgba {
        rgb(0x9ca3af)
    }
    pub fn g500() -> Rgba {
        rgb(0x6b7280)
    }
    pub fn g600() -> Rgba {
        rgb(0x4b5563)
    }
    pub fn g700() -> Rgba {
        rgb(0x374151)
    }
    pub fn g800() -> Rgba {
        rgb(0x1f2937)
    }
    pub fn g900() -> Rgba {
        rgb(0x111827)
    }
}

/// Settings-page surface tints.
pub mod tint {
    use super::*;
    pub fn success_bg_dark() -> Rgba {
        rgba(0x6ccb5f26)
    }
    pub fn warning_bg_dark() -> Rgba {
        rgba(0xfcb90026)
    }
    pub fn error_bg_dark() -> Rgba {
        rgba(0xff5f5f26)
    }
    pub fn green50() -> Rgba {
        rgb(0xf0fdf4)
    }
    pub fn green200() -> Rgba {
        rgb(0xbbf7d0)
    }
    pub fn green600() -> Rgba {
        rgb(0x16a34a)
    }
    pub fn green700() -> Rgba {
        rgb(0x15803d)
    }
    pub fn amber50() -> Rgba {
        rgb(0xfffbeb)
    }
    pub fn amber200() -> Rgba {
        rgb(0xfde68a)
    }
    pub fn amber700() -> Rgba {
        rgb(0xb45309)
    }
    pub fn red50() -> Rgba {
        rgb(0xfef2f2)
    }
    pub fn red400() -> Rgba {
        rgb(0xf87171)
    }
    pub fn red500() -> Rgba {
        rgb(0xef4444)
    }
    pub fn red600() -> Rgba {
        rgb(0xdc2626)
    }
    pub fn yellow300() -> Rgba {
        rgb(0xfcd34d)
    }
    pub fn yellow400() -> Rgba {
        rgb(0xfbbf24)
    }
    pub fn yellow700() -> Rgba {
        rgb(0xa16207)
    }
    pub fn yellow800() -> Rgba {
        rgb(0x92400e)
    }
    /// Settings light page background (custom `#f0f3f9` in React).
    pub fn settings_light_bg() -> Rgba {
        rgb(0xf0f3f9)
    }
}

/// Tertiary background dispatch (port of `getTertiaryBackgroundStyle`).
pub fn tertiary_bg(is_dark: bool, tertiary_opacity: f32) -> Rgba {
    if is_dark {
        dark::tertiary(tertiary_opacity)
    } else {
        light::tertiary(tertiary_opacity)
    }
}

pub mod dark {
    use super::*;
    pub fn bg_primary() -> Rgba {
        rgb(0x202020)
    }
    pub fn bg_secondary() -> Rgba {
        rgb(0x2d2d2d)
    }
    pub fn bg_tertiary() -> Rgba {
        rgb(0x383838)
    }
    pub fn bg_card() -> Rgba {
        rgb(0x2d2d2d)
    }
    pub fn bg_card_hover() -> Rgba {
        rgb(0x3d3d3d)
    }
    pub fn text_primary() -> Rgba {
        rgb(0xffffff)
    }
    pub fn text_secondary() -> Rgba {
        rgb(0xc5c5c5)
    }
    pub fn text_tertiary() -> Rgba {
        rgb(0x9e9e9e)
    }
    pub fn text_disabled() -> Rgba {
        rgb(0x6e6e6e)
    }
    pub fn border() -> Rgba {
        rgb(0x454545)
    }
    pub fn border_subtle() -> Rgba {
        rgb(0x3a3a3a)
    }
    /// Acrylic base at the configured window opacity (matches `--win11-*-bg-alpha`).
    pub fn acrylic(opacity: f32) -> Rgba {
        rgba(0x20202000 | alpha_byte(opacity))
    }
    /// Card background (port of `getCardBackgroundStyle`, secondary opacity).
    pub fn card(secondary_opacity: f32) -> Rgba {
        rgba(0x2d2d2d00 | alpha_byte(secondary_opacity))
    }
    /// Tertiary background (port of `getTertiaryBackgroundStyle`).
    pub fn tertiary(tertiary_opacity: f32) -> Rgba {
        rgba(0x38383800 | alpha_byte(tertiary_opacity))
    }
}

pub mod light {
    use super::*;
    pub fn bg_primary() -> Rgba {
        rgb(0xf3f3f3)
    }
    pub fn bg_secondary() -> Rgba {
        rgb(0xffffff)
    }
    pub fn bg_tertiary() -> Rgba {
        rgb(0xe5e5e5)
    }
    pub fn bg_card() -> Rgba {
        rgb(0xffffff)
    }
    pub fn bg_card_hover() -> Rgba {
        rgb(0xf5f5f5)
    }
    pub fn text_primary() -> Rgba {
        rgb(0x1a1a1a)
    }
    pub fn text_secondary() -> Rgba {
        rgb(0x5c5c5c)
    }
    pub fn border() -> Rgba {
        rgb(0xe5e5e5)
    }
    pub fn acrylic(opacity: f32) -> Rgba {
        rgba(0xf3f3f300 | alpha_byte(opacity))
    }
    pub fn card(secondary_opacity: f32) -> Rgba {
        rgba(0xffffff00 | alpha_byte(secondary_opacity))
    }
    pub fn tertiary(tertiary_opacity: f32) -> Rgba {
        rgba(0xe5e5e500 | alpha_byte(tertiary_opacity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_values_match_tailwind_config() {
        assert_eq!(dark::bg_primary(), rgb(0x202020));
        assert_eq!(dark::bg_card_hover(), rgb(0x3d3d3d));
        assert_eq!(accent(), rgb(0x0078d4));
        assert_eq!(light::bg_primary(), rgb(0xf3f3f3));
        assert_eq!(light::text_primary(), rgb(0x1a1a1a));
        assert_eq!(RADIUS_CARD, 8.0);
        assert_eq!(RADIUS_WINDOW, 12.0);
    }

    #[test]
    fn acrylic_carries_window_opacity() {
        // Default 0.70 → alpha ≈ 0.70.
        let c = dark::acrylic(0.70);
        assert!((c.r - 0x20 as f32 / 255.0).abs() < 0.01);
        assert!((c.g - 0x20 as f32 / 255.0).abs() < 0.01);
        assert!((c.b - 0x20 as f32 / 255.0).abs() < 0.01);
        assert!((c.a - 0.70).abs() < 0.01);
        // Fully opaque → a == 1.0 (solid branch parity).
        assert!((dark::card(1.0).a - 1.0).abs() < f32::EPSILON);
    }
}
