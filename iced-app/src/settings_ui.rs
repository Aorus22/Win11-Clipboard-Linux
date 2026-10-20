//! The Settings window: libcosmic settings widgets, so it looks like any COSMIC app.
//!
//! Sections: Appearance, Auto-delete, Window transparency, UI scale, History,
//! Custom kaomoji, Features, Keyboard shortcuts, Reset. Changes apply live;
//! sliders persist on release.

use app_shared::settings::{AppSettings, CustomKaomoji};
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings};
use cosmic::{theme, Element};
use win11_clipboard_history_lib::rendering_env;

use crate::widgets::{ic, Ic};

const UNITS: [&str; 4] = ["minutes", "hours", "days", "weeks"];
const THEMES: [(&str, &str); 3] = [("system", "System"), ("light", "Light"), ("dark", "Dark")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    SmartActions,
    Autostart,
    DynamicTray,
    CloseOnFocusLoss,
    AllowTransparency,
}

pub struct State {
    autodelete: String,
    max_history: String,
    kaomoji_text: String,
    kaomoji_category: String,
    pub shortcut_status: Option<Result<String, String>>,
    transparency_disabled: bool,
    transparency_reason: String,
}

impl Default for State {
    fn default() -> Self {
        Self::new(&app_shared::settings::load())
    }
}

impl State {
    pub fn new(settings: &AppSettings) -> Self {
        let env = rendering_env::get_rendering_env();
        Self {
            autodelete: if settings.auto_delete_interval == 0 {
                String::new()
            } else {
                settings.auto_delete_interval.to_string()
            },
            max_history: settings.max_history_size.to_string(),
            kaomoji_text: String::new(),
            kaomoji_category: "Custom".to_string(),
            shortcut_status: None,
            transparency_disabled: env.transparency_disabled,
            transparency_reason: env.reason.clone(),
        }
    }

