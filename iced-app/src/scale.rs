//! The UI scale of the clipboard popup (Settings → UI scale).
//!
//! libcosmic keeps a scale factor in its core but never applies it to rendering, so
//! the popup scales itself: every size, spacing, padding and font of its views goes
//! through these helpers, and the popup window is created at `base size × scale`.

use std::borrow::Cow;
use std::sync::atomic::{AtomicU32, Ordering};

use cosmic::iced::widget::text::Text;
use cosmic::iced::{Padding, Pixels};
use cosmic::iced::widget::text::LineHeight;
use cosmic::Renderer;

static SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0

pub fn set(value: f32) {
    SCALE.store(value.clamp(0.5, 2.0).to_bits(), Ordering::Relaxed);
}

pub fn get() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

/// A length in layout px, scaled.
pub fn px(value: f32) -> f32 {
    value * get()
}

/// A small integer length (spacing, padding, icon size), scaled and rounded.
pub fn u(value: u16) -> u16 {
    (f32::from(value) * get()).round().max(1.0) as u16
}

pub fn pad2(vertical: u16, horizontal: u16) -> Padding {
    Padding::from([f32::from(vertical) * get(), f32::from(horizontal) * get()])
}

pub fn pad4(top: u16, right: u16, bottom: u16, left: u16) -> Padding {
    let s = get();
    Padding {
        top: f32::from(top) * s,
        right: f32::from(right) * s,
        bottom: f32::from(bottom) * s,
        left: f32::from(left) * s,
    }
}

fn text<'a>(content: Cow<'a, str>, size: f32, line: f32, semibold: bool) -> Text<'a, cosmic::Theme, Renderer> {
    let s = get();
    let t = Text::new(content)
        .size(Pixels(size * s))
        .line_height(LineHeight::Absolute(Pixels(line * s)));
    if semibold {
        t.font(cosmic::font::semibold())
    } else {
        t.font(cosmic::font::default())
    }
}

pub fn body<'a>(content: impl Into<Cow<'a, str>>) -> Text<'a, cosmic::Theme, Renderer> {
    text(content.into(), 14.0, 20.0, false)
}

pub fn heading<'a>(content: impl Into<Cow<'a, str>>) -> Text<'a, cosmic::Theme, Renderer> {
    text(content.into(), 14.0, 20.0, true)
}

pub fn caption<'a>(content: impl Into<Cow<'a, str>>) -> Text<'a, cosmic::Theme, Renderer> {
    text(content.into(), 10.0, 14.0, false)
}

pub fn caption_heading<'a>(content: impl Into<Cow<'a, str>>) -> Text<'a, cosmic::Theme, Renderer> {
    text(content.into(), 10.0, 14.0, true)
}

pub fn title3<'a>(content: impl Into<Cow<'a, str>>) -> Text<'a, cosmic::Theme, Renderer> {
    text(content.into(), 20.0, 30.0, true)
}

pub fn title4<'a>(content: impl Into<Cow<'a, str>>) -> Text<'a, cosmic::Theme, Renderer> {
    text(content.into(), 17.0, 25.0, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_rounds_and_clamps() {
        set(1.5);
        assert_eq!(u(8), 12);
        assert!((px(40.0) - 60.0).abs() < 1e-4);
        set(9.0);
        assert!((get() - 2.0).abs() < 1e-6);
        set(1.0);
        assert_eq!(u(1), 1);
    }
}
