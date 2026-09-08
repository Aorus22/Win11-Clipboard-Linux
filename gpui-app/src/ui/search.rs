//! Search field — port of `SearchBar.tsx`.
//!
//! Single-line editor (trimmed `examples/input.rs` pattern): typing, Backspace,
//! Delete, arrows, Home/End. Focus ring mirrors `focus:ring-2 ring-win11-bg-accent`.
//! Clicking the field or its buttons never steals focus (plain divs with
//! `on_click` are not focusable), matching the React `tabIndex={-1}` buttons.

use gpui::{Context, FocusHandle, KeyDownEvent, Window, div, prelude::*, px};

use super::icons::{self, icon};
use super::popup::{Popup, SearchWhich};
use crate::theme;

pub struct SearchState {
    pub text: String,
    /// Cursor as a char-boundary byte index into `text`.
    pub cursor: usize,
    pub focus: FocusHandle,
    pub regex_mode: bool,
}

impl SearchState {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            focus,
            regex_mode: false,
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    fn prev_boundary(&self) -> usize {
        let mut i = self.cursor;
        while i > 0 {
            i -= 1;
            if self.text.is_char_boundary(i) {
                break;
            }
        }
        i
    }

    fn next_boundary(&self) -> usize {
        let mut i = (self.cursor + 1).min(self.text.len());
        while i < self.text.len() && !self.text.is_char_boundary(i) {
            i += 1;
        }
        i
    }

    /// Returns true if the keystroke was consumed.
    pub fn handle_key(&mut self, event: &KeyDownEvent) -> bool {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;
        if mods.control || mods.alt || mods.platform {
            return false;
        }
        match key {
            "backspace" => {
                if self.cursor > 0 {
                    let prev = self.prev_boundary();
                    self.text.drain(prev..self.cursor);
                    self.cursor = prev;
                }
                true
            }
            "delete" => {
                if self.cursor < self.text.len() {
                    let next = self.next_boundary();
                    self.text.drain(self.cursor..next);
                }
                true
            }
            "left" => {
                self.cursor = self.prev_boundary();
                true
            }
            "right" => {
                self.cursor = self.next_boundary();
                true
            }
            "home" => {
                self.cursor = 0;
                true
            }
            "end" => {
                self.cursor = self.text.len();
                true
            }
            _ => {
                // Printable single char (mirrors `isPrintableKey`: len 1, no modifiers).
                let ch = event
                    .keystroke
                    .key_char
                    .clone()
                    .unwrap_or_else(|| event.keystroke.key.clone());
                if ch.chars().count() == 1 {
                    self.text.insert_str(self.cursor, &ch);
                    self.cursor += ch.len();
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn render(&self, is_dark: bool, opacity: f32, window: &Window, cx: &mut Context<Popup>) -> impl IntoElement {
        self.render_for(SearchWhich::Clipboard, "Search history...", is_dark, opacity, window, cx)
    }

    pub fn render_for(
        &self,
        which: SearchWhich,
        placeholder: &str,
        is_dark: bool,
        opacity: f32,
        window: &Window,
        cx: &mut Context<Popup>,
    ) -> impl IntoElement {
        // Colors mirror SearchBar.tsx exactly.
        let text = if is_dark {
            theme::dark::text_primary()
        } else {
            theme::light::text_primary()
        };
        let dim = theme::dark::text_disabled();
        let bg = theme::tertiary_bg(is_dark, opacity);
        let focused = self.focus.is_focused(window);
        div()
            .id("search-bar")
            .flex()
            .flex_row()
            .items_center()
            .h(px(36.))
            .px(px(12.))
            .gap(px(8.))
            .rounded(px(8.))
            .bg(bg)
            .border_2()
            .border_color(if focused {
                theme::accent()
            } else {
                gpui::rgba(0x00000000)
            })
            .track_focus(&self.focus)
            .on_click(cx.listener(|this, _, window, cx| {
                this.search.focus.focus(window);
                cx.notify();
            }))
            .child(icon(icons::SEARCH, px(16.)).text_color(dim).flex_shrink_0())
            .child(self.render_text(text))
            .children(self.text.is_empty().then(|| {
                div()
                    .flex_1()
                    .text_size(px(12.25))
                    .text_color(dim)
                    .child(placeholder.to_string())
            }))
            .child(self.render_buttons(which, is_dark, cx))
    }

    fn render_text(&self, text: gpui::Rgba) -> impl IntoElement {
        let cursor = self.cursor.min(self.text.len());
        let (before, after) = self.text.split_at(cursor);
        div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .text_size(px(12.25))
            .text_color(text)
            .overflow_hidden()
            .child(before.to_string())
            // Caret: thin accent bar between the two runs — correct position by construction.
            .child(div().w(px(1.5)).h(px(15.)).bg(theme::accent()))
            .child(after.to_string())
    }

    fn render_buttons(&self, which: SearchWhich, is_dark: bool, cx: &mut Context<Popup>) -> impl IntoElement {
        let secondary = if is_dark {
            theme::dark::text_secondary()
        } else {
            theme::light::text_secondary()
        };
        let dim = theme::dark::text_disabled();
        let hover_bg = if is_dark {
            theme::dark::bg_card_hover()
        } else {
            theme::light::bg_card_hover()
        };
        let primary = if is_dark {
            theme::dark::text_primary()
        } else {
            theme::light::text_primary()
        };
        let has_text = !self.text.is_empty();
        let regex_mode = self.regex_mode;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .children(has_text.then(|| clear_button(which, dim, primary, hover_bg, cx)))
            .child(regex_button(which, regex_mode, secondary, hover_bg, cx))
    }
}

fn clear_button(
    which: SearchWhich,
    dim: gpui::Rgba,
    primary: gpui::Rgba,
    hover_bg: gpui::Rgba,
    cx: &mut Context<Popup>,
) -> impl IntoElement {
    div()
        .id("search-clear")
        .p(px(4.))
        .rounded(px(4.))
        .cursor_pointer()
        .text_color(dim)
        .hover(move |s| s.text_color(primary).bg(hover_bg))
        .on_click(cx.listener(move |this, _, _, cx| {
            let s = this.search_mut(which);
            s.clear();
            this.after_picker_filter_change(which);
            cx.notify();
        }))
        .child(icon(icons::X, px(14.)).flex_shrink_0())
}

fn regex_button(
    which: SearchWhich,
    regex_mode: bool,
    secondary: gpui::Rgba,
    hover_bg: gpui::Rgba,
    cx: &mut Context<Popup>,
) -> impl IntoElement {
    div()
        .id("search-regex")
        .p(px(4.))
        .rounded(px(4.))
        .cursor_pointer()
        .bg(if regex_mode {
            gpui::rgba(0x0078d422)
        } else {
            gpui::rgba(0x00000000)
        })
        .text_color(if regex_mode {
            theme::accent()
        } else {
            secondary
        })
        .hover(move |s| {
            s.bg(if regex_mode {
                gpui::rgba(0x0078d422)
            } else {
                hover_bg
            })
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            let s = this.search_mut(which);
            s.regex_mode = !s.regex_mode;
            this.after_picker_filter_change(which);
            cx.notify();
        }))
        .child(icon(icons::REGEX, px(14.)).flex_shrink_0())
}