    fn locked(&self, settings: &AppSettings) -> bool {
        self.transparency_disabled && !settings.allow_transparency
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Theme(&'static str),
    AutoDeleteText(String),
    AutoDeleteUnit(&'static str),
    MaxHistoryText(String),
    DarkOpacity(f32),
    LightOpacity(f32),
    UiScale(f32),
    SliderReleased,
    Toggle(Feature),
    KaomojiText(String),
    KaomojiAdd,
    KaomojiRemove(usize),
    RegisterShortcuts,
    ResetDefaults,
    Close,
    Drag,
}

#[derive(Default)]
pub struct Outcome {
    /// Persist the settings and push them to the backend / tray.
    pub save: bool,
    pub close: bool,
    pub drag: bool,
    /// Re-apply the COSMIC theme (System / Light / Dark changed).
    pub theme: bool,
    /// Re-apply the UI scale.
    pub scale: bool,
    /// Reset also restarts the first-run wizard.
    pub reopen_wizard: bool,
}

pub fn update(state: &mut State, settings: &mut AppSettings, msg: Msg) -> Outcome {
    let mut out = Outcome::default();
    match msg {
        Msg::Close => out.close = true,
        Msg::Drag => out.drag = true,
        Msg::Theme(mode) => {
            settings.theme_mode = mode.to_string();
            out.save = true;
            out.theme = true;
        }
        Msg::AutoDeleteText(text) => {
            let digits: String = text.chars().filter(|c| c.is_ascii_digit()).take(6).collect();
            settings.auto_delete_interval = digits.parse().unwrap_or(0);
            state.autodelete = digits;
            out.save = true;
        }
        Msg::AutoDeleteUnit(unit) => {
            settings.auto_delete_unit = unit.to_string();
            out.save = true;
        }
        Msg::MaxHistoryText(text) => {
            let digits: String = text.chars().filter(|c| c.is_ascii_digit()).take(6).collect();
            if let Ok(n) = digits.parse::<usize>() {
                settings.max_history_size = n.clamp(1, 100_000);
                out.save = true;
            }
            state.max_history = digits;
        }
        Msg::DarkOpacity(v) => settings.dark_background_opacity = v.clamp(0.0, 1.0),
        Msg::LightOpacity(v) => settings.light_background_opacity = v.clamp(0.0, 1.0),
        Msg::UiScale(v) => {
            settings.ui_scale = (v * 20.0).round() / 20.0;
            settings.ui_scale = settings.ui_scale.clamp(0.5, 2.0);
        }
        Msg::SliderReleased => {
            out.save = true;
            out.scale = true;
        }
        Msg::Toggle(feature) => {
            match feature {
                Feature::SmartActions => settings.enable_smart_actions = !settings.enable_smart_actions,
                Feature::DynamicTray => settings.enable_dynamic_tray_icon = !settings.enable_dynamic_tray_icon,
                Feature::CloseOnFocusLoss => settings.close_on_focus_loss = !settings.close_on_focus_loss,
                Feature::AllowTransparency => settings.allow_transparency = !settings.allow_transparency,
                Feature::Autostart => {
                    let _ = if app_shared::launcher::autostart_is_enabled() {
                        app_shared::launcher::autostart_disable()
                    } else {
                        app_shared::launcher::autostart_enable()
                    };
                }
            }
            out.save = true;
        }
        Msg::KaomojiText(text) => state.kaomoji_text = text,
        Msg::KaomojiAdd => {
            let text = state.kaomoji_text.trim().to_string();
            if !text.is_empty() {
                settings.custom_kaomojis.push(CustomKaomoji {
                    text,
                    category: state.kaomoji_category.clone(),
                    keywords: Vec::new(),
                });
                state.kaomoji_text.clear();
                out.save = true;
            }
        }
        Msg::KaomojiRemove(index) => {
            if index < settings.custom_kaomojis.len() {
                settings.custom_kaomojis.remove(index);
                out.save = true;
            }
        }
        Msg::RegisterShortcuts => {
            state.shortcut_status = Some(match app_shared::platform::shortcuts::register() {
                Some(result) => result.map(|_| "Shortcuts registered successfully!".to_string()),
                None => Err(format!(
                    "Automatic registration supports GNOME and COSMIC. Bind Super+V to `{} --toggle` manually.",
                    app_shared::launcher::launch_command()
                )),
            });
        }
        Msg::ResetDefaults => {
            *settings = AppSettings::default();
            *state = State::new(settings);
            let _ = app_shared::launcher::reset_first_run();
            out.save = true;
            out.theme = true;
            out.scale = true;
            out.reopen_wizard = true;
        }
    }
    settings.validate();
    out
}

// --- View ---

pub fn view<'a>(state: &'a State, settings: &'a AppSettings, focused: bool) -> Element<'a, Msg> {
    let content = widget::column::with_capacity(9)
        .spacing(theme::spacing().space_m)
        .padding([16, 24, 24, 24])
        .push(appearance(settings))
        .push(auto_delete(state, settings))
        .push(transparency(state, settings))
        .push(ui_scale(settings))
        .push(history(state))
        .push(kaomoji(state, settings))
        .push(features(state, settings))
        .push(shortcuts(state))
        .push(reset());

    widget::container(widget::column::with_children(vec![
        widget::header_bar()
            .title("Settings — Clipboard History")
            .on_drag(Msg::Drag)
            .on_close(Msg::Close)
            .focused(focused)
            .into(),
        cosmic::iced::widget::scrollable(content).width(Length::Fill).height(Length::Fill).into(),
    ]))
    .class(theme::Container::WindowBackground)
    .into()
}

fn choice<'a, T: Copy + PartialEq + 'static>(
    options: Vec<(T, &'static str)>,
    selected: T,
    message: impl Fn(T) -> Msg + 'static,
) -> Element<'a, Msg> {
    let mut row = widget::row::with_capacity(options.len()).spacing(theme::spacing().space_xxs);
    for (value, label) in options {
        let button = if value == selected {
            widget::button::suggested(label)
        } else {
            widget::button::standard(label)
        };
        row = row.push(button.on_press(message(value)));
    }
    row.into()
}

fn appearance<'a>(settings: &'a AppSettings) -> Element<'a, Msg> {
    let current: &'static str = THEMES
        .iter()
        .map(|(key, _)| *key)
        .find(|key| *key == settings.theme_mode)
        .unwrap_or("system");
    settings::section()
        .title("Appearance")
        .add(
            settings::item::builder("Theme")
                .description("Follow the COSMIC theme, or force light or dark.")
                .control(choice(THEMES.to_vec(), current, Msg::Theme)),
        )
        .into()
}

fn auto_delete<'a>(state: &'a State, settings: &'a AppSettings) -> Element<'a, Msg> {
    let selected = UNITS.iter().position(|u| *u == settings.auto_delete_unit);
    settings::section()
        .title("Auto-delete")
        .add(
            settings::item::builder("Delete items older than")
                .description("Pinned items are never deleted. Empty or 0 turns it off.")
                .control(
                    widget::text_input("off", &state.autodelete)
                        .on_input(Msg::AutoDeleteText)
                        .width(Length::Fixed(90.0)),
                ),
        )
        .add(
            settings::item::builder("Unit")
                .control(widget::dropdown(UNITS.as_slice(), selected, |i| Msg::AutoDeleteUnit(UNITS[i]))),
        )
        .into()
}

fn transparency<'a>(state: &'a State, settings: &'a AppSettings) -> Element<'a, Msg> {
    let locked = state.locked(settings);
    let mut section = settings::section().title("Window transparency");
    if state.transparency_disabled {
        section = section.add(
            settings::item::builder("Transparency is limited here")
                .description(state.transparency_reason.clone())
                .control(
                    widget::toggler(settings.allow_transparency)
                        .on_toggle(|_| Msg::Toggle(Feature::AllowTransparency)),
                ),
        );
    }
    let slider = |value: f32, change: fn(f32) -> Msg| -> Element<'a, Msg> {
        widget::row::with_capacity(2)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(
                widget::slider(0.0..=1.0, value, change)
                    .step(0.01_f32)
                    .on_release(Msg::SliderReleased)
                    .width(Length::Fixed(200.0)),
            )
            .push(widget::text::body(format!("{}%", (value * 100.0).round() as i32)))
            .into()
    };
    let item = |title: &'static str, value: f32, change: fn(f32) -> Msg| {
        let control = if locked {
            widget::text::caption("Locked").into()
        } else {
            slider(value, change)
        };
        settings::item::builder(title).control(control)
    };
    section
        .add(item("Dark background opacity", settings.dark_background_opacity, Msg::DarkOpacity))
        .add(item("Light background opacity", settings.light_background_opacity, Msg::LightOpacity))
        .into()
}

