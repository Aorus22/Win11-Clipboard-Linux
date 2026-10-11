//! The Windows 11 look of the clipboard popup: palette and widget styles.
//!
//! Values are the design tokens of the GPUI build (`gpui-app/src/theme.rs`, itself a
//! port of the React/Tailwind app), so the popup looks the same in both: acrylic
//! surface at the user's opacity, 8 px cards, 6 px buttons, accent `#0078d4`.
//! Settings and the wizard keep the COSMIC look.

use cosmic::iced::widget::container;
use cosmic::iced::{border::Radius, Background, Border, Color, Length};
use cosmic::widget::{self, button};
use cosmic::{theme, Element};

use crate::scale;
use crate::widgets::{self, Ic};

pub const ACCENT: u32 = 0x0078d4;
pub const ACCENT_HOVER: u32 = 0x1a86d9;
pub const ON_ACCENT: u32 = 0xffffff;
pub const ERROR: u32 = 0xff5f5f;
pub const CLOSE_HOVER: u32 = 0xc42b1c;

pub fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn rgba(hex: u32, alpha: f32) -> Color {
    Color { a: alpha.clamp(0.0, 1.0), ..rgb(hex) }
}

pub const CLEAR: Color = Color::TRANSPARENT;

/// The tokens of one theme at the user's opacity.
#[derive(Clone, Copy, Debug)]
pub struct Pal {
    pub dark: bool,
    pub acrylic: Color,
    /// Card background (secondary opacity).
    pub card: Color,
    /// Tertiary fill (tabs, pills, search): tertiary opacity.
    pub tertiary: Color,
    /// Tertiary fill at the secondary opacity (the search bar).
    pub tertiary_soft: Color,
    pub card_hover: Color,
    pub text: Color,
    pub text2: Color,
    pub text3: Color,
    pub disabled: Color,
    pub border: Color,
    pub border_subtle: Color,
    /// Opaque tertiary (empty-state circle).
    pub bg_tertiary: Color,
}

impl Pal {
    pub fn new(dark: bool, base_opacity: f32) -> Self {
        let secondary = (base_opacity + 0.3).min(1.0);
        let tertiary = (secondary + 0.3).min(1.0);
        if dark {
            Self {
                dark,
                acrylic: rgba(0x202020, base_opacity),
                card: rgba(0x2d2d2d, secondary),
                tertiary: rgba(0x383838, tertiary),
                tertiary_soft: rgba(0x383838, secondary),
                card_hover: rgb(0x3d3d3d),
                text: rgb(0xffffff),
                text2: rgb(0xc5c5c5),
                text3: rgb(0x9e9e9e),
                disabled: rgb(0x6e6e6e),
                border: rgb(0x454545),
                border_subtle: rgb(0x3a3a3a),
                bg_tertiary: rgb(0x383838),
            }
        } else {
            Self {
                dark,
                acrylic: rgba(0xf3f3f3, base_opacity),
                card: rgba(0xffffff, secondary),
                tertiary: rgba(0xe5e5e5, tertiary),
                tertiary_soft: rgba(0xe5e5e5, secondary),
                card_hover: rgb(0xf5f5f5),
                text: rgb(0x1a1a1a),
                text2: rgb(0x5c5c5c),
                text3: rgb(0x5c5c5c),
                disabled: rgb(0x6e6e6e),
                border: rgb(0xe5e5e5),
                border_subtle: rgb(0xe5e5e5),
                bg_tertiary: rgb(0xe5e5e5),
            }
        }
    }

    /// Every colour at `t` (0..1) of its opacity: the appear animation.
    pub fn faded(mut self, t: f32) -> Self {
        let f = |c: Color| Color { a: c.a * t.clamp(0.0, 1.0), ..c };
        self.acrylic = f(self.acrylic);
        self.card = f(self.card);
        self.tertiary = f(self.tertiary);
        self.tertiary_soft = f(self.tertiary_soft);
        self.card_hover = f(self.card_hover);
        self.text = f(self.text);
        self.text2 = f(self.text2);
        self.text3 = f(self.text3);
        self.disabled = f(self.disabled);
        self.border = f(self.border);
        self.border_subtle = f(self.border_subtle);
        self.bg_tertiary = f(self.bg_tertiary);
        self
    }

