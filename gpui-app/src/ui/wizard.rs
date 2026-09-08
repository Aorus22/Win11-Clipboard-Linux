//! Setup wizard — port of `SetupWizard.tsx` (5 steps).
//!
//! Shown on first run (own marker) or `--setup`. Completion marks first-run done,
//! requests the popup window, and closes the wizard window. Backend calls are
//! synchronous (no spinners mid-call — statuses render after).

use std::sync::Arc;

use gpui::{
    ClickEvent, Context, FocusHandle, Focusable, KeyDownEvent, Render, SharedString, Window,
    div, prelude::*, px,
};

use super::icons::{self, icon};
use crate::app_state::{self, Shared};
use crate::backend::BackendService;
use crate::settings::system_prefers_dark;
use crate::theme;
use win11_clipboard_history_lib::{
    permission_checker::{self, PermissionStatus},
    shortcut_conflict_detector::ConflictDetectionResult,
    shortcut_setup,
};

const COPY_PATH: &str = "/usr/bin/win11-clipboard-history";

pub struct WizardState {
    pub shared: Shared,
    pub backend: Arc<BackendService>,
    pub focus: FocusHandle,
    pub step: usize,
    pub permissions: Option<PermissionStatus>,
    pub shortcut_tools: Option<shortcut_setup::ShortcutToolsStatus>,
    pub conflicts: Option<ConflictDetectionResult>,
    pub fixing: bool,
    pub fix_error: Option<String>,
    pub registering: bool,
    pub shortcut_registered: bool,
    pub show_manual: bool,
    pub resolving: bool,
    pub conflicts_resolved: bool,
    pub conflict_error: Option<String>,
    pub copied: bool,
    pub hovered_button: Option<String>,
}

impl WizardState {
    pub fn new(
        shared: Shared,
        backend: Arc<BackendService>,
        focus: FocusHandle,
        _cx: &mut gpui::App,
    ) -> Self {
        let mut this = Self {
            shared,
            backend,
            focus,
            step: 0,
            permissions: None,
            shortcut_tools: None,
            conflicts: None,
            fixing: false,
            fix_error: None,
            registering: false,
            shortcut_registered: false,
            show_manual: false,
            resolving: false,
            conflicts_resolved: false,
            conflict_error: None,
            copied: false,
            hovered_button: None,
        };
        this.refresh_all();
        this
    }

    fn is_dark(&self) -> bool {
        system_prefers_dark()
    }

    fn refresh_all(&mut self) {
        self.permissions = Some(permission_checker::check_permissions());
        self.shortcut_tools = Some(shortcut_setup::check_shortcut_tools());
        self.conflicts = Some(shortcut_setup::detect_conflicts());
    }

    fn fix_permissions(&mut self, cx: &mut Context<Self>) {
        self.fixing = true;
        self.fix_error = None;
        cx.notify();
        match permission_checker::fix_permissions_now() {
            Ok(_) => {
                self.permissions = Some(permission_checker::check_permissions());
            }
            Err(e) => self.fix_error = Some(e),
        }
        self.fixing = false;
        cx.notify();
    }

    fn resolve_conflicts(&mut self, cx: &mut Context<Self>) {
        self.resolving = true;
        self.conflict_error = None;
        cx.notify();
        match shortcut_setup::resolve_conflicts() {
            Ok(_) => {
                self.conflicts_resolved = true;
                self.conflicts = Some(shortcut_setup::detect_conflicts());
                self.shortcut_tools = Some(shortcut_setup::check_shortcut_tools());
            }
            Err(e) => self.conflict_error = Some(e),
        }
        self.resolving = false;
        cx.notify();
    }

    fn register_shortcut(&mut self, cx: &mut Context<Self>) {
        self.registering = true;
        cx.notify();
        match shortcut_setup::register_de_shortcut() {
            Ok(_) => {
                self.shortcut_registered = true;
                // Parity micro-delta: React advances after a 1.5s beat; advance at once.
                self.step = 3;
            }
            Err(_) => self.show_manual = true,
        }
        self.registering = false;
        cx.notify();
    }

    fn enable_autostart(&mut self, cx: &mut Context<Self>) {
        // Own autostart entry (never the Tauri one) — errors are non-fatal here.
        let _ = app_state::autostart_enable();
        self.step = 4;
        cx.notify();
    }

