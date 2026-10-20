//! The five-step setup wizard: Welcome → Permissions → Shortcut → Autostart → Done.
//!
//! Like the GPUI build, the system calls (polkit prompt for `/dev/uinput`, shortcut
//! registration) run synchronously on the UI thread: they are short, user-initiated
//! and the window has nothing else to do meanwhile.

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings};
use cosmic::{theme, Element};
use win11_clipboard_history_lib::permission_checker::{self, PermissionStatus};
use win11_clipboard_history_lib::shortcut_conflict_detector::ConflictDetectionResult;
use win11_clipboard_history_lib::shortcut_setup;

use crate::widgets::{ic, Ic};

const STEPS: usize = 5;

pub struct State {
    pub step: usize,
    permissions: PermissionStatus,
    conflicts: ConflictDetectionResult,
    can_register: bool,
    manual_instructions: String,
    fix_error: Option<String>,
    conflicts_resolved: bool,
    conflict_error: Option<String>,
    shortcut_registered: bool,
    show_manual: bool,
    /// Result of the desktop-specific extra setup (GNOME extension), if any ran.
    extension_note: Option<Result<String, String>>,
}

impl Default for State {
    fn default() -> Self {
        let tools = shortcut_setup::check_shortcut_tools();
        Self {
            step: 0,
            permissions: permission_checker::check_permissions(),
            conflicts: shortcut_setup::detect_conflicts(),
            can_register: tools.can_register_automatically,
            manual_instructions: tools.manual_instructions,
            fix_error: None,
            conflicts_resolved: false,
            conflict_error: None,
            shortcut_registered: false,
            show_manual: false,
            extension_note: None,
        }
    }
}

/// The command a manual shortcut should run — always THIS binary.
pub fn toggle_command() -> String {
    format!("{} --toggle", app_shared::launcher::launch_command())
}

#[derive(Debug, Clone)]
pub enum Msg {
    Next,
    Back,
    FixPermissions,
    ResolveConflicts,
    RegisterShortcut,
    ShowManual,
    CopyCommand,
    EnableAutostart,
    Finish,
    Close,
    Drag,
}

#[derive(Default)]
pub struct Outcome {
    pub close: bool,
    pub drag: bool,
    /// The wizard is complete: close it and open the popup.
    pub finished: bool,
    /// Text to put on the clipboard.
    pub copy: Option<String>,
}

pub fn update(state: &mut State, msg: Msg) -> Outcome {
    let mut out = Outcome::default();
    match msg {
        Msg::Close => out.close = true,
        Msg::Drag => out.drag = true,
        Msg::Next => state.step = (state.step + 1).min(STEPS - 1),
        Msg::Back => state.step = state.step.saturating_sub(1),
        Msg::FixPermissions => {
            state.fix_error = None;
            match permission_checker::fix_permissions_now() {
                Ok(_) => state.permissions = permission_checker::check_permissions(),
                Err(e) => state.fix_error = Some(e),
            }
        }
        Msg::ResolveConflicts => {
            state.conflict_error = None;
            match shortcut_setup::resolve_conflicts() {
                Ok(_) => {
                    state.conflicts_resolved = true;
                    state.conflicts = shortcut_setup::detect_conflicts();
                }
                Err(e) => state.conflict_error = Some(e),
            }
        }
        Msg::RegisterShortcut => match app_shared::platform::shortcuts::register() {
            Some(Ok(_)) => {
                state.shortcut_registered = true;
                state.extension_note = app_shared::platform::setup_desktop_integration();
                state.step = 3;
            }
            Some(Err(_)) | None => state.show_manual = true,
        },
        Msg::ShowManual => state.show_manual = true,
        Msg::CopyCommand => out.copy = Some(toggle_command()),
        Msg::EnableAutostart => {
            let _ = app_shared::launcher::autostart_enable();
            state.step = 4;
        }
        Msg::Finish => {
            let _ = app_shared::launcher::mark_first_run_complete();
            out.finished = true;
            out.close = true;
        }
    }
    out
}

// --- View ---