    pub fn accent(&self) -> Color {
        rgb(ACCENT)
    }

    /// White / black at a fraction, like Tailwind's `white/20`, `black/20`.
    pub fn wash(&self, pct: f32) -> Color {
        if self.dark { rgba(0xffffff, pct) } else { rgba(0x000000, pct) }
    }
}

// --- Text ---

fn text<'a>(content: impl Into<std::borrow::Cow<'a, str>>, size: f32, color: Color, semibold: bool) -> widget::Text<'a, cosmic::Theme, cosmic::Renderer> {
    let t = if semibold { scale::heading(content) } else { scale::body(content) };
    let s = scale::get();
    t.size(cosmic::iced::Pixels(size * s))
        .line_height(cosmic::iced::widget::text::LineHeight::Absolute(cosmic::iced::Pixels(size * 1.45 * s)))
        .class(theme::Text::Color(color))
}

/// 12.25 px body text.
pub fn body<'a>(content: impl Into<std::borrow::Cow<'a, str>>, color: Color) -> widget::Text<'a, cosmic::Theme, cosmic::Renderer> {
    text(content, 12.25, color, false)
}

/// 12.25 px semibold.
pub fn strong<'a>(content: impl Into<std::borrow::Cow<'a, str>>, color: Color) -> widget::Text<'a, cosmic::Theme, cosmic::Renderer> {
    text(content, 12.25, color, true)
}

/// 10.5 px small text.
pub fn small<'a>(content: impl Into<std::borrow::Cow<'a, str>>, color: Color) -> widget::Text<'a, cosmic::Theme, cosmic::Renderer> {
    text(content, 10.5, color, false)
}

/// 14 px title.
pub fn title<'a>(content: impl Into<std::borrow::Cow<'a, str>>, color: Color) -> widget::Text<'a, cosmic::Theme, cosmic::Renderer> {
    text(content, 14.0, color, true)
}

/// An icon at `size` layout px in `color`.
pub fn icon<'a, M: 'a>(ic: Ic, size: u16, color: Color) -> Element<'a, M> {
    widget::container(widgets::ic(ic, size))
        .class(theme::Container::custom(move |_: &cosmic::Theme| container::Style {
            icon_color: Some(color),
            text_color: Some(color),
            ..Default::default()
        }))
        .into()
}

// --- Containers ---

fn radius(r: f32) -> Radius {
    Radius::from(r * scale::get())
}

/// The popup surface: acrylic at the user's opacity, 12 px corners, hairline border.
pub fn surface<'a>(p: Pal) -> theme::Container<'a> {
    let r = radius(12.0);
    theme::Container::custom(move |_: &cosmic::Theme| container::Style {
        icon_color: Some(p.text),
        text_color: Some(p.text),
        background: Some(Background::Color(p.acrylic)),
        border: Border { radius: r, width: 1.0, color: p.border },
        ..Default::default()
    })
}

/// A filled rounded box (icon boxes, pills, circles).
pub fn fill<'a>(bg: Color, r: f32, fg: Color) -> theme::Container<'a> {
    let r = radius(r);
    theme::Container::custom(move |_: &cosmic::Theme| container::Style {
        icon_color: Some(fg),
        text_color: Some(fg),
        background: Some(Background::Color(bg)),
        border: Border { radius: r, ..Default::default() },
        ..Default::default()
    })
}

/// A history card: card fill, 1 px border (accent when focused or pinned).
pub fn card<'a>(p: Pal, hovered: bool, accented: bool) -> theme::Container<'a> {
    let r = radius(8.0);
    theme::Container::custom(move |_: &cosmic::Theme| container::Style {
        icon_color: Some(p.text),
        text_color: Some(p.text),
        background: Some(Background::Color(if hovered { p.card_hover } else { p.card })),
        border: Border { radius: r, width: 1.0, color: if accented { rgb(ACCENT) } else { p.border_subtle } },
        ..Default::default()
    })
}

