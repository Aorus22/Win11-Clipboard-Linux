//! Single-instance guard + IPC for `--toggle` / `--settings-open` client mode.
//!
//! The first instance binds `$XDG_RUNTIME_DIR/win11-clipboard-gpui.sock` and
//! forwards newline-delimited words to the main loop. A second invocation
//! sends its word and exits immediately (fast toggle path for DE shortcuts).
//!
//! Delivery into the app is event-driven: the accept thread pushes into a
//! `tokio::sync::mpsc` unbounded channel and the main loop wakes on
//! `poll_recv` immediately instead of waiting out the poll interval.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

/// Signals a running instance can act on (polled by the main loop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSignal {
    Toggle,
    OpenSettings,
    Quit,
    /// System color-scheme changed (portal listener thread).
    ThemeChanged,
}

impl AppSignal {
    fn word(self) -> &'static str {
        match self {
            AppSignal::Toggle => "toggle",
            AppSignal::OpenSettings => "settings",
            AppSignal::Quit => "quit",
            AppSignal::ThemeChanged => "theme",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        match word.trim() {
            "toggle" => Some(AppSignal::Toggle),
            "settings" => Some(AppSignal::OpenSettings),
            "quit" => Some(AppSignal::Quit),
            "theme" => Some(AppSignal::ThemeChanged),
            _ => None,
        }
    }
}

pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("GPUI_SOCK_PATH") {
        return PathBuf::from(p);
    }
    dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(crate::identity::get().socket_name)
}

pub enum InstanceRole {
    /// This process owns the socket; listener thread forwards signals.
    Primary,
    /// Another instance was notified; the caller should exit promptly.
    Notified,
}

/// Acquire the single-instance socket. On success as primary, spawns the accept
/// thread delivering `AppSignal`s to `tx`. If the socket is stale (file exists
/// but nobody listens), it is reclaimed.
pub fn acquire(tx: UnboundedSender<AppSignal>) -> InstanceRole {
    let path = socket_path();
    match UnixListener::bind(&path) {
        Ok(listener) => {
            std::thread::Builder::new()
                .name("gpui-instance-ipc".to_string())
                .spawn(move || accept_loop(listener, tx))
                .expect("spawn instance IPC thread");
            InstanceRole::Primary
        }
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // Someone may hold it — probe by connecting.
            if probe() {
                InstanceRole::Notified
            } else {
                // Stale socket file: reclaim.
                let _ = std::fs::remove_file(&path);
                match UnixListener::bind(&path) {
                    Ok(listener) => {
                        std::thread::Builder::new()
                            .name("gpui-instance-ipc".to_string())
                            .spawn(move || accept_loop(listener, tx))
                            .expect("spawn instance IPC thread");
                        InstanceRole::Primary
                    }
                    Err(_) => InstanceRole::Primary,
                }
            }
        }
        Err(_) => InstanceRole::Primary,
    }
}

/// What a client-mode flag should do once the socket has answered (or not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAction {
    /// A live instance took the signal: exit now.
    Notified,
    /// Nobody is listening — become the instance, showing the popup.
    StartShowing,
    /// Nobody is listening — become the instance, with Settings open.
    StartSettings,
    /// Nothing left to do (`--quit` with no daemon is already satisfied).
    NothingToDo,
}

/// Decide the fate of `--toggle` / `--settings-open` / `--quit` from the result
/// of [`send_signal`].
///
/// Falling through to a start is what keeps the desktop shortcut alive: the
/// `<exe> --toggle` entry and the GNOME top-bar menu are the only entry points a
/// user has, and a daemon that crashed, was killed, or never started (a session
/// with autostart disabled) would otherwise turn both into silent no-ops.
pub fn client_action(signal: AppSignal, delivered: Result<(), String>) -> ClientAction {
    if delivered.is_ok() {
        return ClientAction::Notified;
    }
    match signal {
        AppSignal::Toggle => ClientAction::StartShowing,
        AppSignal::OpenSettings => ClientAction::StartSettings,
        // `--quit` with no listener: the daemon is already gone.
        AppSignal::Quit => ClientAction::NothingToDo,
        // Only ever sent from inside a running instance.
        AppSignal::ThemeChanged => ClientAction::NothingToDo,
    }
}

