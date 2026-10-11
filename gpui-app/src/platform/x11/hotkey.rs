//! X11-only global hotkeys via the `global-hotkey` crate.
//!
//! Wayland deliberately skips grabs (protocol limitation — same constraint the
//! Tauri build works around with DE-level registration). There the `--toggle`
//! IPC path (DE shortcut Exec) is the toggle mechanism.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::instance::AppSignal;
use win11_clipboard_history_lib::session;

pub struct Hotkeys {
    _manager: GlobalHotKeyManager,
    ids: Vec<u32>,
}

/// Register Super+V and Ctrl+Alt+V on X11. Returns `None` on Wayland or when
/// the platform backend refuses (logged, non-fatal — tray/`--toggle` remain).
pub fn register_hotkeys() -> Option<Hotkeys> {
    if session::is_wayland() {
        eprintln!("[hotkey] Wayland session: skipping key grabs (use DE shortcut → --toggle)");
        return None;
    }
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[hotkey] manager unavailable: {e}");
            return None;
        }
    };
    let defs = [
        (Modifiers::SUPER, Code::KeyV),
        (
            Modifiers::CONTROL | Modifiers::ALT,
            Code::KeyV,
        ),
    ];
    let mut ids = Vec::new();
    for (mods, code) in defs {
        let hotkey = HotKey::new(Some(mods), code);
        let id = hotkey.id();
        match manager.register(hotkey) {
            Ok(()) => ids.push(id),
            Err(e) => eprintln!("[hotkey] register failed: {e}"),
        }
    }
    if ids.is_empty() {
        return None;
    }
    eprintln!("[hotkey] grabbed {} shortcut(s) on X11", ids.len());
    Some(Hotkeys {
        _manager: manager,
        ids,
    })
}

impl Hotkeys {
    /// Drain pressed events for our ids into toggle signals. Main poll loop calls this.
    pub fn poll(&self) -> Vec<AppSignal> {
        let mut out = Vec::new();
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state == HotKeyState::Pressed && self.ids.contains(&event.id) {
                out.push(AppSignal::Toggle);
            }
        }
        out
    }
}