    fn complete(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        let _ = app_state::mark_first_run_complete();
        self.shared.lock().open_popup_requested = true;
        window.remove_window();
    }

    fn copy_path(&mut self, cx: &mut Context<Self>) {
        if self.backend.copy_text(COPY_PATH).is_ok() {
            self.copied = true;
            cx.notify();
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Enter advances on welcome/done steps; Esc closes (marks complete only on done).
        if event.keystroke.key.as_str() == "enter" && !event.keystroke.modifiers.control {
            match self.step {
                0 => {
                    self.step = 1;
                    cx.notify();
                }
                4 => self.complete(window, cx),
                _ => {}
            }
        }
    }
}

impl Focusable for WizardState {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

// --- Render ---

impl Render for WizardState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_dark = self.is_dark();
        div()
            .id("wizard-root")
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .p(px(24.))
            .bg(if is_dark {
                theme::dark::bg_primary()
            } else {
                theme::light::bg_primary()
            })
            .text_color(if is_dark {
                theme::dark::text_primary()
            } else {
                theme::light::text_primary()
            })
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .child(
                div()
                    .w_full()
                    .max_w(px(384.))
                    .flex()
                    .flex_col()
                    .child(self.render_step(cx))
                    .child(self.render_dots(cx)),
            )
    }
}

