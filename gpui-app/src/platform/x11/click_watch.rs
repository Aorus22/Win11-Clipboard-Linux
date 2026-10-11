//! Global outside-click dismissal for focus-follows-mouse desktops.
//!
//! The popup's focus-loss watcher cannot distinguish "hover moved focus to
//! another app" (must stay open) from "the user clicked elsewhere" (must
//! close), and X11 never sees clicks that land on Wayland-native surfaces
//! anyway. So button presses are read straight from evdev
//! (`/dev/input/event*`); a press the popup window did not receive requests a
//! hide through the same shared flag the focus watcher uses.
//!
//! Tap-to-click needs special handling: a tap on a touchpad often arrives
//! only as `BTN_TOUCH` down/up with no `BTN_LEFT` at all (libinput synthesizes
//! the click at the compositor level), so a watcher that only listens for
//! `BTN_LEFT/RIGHT/MIDDLE` is deaf to taps. A touch-down→release is therefore
//! treated as a click — but only when the gesture did not *move*: scrolling
//! with two fingers is the very same `BTN_TOUCH` down/up as a tap, and a quick
//! flick finishes well inside [`TAP_MAX`], which is what made every touchpad
//! scroll close the popup. Raw captures from this machine tell the two apart
//! cleanly: tap-to-click reports a single position sample (travel 0) while
//! scroll/flick gestures move the contact 147–410 device units. A finger that
//! rests longer than [`TAP_MAX`] is a rest/drag, not a click, and is ignored
//! too. See [`GestureTracker`].
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
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const BTN_TOUCH: u16 = 0x14a;
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;
const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;
const PRESS: i32 = 1;
const RELEASE: i32 = 0;
/// Upper bound for a touch-down→release to count as a tap. A finger resting
/// on the surface longer than this is a rest/drag, not a click, and must not
/// dismiss anything.
const TAP_MAX: Duration = Duration::from_millis(300);
/// How far a contact may stray from where it landed and still have been a tap,
/// in raw device units. This touchpad reports 12 units/mm, so this is ≈2 mm:
/// far more than the jitter of a tap (recorded taps: 0 units) and far less than
/// any scroll or flick (recorded: 147–410 units).
const TAP_TRAVEL_MAX: i32 = 24;
/// Highest multi-touch slot index tracked. The kernel allows up to 9
/// (`ABS_MT_SLOT` max) and this machine's touchpad reports 4.
const MAX_SLOTS: usize = 16;
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
    // One gesture tracker per device: `BTN_TOUCH` brackets the whole gesture,
    // even a multi-finger one (a second finger produces no new touch press).
    let mut gesture = GestureTracker::default();
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
        if kind == EV_KEY && value == PRESS && (code == BTN_LEFT || code == BTN_RIGHT || code == BTN_MIDDLE)
        {
            on_button_press(shared);
            continue;
        }
        if gesture.feed(kind, code, value, Instant::now()) {
            on_button_press(shared);
        }
    }
}

/// The positions a single touch contact reported, as a bounding box.
///
/// The box (rather than "how far from the landing point") is what makes this
/// independent of event order — a device may report x and y in either order or
/// update only one axis at a time, and the recorded swipe below does exactly
/// that — and it still captures a back-and-forth movement.
#[derive(Clone, Copy, Default)]
struct Contact {
    seen_x: bool,
    seen_y: bool,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    active: bool,
}

impl Contact {
    fn place(&mut self, x: Option<i32>, y: Option<i32>) {
        if let Some(value) = x {
            if self.seen_x {
                self.min_x = self.min_x.min(value);
                self.max_x = self.max_x.max(value);
            } else {
                self.min_x = value;
                self.max_x = value;
                self.seen_x = true;
            }
        }
        if let Some(value) = y {
            if self.seen_y {
                self.min_y = self.min_y.min(value);
                self.max_y = self.max_y.max(value);
            } else {
                self.min_y = value;
                self.max_y = value;
                self.seen_y = true;
            }
        }
    }