pub fn view<'a>(state: &'a State, focused: bool) -> Element<'a, Msg> {
    let body: Element<'a, Msg> = match state.step {
        0 => welcome(),
        1 => permissions(state),
        2 => shortcut(state),
        3 => autostart(),
        _ => done(),
    };
    let dots = widget::row::with_children(
        (0..STEPS)
            .map(|i| {
                let on = i == state.step;
                widget::container(widget::space().width(if on { 22 } else { 8 }).height(8))
                    .class(theme::Container::custom(move |t: &cosmic::Theme| {
                        let c = t.cosmic();
                        let color: cosmic::iced::Color = if on { c.accent_color().into() } else { c.background(false).on.into() };
                        cosmic::iced::widget::container::Style {
                            background: Some(cosmic::iced::Background::Color(cosmic::iced::Color { a: if on { 1.0 } else { 0.25 }, ..color })),
                            border: cosmic::iced::Border { radius: 4.0.into(), ..Default::default() },
                            ..Default::default()
                        }
                    }))
                    .into()
            })
            .collect::<Vec<Element<'a, Msg>>>(),
    )
    .spacing(6)
    .align_y(Alignment::Center);

    widget::container(widget::column::with_children(vec![
        widget::header_bar()
            .title("Setup — Clipboard History")
            .on_drag(Msg::Drag)
            .on_close(Msg::Close)
            .focused(focused)
            .into(),
        widget::container(body).padding(28).width(Length::Fill).height(Length::Fill).into(),
        widget::container(dots).width(Length::Fill).align_x(Alignment::Center).padding([0, 0, 20, 0]).into(),
    ]))
    .class(theme::Container::WindowBackground)
    .into()
}

fn page<'a>(title: &'static str, subtitle: &'static str, content: Vec<Element<'a, Msg>>, actions: Vec<Element<'a, Msg>>) -> Element<'a, Msg> {
    let mut col = widget::column::with_capacity(content.len() + 3)
        .spacing(theme::spacing().space_s)
        .push(widget::text::title2(title))
        .push(widget::text::body(subtitle));
    for item in content {
        col = col.push(item);
    }
    let actions = widget::row::with_children(actions).spacing(8).align_y(Alignment::Center);
    widget::column::with_capacity(2)
        .push(widget::scrollable(col).height(Length::Fill))
        .push(widget::container(actions).width(Length::Fill).align_x(Alignment::End).padding([12, 0, 0, 0]))
        .into()
}

fn status<'a>(ok: bool, text: String) -> Element<'a, Msg> {
    widget::row::with_capacity(2)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(ic(if ok { Ic::Check } else { Ic::X }, 16))
        .push(widget::text::body(text))
        .into()
}

fn welcome<'a>() -> Element<'a, Msg> {
    page(
        "Welcome to Clipboard History",
        "A Windows 11-style clipboard manager. Three quick steps and you are set.",
        vec![
            widget::text::body("• Press Super+V anywhere to open your history, emoji, kaomoji and symbols.").into(),
            widget::text::body("• Paste straight into the app you were using.").into(),
            widget::text::body("• Pin what you need, search the rest.").into(),
        ],
        vec![widget::button::suggested("Get started").on_press(Msg::Next).into()],
    )
}

fn permissions<'a>(state: &'a State) -> Element<'a, Msg> {
    let p = &state.permissions;
    let mut content: Vec<Element<'a, Msg>> = vec![status(
        p.uinput_accessible,
        if p.uinput_accessible {
            format!("{} is accessible — pasting works.", p.uinput_path)
        } else {
            format!("{} is not accessible: pasting needs it.", p.uinput_path)
        },
    )];
    if !p.uinput_accessible {
        content.push(widget::text::caption(p.suggestion.clone()).into());
        content.push(widget::button::suggested("Fix permissions").on_press(Msg::FixPermissions).into());
    }
    if let Some(error) = &state.fix_error {
        content.push(widget::text::caption(format!("Could not fix: {error}")).into());
    }
    page(
        "Permissions",
        "Pasting types Ctrl+V for you through /dev/uinput.",
        content,
        vec![
            widget::button::standard("Back").on_press(Msg::Back).into(),
            widget::button::suggested(if p.uinput_accessible { "Continue" } else { "Skip" }).on_press(Msg::Next).into(),
        ],
    )
}

