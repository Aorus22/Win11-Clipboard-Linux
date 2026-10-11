//! Helpers shared by the headless UI tests (pattern from iced-wayland-widget).

/// The headless renderer crashes when several are created at once, so every test that
/// builds a `Simulator` holds this lock.
pub(crate) static HEADLESS: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn headless() -> std::sync::MutexGuard<'static, ()> {
    HEADLESS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

use iced_test::selector::{Candidate, Text};

/// Selects the first plain text widget reading `label`. Unlike `&str` it never inspects
/// text inputs, whose state libcosmic leaves unimplemented for the selector.
pub(crate) fn text(label: &'static str) -> impl FnMut(Candidate<'_>) -> Option<Text> {
    let mut seen = 0;
    move |candidate| match candidate {
        Candidate::Text { id, bounds, visible_bounds, content } if content == label => {
            seen += 1;
            (seen == 1).then(|| Text::Raw { id: id.cloned(), bounds, visible_bounds })
        }
        _ => None,
    }
}

use cosmic::iced::Rectangle;
use iced_test::selector::Bounded;

/// A plain area to click, for labels the simulator reports as not visible.
#[derive(Debug, Clone)]
pub(crate) struct Area(pub Rectangle);

impl Bounded for Area {
    fn bounds(&self) -> Rectangle {
        self.0
    }
    fn visible_bounds(&self) -> Option<Rectangle> {
        Some(self.0)
    }
}

/// The first text widget reading `label`, clickable even when its visible bounds are unknown.
pub(crate) fn text_area(label: &'static str) -> impl FnMut(Candidate<'_>) -> Option<Area> {
    let mut seen = 0;
    move |candidate| match candidate {
        Candidate::Text { bounds, content, .. } if content == label => {
            seen += 1;
            (seen == 1).then(|| Area(bounds))
        }
        _ => None,
    }
}
