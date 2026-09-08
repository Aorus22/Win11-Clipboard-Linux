//! Settings window — port of `SettingsApp.tsx` (all sections).
//!
//! Second GPUI window (480×520, decorated, centered), opened via `--settings`.
//! All edits apply live through `SharedConfig`; disk commit mirrors the React
//! save behavior (immediate, except opacity/ui-scale sliders: live visual,
//! commit on release).

use std::sync::Arc;

use gpui::{Context, FocusHandle, Focusable, KeyDownEvent, Render, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

use super::controls::{SliderId, TextField, slider, switch};
use super::icons::{self, icon};
use crate::app_state::{self, Shared};
use crate::backend::BackendService;
use crate::settings::{AppSettings, resolve_dark};
use crate::theme;
use win11_clipboard_history_lib::rendering_env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    fn as_str(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }
}

pub struct SettingsState {
    pub shared: Shared,
    pub backend: Arc<BackendService>,
    pub focus: FocusHandle,
    pub dragging: Option<SliderId>,
    pub kaomoji_input: TextField,
    pub autodelete_input: TextField,
    pub maxhistory_input: TextField,
    pub dark_opacity_focus: FocusHandle,
    pub light_opacity_focus: FocusHandle,
    pub uiscale_focus: FocusHandle,
    pub saved_flash: bool,
    pub save_error: Option<String>,
    pub shortcut_status: Option<Result<String, String>>,
    pub transparency_disabled: bool,
    pub transparency_reason: String,
}

