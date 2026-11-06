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
    selected_backend: String,
    backend_choice_error: Option<String>,
    restart_required: bool,
    relogin_required: bool,
}

impl Default for State {
    fn default() -> Self {
        let tools = shortcut_setup::check_shortcut_tools();
        let saved_backend = app_shared::settings::load().platform_backend;
        let selected_backend = match saved_backend.as_str() {
            "wayland" | "xwayland" => saved_backend,
            "x11" if app_shared::platform::wayland_session() => "xwayland".to_string(),
            "x11" => saved_backend,
            _ if backend_choice_available() => String::new(),
            _ => "x11".to_string(),
        };
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
            selected_backend,
            backend_choice_error: None,
            restart_required: app_shared::platform::needs_setup(),
            relogin_required: false,
        }
    }
}

fn backend_choice_available() -> bool {
    app_shared::platform::wayland_session() || app_shared::platform::needs_setup()
}

fn native_backend_choice_available() -> bool {
    app_shared::platform::wayland_session()
        && matches!(
            app_shared::platform::shortcuts::current_desktop(),
            app_shared::platform::shortcuts::Desktop::Gnome
                | app_shared::platform::shortcuts::Desktop::Cosmic
        )
}

fn choice_backend_name(choice: &str) -> Option<&'static str> {
    match choice {
        "x11" | "xwayland" => Some("x11"),
        "wayland" => match app_shared::platform::shortcuts::current_desktop() {
            app_shared::platform::shortcuts::Desktop::Gnome => Some("gnome"),
            app_shared::platform::shortcuts::Desktop::Cosmic => Some("cosmic"),
            app_shared::platform::shortcuts::Desktop::Other => None,
        },
        _ => None,
    }
}

fn save_backend_choice(state: &mut State) -> bool {
    // The UI state is also exercised without starting the platform layer in
    // unit tests. The real app always initializes it before opening this wizard.
    if !app_shared::platform::is_initialized() {
        return true;
    }
    if choice_backend_name(&state.selected_backend).is_none()
        || (!app_shared::platform::selection_ready(&state.selected_backend)
            && !(state.selected_backend == "wayland"
                && app_shared::platform::shortcuts::current_desktop()
                    == app_shared::platform::shortcuts::Desktop::Gnome
                && app_shared::platform::wayland_session()))
    {
        state.backend_choice_error = Some(
            "Choose an available backend. XWayland requires an active XWayland display.".into(),
        );
        return false;
    }
    let mut settings = app_shared::settings::load();
    settings.platform_backend = state.selected_backend.clone();
    state.backend_choice_error = app_shared::settings::save(&settings).err();
    if state.selected_backend == "wayland" && state.extension_note.is_none() {
        state.extension_note = app_shared::platform::setup_desktop_integration();
    }
    let target = choice_backend_name(&state.selected_backend);
    let ready = app_shared::platform::selection_ready(&state.selected_backend);
    state.restart_required = target != Some(app_shared::platform::backend().name()) || !ready;
    state.relogin_required = state.selected_backend == "wayland"
        && app_shared::platform::shortcuts::current_desktop()
            == app_shared::platform::shortcuts::Desktop::Gnome
        && state.extension_note.as_ref().is_some_and(|note| note.is_ok())
        && !ready;
    state.backend_choice_error.is_none()
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
    SelectBackend(String),
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
    pub restart_backend: bool,
    pub relogin: bool,
    /// Text to put on the clipboard.
    pub copy: Option<String>,
}

pub fn update(state: &mut State, msg: Msg) -> Outcome {
    let mut out = Outcome::default();
    match msg {
        Msg::Close => {
            if !app_shared::platform::needs_setup() {
                out.close = true;
            }
        }
        Msg::Drag => out.drag = true,
        Msg::Next => {
            if state.step == 0 && !save_backend_choice(state) {
                return out;
            }
            state.step = (state.step + 1).min(STEPS - 1);
        }
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
                if state.selected_backend == "wayland" {
                    state.extension_note = app_shared::platform::setup_desktop_integration();
                }
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
        Msg::SelectBackend(backend) => {
            if !["wayland", "xwayland", "x11"].contains(&backend.as_str()) {
                return out;
            }
            state.selected_backend = backend;
            if state.selected_backend != "wayland" {
                state.extension_note = None;
                state.relogin_required = false;
            }
            save_backend_choice(state);
        }
        Msg::Finish => {
            if state.backend_choice_error.is_some() {
                return out;
            }
            if state.restart_required {
                if state.extension_note.as_ref().is_some_and(|note| note.is_err()) {
                    state.backend_choice_error = Some(
                        "The selected backend is not ready. Choose XWayland or fix the GNOME extension.".into(),
                    );
                    return out;
                }
                if !state.relogin_required
                    && !app_shared::platform::selection_ready(&state.selected_backend)
                {
                    state.backend_choice_error = Some(
                        "The selected backend is not available yet. Choose an available backend.".into(),
                    );
                    return out;
                }
                let _ = app_shared::launcher::mark_first_run_complete();
                if state.relogin_required {
                    out.relogin = true;
                } else {
                    match crate::spawn_backend_restart() {
                        Ok(()) => out.restart_backend = true,
                        Err(error) => state.backend_choice_error = Some(error),
                    }
                }
                out.close = out.relogin || out.restart_backend;
            } else {
                let _ = app_shared::launcher::mark_first_run_complete();
                out.finished = true;
                out.close = true;
            }
        }
    }
    out
}

