//! The libcosmic application: windows, messages, the popup lifecycle.
//!
//! The popup is one of two things, chosen by the platform backend:
//! - `Overlay` (COSMIC): a transparent full-output layer surface, created on every
//!   show and destroyed on every hide; the card is drawn inside it at the pointer.
//! - `Window` (GNOME / X11): a persistent toplevel that the backend places,
//!   shows and hides.

use std::sync::Arc;
use std::time::{Duration, Instant};

use app_shared::backend::BackendService;
use app_shared::geometry::{bottom_center, clamp_to_monitor, MonitorRect};
use app_shared::platform::{self, PopupSurface};
use app_shared::settings::{self, AppSettings};
use cosmic::app::{Core, Task};
use cosmic::cctk::sctk::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use cosmic::core::Auto;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, IcedOutput, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::{blur, layer_surface};
use cosmic::iced::widget::scrollable;
use cosmic::iced::{
    event, keyboard, mouse, time, window, Length, Limits, Padding, Point, Rectangle, Size,
    Subscription, Vector,
};
use cosmic::widget;
use cosmic::{ApplicationExt, Element};
use win11_clipboard_history_lib::focus_manager;

use crate::glass;
use crate::hooks::Hooks;
use crate::popup::{self, Popup, Tab, ViewCtx, POPUP_H, POPUP_W};
use crate::tray::{TrayControl, TrayState};
use crate::{pickers_ui, settings_ui, wizard, Command, Flags, COMMANDS};

const APP_ID: &str = "dev.gustavosett.clipboard-history-iced";
/// How long the overlay waits for the pointer's first position before it opens
/// bottom-center (COSMIC reports no pointer until it moves).
/// A second nudge if the first one got no answer (the surface may not have been ready).
const POINTER_RETRY: Duration = Duration::from_millis(110);
const POINTER_WAIT: Duration = Duration::from_millis(330);
/// Focus must stay away this long before the popup dismisses itself.
const FOCUS_LOSS_GRACE: Duration = Duration::from_millis(350);
/// Appear animation of the popup.
const FADE_IN: Duration = Duration::from_millis(250);
const POPUP_TITLE: &str = "Clipboard History Popup";

type T = cosmic::iced::Task<Message>;

#[derive(Debug, Clone)]
pub struct KeyEvent {
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
    /// A focused widget (a text input) already consumed it.
    captured: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    Command(Command),
    Popup(popup::Msg),
    OverlayOpened(window::Id, Size),
    OverlayMoved(Point),
    OverlayPress,
    OverlayRelease,
    PointerRetry(window::Id),
    PointerWait(window::Id),
    PopupWindowOpened(window::Id),
    WindowScale(window::Id, f32),
    Key(KeyEvent),
    Focus(window::Id, bool),
    Tick,
    /// A frame of the fade-in animation.
    Frame,
    Pasted(Result<(), String>),
    Settings(settings_ui::Msg),
    SettingsClosed(window::Id),
    Wizard(wizard::Msg),
    WizardClosed(window::Id),
    Noop,
}

struct Overlay {
    id: window::Id,
    /// Card top-left; `None` until the pointer's first position (or the wait ends).
    pos: Option<Point>,
    size: Size,
    sized: bool,
    cursor: Point,
    grab: Option<Vector>,
}

enum Surface {
    None,
    Overlay(Overlay),
    Window { id: window::Id, scale: f32 },
}

pub struct App {
    core: Core,
    flags: Flags,
    settings: AppSettings,
    backend: Arc<BackendService>,
    hooks: Arc<Hooks>,
    tray: Option<TrayControl>,
    popup: Popup,
    surface: Surface,
    visible: bool,
    /// Show requested while the persistent window was still opening.
    show_when_open: bool,
    last_version: u64,
    recent_emojis: Vec<String>,
    focus_loss_armed: Option<Instant>,
    /// When the popup was last shown (drives the 250 ms fade-in).
    shown_at: Option<Instant>,
    /// Blur state last sent to the compositor (None = never).
    blurred: Option<bool>,
    pending_title: Option<window::Id>,
    /// libcosmic-level tasks (theme, scale) queued by `handle`.
    extra: Vec<Task<Message>>,
    settings_win: Option<(window::Id, settings_ui::State)>,
    wizard_win: Option<(window::Id, wizard::State)>,
}

fn app_task(task: T) -> Task<Message> {
    task.map(cosmic::Action::App)
}

impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, flags: Flags) -> (Self, Task<Message>) {
        // libcosmic blurs every window it did not create itself, which would cover
        // the whole overlay surface; the card asks for its own blur region instead.
        core.set_auto_blur(Auto::Popup | Auto::System);
        let settings = settings::load();
        let backend = BackendService::new(&settings);
        let hooks = Hooks::new(flags.commands.clone());
        let tray = TrayControl::start(
            flags.commands.clone(),
            TrayState { dynamic_icon: settings.enable_dynamic_tray_icon, dark: cosmic::theme::active().cosmic().is_dark },
        );
        let mut app = App {
            core,
            settings,
            last_version: backend.version(),
            backend,
            hooks,
            tray: Some(tray),
            popup: Popup::new(),
            surface: Surface::None,
            visible: false,
            show_when_open: false,
            recent_emojis: Vec::new(),
            focus_loss_armed: None,
            shown_at: None,
            blurred: None,
            pending_title: None,
            extra: Vec::new(),
            settings_win: None,
            wizard_win: None,
            flags,
        };
        platform::backend().spawn_outside_click_watcher(app.hooks.clone());
        let mut tasks: Vec<T> = Vec::new();
        app.apply_theme_and_scale();
        if app.flags.open_wizard {
            tasks.push(app.open_wizard());
        } else if app.flags.open_settings {
            tasks.push(app.open_settings());
        } else if !app.flags.background {
            tasks.push(app.show(Tab::Clipboard));
        }
        let startup = std::mem::take(&mut app.extra);
        (app, Task::batch(startup.into_iter().chain([app_task(T::batch(tasks))])))
    }

    fn view(&self) -> Element<'_, Message> {
        widget::space().into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if let Some((win, state)) = self.settings_win.as_ref().filter(|(w, _)| *w == id) {
            let focused = self.core.focused_window() == Some(*win);
            return settings_ui::view(state, &self.settings, focused).map(Message::Settings);
        }
        if let Some((win, state)) = self.wizard_win.as_ref().filter(|(w, _)| *w == id) {
            let focused = self.core.focused_window() == Some(*win);
            return wizard::view(state, focused).map(Message::Wizard);
        }
        let ctx = self.view_ctx();
        match &self.surface {
            Surface::Overlay(ov) if ov.id == id => {
                // No position yet (COSMIC sends no pointer position until it moves, so the
                // card waits up to POINTER_WAIT): draw nothing rather than a card in a corner.
                let Some(pos) = ov.pos else {
                    return widget::mouse_area(widget::space().width(Length::Fill).height(Length::Fill))
                        .on_move(Message::OverlayMoved)
                        .on_press(Message::OverlayPress)
                        .into();
                };
                let card = self.popup.view(&ctx).map(Message::Popup);
                let card = widget::mouse_area(card);
                widget::mouse_area(
                    widget::container(card)
                        .padding(Padding { top: pos.y.max(0.0), left: pos.x.max(0.0), right: 0.0, bottom: 0.0 })
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .on_move(Message::OverlayMoved)
                .on_press(Message::OverlayPress)
                .on_release(Message::OverlayRelease)
                .into()
            }
            Surface::Window { id: win, .. } if *win == id => self.popup.view(&ctx).map(Message::Popup),
            _ => widget::space().into(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let mut tasks = vec![app_task(self.handle(message))];
        if let Some(id) = self.pending_title.take() {
            tasks.push(self.set_window_title(POPUP_TITLE.into(), id));
        }
        tasks.append(&mut self.extra);
        Task::batch(tasks)
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![Subscription::run(command_stream)];
        if self.visible {
            subs.push(event::listen_with(|ev, status, id| match ev {
                cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(Message::Key(KeyEvent {
                        key,
                        modifiers,
                        captured: matches!(status, event::Status::Captured),
                    }))
                }
                cosmic::iced::Event::Window(window::Event::Opened { size, .. }) => {
                    Some(Message::OverlayOpened(id, size))
                }
                // Whatever widget handled it, a released button ends a card drag.
                cosmic::iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    Some(Message::OverlayRelease)
                }
                cosmic::iced::Event::Mouse(mouse::Event::CursorLeft) => Some(Message::OverlayRelease),
                cosmic::iced::Event::Window(window::Event::Focused) => Some(Message::Focus(id, true)),
                cosmic::iced::Event::Window(window::Event::Unfocused) => Some(Message::Focus(id, false)),
                _ => None,
            }));
            subs.push(time::every(Duration::from_millis(250)).map(|_| Message::Tick));
            if self.fade() < 1.0 {
                subs.push(time::every(Duration::from_millis(16)).map(|_| Message::Frame));
            }
        } else if matches!(self.surface, Surface::Window { .. }) {
            // The persistent window announces itself while hidden too.
            subs.push(event::listen_with(|ev, _status, id| match ev {
                cosmic::iced::Event::Window(window::Event::Opened { size, .. }) => {
                    Some(Message::OverlayOpened(id, size))
                }
                _ => None,
            }));
        }
        Subscription::batch(subs)
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        if self.settings_win.as_ref().is_some_and(|(w, _)| *w == id) {
            return Some(Message::SettingsClosed(id));
        }
        if self.wizard_win.as_ref().is_some_and(|(w, _)| *w == id) {
            return Some(Message::WizardClosed(id));
        }
        None
    }
}

fn command_stream() -> impl cosmic::iced::futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(8, async |mut output| {
        use cosmic::iced::futures::SinkExt;
        let receiver = COMMANDS.lock().unwrap().take();
        let Some(mut receiver) = receiver else {
            std::future::pending::<()>().await;
            return;
        };
        while let Some(command) = receiver.recv().await {
            if output.send(Message::Command(command)).await.is_err() {
                break;
            }
        }
    })
}

