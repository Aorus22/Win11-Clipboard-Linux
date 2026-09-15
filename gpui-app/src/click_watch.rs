//! Global outside-click dismissal for focus-follows-mouse desktops.
//!
//! The popup's focus-loss watcher cannot distinguish "hover moved focus to
//! another app" (must stay open) from "the user clicked elsewhere" (must
//! close), and X11 never sees clicks that land on Wayland-native surfaces
//! anyway. So button presses are read straight from evdev
//! (`/dev/input/event*`); a press the popup window did not receive requests a
//! hide through the same shared flag the focus watcher uses.
//!
//! Deciding *where* the press happened must not come from X. Under Wayland the
//! X pointer is frozen while the real pointer is over a Wayland-native window
//! (Zed, Files, any GTK/Qt app on Wayland): X keeps reporting the last position
//! it had over an X surface, which is our own popup, so every outside click
//! looks like an inside one — the bug that made dismissal work over XWayland
//! apps (Electron/Freebuff) and fail over native Wayland ones. Instead the
//! popup reports the presses it handles (`Shared::popup_pointer_down_seq`) and
//! a press the popup did not handle is outside by definition. The short wait
//! covers the gap between the kernel handing us the event and the compositor
//! delivering the click to the window.
//!
//! Requires read access to `/dev/input` (the user must be in the `input`
//! group). When no event device is readable, the watcher disables itself
//! with a one-time log line and physical-click dismissal is unavailable
//! (focus-loss dismissal still applies, subject to its own setting).

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::app_state::Shared;

const EV_KEY: u16 = 0x01;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const PRESS: i32 = 1;
/// How long to wait for the popup window to report the click it received. The
/// kernel hands us the press before the compositor has delivered it to the
/// window, so the answer only exists a few milliseconds later.
const DELIVERY_GRACE: Duration = Duration::from_millis(80);
/// A press whose recorded time is older than this is not the press we are
/// looking at. Guards the case where this thread was descheduled long enough
/// for the popup to record the click before we sampled the counter.
const RECENT_DOWN: Duration = Duration::from_millis(150);
/// `struct input_event` on 64-bit: timeval(16) + type(2) + code(2) + value(4).
const EVENT_SIZE: usize = 24;

/// Spawn the supervisor thread. Returns immediately; never fails — worst case
/// the supervisor logs that it disabled itself.
pub fn spawn_click_watcher(shared: Shared) {
    std::thread::Builder::new()
        .name("gpui-click-watch".to_string())
        .spawn(move || supervise(shared))
        .expect("spawn click watcher thread");
}

fn supervise(shared: Shared) {
    let mut known: HashSet<PathBuf> = HashSet::new();
    let mut logged_disabled = false;
    loop {
        match list_event_devices() {
            Ok(devices) => {
                // Forget vanished nodes so a replugged device is picked up.
                known.retain(|path| devices.contains(path));
                let mut readable = !known.is_empty();
                for device in &devices {
                    if known.contains(device) {
                        continue;
                    }
                    // Probe readability before spawning a reader for it.
                    if File::open(device).is_ok() {
                        readable = true;
                        known.insert(device.clone());
                        spawn_reader(device.clone(), shared.clone());
                    }
                }
                if !readable && !logged_disabled {
                    logged_disabled = true;
                    eprintln!(
                        "[click-watch] disabled: no readable /dev/input/event* \
                         (add the user to the `input` group for outside-click dismissal)"
                    );
                }
            }
            Err(error) => eprintln!("[click-watch] device scan failed: {error}"),
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

fn list_event_devices() -> std::io::Result<Vec<PathBuf>> {
    let mut devices = Vec::new();
    for entry in std::fs::read_dir("/dev/input")? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("event") {
            continue;
        }
        if name["event".len()..].chars().all(|c| c.is_ascii_digit()) {
            devices.push(entry.path());
        }
    }
    Ok(devices)
}

fn spawn_reader(path: PathBuf, shared: Shared) {
    let label = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "event?".to_string());
    let spawn_result = std::thread::Builder::new()
        .name(format!("gpui-click-{label}"))
        .spawn({
            let shared = shared.clone();
            move || read_loop(&path, &shared)
        });
    if spawn_result.is_err() {
        eprintln!(
            "[click-watch] failed to spawn reader for {}",
            label
        );
    }
}

/// Blocking read loop; exits on any I/O error (unplug/EOF) — the supervisor
/// respawns for the node if it reappears.
fn read_loop(path: &Path, shared: &Shared) {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("[click-watch] could not open {}: {error}", path.display());
            return;
        }
    };
    crate::drag_log::log(&format!(
        "[click-watch] reader attached: {}",
        path.display()
    ));
    let mut record = [0u8; EVENT_SIZE];
    loop {
        match file.read_exact(&mut record) {
            Ok(_) => {}
            Err(error) => {
                eprintln!("[click-watch] reader {} exited: {error}", path.display());
                return;
            }
        }
        let kind = u16::from_le_bytes([record[16], record[17]]);
        let code = u16::from_le_bytes([record[18], record[19]]);
        let value = i32::from_le_bytes([record[20], record[21], record[22], record[23]]);
        if kind == EV_KEY
            && (code == BTN_LEFT || code == BTN_RIGHT || code == BTN_MIDDLE)
            && value == PRESS
        {
            on_button_press(shared);
        }
    }
}