    /// How far this contact strayed, in device units (0 = never moved).
    fn travel(&self) -> i32 {
        let dx = if self.seen_x { self.max_x - self.min_x } else { 0 };
        let dy = if self.seen_y { self.max_y - self.min_y } else { 0 };
        dx.max(dy)
    }
}

/// Classifies a touch gesture from the raw evdev stream.
///
/// `BTN_TOUCH` alone cannot separate a tap from a scroll: libinput synthesizes
/// tap-to-click in the compositor (so the touchpad node sees no `BTN_LEFT` for
/// it), and both gestures are just "touch down … touch up". Movement does
/// separate them, so every contact's travel is accumulated across the gesture
/// and [`is_tap`] decides on release.
#[derive(Default)]
struct GestureTracker {
    /// When the current gesture's first finger landed (`BTN_TOUCH` press, or
    /// the first contact on a device that reports no `BTN_TOUCH`).
    started: Option<Instant>,
    contacts: [Contact; MAX_SLOTS],
    /// `ABS_MT_SLOT` currently being reported.
    slot: usize,
    /// Contacts currently down, and the most that were down at once.
    down: usize,
    max_down: usize,
    /// A wheel axis was reported: a scroll, whatever the contacts did.
    scrolled: bool,
}

impl GestureTracker {
    fn begin(&mut self, now: Instant) {
        self.started = Some(now);
        self.contacts = Default::default();
        self.slot = 0;
        self.down = 0;
        self.max_down = 0;
        self.scrolled = false;
    }

    fn current(&mut self) -> &mut Contact {
        &mut self.contacts[self.slot]
    }

    /// Furthest any contact of this gesture strayed.
    fn travel(&self) -> i32 {
        self.contacts.iter().map(Contact::travel).max().unwrap_or(0)
    }

    /// Feed one evdev event. Returns `true` when the gesture just completed as
    /// a tap, i.e. when the caller should treat it as a click.
    fn feed(&mut self, kind: u16, code: u16, value: i32, now: Instant) -> bool {
        match kind {
            EV_KEY if code == BTN_TOUCH => {
                if value == PRESS {
                    // The kernel reports the first contact *before* `BTN_TOUCH`
                    // (recorded order: `MT_TRACKING_ID`, positions, then the
                    // press), so this must not wipe a gesture already in
                    // progress — it only starts one for devices that send no
                    // contact events ahead of it.
                    if self.started.is_none() {
                        self.begin(now);
                    }
                } else if value == RELEASE {
                    let held = self.started.take().map(|started| now.saturating_duration_since(started));
                    let tap = is_tap(held, self.travel(), self.scrolled, self.max_down);
                    self.contacts = Default::default();
                    self.down = 0;
                    self.scrolled = false;
                    return tap;
                }
            }
            EV_REL if code == REL_WHEEL || code == REL_HWHEEL => self.scrolled = true,
            EV_ABS => match code {
                ABS_MT_SLOT => {
                    self.slot = usize::try_from(value).unwrap_or(0).min(MAX_SLOTS - 1);
                }
                ABS_MT_TRACKING_ID => {
                    if self.started.is_none() {
                        // Devices that report contacts without `BTN_TOUCH`.
                        self.started = Some(now);
                    }
                    let slot = self.slot;
                    if value >= 0 {
                        // New contact in this slot: its box starts empty.
                        let contact = &mut self.contacts[slot];
                        if !contact.active {
                            contact.active = true;
                            self.down += 1;
                            self.max_down = self.max_down.max(self.down);
                        }
                        contact.seen_x = false;
                        contact.seen_y = false;
                    } else if self.contacts[slot].active {
                        self.contacts[slot].active = false;
                        self.down = self.down.saturating_sub(1);
                    }
                }
                ABS_MT_POSITION_X => self.current().place(Some(value), None),
                ABS_MT_POSITION_Y => self.current().place(None, Some(value)),
                // Single-touch mirrors of the primary contact; they only ever
                // grow a box, so tap jitter stays 0 and a drag still shows.
                ABS_X => self.contacts[0].place(Some(value), None),
                ABS_Y => self.contacts[0].place(None, Some(value)),
                _ => {}
            },
            _ => {}
        }
        false
    }
}