impl App {
    fn view_ctx(&self) -> ViewCtx {
        let theme = cosmic::theme::active();
        let cosmic = theme.cosmic();
        let dark = cosmic.is_dark;
        let mut opacity = if dark { self.settings.dark_background_opacity } else { self.settings.light_background_opacity };
        if cosmic.frosted_windows {
            // COSMIC's "frosted" blur level sets how see-through its windows are
            // (Appearance → blur strength); the popup follows it, and only the user's
            // own slider can make it more transparent still.
            opacity = opacity.min(cosmic.background(true).base.alpha);
        }
        ViewCtx {
            pal: crate::win11::Pal::new(dark, opacity).faded(self.fade()),
            smart_actions: self.settings.enable_smart_actions,
            recent_emojis: self.recent_emojis.clone(),
            custom_kaomoji: self.custom_kaomoji(),
        }
    }

    /// 0..1 progress of the appear animation (1 once it is over).
    fn fade(&self) -> f32 {
        match self.shown_at {
            Some(at) => (at.elapsed().as_secs_f32() / FADE_IN.as_secs_f32()).clamp(0.0, 1.0),
            None => 1.0,
        }
    }

    fn custom_kaomoji(&self) -> Vec<app_shared::pickers::Kaomoji> {
        self.settings
            .custom_kaomojis
            .iter()
            .enumerate()
            .map(|(i, c)| app_shared::pickers::Kaomoji {
                id: format!("custom-{i}"),
                text: c.text.clone(),
                category: c.category.clone(),
                keywords: c.keywords.clone(),
            })
            .collect()
    }

    /// Card size in px, UI scale included.
    fn card_size(&self) -> (f32, f32) {
        (POPUP_W * self.settings.ui_scale, POPUP_H * self.settings.ui_scale)
    }

    fn window_card_size(&self) -> (f32, f32) {
        self.card_size()
    }

    /// Queue the COSMIC theme choice and the UI scale for libcosmic.
    fn apply_theme_and_scale(&mut self) {
        let theme = match self.settings.theme_mode.as_str() {
            "dark" => cosmic::theme::system_dark(),
            "light" => cosmic::theme::system_light(),
            _ => cosmic::theme::system_preference(),
        };
        self.extra.push(cosmic::command::set_theme(theme));
        crate::scale::set(self.settings.ui_scale);
    }

    fn refresh_pickers(&mut self) {
        self.recent_emojis = self.backend.recent_emojis().into_iter().map(|e| e.char).collect();
        let custom = self.custom_kaomoji();
        self.popup.pickers.refresh_all(&self.recent_emojis, &custom);
    }

    fn handle(&mut self, message: Message) -> T {
        match message {
            Message::Noop | Message::Frame => T::none(),
            Message::Command(command) => self.command(command),
            Message::Popup(msg) => self.popup_msg(msg),
            Message::Pasted(result) => {
                if let Err(e) = result {
                    eprintln!("[app] paste failed: {e}");
                }
                self.popup.items = self.backend.snapshot();
                T::none()
            }
            Message::Tick => {
                if self.visible && self.blurred.is_some_and(|on| on != glass::wanted()) {
                    // COSMIC's frosted-glass setting changed while the popup was open.
                    return self.blur_task();
                }
                if self.visible {
                    let version = self.backend.version();
                    if version != self.last_version {
                        self.last_version = version;
                        self.popup.items = self.backend.snapshot();
                        self.popup.warm_thumbs();
                    }
                    if self.settings.close_on_focus_loss {
                        if let Some(at) = self.focus_loss_armed {
                            if at.elapsed() >= FOCUS_LOSS_GRACE {
                                self.focus_loss_armed = None;
                                return self.hide();
                            }
                        }
                    }
                }
                T::none()
            }
            Message::Focus(id, focused) => {
                if self.is_popup_window(id) && self.visible {
                    if focused {
                        self.focus_loss_armed = None;
                    } else {
                        self.focus_loss_armed = Some(Instant::now());
                    }
                }
                T::none()
            }
            Message::Key(key) => self.key(key),
            Message::OverlayOpened(id, size) => self.surface_opened(id, size),
            Message::OverlayMoved(point) => self.overlay_moved(point),
            Message::OverlayPress => {
                // Anything the card did not capture is outside it.
                if let Surface::Overlay(ov) = &self.surface {
                    let (w, h) = self.card_size();
                    let pos = ov.pos.unwrap_or_default();
                    let inside = ov.cursor.x >= pos.x
                        && ov.cursor.x < pos.x + w
                        && ov.cursor.y >= pos.y
                        && ov.cursor.y < pos.y + h;
                    if !inside {
                        return self.hide();
                    }
                }
                T::none()
            }
            Message::OverlayRelease => {
                if let Surface::Overlay(ov) = &mut self.surface {
                    ov.grab = None;
                }
                T::none()
            }
            Message::PointerRetry(id) => {
                if matches!(&self.surface, Surface::Overlay(ov) if ov.id == id && ov.pos.is_none()) {
                    platform::backend().prime_pointer();
                }
                T::none()
            }
            Message::PointerWait(id) => {
                let (w, h) = self.card_size();
                if let Surface::Overlay(ov) = &mut self.surface {
                    if ov.id == id && ov.pos.is_none() {
                        eprintln!("[app] overlay: no pointer report, bottom-center");
                        let mon = MonitorRect { x: 0, y: 0, w: ov.size.width as i32, h: ov.size.height as i32 };
                        let (x, y) = bottom_center(&mon, w as i32, h as i32);
                        ov.pos = Some(Point::new(x as f32, y as f32));
                        return self.overlay_ready();
                    }
                }
                T::none()
            }
            Message::PopupWindowOpened(id) => {
                self.pending_title = Some(id);
                // The scale decides the physical size the X11 backend looks for.
                window::scale_factor(id).map(move |scale| Message::WindowScale(id, scale))
            }
            Message::WindowScale(id, scale) => {
                if let Surface::Window { id: win, scale: s } = &mut self.surface {
                    if *win == id {
                        *s = scale;
                    }
                }
                if self.show_when_open {
                    self.show_when_open = false;
                    return self.place_and_show_window();
                }
                T::none()
            }
            Message::Settings(msg) => self.settings_message(msg),
            Message::SettingsClosed(id) => {
                if self.settings_win.as_ref().is_some_and(|(w, _)| *w == id) {
                    self.settings_win = None;
                    return window::close(id);
                }
                T::none()
            }
            Message::Wizard(msg) => self.wizard_message(msg),
            Message::WizardClosed(id) => {
                if self.wizard_win.as_ref().is_some_and(|(w, _)| *w == id) {
                    self.wizard_win = None;
                    let _ = app_shared::launcher::mark_first_run_complete();
                    return window::close(id);
                }
                T::none()
            }
        }
    }