impl SettingsState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shared: Shared,
        backend: Arc<BackendService>,
        focus: FocusHandle,
        kaomoji_focus: FocusHandle,
        autodelete_focus: FocusHandle,
        maxhistory_focus: FocusHandle,
        dark_opacity_focus: FocusHandle,
        light_opacity_focus: FocusHandle,
        uiscale_focus: FocusHandle,
        _cx: &mut gpui::App,
    ) -> Self {
        let settings = shared.lock().settings.clone();
        let auto_text = if settings.auto_delete_interval == 0 {
            String::new()
        } else {
            settings.auto_delete_interval.to_string()
        };
        let env = rendering_env::get_rendering_env();
        Self {
            shared,
            backend,
            focus,
            dragging: None,
            kaomoji_input: TextField::new(kaomoji_focus),
            autodelete_input: TextField::with(auto_text, autodelete_focus),
            maxhistory_input: TextField::with(settings.max_history_size.to_string(), maxhistory_focus),
            dark_opacity_focus,
            light_opacity_focus,
            uiscale_focus,
            saved_flash: false,
            save_error: None,
            shortcut_status: None,
            transparency_disabled: env.transparency_disabled,
            transparency_reason: env.reason.clone(),
        }
    }

    fn settings(&self) -> AppSettings {
        self.shared.lock().settings.clone()
    }

    fn is_dark(&self) -> bool {
        resolve_dark(&self.shared.lock().settings)
    }

    fn edit(&mut self, f: impl FnOnce(&mut AppSettings)) {
        f(&mut self.shared.lock().settings);
        self.saved_flash = false;
        self.save_error = None;
    }

    fn commit(&mut self) {
        match app_state::save_now(&self.shared, &self.backend) {
            Ok(()) => {
                self.saved_flash = true;
                self.save_error = None;
            }
            Err(e) => {
                self.saved_flash = false;
                self.save_error = Some(e);
            }
        }
    }

    // --- Setting actions (parity with SettingsApp handlers) ---

    fn set_theme(&mut self, mode: ThemeChoice, cx: &mut Context<Self>) {
        let mode = mode.as_str().to_string();
        self.edit(|s| s.theme_mode = mode);
        self.commit();
        cx.notify();
    }

    fn toggle_bool(&mut self, key: &str, cx: &mut Context<Self>) {
        self.edit(|s| match key {
            "smart" => s.enable_smart_actions = !s.enable_smart_actions,
            "polish" => s.enable_ui_polish = !s.enable_ui_polish,
            "tray" => s.enable_dynamic_tray_icon = !s.enable_dynamic_tray_icon,
            _ => {}
        });
        self.commit();
        cx.notify();
    }

    fn opacity_live(&mut self, dark: bool, value: f32, cx: &mut Context<Self>) {
        self.edit(|s| {
            if dark {
                s.dark_background_opacity = value;
            } else {
                s.light_background_opacity = value;
            }
        });
        cx.notify();
    }

    fn opacity_commit(&mut self, cx: &mut Context<Self>) {
        self.commit();
        cx.notify();
    }

    fn uiscale_live(&mut self, value: f32, cx: &mut Context<Self>) {
        self.edit(|s| s.ui_scale = value.clamp(0.5, 2.0));
        cx.notify();
    }

    fn uiscale_commit(&mut self, cx: &mut Context<Self>) {
        self.commit();
        cx.notify();
    }

    fn slider_step(&mut self, id: SliderId, dir: f32, cx: &mut Context<Self>) {
        let (min, max, step, dark) = match id {
            SliderId::DarkOpacity => (0.0, 1.0, 0.01, Some(true)),
            SliderId::LightOpacity => (0.0, 1.0, 0.01, Some(false)),
            SliderId::UiScale => (0.5, 2.0, 0.1, None),
        };
        if self.transparency_disabled && dark.is_some() {
            return;
        }
        let current = match id {
            SliderId::DarkOpacity => self.settings().dark_background_opacity,
            SliderId::LightOpacity => self.settings().light_background_opacity,
            SliderId::UiScale => self.settings().ui_scale,
        };
        let next = (current + dir * step).clamp(min, max);
        match dark {
            Some(d) => self.opacity_live(d, next, cx),
            None => self.uiscale_live(next, cx),
        }
        self.commit();
        cx.notify();
    }

    fn autodelete_commit(&mut self, cx: &mut Context<Self>) {
        let raw = self.autodelete_input.text.trim().to_string();
        let parsed = if raw.is_empty() {
            Some(0u64)
        } else {
            raw.parse::<u64>().ok()
        };
        match parsed {
            Some(interval) => {
                self.edit(|s| s.auto_delete_interval = interval);
                self.commit();
                self.autodelete_input.text = if interval == 0 {
                    String::new()
                } else {
                    interval.to_string()
                };
                self.autodelete_input.cursor = self.autodelete_input.text.len();
            }
            None => {
                // Parity: invalid input is ignored, field reverts.
                let current = self.settings().auto_delete_interval;
                self.autodelete_input.text = if current == 0 {
                    String::new()
                } else {
                    current.to_string()
                };
                self.autodelete_input.cursor = self.autodelete_input.text.len();
            }
        }
        cx.notify();
    }

    fn set_auto_unit(&mut self, unit: &str, cx: &mut Context<Self>) {
        let unit = unit.to_string();
        self.edit(|s| s.auto_delete_unit = unit);
        self.commit();
        cx.notify();
    }

    fn maxhistory_commit(&mut self, cx: &mut Context<Self>) {
        let raw = self.maxhistory_input.text.trim().to_string();
        let current = self.settings().max_history_size;
        let parsed = raw.parse::<usize>().unwrap_or(current).clamp(1, 100_000);
        self.edit(|s| s.max_history_size = parsed);
        self.commit();
        self.maxhistory_input.text = parsed.to_string();
        self.maxhistory_input.cursor = self.maxhistory_input.text.len();
        cx.notify();
    }

    fn kaomoji_add(&mut self, cx: &mut Context<Self>) {
        let val = self.kaomoji_input.text.trim().to_string();
        if val.is_empty() {
            return;
        }
        self.edit(|s| {
            s.custom_kaomojis.push(crate::settings::CustomKaomoji {
                text: val,
                category: "Custom".to_string(),
                keywords: vec!["custom".to_string()],
            })
        });
        self.commit();
        self.kaomoji_input.clear();
        cx.notify();
    }

    fn kaomoji_remove(&mut self, index: usize, cx: &mut Context<Self>) {
        self.edit(|s| {
            if index < s.custom_kaomojis.len() {
                s.custom_kaomojis.remove(index);
            }
        });
        self.commit();
        cx.notify();
    }

    fn register_shortcuts(&mut self, cx: &mut Context<Self>) {
        // Synchronous (parity outcome; no spinner — the call blocks briefly).
        // GNOME: own gpui-pathed registration. Elsewhere: manual instructions with
        // our binary path (backend DE writers target the Tauri wrapper — not reused).
        if crate::gnome_shortcut::is_gnome() {
            self.shortcut_status = Some(
                crate::gnome_shortcut::register()
                    .map(|_| "Shortcuts registered successfully!".to_string()),
            );
        } else {
            let exe = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "win11-clipboard-history-gpui".to_string());
            self.shortcut_status = Some(Err(format!(
                "Automatic registration supports GNOME. Bind Super+V to `{exe} --toggle` manually."
            )));
        }
        cx.notify();
    }

    fn reset_defaults(&mut self, cx: &mut Context<Self>) {
        self.edit(|s| *s = AppSettings::default());
        self.commit();
        let _ = app_state::reset_first_run();
        self.shared.lock().open_wizard_requested = true;
        // Resync field texts with defaults.
        self.autodelete_input.text.clear();
        self.autodelete_input.cursor = 0;
        self.maxhistory_input.text = self.settings().max_history_size.to_string();
        self.maxhistory_input.cursor = self.maxhistory_input.text.len();
        self.kaomoji_input.clear();
        cx.notify();
    }

    fn slider_end(&mut self, id: SliderId, to_max: bool, cx: &mut Context<Self>) {
        let (bound, dark) = match (id, to_max) {
            (SliderId::UiScale, false) => (0.5, None),
            (SliderId::UiScale, true) => (2.0, None),
            (SliderId::DarkOpacity, false) => (0.0, Some(true)),
            (SliderId::DarkOpacity, true) => (1.0, Some(true)),
            (SliderId::LightOpacity, false) => (0.0, Some(false)),
            (SliderId::LightOpacity, true) => (1.0, Some(false)),
        };
        match dark {
            Some(d) => self.opacity_live(d, bound, cx),
            None => self.uiscale_live(bound, cx),
        }
        self.commit();
        cx.notify();
    }

    fn close(&self, window: &mut Window) {
        window.remove_window();
    }

    // --- Keyboard ---

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();

        // Slider arrows (focused slider adjusts live + commits, like mouseUp).
        let focused_slider = if self.dark_opacity_focus.is_focused(window) {
            Some(SliderId::DarkOpacity)
        } else if self.light_opacity_focus.is_focused(window) {
            Some(SliderId::LightOpacity)
        } else if self.uiscale_focus.is_focused(window) {
            Some(SliderId::UiScale)
        } else {
            None
        };
        if let Some(id) = focused_slider {
            match key {
                "left" | "down" => self.slider_step(id, -1.0, cx),
                "right" | "up" => self.slider_step(id, 1.0, cx),
                "home" => self.slider_end(id, false, cx),
                "end" => self.slider_end(id, true, cx),
                _ => {}
            }
            cx.stop_propagation();
            return;
        }

        // Text fields.
        if self.kaomoji_input.focus.is_focused(window) {
            if key == "enter" {
                self.kaomoji_add(cx);
                cx.stop_propagation();
                return;
            }
            if self.kaomoji_input.handle_key(event) {
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }
        if self.autodelete_input.focus.is_focused(window) {
            if key == "enter" {
                self.autodelete_commit(cx);
                cx.stop_propagation();
                return;
            }
            if self.autodelete_input.handle_key(event) {
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }
        if self.maxhistory_input.focus.is_focused(window) {
            if key == "enter" {
                self.maxhistory_commit(cx);
                cx.stop_propagation();
                return;
            }
            if self.maxhistory_input.handle_key(event) {
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }

        if key == "escape" {
            self.close(window);
            cx.stop_propagation();
        }
    }
}

impl Focusable for SettingsState {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

// --- Render ---

impl Render for SettingsState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.settings();
        let is_dark = self.is_dark();
        div()
            .id("settings-root")
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(if is_dark {
                theme::dark::bg_primary()
            } else {
                theme::tint::settings_light_bg()
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
            .child(self.render_header(cx))
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .px(px(32.))
                    .pb(px(32.))
                    .flex()
                    .flex_col()
                    .gap(px(24.))
                    .child(self.render_appearance(&settings, is_dark, cx))
                    .child(self.render_autodelete(&settings, is_dark, window, cx))
                    .child(self.render_transparency(&settings, is_dark, window, cx))
                    .child(self.render_uiscale(&settings, is_dark, window, cx))
                    .child(self.render_history(&settings, is_dark, window, cx))
                    .child(self.render_kaomoji(&settings, is_dark, window, cx))
                    .child(self.render_features(&settings, is_dark, cx))
                    .child(self.render_shortcuts(is_dark, cx))
                    .child(self.render_reset(is_dark, cx)),
            )
            .child(self.render_footer(is_dark, window, cx))
    }
}

impl SettingsState {
    fn render_header(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let is_dark = self.is_dark();
        let show_pill = self.saved_flash || self.save_error.is_some();
        let is_error = self.save_error.is_some();
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(32.))
            .py(px(24.))
            .flex_shrink_0()
            .child(
                div().flex().flex_col().child(
                    div()
                        .text_size(px(24.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("Personalization"),
                ).child(
                    div()
                        .text_size(px(12.25))
                        .mt(px(4.))
                        .text_color(if is_dark {
                            theme::gray::g400()
                        } else {
                            theme::gray::g500()
                        })
                        .child("Customize the look and feel of your clipboard history"),
                ),
            )
            .child(
                div()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .min_w(px(100.))
                    .children(show_pill.then(|| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .px(px(12.))
                            .py(px(6.))
                            .rounded_full()
                            .text_size(px(10.5))
                            .bg(if is_error {
                                gpui::rgba(0xef444410)
                            } else if is_dark {
                                theme::white_pct(0.10)
                            } else {
                                theme::black_pct(0.05)
                            })
                            .text_color(if is_error {
                                theme::tint::red500()
                            } else if is_dark {
                                gpui::rgb(0xffffff)
                            } else {
                                gpui::rgb(0x000000)
                            })
                            .child(if is_error { "Error saving" } else { "Saved" }.to_string())
                    })),
            )
            .into_any_element()
    }

    fn card(&self, is_dark: bool, body: gpui::AnyElement) -> gpui::AnyElement {
        div()
            .rounded(px(12.))
            .border_1()
            .border_color(if is_dark {
                theme::white_pct(0.05)
            } else {
                gpui::rgba(0xe5e7eb99)
            })
            .bg(if is_dark {
                theme::dark::bg_secondary()
            } else {
                gpui::rgb(0xffffff)
            })
            .overflow_hidden()
            .child(body)
            .into_any_element()
    }

    fn card_head(
        &self,
        is_dark: bool,
        icon_name: &str,
        title: &str,
        desc: Option<&str>,
    ) -> gpui::AnyElement {
        div()
            .p(px(24.))
            .border_b_1()
            .border_color(if is_dark {
                theme::white_pct(0.05)
            } else {
                gpui::rgba(0xe5e7eb99)
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .p(px(8.))
                            .rounded(px(8.))
                            .bg(if is_dark {
                                theme::white_pct(0.05)
                            } else {
                                theme::gray::g100()
                            })
                            .child(icon(icon_name, px(24.)).flex_shrink_0()),
                    )
                    .child(
                        div().flex().flex_col()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title.to_string()),
                            )
                            .children(desc.map(|d| {
                                div()
                                    .text_size(px(10.5))
                                    .mt(px(2.))
                                    .text_color(if is_dark {
                                        theme::gray::g400()
                                    } else {
                                        theme::gray::g500()
                                    })
                                    .child(d.to_string())
                            })),
                    ),
            )
            .into_any_element()
    }

    fn section_title(&self, is_dark: bool, title: &str, desc: &str) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .mb(px(4.))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(if is_dark {
                        theme::gray::g400()
                    } else {
                        theme::gray::g500()
                    })
                    .child(desc.to_string()),
            )
            .into_any_element()
    }

    fn render_appearance(
        &mut self,
        settings: &AppSettings,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mode = settings.theme_mode.as_str();
        let modes = [
            (ThemeChoice::System, "System", icons::MONITOR),
            (ThemeChoice::Light, "light", icons::SUN),
            (ThemeChoice::Dark, "dark", icons::MOON),
        ];
        let mut cards: Vec<gpui::AnyElement> = Vec::new();
        for (choice, label, icon_name) in modes {
            let active = mode == choice.as_str();
            cards.push(
                div()
                    .id(("theme-card", choice as usize))
                    .relative()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.))
                    .p(px(16.))
                    .rounded(px(12.))
                    .border_2()
                    .border_color(if active {
                        theme::accent()
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .bg(if active {
                        gpui::rgba(0x0078d40d)
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .hover(|s| {
                        if active {
                            s
                        } else if is_dark {
                            s.bg(theme::white_pct(0.05))
                        } else {
                            s.bg(theme::gray::g100())
                        }
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_theme(choice, cx);
                    }))
                    .child(
                        div()
                            .w_full()
                            .h(px(52.))
                            .rounded(px(8.))
                            .flex()
                            .flex_row()
                            .overflow_hidden()
                            .border_1()
                            .border_color(if is_dark {
                                theme::white_pct(0.10)
                            } else {
                                theme::gray::g200()
                            })
                            .children(match choice {
                                ThemeChoice::System => vec![
                                    div().flex_1().bg(theme::light::bg_primary()).into_any_element(),
                                    div().flex_1().bg(theme::dark::bg_primary()).into_any_element(),
                                ],
                                ThemeChoice::Light => vec![
                                    div().flex_1().bg(theme::light::bg_primary()).into_any_element(),
                                ],
                                ThemeChoice::Dark => vec![
                                    div().flex_1().bg(theme::dark::bg_primary()).into_any_element(),
                                ],
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(icon(icon_name, px(16.)).flex_shrink_0())
                            .child(
                                div()
                                    .text_size(px(12.25))
                                    .text_color(if active {
                                        theme::accent()
                                    } else if is_dark {
                                        theme::gray::g300()
                                    } else {
                                        theme::gray::g700()
                                    })
                                    .child(label.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(12.))
                            .right(px(12.))
                            .w(px(16.))
                            .h(px(16.))
                            .rounded_full()
                            .border_2()
                            .border_color(if active {
                                theme::accent()
                            } else if is_dark {
                                theme::gray::g600()
                            } else {
                                theme::gray::g300()
                            })
                            .bg(if active {
                                theme::accent()
                            } else {
                                gpui::rgba(0x00000000)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .children(active.then(|| {
                                div()
                                    .w(px(6.))
                                    .h(px(6.))
                                    .rounded_full()
                                    .bg(gpui::rgb(0xffffff))
                            })),
                    )
                    .into_any_element(),
            );
        }
        let tray_on = settings.enable_dynamic_tray_icon;
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .child(self.card_head(is_dark, icons::MONITOR, "Appearance", None))
                .child(
                    div()
                        .p(px(24.))
                        .flex()
                        .flex_col()
                        .gap(px(20.))
                        .child(div().flex().flex_row().gap(px(16.)).children(cards))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(
                                    div().flex().flex_col()
                                        .child(
                                            div()
                                                .text_size(px(12.25))
                                                .child("Dynamic Tray Icon"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .mt(px(2.))
                                                .text_color(if is_dark {
                                                    theme::gray::g400()
                                                } else {
                                                    theme::gray::g500()
                                                })
                                                .child("Adapt tray icon color to system theme."),
                                        ),
                                )
                                .child(super::controls::switch(
                                    ("switch", 0),
                                    tray_on,
                                    is_dark,
                                    cx,
                                    |this, _, _, cx| this.toggle_bool("tray", cx),
                                )),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_autodelete(
        &mut self,
        settings: &AppSettings,
        is_dark: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let units = ["minutes", "hours", "days", "weeks"];
        let current_unit = settings.auto_delete_unit.clone();
        let mut pills: Vec<gpui::AnyElement> = Vec::new();
        for (i, unit) in units.iter().enumerate() {
            let active = current_unit == *unit;
            let unit = unit.to_string();
            let label = unit.clone();
            pills.push(
                div()
                    .id(("autodelete-unit", i))
                    .flex_1()
                    .py(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if active {
                        theme::accent()
                    } else if is_dark {
                        theme::white_pct(0.10)
                    } else {
                        theme::gray::g200()
                    })
                    .bg(if active {
                        theme::accent()
                    } else if is_dark {
                        theme::white_pct(0.05)
                    } else {
                        theme::gray::g100()
                    })
                    .text_size(px(10.5))
                    .text_color(if active {
                        gpui::rgb(0xffffff)
                    } else if is_dark {
                        theme::gray::g400()
                    } else {
                        theme::gray::g600()
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| {
                        if active {
                            s
                        } else if is_dark {
                            s.bg(theme::white_pct(0.10))
                        } else {
                            s.bg(theme::gray::g200())
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_auto_unit(&unit, cx);
                    }))
                    .child(label)
                    .into_any_element(),
            );
        }
        let interval = settings.auto_delete_interval;
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .child(self.section_title(is_dark, "Auto-delete History", "Automatically clear old clipboard items (except pinned)"))
                .child(
                    div()
                        .p(px(24.))
                        .pt(px(20.))
                        .flex()
                        .flex_col()
                        .gap(px(16.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(16.))
                                .child(
                                    div().flex_1().flex().flex_col().gap(px(8.))
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .ml(px(4.))
                                                .opacity(0.6)
                                                .child("Time value"),
                                        )
                                        .child(self.autodelete_input.render(
                                            "autodelete-input",
                                            "0 (Disabled)",
                                            is_dark,
                                            window,
                                            cx,
                                            |s| &mut s.autodelete_input,
                                        )),
                                )
                                .child(
                                    div().flex_1().flex().flex_col().gap(px(8.))
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .ml(px(4.))
                                                .opacity(0.6)
                                                .child("Time unit"),
                                        )
                                        .child(
                                            div().flex().flex_row().gap(px(8.)).children(pills),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .mt(px(16.))
                                .p(px(12.))
                                .rounded(px(8.))
                                .border_1()
                                .border_color(gpui::rgba(0x0078d41a))
                                .bg(gpui::rgba(0x0078d40d))
                                .text_size(px(11.))
                                .opacity(0.7)
                                .child(if interval == 0 {
                                    "Auto-delete is currently disabled.".to_string()
                                } else {
                                    format!(
                                        "Clipboard history items will be deleted after {} {}. Pinned items are never deleted.",
                                        interval, current_unit
                                    )
                                }),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl SettingsState {
    fn render_transparency(
        &mut self,
        settings: &AppSettings,
        is_dark: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let dimmed = self.transparency_disabled;
        let reason = self.transparency_reason.clone();
        let dark_val = settings.dark_background_opacity;
        let light_val = settings.light_background_opacity;
        let dark_pct = format!("{}%", (dark_val * 100.0).round() as i32);
        let light_pct = format!("{}%", (light_val * 100.0).round() as i32);
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .opacity(if dimmed { 0.6 } else { 1.0 })
                .child(self.section_title(is_dark, "Window Transparency", "Control the backdrop opacity intensity"))
                .children(dimmed.then(|| {
                    div()
                        .mx(px(24.))
                        .mt(px(24.))
                        .p(px(12.))
                        .rounded(px(8.))
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(12.))
                        .text_size(px(12.25))
                        .bg(if is_dark {
                            gpui::rgba(0xfcb9001a)
                        } else {
                            theme::tint::amber50()
                        })
                        .text_color(if is_dark {
                            theme::tint::yellow300()
                        } else {
                            theme::tint::yellow800()
                        })
                        .child(icon(icons::ALERT_TRIANGLE, px(20.)).flex_shrink_0())
                        .child(
                            div().flex().flex_col()
                                .child(div().text_size(px(10.5)).child(reason))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .mt(px(4.))
                                        .opacity(0.8)
                                        .child("Transparency and rounded window corners have been automatically disabled to prevent rendering artefacts."),
                                ),
                        )
                }))
                .child(
                    div()
                        .p(px(24.))
                        .flex()
                        .flex_col()
                        .gap(px(32.))
                        .child(self.opacity_row(
                            true, "Dark Mode Opacity", if dimmed { "100%".to_string() } else { dark_pct },
                            dark_val, dimmed, is_dark, window, cx,
                        ))
                        .child(self.opacity_row(
                            false, "Light Mode Opacity", if dimmed { "100%".to_string() } else { light_pct },
                            light_val, dimmed, is_dark, window, cx,
                        )),
                )
                .into_any_element(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn opacity_row(
        &self,
        dark: bool,
        label: &str,
        pct: String,
        value: f32,
        disabled: bool,
        is_dark: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let id = if dark { SliderId::DarkOpacity } else { SliderId::LightOpacity };
        let focus = if dark {
            &self.dark_opacity_focus
        } else {
            &self.light_opacity_focus
        };
        let dragging = self.dragging == Some(id);
        let focused = focus.is_focused(window);
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(12.25)).child(label.to_string()))
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .text_size(px(10.5))
                            .bg(if is_dark {
                                theme::black_pct(0.20)
                            } else {
                                theme::gray::g100()
                            })
                            .child(pct),
                    ),
            )
            .child(super::controls::slider(
                id, value, 0.0, 1.0, 0.01, disabled, dragging, focused,
                focus, is_dark, window, cx,
                move |this, v, cx| this.opacity_live(dark, v, cx),
                move |this, cx| this.opacity_commit(cx),
            ))
            .into_any_element()
    }

    fn render_uiscale(
        &mut self,
        settings: &AppSettings,
        is_dark: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let value = settings.ui_scale;
        let pct = format!("{}%", (value * 100.0).round() as i32);
        let dragging = self.dragging == Some(SliderId::UiScale);
        let focused = self.uiscale_focus.is_focused(window);
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .child(self.section_title(is_dark, "UI Scale", "Adjust the clipboard window size for your display"))
                .child(
                    div()
                        .p(px(24.))
                        .flex()
                        .flex_col()
                        .gap(px(16.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(div().text_size(px(12.25)).child("Clipboard Window Scale"))
                                .child(
                                    div()
                                        .px(px(8.))
                                        .py(px(4.))
                                        .rounded(px(4.))
                                        .text_size(px(10.5))
                                        .bg(if is_dark {
                                            theme::black_pct(0.20)
                                        } else {
                                            theme::gray::g100()
                                        })
                                        .child(pct),
                                ),
                        )
                        .child(super::controls::slider(
                            SliderId::UiScale, value, 0.5, 2.0, 0.1, false, dragging, focused,
                            &self.uiscale_focus, is_dark, window, cx,
                            |this, v, cx| this.uiscale_live(v, cx),
                            |this, cx| this.uiscale_commit(cx),
                        ))
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(if is_dark {
                                    theme::gray::g500()
                                } else {
                                    theme::gray::g400()
                                })
                                .child("This setting only affects the clipboard popup, not this settings window"),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_history(
        &mut self,
        settings: &AppSettings,
        is_dark: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let _ = settings;
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .child(self.section_title(is_dark, "History Settings", "Configure clipboard history behavior"))
                .child(
                    div()
                        .p(px(24.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(
                                    div().flex().flex_col()
                                        .child(div().text_size(px(12.25)).child("Maximum History Size"))
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .mt(px(2.))
                                                .text_color(if is_dark {
                                                    theme::gray::g400()
                                                } else {
                                                    theme::gray::g500()
                                                })
                                                .child("Number of clipboard items to keep (1 - 100,000)"),
                                        ),
                                )
                                .child(
                                    div().w(px(112.)).child(self.maxhistory_input.render(
                                        "maxhistory-input",
                                        "",
                                        is_dark,
                                        window,
                                        cx,
                                        |s| &mut s.maxhistory_input,
                                    )),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_kaomoji(
        &mut self,
        settings: &AppSettings,
        is_dark: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let customs = settings.custom_kaomojis.clone();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (idx, item) in customs.iter().enumerate() {
            let group = format!("kmo-{idx}");
            rows.push(
                div()
                    .group(group.clone())
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if is_dark {
                        theme::white_pct(0.10)
                    } else {
                        theme::gray::g200()
                    })
                    .bg(if is_dark {
                        theme::white_pct(0.05)
                    } else {
                        theme::gray::g100()
                    })
                    .child(
                        div()
                            .text_size(px(12.25))
                            .overflow_hidden()
                            .child(item.text.clone()),
                    )
                    .child(
                        div()
                            .id(("kmo-del", idx))
                            .p(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .opacity(0.0)
                            .group_hover(group, |s| s.opacity(1.0))
                            .text_color(theme::tint::red500())
                            .hover(|s| s.bg(gpui::rgba(0xef44441a)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.kaomoji_remove(idx, cx);
                            }))
                            .child(icon(icons::X, px(14.)).flex_shrink_0()),
                    )
                    .into_any_element(),
            );
        }
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .child(self.section_title(is_dark, "Custom Kaomoji", "Add your own personal kaomojis to the collection"))
                .child(
                    div()
                        .p(px(24.))
                        .flex()
                        .flex_col()
                        .gap(px(24.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.))
                                .child(self.kaomoji_input.render(
                                    "kaomoji-input",
                                    "( ˘ ³˘)♥",
                                    is_dark,
                                    window,
                                    cx,
                                    |s| &mut s.kaomoji_input,
                                ))
                                .child(
                                    div()
                                        .id("kaomoji-add")
                                        .px(px(16.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .bg(theme::accent())
                                        .text_color(gpui::rgb(0xffffff))
                                        .text_size(px(12.25))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .hover(|s| s.opacity(0.9))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.kaomoji_add(cx);
                                        }))
                                        .child("Add"),
                                ),
                        )
                        .child(if customs.is_empty() {
                            div()
                                .text_center()
                                .py(px(16.))
                                .text_size(px(12.25))
                                .opacity(0.6)
                                .text_color(if is_dark {
                                    theme::gray::g500()
                                } else {
                                    theme::gray::g400()
                                })
                                .child("No custom kaomojis yet")
                                .into_any_element()
                        } else {
                            div().flex().flex_col().gap(px(8.)).children(rows).into_any_element()
                        }),
                )
                .into_any_element(),
        )
    }

    fn toggle_row(
        &self,
        id: (&'static str, usize),
        label: &str,
        desc: &str,
        checked: bool,
        key: &'static str,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div().flex().flex_col()
                    .child(div().text_size(px(12.25)).child(label.to_string()))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(if is_dark {
                                theme::gray::g400()
                            } else {
                                theme::gray::g500()
                            })
                            .child(desc.to_string()),
                    ),
            )
            .child(super::controls::switch(id, checked, is_dark, cx, move |this, _, _, cx| {
                this.toggle_bool(key, cx);
            }))
            .into_any_element()
    }

    fn render_features(
        &self,
        settings: &AppSettings,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .child(self.section_title(is_dark, "Features", "Customize the functionality of the application"))
                .child(
                    div()
                        .p(px(24.))
                        .flex()
                        .flex_col()
                        .gap(px(24.))
                        .child(self.toggle_row(
                            ("switch", 1),
                            "Smart Actions",
                            "Automatically detect links, colors, and emails.",
                            settings.enable_smart_actions,
                            "smart",
                            is_dark,
                            cx,
                        ))
                        .child(self.toggle_row(
                            ("switch", 2),
                            "UI Polish",
                            "Enable animations and compact mode support.",
                            settings.enable_ui_polish,
                            "polish",
                            is_dark,
                            cx,
                        )),
                )
                .into_any_element(),
        )
    }

    fn render_shortcuts(&mut self, is_dark: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let rows = [
            ("Super + V", "Open Clipboard History"),
            ("Ctrl + Alt + V", "Alternative shortcut"),
            ("Super + .", "Open Emoji Picker"),
        ];
        let mut row_els: Vec<gpui::AnyElement> = Vec::new();
        for (i, (keys, desc)) in rows.iter().enumerate() {
            let last = i + 1 == rows.len();
            row_els.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(16.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(if last {
                        gpui::rgba(0x00000000)
                    } else if is_dark {
                        theme::white_pct(0.10)
                    } else {
                        theme::gray::g100()
                    })
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .text_size(px(10.5))
                            .bg(if is_dark {
                                theme::white_pct(0.10)
                            } else {
                                theme::gray::g200()
                            })
                            .text_color(if is_dark {
                                theme::gray::g200()
                            } else {
                                theme::gray::g700()
                            })
                            .child(keys.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(if is_dark {
                                theme::gray::g400()
                            } else {
                                theme::gray::g500()
                            })
                            .child(desc.to_string()),
                    )
                    .into_any_element(),
            );
        }
        let status = self.shortcut_status.clone();
        self.card(
            is_dark,
            div()
                .flex()
                .flex_col()
                .child(self.section_title(
                    is_dark,
                    "Keyboard Shortcuts",
                    "Register the application shortcuts in your desktop environment",
                ))
                .child(
                    div()
                        .p(px(24.))
                        .flex()
                        .flex_col()
                        .gap(px(16.))
                        .child(
                            div()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(if is_dark {
                                    theme::white_pct(0.10)
                                } else {
                                    theme::gray::g200()
                                })
                                .bg(if is_dark {
                                    theme::black_pct(0.20)
                                } else {
                                    theme::gray::g100()
                                })
                                .text_size(px(12.25))
                                .children(row_els),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(if is_dark {
                                    theme::gray::g500()
                                } else {
                                    theme::gray::g400()
                                })
                                .child("Shortcuts are only registered when you explicitly request it. If you removed a shortcut from your system settings and it was re-added, use this button to re-register only the ones you want."),
                        )
                        .children(status.clone().map(|res| match res {
                            Ok(msg) => div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(12.25))
                                .text_color(gpui::rgb(0x22c55e))
                                .child(icon(icons::CHECK, px(16.)).flex_shrink_0())
                                .child(msg)
                                .into_any_element(),
                            Err(msg) => div()
                                .p(px(12.))
                                .rounded(px(8.))
                                .bg(if is_dark {
                                    gpui::rgba(0xef44441a)
                                } else {
                                    theme::tint::red50()
                                })
                                .text_color(if is_dark {
                                    theme::tint::red400()
                                } else {
                                    theme::tint::red600()
                                })
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(div().text_size(px(10.5)).child("Registration failed"))
                                .child(div().text_size(px(10.5)).opacity(0.8).child(msg))
                                .into_any_element(),
                        }))
                        .child(
                            div()
                                .id("register-shortkeys")
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .px(px(20.))
                                .py(px(10.))
                                .rounded(px(8.))
                                .bg(theme::accent())
                                .text_color(gpui::rgb(0xffffff))
                                .text_size(px(12.25))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.9))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.register_shortcuts(cx);
                                }))
                                .child(icon(icons::PLUS, px(16.)).flex_shrink_0())
                                .child("Register Shortkeys in System"),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_reset(&mut self, is_dark: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .flex_row()
            .justify_end()
            .pt(px(8.))
            .child(
                div()
                    .id("settings-reset")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .text_size(px(12.25))
                    .cursor_pointer()
                    .text_color(if is_dark {
                        theme::gray::g400()
                    } else {
                        theme::gray::g500()
                    })
                    .hover(|s| {
                        let s = s.text_color(theme::tint::red500());
                        if is_dark {
                            s.bg(gpui::rgba(0xef44441a))
                        } else {
                            s.bg(theme::tint::red50())
                        }
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.reset_defaults(cx);
                    }))
                    .child(icon(icons::RESET, px(16.)).flex_shrink_0())
                    .child("Reset to defaults"),
            )
            .into_any_element()
    }

    fn render_footer(&self, is_dark: bool, window: &Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .px(px(32.))
            .py(px(20.))
            .border_t_1()
            .border_color(if is_dark {
                theme::white_pct(0.05)
            } else {
                theme::gray::g200()
            })
            .bg(if is_dark {
                gpui::rgba(0x2d2d2d80)
            } else {
                theme::gray::g100()
            })
            .flex()
            .flex_row()
            .justify_end()
            .flex_shrink_0()
            .child(
                div()
                    .id("settings-done")
                    .px(px(32.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .bg(theme::accent())
                    .text_color(gpui::rgb(0xffffff))
                    .text_size(px(12.25))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|this, _, window, _| {
                        this.close(window);
                    }))
                    .child("Done"),
            )
            .into_any_element()
    }
}