/// Send one signal word to the running primary. Used by `--toggle` client mode.
pub fn send_signal(signal: AppSignal) -> Result<(), String> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("timeout: {e}"))?;
    writeln!(stream, "{}", signal.word()).map_err(|e| format!("write: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;
    Ok(())
}

fn probe() -> bool {
    UnixStream::connect(socket_path()).is_ok()
}

fn accept_loop(listener: UnixListener, tx: UnboundedSender<AppSignal>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_ok() {
            if let Some(signal) = AppSignal::parse(&line) {
                let _ = tx.send(signal);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-test-{name}-{}.sock", std::process::id()))
    }

    #[test]
    fn signal_words_round_trip() {
        assert_eq!(AppSignal::parse("toggle\n"), Some(AppSignal::Toggle));
        assert_eq!(AppSignal::parse("settings"), Some(AppSignal::OpenSettings));
        assert_eq!(AppSignal::parse("quit"), Some(AppSignal::Quit));
        assert_eq!(AppSignal::parse("bogus"), None);
    }

    /// A delivered signal is always just "exit"; the fallbacks are what make a
    /// dead daemon recoverable rather than a dead shortcut.
    #[test]
    fn client_modes_exit_when_delivered() {
        for signal in [AppSignal::Toggle, AppSignal::OpenSettings, AppSignal::Quit] {
            assert_eq!(client_action(signal, Ok(())), ClientAction::Notified);
        }
    }

    #[test]
    fn client_modes_start_an_instance_when_nobody_listens() {
        let down = Err("connect: Connection refused".to_string());
        assert_eq!(
            client_action(AppSignal::Toggle, down.clone()),
            ClientAction::StartShowing
        );
        assert_eq!(
            client_action(AppSignal::OpenSettings, down.clone()),
            ClientAction::StartSettings
        );
        // Nothing to stop, so this must not start a resident instance.
        assert_eq!(client_action(AppSignal::Quit, down), ClientAction::NothingToDo);
    }

    /// Socket lifecycle in ONE test: the GPUI_SOCK_PATH override is process-global,
    /// so parallel #[test] fns would race on it. Runs sequentially by construction.
    #[test]
    fn socket_primary_notify_and_stale_reclaim() {
        let _serial = crate::test_util::serial_lock();
        let live = temp_path("ipc");
        let stale = temp_path("stale");
        let _ = std::fs::remove_file(&live);
        let _ = std::fs::remove_file(&stale);

        unsafe { std::env::set_var("GPUI_SOCK_PATH", &live) };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        assert!(matches!(acquire(tx), InstanceRole::Primary));
        send_signal(AppSignal::Toggle).expect("send");
        // The accept thread delivers asynchronously; poll briefly instead of
        // assuming it has already run.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let got = loop {
            match rx.try_recv() {
                Ok(sig) => break sig,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
                    if std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("receive signal: {e}"),
            }
        };
        assert_eq!(got, AppSignal::Toggle);
        // A second acquire while the primary lives reports Notified.
        let (tx2, _rx2) = tokio::sync::mpsc::unbounded_channel();
        assert!(matches!(acquire(tx2), InstanceRole::Notified));

        // Stale regular file: acquire must not panic or hang.
        std::fs::write(&stale, "stale").expect("write stale file");
        unsafe { std::env::set_var("GPUI_SOCK_PATH", &stale) };
        let (tx3, _rx3) = tokio::sync::mpsc::unbounded_channel();
        let _ = acquire(tx3);

        unsafe { std::env::remove_var("GPUI_SOCK_PATH") };
        let _ = std::fs::remove_file(&live);
        let _ = std::fs::remove_file(&stale);
    }
}