    fn command(&mut self, command: Command) -> T {
        use app_shared::instance::AppSignal;
        match command {
            Command::HideRequested => {
                if self.visible {
                    self.hide()
                } else {
                    T::none()
                }
            }
            Command::Signal(AppSignal::Toggle) => {
                if self.visible {
                    self.hide()
                } else {
                    self.show(Tab::Clipboard)
                }
            }
            Command::Signal(AppSignal::OpenSettings) => self.open_settings(),
            Command::Signal(AppSignal::Quit) => {
                self.hooks.set_visible(false);
                cosmic::iced::exit()
            }
            Command::Signal(AppSignal::ThemeChanged) => {
                self.sync_tray();
                T::none()
            }
        }
    }

    fn sync_tray(&self) {
        if let Some(tray) = &self.tray {
            tray.set_state(TrayState {
                dynamic_icon: self.settings.enable_dynamic_tray_icon,
                dark: cosmic::theme::active().cosmic().is_dark,
            });
        }
    }

    fn is_popup_window(&self, id: window::Id) -> bool {
        match &self.surface {
            Surface::Overlay(ov) => ov.id == id,
            Surface::Window { id: win, .. } => *win == id,
            Surface::None => false,
        }
    }

    // --- Popup lifecycle ---

    fn show(&mut self, tab: Tab) -> T {
        focus_manager::save_focused_window();
        self.focus_loss_armed = None;
        let items = self.backend.snapshot();
        self.last_version = self.backend.version();
        self.popup.prepare_for_show(tab, items);
        self.refresh_pickers();
        self.popup.warm_thumbs();
        self.visible = true;
        self.shown_at = Some(Instant::now());
        self.hooks.set_visible(true);
        match platform::backend().popup_surface() {
            PopupSurface::Overlay => self.show_overlay(),
            PopupSurface::Window => self.show_window(),
        }
    }

    fn show_overlay(&mut self) -> T {
        let id = window::Id::unique();
        self.surface = Surface::Overlay(Overlay {
            id,
            pos: None,
            size: Size::new(1280.0, 800.0),
            sized: false,
            cursor: Point::ORIGIN,
            grab: None,
        });
        layer_surface::get_layer_surface(SctkLayerSurfaceSettings {
            id,
            layer: Layer::Overlay,
            // Typing must reach the search box without a click first.
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            output: IcedOutput::Active,
            namespace: "win11-clipboard-history".into(),
            margin: IcedMargin::default(),
            size: Some((None, None)),
            exclusive_zone: -1,
            size_limits: Limits::NONE,
            ..Default::default()
        })
    }