/// Was a completed touch gesture a tap (and therefore a click)?
///
/// Pure so the recorded gesture shapes are unit-testable. The duration bound
/// alone is not enough — a two-finger scroll flick finishes in well under
/// [`TAP_MAX`] — so a gesture that moved a contact, or reported a wheel axis,
/// is never a tap.
fn is_tap(held: Option<Duration>, travel: i32, scrolled: bool, contacts: usize) -> bool {
    contacts >= 1
        && !scrolled
        && travel <= TAP_TRAVEL_MAX
        && held.is_some_and(|held| held <= TAP_MAX)
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

    // ---- touch gesture classification (recorded from a real touchpad) ----

    /// Replay `events` through a fresh tracker, `gap` apart, and report whether
    /// the sequence ended as a tap.
    fn replay(events: &[(u16, u16, i32)], gap: Duration) -> bool {
        let mut tracker = GestureTracker::default();
        let mut now = Instant::now();
        let mut tap = false;
        for &(kind, code, value) in events {
            tap = tracker.feed(kind, code, value, now);
            now += gap;
        }
        tap
    }

    /// A tap-to-click as recorded from this machine's touchpad: one contact, a
    /// single position sample, released after 130 ms.
    fn recorded_tap() -> Vec<(u16, u16, i32)> {
        vec![
            (EV_ABS, ABS_MT_TRACKING_ID, 2149),
            (EV_ABS, ABS_MT_POSITION_X, 753),
            (EV_ABS, ABS_MT_POSITION_Y, 287),
            (EV_KEY, BTN_TOUCH, PRESS),
            (EV_ABS, ABS_X, 753),
            (EV_ABS, ABS_Y, 287),
            (EV_ABS, ABS_MT_TRACKING_ID, -1),
            (EV_KEY, BTN_TOUCH, RELEASE),
        ]
    }

    /// A two-finger scroll as recorded: both slots reporting y, moving together.
    fn recorded_two_finger_scroll() -> Vec<(u16, u16, i32)> {
        let mut events = vec![
            (EV_ABS, ABS_MT_SLOT, 0),
            (EV_ABS, ABS_MT_TRACKING_ID, 2153),
            (EV_ABS, ABS_MT_POSITION_X, 607),
            (EV_ABS, ABS_MT_POSITION_Y, 476),
            (EV_KEY, BTN_TOUCH, PRESS),
            (EV_ABS, ABS_MT_SLOT, 1),
            (EV_ABS, ABS_MT_TRACKING_ID, 2154),
            (EV_ABS, ABS_MT_POSITION_X, 510),
            (EV_ABS, ABS_MT_POSITION_Y, 644),
        ];
        for step in 1..=12 {
            let travel = 12 * step;
            events.extend([
                (EV_ABS, ABS_MT_SLOT, 0),
                (EV_ABS, ABS_MT_POSITION_Y, 476 - travel),
                (EV_ABS, ABS_MT_SLOT, 1),
                (EV_ABS, ABS_MT_POSITION_Y, 644 - travel),
            ]);
        }
        events.extend([
            (EV_ABS, ABS_MT_SLOT, 0),
            (EV_ABS, ABS_MT_TRACKING_ID, -1),
            (EV_ABS, ABS_MT_SLOT, 1),
            (EV_ABS, ABS_MT_TRACKING_ID, -1),
            (EV_KEY, BTN_TOUCH, RELEASE),
        ]);
        events
    }

    /// The fastest gesture recorded (189 ms, one contact): x moves 300 units
    /// and y is reported only at the start.
    fn recorded_single_finger_flick() -> Vec<(u16, u16, i32)> {
        let mut events = vec![
            (EV_ABS, ABS_MT_TRACKING_ID, 2159),
            (EV_ABS, ABS_MT_POSITION_X, 443),
            (EV_ABS, ABS_MT_POSITION_Y, 465),
            (EV_KEY, BTN_TOUCH, PRESS),
        ];
        for step in 1..=30 {
            events.push((EV_ABS, ABS_MT_POSITION_X, 443 + 10 * step));
        }
        events.extend([
            (EV_ABS, ABS_MT_TRACKING_ID, -1),
            (EV_KEY, BTN_TOUCH, RELEASE),
        ]);
        events
    }

    #[test]
    fn recorded_tap_counts_as_a_click() {
        assert!(replay(&recorded_tap(), Duration::from_millis(18)));
    }

    #[test]
    fn recorded_two_finger_scroll_is_not_a_click() {
        let scroll = recorded_two_finger_scroll();
        // Both the slow (≈310 ms) and the quick flick (≈186 ms, the gesture
        // that used to close the popup) must be ignored.
        assert!(!replay(&scroll, Duration::from_millis(5)));
        assert!(!replay(&scroll, Duration::from_millis(3)));
    }

    #[test]
    fn recorded_single_finger_flick_is_not_a_click() {
        assert!(!replay(&recorded_single_finger_flick(), Duration::from_millis(6)));
    }

    #[test]
    fn resting_finger_is_not_a_tap() {
        let mut tracker = GestureTracker::default();
        let start = Instant::now();
        tracker.feed(EV_ABS, ABS_MT_TRACKING_ID, 7, start);
        tracker.feed(EV_ABS, ABS_MT_POSITION_X, 400, start);
        tracker.feed(EV_ABS, ABS_MT_POSITION_Y, 400, start);
        tracker.feed(EV_KEY, BTN_TOUCH, PRESS, start);
        // Held well past TAP_MAX without moving: a rest, must not dismiss.
        assert!(!tracker.feed(EV_KEY, BTN_TOUCH, RELEASE, start + Duration::from_millis(900)));
    }

    #[test]
    fn two_finger_tap_is_still_a_click() {
        let mut tracker = GestureTracker::default();
        let start = Instant::now();
        let held = Duration::from_millis(120);
        for (slot, x) in [(0, 500), (1, 600)] {
            tracker.feed(EV_ABS, ABS_MT_SLOT, slot, start);
            tracker.feed(EV_ABS, ABS_MT_TRACKING_ID, 40 + slot, start);
            tracker.feed(EV_ABS, ABS_MT_POSITION_X, x, start);
            tracker.feed(EV_ABS, ABS_MT_POSITION_Y, 300, start);
        }
        tracker.feed(EV_KEY, BTN_TOUCH, PRESS, start);
        // A right-click tap (two fingers, no movement) still dismisses.
        assert!(tracker.feed(EV_KEY, BTN_TOUCH, RELEASE, start + held));
    }

    #[test]
    fn wheel_scroll_is_not_a_click() {
        let mut tracker = GestureTracker::default();
        let start = Instant::now();
        tracker.feed(EV_ABS, ABS_MT_POSITION_X, 400, start);
        tracker.feed(EV_KEY, BTN_TOUCH, PRESS, start);
        tracker.feed(EV_REL, REL_WHEEL, -1, start);
        // No travel at all, but a wheel axis means scrolling, never a click.
        assert!(!tracker.feed(EV_KEY, BTN_TOUCH, RELEASE, start + Duration::from_millis(50)));
    }

    #[test]
    fn release_without_a_contact_is_not_a_click() {
        let mut tracker = GestureTracker::default();
        let start = Instant::now();
        tracker.feed(EV_KEY, BTN_TOUCH, PRESS, start);
        assert!(!tracker.feed(EV_KEY, BTN_TOUCH, RELEASE, start + Duration::from_millis(50)));
    }

    #[test]
    fn tap_bounds_are_exclusive_at_the_edges() {
        let held = Duration::from_millis(120);
        assert!(is_tap(Some(held), TAP_TRAVEL_MAX, false, 1));
        assert!(!is_tap(Some(held), TAP_TRAVEL_MAX + 1, false, 1));
        assert!(is_tap(Some(TAP_MAX), 0, false, 1));
        assert!(!is_tap(Some(TAP_MAX + Duration::from_millis(1)), 0, false, 1));
        assert!(!is_tap(Some(held), 0, true, 1));
        assert!(!is_tap(Some(held), 0, false, 0));
        assert!(!is_tap(None, 0, false, 1));
    }
}
