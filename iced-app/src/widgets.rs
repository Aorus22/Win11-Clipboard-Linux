//! Small shared view helpers: Lucide icons, the translucent card style.

use cosmic::iced::{Background, Border, Color, Length};
use cosmic::widget::{self, icon};
use cosmic::{theme, Element};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ic {
    Check,
    ChevronDown,
    ChevronRight,
    ClipboardList,
    History,
    Image,
    LayoutList,
    Link,
    Mail,
    Omega,
    Pin,
    PinFilled,
    Regex,
    Search,
    Smile,
    Trash,
    Type,
    X,
}

macro_rules! svg {
    ($name:literal) => {
        include_bytes!(concat!("../../gpui-app/assets/icons/", $name, ".svg")).as_slice()
    };
}

fn bytes(ic: Ic) -> &'static [u8] {
    match ic {
        Ic::Check => svg!("check"),
        Ic::ChevronDown => svg!("chevron-down"),
        Ic::ChevronRight => svg!("chevron-right"),
        Ic::ClipboardList => svg!("clipboard-list"),
        Ic::History => svg!("history"),
        Ic::Image => svg!("image"),
        Ic::LayoutList => svg!("layout-list"),
        Ic::Link => svg!("link"),
        Ic::Mail => svg!("mail"),
        Ic::Omega => svg!("omega"),
        Ic::Pin => svg!("pin"),
        Ic::PinFilled => svg!("pin-filled"),
        Ic::Regex => svg!("regex"),
        Ic::Search => svg!("search"),
        Ic::Smile => svg!("smile"),
        Ic::Trash => svg!("trash"),
        Ic::Type => svg!("type"),
        Ic::X => svg!("x"),
    }
}

pub fn handle(ic: Ic) -> icon::Handle {
    icon::from_svg_bytes(bytes(ic)).symbolic(true)
}

/// An icon at `size` layout px (scaled by the UI scale).
pub fn ic<'a, M: 'a>(ic: Ic, size: u16) -> Element<'a, M> {
    icon::icon(handle(ic)).size(crate::scale::u(size)).into()
}

/// A small icon-only button.
pub fn icon_button<'a, M: Clone + 'static>(ic_: Ic, message: M) -> Element<'a, M> {
    widget::button::custom(ic(ic_, 16))
        .padding(crate::scale::u(6))
        .class(theme::Button::Icon)
        .on_press(message)
        .into()
}

/// An icon button that can show a selected state (tabs, toggles).
pub fn toggle_button<'a, M: Clone + 'static>(ic_: Ic, selected: bool, message: M) -> Element<'a, M> {
    widget::button::custom(ic(ic_, 16))
        .padding(crate::scale::u(6))
        .class(theme::Button::Icon)
        .selected(selected)
        .on_press(message)
        .into()
}

/// The popup / window surface: the COSMIC window colour at the user's opacity,
/// rounded, with a hairline border.
pub fn surface<'a>(opacity: f32) -> theme::Container<'a> {
    theme::Container::custom(move |theme: &cosmic::Theme| {
        let cosmic = theme.cosmic();
        let base: Color = cosmic.background(false).base.into();
        let on: Color = cosmic.background(false).on.into();
        cosmic::iced::widget::container::Style {
            icon_color: Some(on),
            text_color: Some(on),
            background: Some(Background::Color(Color { a: opacity.clamp(0.0, 1.0), ..base })),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color { a: 0.25, ..on },
            },
            ..Default::default()
        }
    })
}

/// A history / picker card, one step lighter than the surface.
pub fn card<'a>(selected: bool, pinned: bool) -> theme::Container<'a> {
    theme::Container::custom(move |theme: &cosmic::Theme| {
        let cosmic = theme.cosmic();
        let base: Color = cosmic.primary(false).component.base.into();
        let accent: Color = cosmic.accent_color().into();
        let on: Color = cosmic.background(false).on.into();
        cosmic::iced::widget::container::Style {
            icon_color: Some(on),
            text_color: Some(on),
            background: Some(Background::Color(base)),
            border: Border {
                radius: 8.0.into(),
                width: if selected || pinned { 1.5 } else { 1.0 },
                color: if selected || pinned { accent } else { Color { a: 0.18, ..on } },
            },
            ..Default::default()
        }
    })
}

pub fn spacer_h(height: f32) -> Element<'static, ()> {
    widget::space().height(Length::Fixed(height)).into()
}

/// A text button whose label and padding follow the UI scale.
pub fn text_button<'a, M: Clone + 'static>(
    label: impl Into<std::borrow::Cow<'a, str>>,
    class: theme::Button,
    message: Option<M>,
) -> Element<'a, M> {
    widget::button::custom(crate::scale::body(label))
        .padding(crate::scale::pad2(6, 12))
        .class(class)
        .on_press_maybe(message)
        .into()
}
