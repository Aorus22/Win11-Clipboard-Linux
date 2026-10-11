//! Compositor blur behind the popup card, following the COSMIC setting.
//!
//! COSMIC's Appearance → "frosted glass" (`cosmic.frosted_windows`) decides whether
//! windows get a blurred backdrop, and the other iced app of this setup
//! (`iced-wayland-widget`) follows it the same way: libcosmic's automatic blur is
//! off (it would cover the whole overlay surface) and the card asks for its own,
//! rounded region. `WIN11_CLIPBOARD_BLUR=1|0` forces it on or off.

use cosmic::iced::{Point, Rectangle, Size};

/// Whether the backdrop should be blurred right now.
pub fn wanted() -> bool {
    match std::env::var("WIN11_CLIPBOARD_BLUR").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => cosmic::theme::active().cosmic().frosted_windows,
    }
}

/// Strips approximating a rounded rectangle, in surface-local coordinates, for the
/// compositor's blur region (same approach as the GPUI build's and the widgets').
pub fn rounded_regions(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Vec<Rectangle> {
    let rect = |x: f32, y: f32, w: f32, h: f32| Rectangle::new(Point::new(x, y), Size::new(w, h));
    if width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let radius = radius.max(0.0).min(width / 2.0).min(height / 2.0);
    if radius == 0.0 {
        return vec![rect(x, y, width, height)];
    }
    let segments = radius.ceil().clamp(1.0, 10.0) as usize;
    let band = radius / segments as f32;
    let mut regions = Vec::with_capacity(segments * 2 + 1);
    for segment in 0..segments {
        let offset = segment as f32 * band;
        let from_corner = radius - offset;
        let inset = radius - (radius * radius - from_corner * from_corner).max(0.0).sqrt();
        let strip = (width - inset * 2.0).max(0.0);
        for strip_y in [y + offset, y + height - offset - band] {
            regions.push(rect(x + inset, strip_y, strip, band));
        }
    }
    let middle = height - radius * 2.0;
    if middle > 0.0 {
        regions.push(rect(x, y + radius, width, middle));
    }
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_corners_give_one_region() {
        assert_eq!(rounded_regions(0.0, 0.0, 100.0, 50.0, 0.0).len(), 1);
    }

    #[test]
    fn rounded_regions_stay_inside_the_card() {
        for r in rounded_regions(10.0, 20.0, 360.0, 480.0, 12.0) {
            assert!(r.x >= 10.0 && r.x + r.width <= 370.01);
            assert!(r.y >= 20.0 && r.y + r.height <= 500.01);
        }
    }
}