/// A physical button went down somewhere: hide the popup iff it is visible and
/// this press did not land on it.
fn on_button_press(shared: &Shared) {
    let press = Instant::now();
    let seq_before = {
        let state = shared.lock();
        if !state.popup_visible {
            return;
        }
        state.popup_pointer_down_seq
    };
    // The popup records its presses on the UI thread; give it a moment, then
    // look at whether a new one arrived.
    std::thread::sleep(DELIVERY_GRACE);
    let (inside, seq_after, last_down) = {
        let state = shared.lock();
        if !state.popup_visible {
            // Hidden while we waited (another press, paste, Escape): nothing to
            // dismiss, and a stale hide request could close the next show.
            return;
        }
        let last_down = state.popup_last_pointer_down;
        (
            press_was_inside(seq_before, state.popup_pointer_down_seq, last_down, press),
            state.popup_pointer_down_seq,
            last_down,
        )
    };
    crate::drag_log::log(&format!(
        "[click-watch] press: popup_presses {seq_before}->{seq_after} last_down={} -> {}",
        last_down
            .map(|down| format!("{:?} ago", press.saturating_duration_since(down)))
            .unwrap_or_else(|| "never".to_string()),
        if inside { "inside -> keep" } else { "outside -> hide" }
    ));
    if !inside {
        shared.lock().hide_popup_requested = true;
    }
}

/// Did the popup window itself handle the press we just saw?
///
/// `seq_before` is the popup's press counter as we sampled it when the event
/// came off the device, `seq_after`/`last_down` are read back after
/// [`DELIVERY_GRACE`]. A counter bump means the popup handled this very press —
/// a press it had handled earlier would already be part of `seq_before` — and
/// that is the normal case. The timestamp is the fallback for the reverse
/// race: this thread was descheduled long enough that the popup had already
/// recorded the click before we sampled the counter.
fn press_was_inside(
    seq_before: u64,
    seq_after: u64,
    last_down: Option<Instant>,
    press: Instant,
) -> bool {
    if seq_after != seq_before {
        return true;
    }
    last_down.is_some_and(|down| {
        down <= press + DELIVERY_GRACE && press.saturating_duration_since(down) < RECENT_DOWN
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_the_popup_counted_is_inside() {
        let press = Instant::now();
        assert!(press_was_inside(7, 8, Some(press), press));
    }

    #[test]
    fn press_the_popup_ignored_is_outside() {
        let press = Instant::now();
        assert!(!press_was_inside(7, 7, None, press));
        let long_ago = press.checked_sub(Duration::from_secs(2));
        assert!(!press_was_inside(7, 7, long_ago, press));
    }

    #[test]
    fn click_recorded_before_we_sampled_still_counts_as_inside() {
        // This thread woke late: the popup already had the click.
        let press = Instant::now();
        let down = press.checked_sub(Duration::from_millis(20));
        assert!(press_was_inside(7, 7, down, press));
    }

    #[test]
    fn inside_click_from_earlier_is_not_this_press() {
        let press = Instant::now();
        let down = press.checked_sub(Duration::from_millis(400));
        assert!(!press_was_inside(7, 7, down, press));
    }
}