/// A 1 px rule.
pub fn rule<'a, M: 'a>(color: Color) -> Element<'a, M> {
    widget::container(widget::space().width(Length::Fill).height(1))
        .class(theme::Container::custom(move |_: &cosmic::Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        }))
        .width(Length::Fill)
        .height(1)
        .into()
}

// --- Buttons ---

/// How a flat button looks idle and under the pointer.
#[derive(Clone, Copy)]
pub struct Flat {
    pub bg: Color,
    pub hover_bg: Color,
    pub fg: Color,
    pub hover_fg: Color,
    pub radius: f32,
    /// Focus / selection ring (width px, colour), if any.
    pub ring: Option<(f32, Color)>,
}

impl Flat {
    pub fn new(fg: Color, hover_bg: Color) -> Self {
        Self { bg: CLEAR, hover_bg, fg, hover_fg: fg, radius: 6.0, ring: None }
    }

    pub fn bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    pub fn hover_fg(mut self, fg: Color) -> Self {
        self.hover_fg = fg;
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    pub fn ring(mut self, width: f32, color: Color) -> Self {
        self.ring = Some((width, color));
        self
    }

    pub fn class(self) -> theme::Button {
        let r = radius(self.radius);
        let style = move |bg: Color, fg: Color| {
            let mut s = button::Style::new();
            s.background = Some(Background::Color(bg));
            s.border_radius = r;
            if let Some((w, c)) = self.ring {
                s.border_width = w;
                s.border_color = c;
            }
            s.icon_color = Some(fg);
            s.text_color = Some(fg);
            s
        };
        let idle = style(self.bg, self.fg);
        let hover = style(self.hover_bg, self.hover_fg);
        let disabled = idle.clone();
        let pressed = hover.clone();
        theme::Button::Custom {
            active: Box::new(move |_, _| idle.clone()),
            disabled: Box::new(move |_| disabled.clone()),
            hovered: Box::new(move |_, _| hover.clone()),
            pressed: Box::new(move |_, _| pressed.clone()),
        }
    }
}

/// A flat button around `content`, `pad` px (v, h) of padding.
pub fn flat<'a, M: Clone + 'static>(
    content: impl Into<Element<'a, M>>,
    pad: (u16, u16),
    style: Flat,
    message: Option<M>,
) -> Element<'a, M> {
    widget::button::custom(content)
        .padding(scale::pad2(pad.0, pad.1))
        .class(style.class())
        .on_press_maybe(message)
        .into()
}

/// The search field: 36 px tall pill-ish box on the tertiary fill, accent ring while focused.
pub fn search_input<'a, M: Clone + 'static>(p: Pal, placeholder: &str, value: &str) -> widget::TextInput<'a, M> {
    use cosmic::widget::text_input::Appearance;
    let r = radius(8.0);
    let ring = 2.0 * scale::get();
    let look = move |focused: bool| Appearance {
        background: Background::Color(p.tertiary_soft),
        border_radius: r,
        border_offset: None,
        border_width: ring,
        border_color: if focused { rgb(ACCENT) } else { CLEAR },
        label_color: p.text,
        placeholder_color: p.disabled,
        selected_text_color: rgb(0xffffff),
        icon_color: Some(p.disabled),
        text_color: Some(p.text),
        selected_fill: rgba(ACCENT, 0.30),
    };
    widget::text_input(placeholder.to_string(), value.to_string())
        .padding(scale::pad2(9, 12))
        .size(12.25 * scale::get())
        .leading_icon(widgets::ic::<M>(Ic::Search, 16))
        .style(theme::TextInput::Custom {
            active: Box::new(move |_| look(false)),
            error: Box::new(move |_| look(false)),
            hovered: Box::new(move |_| look(false)),
            focused: Box::new(move |_| look(true)),
            disabled: Box::new(move |_| look(false)),
        })
}