fn ui_scale<'a>(settings: &'a AppSettings) -> Element<'a, Msg> {
    settings::section()
        .title("UI scale")
        .add(
            settings::item::builder("Clipboard window size")
                .description("Scales the popup and the windows of this app.")
                .control(
                    widget::row::with_capacity(2)
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .push(
                            widget::slider(0.5..=2.0, settings.ui_scale, Msg::UiScale)
                                .step(0.05_f32)
                                .on_release(Msg::SliderReleased)
                                .width(Length::Fixed(200.0)),
                        )
                        .push(widget::text::body(format!("{:.2}×", settings.ui_scale))),
                ),
        )
        .into()
}

fn history<'a>(state: &'a State) -> Element<'a, Msg> {
    settings::section()
        .title("History")
        .add(
            settings::item::builder("Maximum items")
                .description("Between 1 and 100,000. Oldest unpinned items are dropped first.")
                .control(
                    widget::text_input("50", &state.max_history)
                        .on_input(Msg::MaxHistoryText)
                        .width(Length::Fixed(110.0)),
                ),
        )
        .into()
}

fn kaomoji<'a>(state: &'a State, settings: &'a AppSettings) -> Element<'a, Msg> {
    let mut section = settings::section().title("Custom kaomoji").add(
        settings::item::builder("Add your own")
            .description("Shown in the Kaomoji tab under “Custom”.")
            .control(
                widget::row::with_capacity(2)
                    .spacing(8)
                    .push(
                        widget::text_input("(ﾉ◕ヮ◕)ﾉ", &state.kaomoji_text)
                            .on_input(Msg::KaomojiText)
                            .on_submit(|_| Msg::KaomojiAdd)
                            .width(Length::Fixed(170.0)),
                    )
                    .push(
                        widget::button::suggested("Add")
                            .on_press_maybe((!state.kaomoji_text.trim().is_empty()).then_some(Msg::KaomojiAdd)),
                    ),
            ),
    );
    for (i, k) in settings.custom_kaomojis.iter().enumerate() {
        section = section.add(
            settings::item::builder(k.text.clone())
                .control(widget::button::icon(crate::widgets::handle(Ic::Trash)).on_press(Msg::KaomojiRemove(i))),
        );
    }
    section.into()
}

fn features<'a>(_state: &'a State, settings: &'a AppSettings) -> Element<'a, Msg> {
    let toggle = |title: &'static str, description: &'static str, on: bool, feature: Feature| {
        settings::item::builder(title)
            .description(description)
            .control(widget::toggler(on).on_toggle(move |_| Msg::Toggle(feature)))
    };
    settings::section()
        .title("Features")
        .add(toggle(
            "Smart actions",
            "Detect links, colours and e-mail addresses in copied text.",
            settings.enable_smart_actions,
            Feature::SmartActions,
        ))
        .add(toggle(
            "Start on login",
            "Launch silently in the background.",
            app_shared::launcher::autostart_is_enabled(),
            Feature::Autostart,
        ))
        .add(toggle(
            "Dynamic tray icon",
            "Switch the tray glyph between light and dark with the theme.",
            settings.enable_dynamic_tray_icon,
            Feature::DynamicTray,
        ))
        .add(toggle(
            "Close when focus is lost",
            "Dismiss the popup when another window takes focus.",
            settings.close_on_focus_loss,
            Feature::CloseOnFocusLoss,
        ))
        .into()
}