impl WizardState {
    fn button(
        &self,
        id: String,
        label: String,
        primary: bool,
        disabled: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut WizardState, &ClickEvent, &mut Window, &mut Context<WizardState>)
        + 'static,
    ) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        let hovered = self.hovered_button.as_deref() == Some(id.as_str());
        let id_owned = id.clone();
        div()
            .id(SharedString::from(format!("wizard-btn-{id}")))
            .px(px(20.))
            .py(px(10.))
            .rounded(px(8.))
            .text_size(px(12.25))
            .cursor_pointer()
            .opacity(if disabled { 0.5 } else { 1.0 })
            .text_color(if primary {
                theme::accent()
            } else if is_dark {
                theme::dark::text_secondary()
            } else {
                theme::light::text_secondary()
            })
            .bg(if hovered && !disabled {
                theme::tertiary_bg(is_dark, 0.85)
            } else {
                gpui::rgba(0x00000000)
            })
            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                this.hovered_button = if *hovering { Some(id_owned.clone()) } else { None };
                cx.notify();
            }))
            .on_click(cx.listener(move |this, event, window, cx| {
                if disabled {
                    return;
                }
                on_click(this, event, window, cx);
            }))
            .child(label)
            .into_any_element()
    }

    fn circle(&self, size: f32, glyph: &str, glyph_size: f32) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        div()
            .w(px(size))
            .h(px(size))
            .mx_auto()
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .mb(px(16.))
            .bg(theme::tertiary_bg(is_dark, 0.85))
            .text_color(if is_dark {
                theme::dark::text_secondary()
            } else {
                theme::light::text_secondary()
            })
            .child(icon(glyph, px(glyph_size)).flex_shrink_0())
            .into_any_element()
    }

    fn title(&self, text: &str, size: f32) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        div()
            .text_size(px(size))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .mb(px(if size > 18.0 { 8. } else { 4. }))
            .text_color(if is_dark {
                theme::dark::text_primary()
            } else {
                theme::light::text_primary()
            })
            .child(text.to_string())
            .into_any_element()
    }

    fn subtitle(&self, text: &str) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        div()
            .text_size(px(12.25))
            .text_color(if is_dark {
                theme::dark::text_secondary()
            } else {
                theme::light::text_secondary()
            })
            .child(text.to_string())
            .into_any_element()
    }

    fn status_card(
        &self,
        kind: &str,
        glyph: &str,
        body: gpui::AnyElement,
    ) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        let (bg, border, text) = match kind {
            "success" => (
                if is_dark { gpui::rgba(0x6ccb5f26) } else { theme::tint::green50() },
                if is_dark { gpui::rgba(0x6ccb5f33) } else { theme::tint::green200() },
                if is_dark { gpui::rgb(0x6ccb5f) } else { theme::tint::green700() },
            ),
            "warning" => (
                if is_dark { gpui::rgba(0xfcb90026) } else { theme::tint::amber50() },
                if is_dark { gpui::rgba(0xfcb90033) } else { theme::tint::amber200() },
                if is_dark { gpui::rgb(0xfcb900) } else { theme::tint::amber700() },
            ),
            _ => (
                if is_dark { gpui::rgba(0xff5f5f26) } else { theme::tint::red50() },
                if is_dark { gpui::rgba(0xff5f5f33) } else { theme::tint::red50() },
                if is_dark { gpui::rgb(0xff5f5f) } else { theme::tint::red600() },
            ),
        };
        div()
            .p(px(16.))
            .rounded(px(8.))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(12.))
            .text_size(px(12.25))
            .bg(bg)
            .border_1()
            .border_color(border)
            .text_color(text)
            .child(icon(glyph, px(20.)).flex_shrink_0())
            .child(div().flex_1().child(body))
            .into_any_element()
    }

    fn kbd(&self, text: &str) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        div()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(4.))
            .text_size(px(10.5))
            .bg(theme::tertiary_bg(is_dark, 0.85))
            .child(text.to_string())
            .into_any_element()
    }

    fn render_step(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.step {
            0 => self.render_welcome(cx),
            1 => self.render_permissions(cx),
            2 => self.render_shortcut(cx),
            3 => self.render_autostart(cx),
            _ => self.render_done(cx),
        }
    }

    fn render_welcome(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(self.circle(64.0, icons::ROCKET, 32.0))
            .child(self.title("Welcome to Clipboard History", 20.0))
            .child(
                div()
                    .text_size(px(12.25))
                    .mb(px(32.))
                    .text_color(if self.is_dark() {
                        theme::dark::text_secondary()
                    } else {
                        theme::light::text_secondary()
                    })
                    .child("A Windows 11-style clipboard manager for Linux. Let's set up a few things to get you started."),
            )
            .child(self.button("start".to_string(), "Get Started".to_string(), true, false, cx, |this, _, _, cx| {
                this.step = 1;
                cx.notify();
            }))
            .into_any_element()
    }

    fn render_permissions(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let accessible = self.permissions.as_ref().map(|p| p.uinput_accessible).unwrap_or(false);
        let suggestion = self.permissions.as_ref().map(|p| p.suggestion.clone()).unwrap_or_default();
        let fixing = self.fixing;
        let fix_error = self.fix_error.clone();
        div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(self.circle(56.0, icons::SHIELD, 28.0))
            .child(self.title("Input Permissions", 18.0))
            .child(
                div()
                    .text_size(px(12.25))
                    .mb(px(24.))
                    .text_color(if self.is_dark() {
                        theme::dark::text_secondary()
                    } else {
                        theme::light::text_secondary()
                    })
                    .child("Required to simulate Ctrl+V for pasting."),
            )
            .children((!suggestion.is_empty()).then(|| {
                div().mb(px(16.)).w_full().child(self.status_card(
                    if accessible { "success" } else { "warning" },
                    if accessible { icons::CHECK_CIRCLE } else { icons::ALERT_TRIANGLE },
                    div().child(suggestion).into_any_element(),
                ))
            }))
            .children(fix_error.map(|e| {
                div().mb(px(16.)).w_full().child(self.status_card(
                    "error",
                    icons::ALERT_TRIANGLE,
                    div().child(e).into_any_element(),
                ))
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .justify_center()
                    .children((!accessible).then(|| {
                        self.button("fix".to_string(), if fixing { "Fixing..." } else { "Fix Now" }.to_string(), false, fixing, cx, |this, _, _, cx| {
                            this.fix_permissions(cx);
                        })
                    }))
                    .child(self.button(
                        if accessible { "Continue" } else { "Skip" }.to_string(),
                        if accessible { "Continue" } else { "Skip" }.to_string(),
                        true,
                        false,
                        cx,
                        |this, _, _, cx| {
                            this.step = 2;
                            cx.notify();
                        },
                    )),
            )
            .into_any_element()
    }

    fn render_shortcut(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let de = self
            .shortcut_tools
            .as_ref()
            .map(|t| t.desktop_environment.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        let can_auto = self.shortcut_tools.as_ref().map(|t| t.can_register_automatically).unwrap_or(false);
        let conflict_count = self.conflicts.as_ref().map(|c| c.conflicts.len()).unwrap_or(0);
        let first_conflict = self.conflicts.as_ref().and_then(|c| c.conflicts.first().cloned());
        let can_resolve = self.conflicts.as_ref().map(|c| c.can_auto_resolve).unwrap_or(false);
        let registering = self.registering;
        let resolving = self.resolving;
        let mut body = div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(self.circle(56.0, icons::KEYBOARD, 28.0))
            .child(self.title("Keyboard Shortcut", 18.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .text_size(px(12.25))
                    .mb(px(16.))
                    .child(div().child("Set up".to_string()))
                    .child(self.kbd("Super + V"))
                    .child(div().child("to open clipboard.".to_string())),
            )
            .child(
                div()
                    .w_full()
                    .mb(px(16.))
                    .p(px(12.))
                    .rounded(px(8.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(12.25))
                    .bg(theme::tertiary_bg(self.is_dark(), 0.85))
                    .child(icon(icons::SETTINGS_GEAR, px(16.)).flex_shrink_0())
                    .child(div().child(format!("Detected: {de}"))),
            );
        // Conflict warning.
        if conflict_count > 0 && !self.conflicts_resolved {
            let detail = first_conflict
                .map(|c| format!("Super+V is already used by {} for \"{}\"", c.owner, c.current_action))
                .unwrap_or_default();
            body = body.child(
                div().w_full().mb(px(16.)).child(self.status_card(
                    "warning",
                    icons::ALERT_CIRCLE,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(div().child(format!(
                            "{conflict_count} shortcut conflict{} detected",
                            if conflict_count > 1 { "s" } else { "" }
                        )))
                        .child(div().text_size(px(10.5)).opacity(0.9).child(detail))
                        .children(can_resolve.then(|| {
                            self.button("resolve-conflicts".to_string(), if resolving { "Resolving..." } else { "Auto-Fix Conflicts" }.to_string(), false, resolving, cx, |this, _, _, cx| {
                                this.resolve_conflicts(cx);
                            })
                        }))
                        .children((!can_resolve).then(|| {
                            div()
                                .text_size(px(10.5))
                                .opacity(0.75)
                                .child("Manual resolution required. See instructions below.")
                                .into_any_element()
                        }))
                        .into_any_element(),
                )),
            );
        }
        if self.conflicts_resolved {
            body = body.child(
                div().w_full().mb(px(16.)).child(self.status_card(
                    "success",
                    icons::CHECK_CIRCLE,
                    div().child("Conflicts resolved! Super+V is now available.").into_any_element(),
                )),
            );
        }
        if let Some(err) = self.conflict_error.clone() {
            body = body.child(
                div().w_full().mb(px(16.)).child(self.status_card(
                    "error",
                    icons::ALERT_TRIANGLE,
                    div()
                        .flex()
                        .flex_col()
                        .child(div().child("Failed to resolve conflicts"))
                        .child(div().text_size(px(10.5)).opacity(0.9).child(err))
                        .into_any_element(),
                )),
            );
        }
        if self.shortcut_registered {
            body = body.child(
                div().w_full().mb(px(16.)).child(self.status_card(
                    "success",
                    icons::CHECK_CIRCLE,
                    div().child("Shortcut registered successfully!").into_any_element(),
                )),
            );
        }
        if self.show_manual {
            if let Some(manual) = self
                .shortcut_tools
                .as_ref()
                .map(|t| t.manual_instructions.clone())
            {
                let copied = self.copied;
                body = body
                    .child(
                        div().w_full().mb(px(12.)).child(self.status_card(
                            "warning",
                            icons::ALERT_CIRCLE,
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.))
                                .child(div().child("Manual Setup Required:"))
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .opacity(0.9)
                                        .child(manual),
                                )
                                .into_any_element(),
                        )),
                    )
                    .child(self.button(
                        if copied { "Copied!" } else { "Copy command path" }.to_string(),
                        if copied { "Copied!" } else { "Copy command path" }.to_string(),
                        false,
                        false,
                        cx,
                        |this, _, _, cx| this.copy_path(cx),
                    ));
            }
        }
        let show_register = can_auto && !self.shortcut_registered && !self.show_manual;
        let show_manual_btn = !can_auto && !self.show_manual;
        body = body.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .children(show_register.then(|| {
                    self.button(
                        if registering { "Registering..." } else { "Register Automatically" }.to_string(),
                        if registering { "Registering..." } else { "Register Automatically" }.to_string(),
                        true, registering, cx,
                        |this, _, _, cx| this.register_shortcut(cx),
                    )
                }))
                .children(show_manual_btn.then(|| {
                    self.button("Show Manual Instructions".to_string(), "Show Manual Instructions".to_string(), false, false, cx, |this, _, _, cx| {
                        this.show_manual = true;
                        cx.notify();
                    })
                }))
                .child(self.button(
                    if self.shortcut_registered || self.show_manual { "Continue" } else { "Skip" }.to_string(),
                    if self.shortcut_registered || self.show_manual { "Continue" } else { "Skip" }.to_string(),
                    self.shortcut_registered || self.show_manual,
                    false, cx,
                    |this, _, _, cx| {
                        this.step = 3;
                        cx.notify();
                    },
                )),
        );
        body.into_any_element()
    }

    fn render_autostart(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(self.circle(56.0, icons::ROCKET, 28.0))
            .child(self.title("Start on Login?", 18.0))
            .child(
                div()
                    .text_size(px(12.25))
                    .mb(px(24.))
                    .text_color(if self.is_dark() {
                        theme::dark::text_secondary()
                    } else {
                        theme::light::text_secondary()
                    })
                    .child("Start automatically when you log in?"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .justify_center()
                    .child(self.button("Yes, enable".to_string(), "Yes, enable".to_string(), true, false, cx, |this, _, _, cx| {
                        this.enable_autostart(cx);
                    }))
                    .child(self.button("No thanks".to_string(), "No thanks".to_string(), false, false, cx, |this, _, _, cx| {
                        this.step = 4;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_done(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(
                div()
                    .w(px(64.))
                    .h(px(64.))
                    .mx_auto()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .mb(px(24.))
                    .bg(if is_dark {
                        gpui::rgba(0x6ccb5f33)
                    } else {
                        theme::tint::green50()
                    })
                    .text_color(gpui::rgb(0x6ccb5f))
                    .child(icon(icons::CHECK_CIRCLE, px(32.)).flex_shrink_0()),
            )
            .child(self.title("You're all set!", 20.0))
            .child(
                div()
                    .text_size(px(12.25))
                    .mb(px(16.))
                    .text_color(if is_dark {
                        theme::dark::text_secondary()
                    } else {
                        theme::light::text_secondary()
                    })
                    .child("Press the shortcut anytime to open clipboard history."),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .mb(px(24.))
                    .child(icon(icons::KEYBOARD, px(16.)).flex_shrink_0())
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(6.))
                            .rounded(px(8.))
                            .text_size(px(12.25))
                            .border_1()
                            .border_color(if is_dark {
                                theme::dark::border_subtle()
                            } else {
                                theme::light::border()
                            })
                            .bg(theme::tertiary_bg(is_dark, 0.85))
                            .text_color(if is_dark {
                                theme::dark::text_primary()
                            } else {
                                theme::light::text_primary()
                            })
                            .child("Super + V"),
                    ),
            )
            .child(self.button("Start Using".to_string(), "Start Using".to_string(), true, false, cx, |this, _, window, cx| {
                this.complete(window, cx);
            }))
            .into_any_element()
    }

    fn render_dots(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        let step = self.step;
        div()
            .flex()
            .flex_row()
            .justify_center()
            .gap(px(8.))
            .mt(px(32.))
            .children((0..5).map(|i| {
                let clickable = i < step;
                div()
                    .id(("wizard-dot", i))
                    .h(px(6.))
                    .w(px(if i == step { 20. } else { 6. }))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if i == step {
                        theme::accent()
                    } else if i < step {
                        tertiary_text_fallback(is_dark)
                    } else if is_dark {
                        theme::dark::border()
                    } else {
                        theme::light::border()
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if clickable {
                            this.step = i;
                            cx.notify();
                        }
                    }))
                    .into_any_element()
            }))
            .into_any_element()
    }
}

fn tertiary_text_fallback(is_dark: bool) -> gpui::Rgba {
    if is_dark {
        theme::dark::text_tertiary()
    } else {
        theme::light::text_secondary()
    }
}
