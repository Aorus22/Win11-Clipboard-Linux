//! The popup state the outside-click watchers (evdev on X11, the extension on
//! GNOME) read, and the way they dismiss the popup.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use app_shared::platform::PopupHooks;
use tokio::sync::mpsc::UnboundedSender;

use crate::Command;

pub struct Hooks {
    visible: AtomicBool,
    seq: AtomicU64,
    last: Mutex<Option<Instant>>,
    commands: UnboundedSender<Command>,
}

impl Hooks {
    pub fn new(commands: UnboundedSender<Command>) -> Arc<Self> {
        Arc::new(Self {
            visible: AtomicBool::new(false),
            seq: AtomicU64::new(0),
            last: Mutex::new(None),
            commands,
        })
    }

    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::SeqCst);
    }

    /// The popup handled a pointer press itself.
    pub fn note_pointer_down(&self) {
        self.seq.fetch_add(1, Ordering::SeqCst);
        *self.last.lock().unwrap() = Some(Instant::now());
    }
}

impl PopupHooks for Hooks {
    fn popup_visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }

    fn pointer_down_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    fn last_pointer_down(&self) -> Option<Instant> {
        *self.last.lock().unwrap()
    }

    fn request_hide(&self) {
        let _ = self.commands.send(Command::HideRequested);
    }
}