    fn show_window(&mut self) -> T {
        match &self.surface {
            Surface::Window { .. } => self.place_and_show_window(),
            _ => {
                self.show_when_open = true;
                let (w, h) = self.window_card_size();
                let (id, open) = window::open(window::Settings {
                    size: Size::new(w, h),
                    decorations: false,
                    transparent: true,
                    resizable: false,
                    platform_specific: window::settings::PlatformSpecific {
                        application_id: APP_ID.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
                self.surface = Surface::Window { id, scale: 1.0 };
                open.map(Message::PopupWindowOpened)
            }
        }
    }

    /// Persistent window: pick the cursor-follow position, place, map, focus.
    fn place_and_show_window(&mut self) -> T {
        let Surface::Window { id, scale } = &self.surface else {
            return T::none();
        };
        let (id, scale) = (*id, *scale);
        let backend = platform::backend();
        let frame = backend.coordinate_scale(scale);
        let (w, h) = self.window_card_size();
        let (pw, ph) = (w * frame, h * frame);
        let origin = match backend.cursor_position() {
            Some((cx, cy)) => {
                let mon = backend.monitor_at(cx, cy).unwrap_or(MonitorRect { x: 0, y: 0, w: 1920, h: 1080 });
                clamp_to_monitor(&mon, pw as i32, ph as i32, cx, cy)
            }
            None => {
                let mon = backend.monitor_at(0, 0).unwrap_or(MonitorRect { x: 0, y: 0, w: 1920, h: 1080 });
                bottom_center(&mon, pw as i32, ph as i32)
            }
        };
        focus_manager::note_popup_window(backend.popup_window_id());
        if let Err(e) = backend.place_popup((pw, ph), origin.0, origin.1) {
            eprintln!("[app] move popup failed: {e}");
        }
        if let Err(e) = backend.set_popup_visible((pw, ph), true) {
            eprintln!("[app] map popup failed: {e}");
        }
        // Some window managers place a re-mapped window themselves (winit sets no
        // position hint), so ask again now that it is mapped.
        if let Err(e) = backend.place_popup((pw, ph), origin.0, origin.1) {
            eprintln!("[app] move popup failed: {e}");
        }
        T::batch([window::gain_focus(id), self.blur_task()])
    }

    fn hide(&mut self) -> T {
        self.visible = false;
        self.hooks.set_visible(false);
        self.focus_loss_armed = None;
        let _ = focus_manager::restore_focused_window();
        focus_manager::note_popup_window(None);
        match std::mem::replace(&mut self.surface, Surface::None) {
            Surface::Overlay(ov) => layer_surface::destroy_layer_surface(ov.id),
            Surface::Window { id, scale } => {
                let backend = platform::backend();
                let (w, h) = self.window_card_size();
                let frame = backend.coordinate_scale(scale);
                if let Err(e) = backend.set_popup_visible((w * frame, h * frame), false) {
                    eprintln!("[app] unmap popup failed: {e}");
                }
                self.surface = Surface::Window { id, scale };
                T::none()
            }
            Surface::None => T::none(),
        }
    }

    /// The surface exists and its size is known: place the card, ask for blur,
    /// focus the search box when it is open.
    fn surface_opened(&mut self, id: window::Id, size: Size) -> T {
        match &mut self.surface {
            Surface::Overlay(ov) if ov.id == id => {
                ov.size = size;
                if ov.sized {
                    return T::none();
                }
                ov.sized = true;
                // The surface is mapped: have the compositor report where the pointer is.
                platform::backend().prime_pointer();
                T::batch([
                    T::perform(
                        async move {
                            tokio::time::sleep(POINTER_RETRY).await;
                            id
                        },
                        Message::PointerRetry,
                    ),
                    T::perform(
                        async move {
                            tokio::time::sleep(POINTER_WAIT).await;
                            id
                        },
                        Message::PointerWait,
                    ),
                ])
            }
            Surface::Window { id: win, .. } if *win == id => {
                // Window opened: nothing to do, placement is driven by the show request.
                T::none()
            }
            _ => T::none(),
        }
    }

    fn overlay_moved(&mut self, point: Point) -> T {
        let (w, h) = self.card_size();
        let Surface::Overlay(ov) = &mut self.surface else {
            return T::none();
        };
        ov.cursor = point;
        let clamp = |p: Point, size: Size| {
            let mon = MonitorRect { x: 0, y: 0, w: size.width as i32, h: size.height as i32 };
            let (x, y) = clamp_to_monitor(&mon, w as i32, h as i32, p.x as i32, p.y as i32);
            Point::new(x as f32, y as f32)
        };
        if ov.pos.is_none() {
            eprintln!("[app] overlay: pointer reported at ({:.0},{:.0})", point.x, point.y);
            ov.pos = Some(clamp(point, ov.size));
            return self.overlay_ready();
        }
        if let Some(grab) = ov.grab {
            ov.pos = Some(clamp(Point::new(point.x - grab.x, point.y - grab.y), ov.size));
            // The blurred region follows the card.
            return self.blur_task();
        }
        T::none()
    }

    /// The frosted look of COSMIC windows for Settings / the wizard, which the
    /// disabled automatic blur no longer gives: the whole window, when the setting is on.
    fn window_blur(&self, id: window::Id) -> T {
        if !glass::wanted() {
            return T::none();
        }
        let everything = Rectangle::new(Point::ORIGIN, Size::new(f32::MAX, f32::MAX));
        blur::blur(id, Some(vec![everything])).map(|()| Message::Noop)
    }

    /// Blur behind the card (only while COSMIC's frosted-glass setting is on).
    /// Rounded like the card; re-sent whenever the card moves or the setting changes.
    fn blur_task(&mut self) -> T {
        let (w, h) = self.card_size();
        let (id, x, y) = match &self.surface {
            Surface::Overlay(ov) => match ov.pos {
                Some(pos) => (ov.id, pos.x, pos.y),
                None => return T::none(),
            },
            Surface::Window { id, .. } => (*id, 0.0, 0.0),
            Surface::None => return T::none(),
        };
        let on = glass::wanted();
        self.blurred = Some(on);
        let regions = on.then(|| glass::rounded_regions(x, y, w, h, 12.0 * self.settings.ui_scale));
        blur::blur(id, regions).map(|()| Message::Noop)
    }

    /// The card has a position: blur behind it only, and focus the search field.
    fn overlay_ready(&mut self) -> T {
        let blur_task = self.blur_task();
        let focus = if self.popup.search_open { widget::text_input::focus(self.popup.search_id.clone()) } else { T::none() };
        T::batch([blur_task, focus])
    }

    // --- Popup messages ---

    fn popup_msg(&mut self, msg: popup::Msg) -> T {
        use popup::Msg;
        match msg {
            Msg::Noop => T::none(),
            Msg::Pressed => {
                self.hooks.note_pointer_down();
                T::none()
            }
            Msg::Close => self.hide(),
            Msg::DragStart => self.begin_drag(),
            Msg::Tab(tab) => {
                self.popup.tab = tab;
                self.popup.focused = 0;
                self.refresh_pickers();
                T::none()
            }
            Msg::Search(text) => {
                self.popup.search = text;
                self.popup.after_filter_change();
                self.scroll_list_to(0.0)
            }
            Msg::SearchOpen(open) => self.set_search_open(open),
            Msg::ToggleRegex => {
                self.popup.regex = !self.popup.regex;
                self.popup.after_filter_change();
                T::none()
            }
            Msg::Paste(id) => self.paste_item(&id),
            Msg::Pin(id) => {
                self.backend.toggle_pin(&id);
                self.popup.items = self.backend.snapshot();
                T::none()
            }
            Msg::Delete(id) => {
                self.backend.remove_item(&id);
                self.popup.items = self.backend.snapshot();
                let len = self.popup.visible_items().len();
                self.popup.focused = self.popup.focused.min(len.saturating_sub(1));
                T::none()
            }
            Msg::ClearAll => {
                self.backend.clear();
                self.popup.items = self.backend.snapshot();
                self.popup.focused = 0;
                self.scroll_list_to(0.0)
            }
            Msg::ToggleCompact => {
                self.popup.compact = !self.popup.compact;
                self.popup.save_ui_state();
                T::none()
            }
            Msg::TogglePinned => {
                self.popup.pinned_expanded = !self.popup.pinned_expanded;
                self.popup.save_ui_state();
                T::none()
            }
            Msg::Hover(id) => {
                self.popup.hovered = id;
                T::none()
            }
            Msg::OpenUrl(url) => {
                let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                T::none()
            }
            Msg::Scrolled(viewport) => {
                self.popup.scroll = viewport.absolute_offset().y;
                self.popup.viewport_h = viewport.bounds().height;
                T::none()
            }
            Msg::Picker(msg) => self.picker_msg(msg),
        }
    }

    fn set_search_open(&mut self, open: bool) -> T {
        self.popup.search_open = open;
        if open {
            widget::text_input::focus(self.popup.search_id.clone())
        } else {
            self.popup.search.clear();
            self.popup.after_filter_change();
            self.scroll_list_to(0.0)
        }
    }

    fn scroll_list_to(&mut self, y: f32) -> T {
        self.popup.scroll = y;
        scrollable::scroll_to(self.popup.list_id.clone().into(), scrollable::AbsoluteOffset { x: Some(0.0), y: Some(y) })
    }

    fn scroll_picker_to(&mut self, tab: Tab, y: f32) -> T {
        let Some(tabs) = self.popup.pickers.tabs_mut(tab) else {
            return T::none();
        };
        tabs.scroll = y;
        scrollable::scroll_to(tabs.grid_id.clone().into(), scrollable::AbsoluteOffset { x: Some(0.0), y: Some(y) })
    }

    fn paste_item(&mut self, id: &str) -> T {
        let Some(item) = self.popup.items.iter().find(|i| i.id == id).cloned() else {
            return T::none();
        };
        let hide = self.hide();
        let backend = self.backend.clone();
        // Off the UI thread, after the hide went out: the keystroke has to land in
        // the window that gets focus back, not in the popup.
        let paste = T::perform(
            async move { tokio::task::spawn_blocking(move || backend.paste(&item)).await.map_err(|e| e.to_string())? },
            Message::Pasted,
        );
        T::batch([hide, paste])
    }

    fn paste_text(&mut self, text: String, record_emoji: bool) -> T {
        let hide = self.hide();
        let backend = self.backend.clone();
        let paste = T::perform(
            async move {
                tokio::task::spawn_blocking(move || backend.paste_text(&text, record_emoji))
                    .await
                    .map_err(|e| e.to_string())?
            },
            Message::Pasted,
        );
        T::batch([hide, paste])
    }

    fn picker_msg(&mut self, msg: pickers_ui::Msg) -> T {
        use pickers_ui::Msg;
        match msg {
            Msg::Noop => T::none(),
            Msg::Hover(h) => {
                self.popup.pickers.hovered = h;
                T::none()
            }
            Msg::Search(tab, text) => {
                if let Some(tabs) = self.popup.pickers.tabs_mut(tab) {
                    tabs.search = text;
                    tabs.focused = 0;
                }
                let custom = self.custom_kaomoji();
                self.popup.pickers.refresh(tab, &self.recent_emojis, &custom);
                self.scroll_picker_to(tab, 0.0)
            }
            Msg::Category(tab, category) => {
                if let Some(tabs) = self.popup.pickers.tabs_mut(tab) {
                    tabs.category = category;
                    tabs.focused = 0;
                }
                let custom = self.custom_kaomoji();
                self.popup.pickers.refresh(tab, &self.recent_emojis, &custom);
                self.scroll_picker_to(tab, 0.0)
            }
            Msg::Scrolled(tab, viewport) => {
                if let Some(tabs) = self.popup.pickers.tabs_mut(tab) {
                    tabs.scroll = viewport.absolute_offset().y;
                }
                T::none()
            }
            Msg::PickEmoji(ch) => self.paste_text(ch, true),
            Msg::PickKaomoji(text) => self.paste_text(text, false),
            Msg::PickSymbol(index) => {
                let Some(symbol) = self.popup.pickers.symbol_list.get(index).cloned() else {
                    return T::none();
                };
                app_shared::pickers::record_symbol_usage(&symbol);
                self.paste_text(symbol.char, false)
            }
        }
    }

    // --- Keyboard ---

    fn key(&mut self, event: KeyEvent) -> T {
        use keyboard::key::Named;
        let KeyEvent { key, modifiers, captured } = event;
        let ctrl = modifiers.control() && !modifiers.alt();
        let tab = self.popup.tab;

        if ctrl && matches!(&key, keyboard::Key::Character(c) if c.eq_ignore_ascii_case("f")) {
            return if tab == Tab::Clipboard {
                let open = !self.popup.search_open;
                self.set_search_open(open)
            } else {
                let id = self.popup.pickers.tabs(tab).map(|t| t.search_id.clone());
                id.map(widget::text_input::focus).unwrap_or_else(T::none)
            };
        }
        if key == keyboard::Key::Named(Named::Escape) {
            return if tab == Tab::Clipboard && self.popup.search_open {
                self.set_search_open(false)
            } else {
                self.hide()
            };
        }
        if captured {
            return T::none();
        }

        if tab != Tab::Clipboard {
            return self.picker_key(tab, &key, ctrl, &modifiers);
        }
        match &key {
            keyboard::Key::Named(Named::Enter) => {
                match self.popup.focused_item_id() {
                    Some(id) => self.paste_item(&id),
                    None => T::none(),
                }
            }
            keyboard::Key::Named(Named::ArrowUp) => {
                self.popup.up();
                self.follow_focus()
            }
            keyboard::Key::Named(Named::ArrowDown) => {
                self.popup.move_focus(1);
                self.follow_focus()
            }
            keyboard::Key::Named(Named::Home) => {
                self.popup.focused = 0;
                self.scroll_list_to(0.0)
            }
            keyboard::Key::Named(Named::End) => {
                let len = self.popup.visible_items().len();
                self.popup.focused = len.saturating_sub(1);
                self.follow_focus()
            }
            keyboard::Key::Named(Named::ArrowLeft) => {
                self.popup.horizontal(true);
                self.after_tab_change()
            }
            keyboard::Key::Named(Named::ArrowRight) => {
                self.popup.horizontal(false);
                self.after_tab_change()
            }
            keyboard::Key::Character(c) if !modifiers.control() && !modifiers.alt() && !modifiers.logo() => {
                // Type to filter: open the search and start it with this character.
                self.popup.search_open = true;
                self.popup.search.push_str(c);
                self.popup.after_filter_change();
                T::batch([widget::text_input::focus(self.popup.search_id.clone()), self.scroll_list_to(0.0)])
            }
            _ => T::none(),
        }
    }

    fn after_tab_change(&mut self) -> T {
        self.refresh_pickers();
        self.follow_focus()
    }

    fn follow_focus(&mut self) -> T {
        match self.popup.scroll_target() {
            Some(y) => self.scroll_list_to(y),
            None => T::none(),
        }
    }

    fn picker_key(&mut self, tab: Tab, key: &keyboard::Key, ctrl: bool, modifiers: &keyboard::Modifiers) -> T {
        use keyboard::key::Named;
        let name = match key {
            keyboard::Key::Named(Named::ArrowUp) => Some("up"),
            keyboard::Key::Named(Named::ArrowDown) => Some("down"),
            keyboard::Key::Named(Named::ArrowLeft) => Some("left"),
            keyboard::Key::Named(Named::ArrowRight) => Some("right"),
            keyboard::Key::Named(Named::Home) => Some("home"),
            keyboard::Key::Named(Named::End) => Some("end"),
            keyboard::Key::Named(Named::PageUp) => Some("pageup"),
            keyboard::Key::Named(Named::PageDown) => Some("pagedown"),
            _ => None,
        };
        if ctrl && matches!(name, Some("left") | Some("right")) {
            self.popup.tab = if name == Some("left") { self.popup.tab.prev() } else { self.popup.tab.next() };
            self.popup.focused = 0;
            return self.after_tab_change();
        }
        if let Some(name) = name {
            if self.popup.pickers.nav(tab, name, ctrl) {
                return match self.popup.pickers.scroll_target(tab) {
                    Some(y) => self.scroll_picker_to(tab, y),
                    None => T::none(),
                };
            }
            return T::none();
        }
        match key {
            keyboard::Key::Named(Named::Enter) => match tab {
                Tab::Emoji => self.popup.pickers.emoji_at_focus().map(|e| self.paste_text(e, true)).unwrap_or_else(T::none),
                Tab::Kaomoji => self.popup.pickers.kaomoji_at_focus().map(|k| self.paste_text(k, false)).unwrap_or_else(T::none),
                Tab::Symbols => {
                    let focused = self.popup.pickers.symbol.focused;
                    self.picker_msg(pickers_ui::Msg::PickSymbol(focused))
                }
                Tab::Clipboard => T::none(),
            },
            keyboard::Key::Character(c) if !modifiers.control() && !modifiers.alt() && !modifiers.logo() => {
                let custom = self.custom_kaomoji();
                if let Some(tabs) = self.popup.pickers.tabs_mut(tab) {
                    tabs.search.push_str(c);
                    tabs.focused = 0;
                }
                self.popup.pickers.refresh(tab, &self.recent_emojis, &custom);
                let id = self.popup.pickers.tabs(tab).map(|t| t.search_id.clone());
                T::batch([id.map(widget::text_input::focus).unwrap_or_else(T::none), self.scroll_picker_to(tab, 0.0)])
            }
            _ => T::none(),
        }
    }

    // --- Dragging ---

    fn begin_drag(&mut self) -> T {
        match &mut self.surface {
            Surface::Overlay(ov) => {
                let pos = ov.pos.unwrap_or_default();
                ov.grab = Some(Vector::new(ov.cursor.x - pos.x, ov.cursor.y - pos.y));
                T::none()
            }
            Surface::Window { id, scale } => {
                let backend = platform::backend();
                let (w, h) = (POPUP_W * self.settings.ui_scale, POPUP_H * self.settings.ui_scale);
                let frame = backend.coordinate_scale(*scale);
                match backend.begin_drag((w * frame, h * frame)) {
                    Ok(mut drag) => {
                        app_shared::drag_log::log(drag.describe_start());
                        std::thread::spawn(move || loop {
                            match drag.tick() {
                                platform::Tick::Moved(..) | platform::Tick::Still => {}
                                platform::Tick::Finished(reason) => {
                                    app_shared::drag_log::log(drag.describe_finish(&reason));
                                    break;
                                }
                            }
                            std::thread::sleep(platform::DRAG_POLL_INTERVAL);
                        });
                        T::none()
                    }
                    // The compositor moves the window (the only legal move on Wayland).
                    Err(reason) => {
                        app_shared::drag_log::log(format!("client-side drag unavailable: {reason}"));
                        window::drag(*id)
                    }
                }
            }
            Surface::None => T::none(),
        }
    }

    // --- Settings / wizard windows ---

    fn open_settings(&mut self) -> T {
        if let Some((id, _)) = &self.settings_win {
            return window::gain_focus(*id);
        }
        let (id, open) = window::open(window::Settings {
            size: Size::new(480.0, 640.0),
            min_size: Some(Size::new(420.0, 480.0)),
            decorations: false,
            transparent: true,
            resizable: true,
            platform_specific: window::settings::PlatformSpecific {
                application_id: format!("{APP_ID}.settings"),
                ..Default::default()
            },
            ..Default::default()
        });
        self.settings_win = Some((id, settings_ui::State::new(&self.settings)));
        T::batch([open.map(|_| Message::Noop), self.window_blur(id)])
    }

    fn open_wizard(&mut self) -> T {
        if let Some((id, _)) = &self.wizard_win {
            return window::gain_focus(*id);
        }
        let (id, open) = window::open(window::Settings {
            size: Size::new(550.0, 650.0),
            decorations: false,
            transparent: true,
            resizable: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: format!("{APP_ID}.setup"),
                ..Default::default()
            },
            ..Default::default()
        });
        self.wizard_win = Some((id, wizard::State::default()));
        T::batch([open.map(|_| Message::Noop), self.window_blur(id)])
    }

    fn settings_message(&mut self, msg: settings_ui::Msg) -> T {
        let Some((id, state)) = self.settings_win.as_mut() else {
            return T::none();
        };
        let id = *id;
        let old_scale = self.settings.ui_scale;
        let outcome = settings_ui::update(state, &mut self.settings, msg);
        if outcome.save {
            let _ = settings::save(&self.settings);
            self.backend.set_max_history_size(self.settings.max_history_size);
            self.sync_tray();
        }
        let mut resize: T = T::none();
        if outcome.theme || outcome.scale {
            self.apply_theme_and_scale();
        }
        if (self.settings.ui_scale - old_scale).abs() > f32::EPSILON {
            // The persistent popup window was created at the old size: drop it so the
            // next show builds one at the new size.
            if self.visible {
                resize = self.hide();
            }
            if let Surface::Window { id: win, .. } = self.surface {
                self.surface = Surface::None;
                resize = T::batch([resize, window::close(win)]);
            }
        }
        if outcome.reopen_wizard {
            return T::batch([resize, self.open_wizard()]);
        }
        if outcome.drag {
            return T::batch([resize, window::drag(id)]);
        }
        if outcome.close {
            self.settings_win = None;
            return T::batch([resize, window::close(id)]);
        }
        resize
    }

    fn wizard_message(&mut self, msg: wizard::Msg) -> T {
        let Some((id, state)) = self.wizard_win.as_mut() else {
            return T::none();
        };
        let id = *id;
        let outcome = wizard::update(state, msg);
        if let Some(text) = outcome.copy {
            let _ = self.backend.copy_text(&text);
        }
        if outcome.drag {
            return window::drag(id);
        }
        if outcome.close {
            self.wizard_win = None;
            let close = window::close(id);
            return if outcome.finished && !self.visible {
                T::batch([close, self.show(Tab::Clipboard)])
            } else {
                close
            };
        }
        T::none()
    }
}

#[allow(dead_code)]
fn unused(_: mouse::Button) {}