fn shortcut<'a>(state: &'a State) -> Element<'a, Msg> {
    let mut content: Vec<Element<'a, Msg>> = Vec::new();
    let conflicts = &state.conflicts;
    if !conflicts.conflicts.is_empty() && !state.conflicts_resolved {
        let mut section = settings::section().title("Shortcut conflicts");
        for c in &conflicts.conflicts {
            section = section.add(
                settings::item::builder(format!("{} — {}", c.binding, c.owner))
                    .description(c.current_action.clone())
                    .control(widget::space()),
            );
        }
        content.push(section.into());
        if conflicts.can_auto_resolve {
            content.push(widget::button::standard("Resolve conflicts").on_press(Msg::ResolveConflicts).into());
        }
    }
    if state.conflicts_resolved {
        content.push(status(true, "Conflicts resolved.".into()));
    }
    if let Some(error) = &state.conflict_error {
        content.push(widget::text::caption(format!("Could not resolve: {error}")).into());
    }
    if state.shortcut_registered {
        content.push(status(true, "Shortcut registered successfully!".into()));
    }
    if let Some(note) = &state.extension_note {
        content.push(status(note.is_ok(), note.clone().unwrap_or_else(|e| e)));
    }
    if state.show_manual {
        content.push(
            settings::section()
                .title("Manual setup")
                .add(widget::text::body(state.manual_instructions.clone()))
                .add(widget::text::monotext(toggle_command()))
                .add(widget::button::standard("Copy command").on_press(Msg::CopyCommand))
                .into(),
        );
    }
    let primary: Element<'a, Msg> = if state.shortcut_registered || state.show_manual {
        widget::button::suggested("Continue").on_press(Msg::Next).into()
    } else if state.can_register {
        widget::button::suggested("Register automatically").on_press(Msg::RegisterShortcut).into()
    } else {
        widget::button::suggested("Show manual setup").on_press(Msg::ShowManual).into()
    };
    let mut actions: Vec<Element<'a, Msg>> = vec![widget::button::standard("Back").on_press(Msg::Back).into()];
    if !(state.shortcut_registered || state.show_manual) {
        actions.push(widget::button::text("Skip").on_press(Msg::Next).into());
    }
    actions.push(primary);
    page("Keyboard shortcut", "Super+V should open the clipboard history.", content, actions)
}

fn autostart<'a>() -> Element<'a, Msg> {
    page(
        "Start on login",
        "Run quietly in the background so Super+V is always ready.",
        vec![widget::text::body("The app starts hidden (tray only) and never opens a window by itself.").into()],
        vec![
            widget::button::standard("Back").on_press(Msg::Back).into(),
            widget::button::text("Not now").on_press(Msg::Next).into(),
            widget::button::suggested("Enable autostart").on_press(Msg::EnableAutostart).into(),
        ],
    )
}

fn done<'a>() -> Element<'a, Msg> {
    page(
        "You are all set",
        "Press Super+V to open your clipboard history.",
        vec![widget::text::body("You can change anything later in Settings (tray icon → Settings).").into()],
        vec![widget::button::suggested("Start using it").on_press(Msg::Finish).into()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        win11_clipboard_history_lib::rendering_env::init();
        State::default()
    }

    #[test]
    fn steps_stay_in_range() {
        let mut s = state();
        for _ in 0..10 {
            update(&mut s, Msg::Next);
        }
        assert_eq!(s.step, STEPS - 1);
        for _ in 0..10 {
            update(&mut s, Msg::Back);
        }
        assert_eq!(s.step, 0);
    }

    #[test]
    fn copy_command_names_this_binary() {
        let mut s = state();
        let out = update(&mut s, Msg::CopyCommand);
        assert!(out.copy.unwrap().ends_with("--toggle"));
    }

    #[test]
    fn manual_setup_can_be_opened() {
        let mut s = state();
        update(&mut s, Msg::ShowManual);
        assert!(s.show_manual);
    }

    #[test]
    fn get_started_advances_the_wizard() {
        let _headless = crate::testing::headless();
        let s = state();
        let mut screen = iced_test::Simulator::with_size(
            cosmic::iced::Settings::default(),
            cosmic::iced::Size::new(550.0, 900.0),
            view(&s, true),
        );
        screen.click(crate::testing::text_area("Get started")).unwrap();
        let messages: Vec<_> = screen.into_messages().collect();
        assert!(matches!(messages.as_slice(), [Msg::Next]), "{messages:?}");
    }
}