// --- View ---

pub fn view<'a>(state: &'a State, focused: bool) -> Element<'a, Msg> {
    let body: Element<'a, Msg> = match state.step {
        0 => welcome(state),
        1 => permissions(state),
        2 => shortcut(state),
        3 => autostart(),
        _ => done(state),
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

fn welcome<'a>(state: &'a State) -> Element<'a, Msg> {
    let mut content: Vec<Element<'a, Msg>> = vec![
        widget::text::body("• Press Super+V anywhere to open your history, emoji, kaomoji and symbols.").into(),
        widget::text::body("• Paste straight into the app you were using.").into(),
        widget::text::body("• Pin what you need, search the rest.").into(),
    ];
    if backend_choice_available() {
        content.push(widget::text::body("Choose the popup backend. The app will not switch to another backend if this one is unavailable.").into());
        let mut choices: Vec<Element<'a, Msg>> = Vec::new();
        if native_backend_choice_available() {
            let selected = state.selected_backend == "wayland";
            let label = if selected { "✓ Wayland" } else { "Wayland" };
            choices.push(if selected {
                widget::button::suggested(label).on_press(Msg::SelectBackend("wayland".into())).into()
            } else {
                widget::button::standard(label).on_press(Msg::SelectBackend("wayland".into())).into()
            });
        }
        if app_shared::platform::xwayland_available() {
            let selected = state.selected_backend == "xwayland";
            let label = if selected { "✓ XWayland" } else { "XWayland" };
            choices.push(if selected {
                widget::button::suggested(label).on_press(Msg::SelectBackend("xwayland".into())).into()
            } else {
                widget::button::standard(label).on_press(Msg::SelectBackend("xwayland".into())).into()
            });
        }
        if !app_shared::platform::wayland_session()
            && app_shared::platform::needs_setup()
            && app_shared::platform::x11_available()
        {
            let selected = state.selected_backend == "x11";
            let label = if selected { "✓ X11" } else { "X11" };
            choices.push(if selected {
                widget::button::suggested(label).on_press(Msg::SelectBackend("x11".into())).into()
            } else {
                widget::button::standard(label).on_press(Msg::SelectBackend("x11".into())).into()
            });
        }
        if choices.is_empty() {
            content.push(widget::text::caption("No supported backend is available. Enable XWayland or start GNOME/COSMIC Wayland, then reopen setup.").into());
        } else {
            content.push(widget::row::with_children(choices).spacing(8).align_y(Alignment::Center).into());
        }
        if let Some(error) = &state.backend_choice_error {
            content.push(widget::text::caption(error.clone()).into());
        }
    }
    page(
        "Welcome to Clipboard History",
        "A Windows 11-style clipboard manager. Three quick steps and you are set.",
        content,
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

fn done<'a>(state: &'a State) -> Element<'a, Msg> {
    page(
        "You are all set",
        if state.relogin_required {
            "Log out and back in to activate the GNOME helper, then open the app. It will use the selected backend."
        } else if state.restart_required {
            "Restart the app to apply the selected backend."
        } else {
            "Press Super+V to open your clipboard history."
        },
        vec![
            widget::text::body("You can change anything later in Settings (tray icon → Settings)." ).into(),
            state.backend_choice_error.as_ref().map(|error| widget::text::caption(error.clone()).into()).unwrap_or_else(|| widget::space().into()),
        ],
        vec![widget::button::suggested(if state.relogin_required { "Finish setup" } else if state.restart_required { "Restart to apply" } else { "Start using it" }).on_press(Msg::Finish).into()],
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
