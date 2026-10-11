//! Make the compositor tell a new overlay where the pointer is.
//!
//! Wayland gives a client the pointer position only through `wl_pointer.enter` /
//! `motion`, and COSMIC sends neither for a surface that appeared under a pointer that
//! is not moving. So a popup opened with Super+V and an idle mouse cannot follow the
//! cursor. Moving the pointer one unit and back makes the compositor re-evaluate pointer
//! focus and send `enter` with the exact position; the net movement is zero.
//!
//! The nudge goes through a persistent virtual mouse made with `/dev/uinput`, the same
//! access the paste keystroke already needs; without it the caller keeps its fallback.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const SYN_REPORT: u16 = 0x00;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const BTN_LEFT: u16 = 0x110;

const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
const UI_SET_RELBIT: libc::c_ulong = 0x40045566;
const UI_DEV_SETUP: libc::c_ulong = 0x405c5503;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

struct Mouse {
    file: File,
}

static MOUSE: OnceLock<Mutex<Option<Mouse>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Mouse>> {
    MOUSE.get_or_init(|| Mutex::new(None))
}

fn event(type_: u16, code: u16, value: i32) -> libc::input_event {
    let mut e: libc::input_event = unsafe { std::mem::zeroed() };
    e.type_ = type_;
    e.code = code;
    e.value = value;
    e
}

fn bytes(events: &[libc::input_event]) -> &[u8] {
    // SAFETY: input_event is a plain C struct; the slice covers exactly the source.
    unsafe { std::slice::from_raw_parts(events.as_ptr().cast::<u8>(), std::mem::size_of_val(events)) }
}

impl Mouse {
    fn create() -> Result<Self, String> {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .map_err(|e| format!("open /dev/uinput: {e}"))?;
        let fd = file.as_raw_fd();
        let ioctl = |request: libc::c_ulong, arg: libc::c_int| -> Result<(), String> {
            if unsafe { libc::ioctl(fd, request, arg) } < 0 {
                Err(format!("uinput ioctl {request:#x}: {}", std::io::Error::last_os_error()))
            } else {
                Ok(())
            }
        };
        ioctl(UI_SET_EVBIT, EV_KEY as libc::c_int)?;
        ioctl(UI_SET_KEYBIT, BTN_LEFT as libc::c_int)?; // libinput wants a button to call it a pointer
        ioctl(UI_SET_EVBIT, EV_REL as libc::c_int)?;
        ioctl(UI_SET_RELBIT, REL_X as libc::c_int)?;
        ioctl(UI_SET_RELBIT, REL_Y as libc::c_int)?;

        #[repr(C)]
        struct Setup {
            id: libc::input_id,
            name: [u8; 80],
            ff_effects_max: u32,
        }
        let name = format!("win11-clipboard-pointer-{}", std::process::id());
        let mut setup = Setup {
            id: libc::input_id { bustype: 0x03, vendor: 0x1234, product: 0x5679, version: 1 },
            name: [0; 80],
            ff_effects_max: 0,
        };
        setup.name[..name.len()].copy_from_slice(name.as_bytes());
        if unsafe { libc::ioctl(fd, UI_DEV_SETUP, &setup) } < 0 {
            return Err(format!("uinput setup: {}", std::io::Error::last_os_error()));
        }
        if unsafe { libc::ioctl(fd, UI_DEV_CREATE) } < 0 {
            return Err(format!("uinput create: {}", std::io::Error::last_os_error()));
        }
        // Give udev and the compositor time to pick the device up.
        let started = Instant::now();
        while !node_exists(&name) && started.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(150));
        Ok(Self { file })
    }

    fn jiggle(&mut self) -> Result<(), String> {
        let frame = |dx: i32| [event(EV_REL, REL_X, dx), event(EV_REL, REL_Y, 0), event(EV_SYN, SYN_REPORT, 0)];
        self.file.write_all(bytes(&frame(1))).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(3));
        self.file.write_all(bytes(&frame(-1))).map_err(|e| e.to_string())
    }
}

impl Drop for Mouse {
    fn drop(&mut self) {
        unsafe { libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY) };
    }
}

fn node_exists(name: &str) -> bool {
    std::fs::read_dir("/sys/class/input")
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            std::fs::read_to_string(entry.path().join("name")).is_ok_and(|n| n.trim() == name)
        })
}

/// Create the virtual mouse in the background (it takes a moment to appear).
pub fn warm_up() {
    std::thread::spawn(|| {
        let mut guard = slot().lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            match Mouse::create() {
                Ok(mouse) => {
                    eprintln!("[app] pointer nudge ready (virtual mouse)");
                    *guard = Some(mouse);
                }
                Err(e) => eprintln!("[app] pointer nudge unavailable: {e}"),
            }
        }
    });
}

/// Move the pointer one unit and back, off the calling thread.
pub fn nudge() {
    std::thread::spawn(|| {
        let mut guard = slot().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mouse) = guard.as_mut() {
            if let Err(e) = mouse.jiggle() {
                eprintln!("[app] pointer nudge failed: {e}");
            }
        }
    });
}
