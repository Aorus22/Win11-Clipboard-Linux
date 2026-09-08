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