fn shortcuts<'a>(state: &'a State) -> Element<'a, Msg> {
    let mut section = settings::section().title("Keyboard shortcuts");
    for (keys, what) in [
        ("Super + V", "Open Clipboard History"),
        ("Ctrl + Alt + V", "Alternative shortcut"),
    ] {
        section = section.add(settings::item::builder(keys).description(what).control(widget::space()));
    }
    section = section.add(
        settings::item::builder("Register in your desktop")
            .description("Only done when you ask for it.")
            .control(widget::button::suggested("Register shortcuts").on_press(Msg::RegisterShortcuts)),
    );
    if let Some(status) = &state.shortcut_status {
        let (icon, text) = match status {
            Ok(msg) => (Ic::Check, msg.clone()),
            Err(msg) => (Ic::X, msg.clone()),
        };
        section = section.add(
            widget::row::with_capacity(2)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(ic(icon, 16))
                .push(widget::text::body(text)),
        );
    }
    section.into()
}

fn reset<'a>() -> Element<'a, Msg> {
    widget::container(widget::button::destructive("Reset to defaults").on_press(Msg::ResetDefaults))
        .width(Length::Fill)
        .align_x(Alignment::End)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> (State, AppSettings) {
        let settings = AppSettings::default();
        (State::new(&settings), settings)
    }

    #[test]
    fn numeric_fields_ignore_non_digits_and_clamp() {
        rendering_env::init();
        let (mut state, mut settings) = fresh();
        let out = update(&mut state, &mut settings, Msg::MaxHistoryText("12ab3".into()));
        assert!(out.save);
        assert_eq!(settings.max_history_size, 123);
        update(&mut state, &mut settings, Msg::MaxHistoryText("999999".into()));
        assert_eq!(settings.max_history_size, 100_000);
        update(&mut state, &mut settings, Msg::AutoDeleteText("7x".into()));
        assert_eq!(settings.auto_delete_interval, 7);
        update(&mut state, &mut settings, Msg::AutoDeleteText("".into()));
        assert_eq!(settings.auto_delete_interval, 0);
    }

    #[test]
    fn sliders_apply_live_and_persist_on_release() {
        rendering_env::init();
        let (mut state, mut settings) = fresh();
        let live = update(&mut state, &mut settings, Msg::DarkOpacity(0.4));
        assert!(!live.save);
        assert!((settings.dark_background_opacity - 0.4).abs() < 1e-6);
        update(&mut state, &mut settings, Msg::UiScale(1.37));
        assert!((settings.ui_scale - 1.35).abs() < 1e-6);
        let done = update(&mut state, &mut settings, Msg::SliderReleased);
        assert!(done.save && done.scale);
    }

    #[test]
    fn kaomoji_add_and_remove() {
        rendering_env::init();
        let (mut state, mut settings) = fresh();
        update(&mut state, &mut settings, Msg::KaomojiText("  (^_^)  ".into()));
        let out = update(&mut state, &mut settings, Msg::KaomojiAdd);
        assert!(out.save);
        assert_eq!(settings.custom_kaomojis.len(), 1);
        assert_eq!(settings.custom_kaomojis[0].text, "(^_^)");
        update(&mut state, &mut settings, Msg::KaomojiRemove(0));
        assert!(settings.custom_kaomojis.is_empty());
    }

    #[test]
    fn theme_choice_is_validated() {
        rendering_env::init();
        let (mut state, mut settings) = fresh();
        let out = update(&mut state, &mut settings, Msg::Theme("dark"));
        assert!(out.theme && out.save);
        assert_eq!(settings.theme_mode, "dark");
    }

    #[test]
    fn settings_buttons_send_messages() {
        let _headless = crate::testing::headless();
        rendering_env::init();
        let (state, settings) = fresh();
        let mut screen = iced_test::Simulator::with_size(
            cosmic::iced::Settings::default(),
            cosmic::iced::Size::new(480.0, 2400.0),
            view(&state, &settings, true),
        );
        screen.click(crate::testing::text("Register shortcuts")).unwrap();
        screen.click(crate::testing::text("Reset to defaults")).unwrap();
        let messages: Vec<_> = screen.into_messages().collect();
        assert!(matches!(messages.as_slice(), [Msg::RegisterShortcuts, Msg::ResetDefaults]), "{messages:?}");
    }
}
