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

pub fn prev_char_boundary(text: &str, cursor: usize) -> usize {
    let mut i = cursor.min(text.len());
    while i > 0 {
        i -= 1;
        if text.is_char_boundary(i) {
            break;
        }
    }
    i
}

pub fn next_char_boundary(text: &str, cursor: usize) -> usize {
    let mut i = (cursor + 1).min(text.len());
    while i < text.len() && !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Shared single-line editing semantics (search fields + settings text fields):
/// Backspace/Delete/arrows/Home/End + printable-char insertion, plus a
/// minimal selection model: Ctrl+A (Cmd+A parity) selects all, Shift+arrows
/// extend, destructive edits replace the selection. Returns consumed.
pub fn edit_text(
    text: &mut String,
    cursor: &mut usize,
    anchor: &mut Option<usize>,
    event: &KeyDownEvent,
) -> bool {
    *cursor = (*cursor).min(text.len());
    if let Some(a) = anchor {
        *a = (*a).min(text.len());
    }
    let key = event.keystroke.key.as_str();
    let mods = &event.keystroke.modifiers;
    // Select-all before the control-bail below.
    if (mods.control || mods.platform) && !mods.alt && key == "a" {
        *anchor = Some(0);
        *cursor = text.len();
        return true;
    }
    if mods.control || mods.alt || mods.platform {
        return false;
    }
    let shift = mods.shift;
    match key {
        "backspace" => {
            if !take_selection(text, cursor, anchor) && *cursor > 0 {
                let prev = prev_char_boundary(text, *cursor);
                text.drain(prev..*cursor);
                *cursor = prev;
            }
            *anchor = None;
            true
        }
        "delete" => {
            if !take_selection(text, cursor, anchor) && *cursor < text.len() {
                let next = next_char_boundary(text, *cursor);
                text.drain(*cursor..next);
            }
            *anchor = None;
            true
        }
        "left" => {
            move_cursor(cursor, anchor, shift, prev_char_boundary(text, *cursor));
            true
        }
        "right" => {
            let next = next_char_boundary(text, *cursor);
            // next_char_boundary steps past the end; clamp (it never
            // returns > len, but the anchor math below needs cursor ≤ len).
            move_cursor(cursor, anchor, shift, next.min(text.len()));
            true
        }
        "home" => {
            move_cursor(cursor, anchor, shift, 0);
            true
        }
        "end" => {
            move_cursor(cursor, anchor, shift, text.len());
            true
        }
        _ => {
            let ch = event
                .keystroke
                .key_char
                .clone()
                .unwrap_or_else(|| event.keystroke.key.clone());
            if ch.chars().count() == 1 {
                take_selection(text, cursor, anchor);
                text.insert_str(*cursor, &ch);
                *cursor += ch.len();
                *anchor = None;
                true
            } else {
                false
            }
        }
    }
}

/// Delete the selected range, if any, leaving the cursor at its start.
fn take_selection(text: &mut String, cursor: &mut usize, anchor: &mut Option<usize>) -> bool {
    if let Some(a) = anchor.take() {
        let (lo, hi) = (a.min(*cursor), a.max(*cursor));
        if lo != hi {
            text.drain(lo..hi);
            *cursor = lo;
            return true;
        }
    }
    false
}

/// Move the cursor, extending the selection while Shift is held.
fn move_cursor(cursor: &mut usize, anchor: &mut Option<usize>, shift: bool, next: usize) {
    if shift {
        if anchor.is_none() {
            *anchor = Some(*cursor);
        }
        *cursor = next;
        if *anchor == Some(*cursor) {
            *anchor = None;
        }
    } else {
        *cursor = next;
        *anchor = None;
    }
}

pub struct SearchState {
    pub text: String,
    /// Cursor as a char-boundary byte index into `text`.
    pub cursor: usize,
    /// Selection anchor (byte index); the selection spans anchor..cursor.
    /// `None` (or equal to cursor) means no selection.
    pub sel_anchor: Option<usize>,
    pub focus: FocusHandle,
    pub regex_mode: bool,
}

impl SearchState {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            sel_anchor: None,
            focus,
            regex_mode: false,
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.sel_anchor = None;
    }

    fn prev_boundary(&self) -> usize {
        prev_char_boundary(&self.text, self.cursor)
    }

    fn next_boundary(&self) -> usize {
        next_char_boundary(&self.text, self.cursor)
    }

    /// Selected byte range (sorted, non-empty), if any.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let a = self.sel_anchor?;
        let c = self.cursor.min(self.text.len());
        let (lo, hi) = (a.min(c), a.max(c));
        (lo != hi).then_some((lo, hi))
    }

    /// Returns true if the keystroke was consumed.
    pub fn handle_key(&mut self, event: &KeyDownEvent) -> bool {
        edit_text(&mut self.text, &mut self.cursor, &mut self.sel_anchor, event)
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
            // The magnifier inherits this from the bar (icons take the color of
            // their container — see `icons::icon`).
            .text_color(dim)
            .on_click(cx.listener(move |this, _, window, cx| {
                // Focus THIS field (not always the clipboard one) and drop
                // any selection — a click places a fresh caret.
                let s = this.search_mut(which);
                s.focus.focus(window);
                s.sel_anchor = None;
                cx.notify();
            }))
            .child(icon(icons::SEARCH, px(16.)).flex_shrink_0())
            .child(self.render_text(text, dim, placeholder, focused))
            .child(self.render_buttons(which, is_dark, cx))
    }

    /// Single flex_1 text area: either the placeholder (left-aligned, when
    /// empty) or the text with caret + selection highlight. One child — never
    /// both — so the placeholder always starts at the field's left edge.
    fn render_text(
        &self,
        text: gpui::Rgba,
        dim: gpui::Rgba,
        placeholder: &str,
        focused: bool,
    ) -> impl IntoElement {
        // Selection wash: accent at ~30%.
        let selected = gpui::rgba(0x0078d44d);
        let caret = || div().w(px(1.5)).h(px(15.)).bg(theme::accent());
        let base = div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .text_size(px(12.25))
            .overflow_hidden();
        if self.text.is_empty() {
            base.text_color(dim)
                .children(focused.then(caret))
                .child(placeholder.to_string())
        } else {
            let cursor = self.cursor.min(self.text.len());
            let mut row = base.text_color(text);
            match self.selection_range() {
                Some((lo, hi)) => {
                    row = row.child(self.text[..lo].to_string());
                    if focused && cursor == lo {
                        row = row.child(caret());
                    }
                    row = row.child(
                        div()
                            .bg(selected)
                            .rounded(px(2.))
                            .child(self.text[lo..hi].to_string()),
                    );
                    if focused && cursor == hi {
                        row = row.child(caret());
                    }
                    row.child(self.text[hi..].to_string())
                }
                None => {
                    row = row.child(self.text[..cursor].to_string());
                    if focused {
                        row = row.child(caret());
                    }
                    row.child(self.text[cursor..].to_string())
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{KeyDownEvent, Keystroke, Modifiers};

    fn no_mods() -> Modifiers {
        Modifiers {
            control: false,
            alt: false,
            shift: false,
            platform: false,
            function: false,
        }
    }

    fn key(key: &str, mods: Modifiers) -> KeyDownEvent {
        // key_char mirrors what the platform reports for printable keys.
        let printable = !mods.control && !mods.platform && key.chars().count() == 1;
        KeyDownEvent {
            keystroke: Keystroke {
                modifiers: mods,
                key: key.to_string(),
                key_char: printable.then(|| key.to_string()),
            },
            is_held: false,
        }
    }

    fn ctrl_a() -> KeyDownEvent {
        key(
            "a",
            Modifiers {
                control: true,
                ..no_mods()
            },
        )
    }

    fn edit(text: &str, cursor: usize) -> (String, usize, Option<usize>) {
        (text.to_string(), cursor, None)
    }

    #[test]
    fn plain_typing_and_backspace_unchanged() {
        let (mut text, mut cursor, mut anchor) = edit("hi", 2);
        assert!(edit_text(&mut text, &mut cursor, &mut anchor, &key("!", no_mods())));
        assert_eq!((text.as_str(), cursor, anchor), ("hi!", 3, None));
        assert!(edit_text(
            &mut text,
            &mut cursor,
            &mut anchor,
            &key("backspace", no_mods())
        ));
        assert_eq!((text.as_str(), cursor, anchor), ("hi", 2, None));
    }

    #[test]
    fn ctrl_a_selects_all() {
        let (mut text, mut cursor, mut anchor) = edit("hello", 2);
        assert!(edit_text(&mut text, &mut cursor, &mut anchor, &ctrl_a()));
        assert_eq!(text.as_str(), "hello");
        assert_eq!((cursor, anchor), (5, Some(0)));
    }

    #[test]
    fn typing_replaces_selection() {
        let (mut text, mut cursor, mut anchor) = edit("hello", 5);
        edit_text(&mut text, &mut cursor, &mut anchor, &ctrl_a());
        assert!(edit_text(&mut text, &mut cursor, &mut anchor, &key("x", no_mods())));
        assert_eq!((text.as_str(), cursor, anchor), ("x", 1, None));
    }

    #[test]
    fn backspace_deletes_selection() {
        let (mut text, mut cursor, mut anchor) = edit("hello", 5);
        edit_text(&mut text, &mut cursor, &mut anchor, &ctrl_a());
        assert!(edit_text(
            &mut text,
            &mut cursor,
            &mut anchor,
            &key("backspace", no_mods())
        ));
        assert_eq!((text.as_str(), cursor, anchor), ("", 0, None));
    }

    #[test]
    fn shift_arrow_extends_and_plain_arrow_collapses() {
        let (mut text, mut cursor, mut anchor) = edit("hello", 2);
        let shift = Modifiers {
            shift: true,
            ..no_mods()
        };
        assert!(edit_text(&mut text, &mut cursor, &mut anchor, &key("right", shift)));
        assert_eq!((cursor, anchor), (3, Some(2)));
        // Plain arrow collapses the selection.
        assert!(edit_text(
            &mut text,
            &mut cursor,
            &mut anchor,
            &key("left", no_mods())
        ));
        assert_eq!((cursor, anchor), (2, None));
        assert_eq!(text.as_str(), "hello");
    }

    #[test]
    fn ctrl_other_keys_still_ignored() {
        let (mut text, mut cursor, mut anchor) = edit("hi", 2);
        let ctrl_c = key(
            "c",
            Modifiers {
                control: true,
                ..no_mods()
            },
        );
        assert!(!edit_text(&mut text, &mut cursor, &mut anchor, &ctrl_c));
        assert_eq!((text.as_str(), cursor, anchor), ("hi", 2, None));
    }
}
