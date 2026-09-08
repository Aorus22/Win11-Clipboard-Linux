//! Single-instance guard + IPC for `--toggle` / `--settings-open` client mode.
//!
//! The first instance binds `$XDG_RUNTIME_DIR/win11-clipboard-gpui.sock` and
//! forwards newline-delimited words to the main poll loop. A second invocation
//! sends its word and exits immediately (fast toggle path for DE shortcuts).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;

pub const SOCKET_NAME: &str = "win11-clipboard-gpui.sock";

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
        .join(SOCKET_NAME)
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
pub fn acquire(tx: Sender<AppSignal>) -> InstanceRole {
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

fn accept_loop(listener: UnixListener, tx: Sender<AppSignal>) {
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
    use std::sync::mpsc::channel;

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
        let (tx, rx) = channel();
        assert!(matches!(acquire(tx), InstanceRole::Primary));
        send_signal(AppSignal::Toggle).expect("send");
        let got = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("receive signal");
        assert_eq!(got, AppSignal::Toggle);
        // A second acquire while the primary lives reports Notified.
        let (tx2, _rx2) = channel();
        assert!(matches!(acquire(tx2), InstanceRole::Notified));

        // Stale regular file: acquire must not panic or hang.
        std::fs::write(&stale, "stale").expect("write stale file");
        unsafe { std::env::set_var("GPUI_SOCK_PATH", &stale) };
        let (tx3, _rx3) = channel();
        let _ = acquire(tx3);

        unsafe { std::env::remove_var("GPUI_SOCK_PATH") };
        let _ = std::fs::remove_file(&live);
        let _ = std::fs::remove_file(&stale);
    }
}
