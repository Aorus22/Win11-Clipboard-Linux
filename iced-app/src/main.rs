//! Windows 11-style Clipboard History for Linux — iced / libcosmic frontend.
//!
//! - Default: resident app; Super+V (`--toggle`) or the tray opens the popup.
//! - `--background`: tray-only start (no popup until toggled).
//! - `--settings` / `--setup`: open Settings / the setup wizard on start.
//! - `--toggle` / `--settings-open` / `--quit`: signal a running instance, exit fast.
//! - `--register-shortcuts` / `--unregister-shortcuts` / `--install-gnome-extension`: headless setup.

mod app;
mod emoji_image;
mod glass;
mod hooks;
mod pickers_ui;
mod popup;
mod scale;
mod settings_ui;
#[cfg(test)]
mod testing;
mod tray;
mod widgets;
mod win11;
mod wizard;

use std::sync::Mutex;

use app_shared::identity::{self, AppIdentity};
use app_shared::instance::{self, AppSignal, InstanceRole};
use app_shared::platform;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// What the running app is asked to do by threads that are not the UI.
#[derive(Clone, Debug)]
pub enum Command {
    Signal(AppSignal),
    /// An outside-click watcher wants the popup closed.
    HideRequested,
}

pub static COMMANDS: Mutex<Option<UnboundedReceiver<Command>>> = Mutex::new(None);

pub struct Flags {
    pub commands: UnboundedSender<Command>,
    pub open_settings: bool,
    pub open_wizard: bool,
    pub background: bool,
}

fn has_arg(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn main() -> cosmic::iced::Result {
    env_logger::init();
    identity::init(AppIdentity::ICED);
    // Must run before the toolkit starts: it may rewrite the environment (x11 backend).
    platform::init();
    win11_clipboard_history_lib::rendering_env::init();

    let args: Vec<String> = std::env::args().collect();
    if has_arg(&args, "--version") || has_arg(&args, "-v") {
        println!("win11-clipboard-history-iced {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    for (flag, signal) in [
        ("--toggle", AppSignal::Toggle),
        ("--settings-open", AppSignal::OpenSettings),
        ("--quit", AppSignal::Quit),
    ] {
        if has_arg(&args, flag) {
            match instance::send_signal(signal) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("[app] no running instance ({e})");
                    std::process::exit(1);
                }
            }
        }
    }
    if has_arg(&args, "--register-shortcuts") {
        exit_with(platform::shortcuts::register_headless(), "register failed");
    }
    if has_arg(&args, "--unregister-shortcuts") {
        exit_with(platform::shortcuts::unregister_headless(), "unregister failed");
    }
    if has_arg(&args, "--install-gnome-extension") {
        exit_with(
            platform::gnome::install::install()
                .map(|outcome| platform::gnome::install::describe(&outcome).to_string()),
            "install failed",
        );
    }

    // Single instance: a duplicate normal start exits quietly.
    let (signal_tx, mut signal_rx) = tokio::sync::mpsc::unbounded_channel::<AppSignal>();
    match instance::acquire(signal_tx) {
        InstanceRole::Notified => {
            eprintln!("[app] another instance is running; exiting");
            return Ok(());
        }
        InstanceRole::Primary => {}
    }
    let (commands, receiver) = tokio::sync::mpsc::unbounded_channel();
    *COMMANDS.lock().unwrap() = Some(receiver);
    let forward = commands.clone();
    std::thread::spawn(move || {
        while let Some(signal) = signal_rx.blocking_recv() {
            let _ = forward.send(Command::Signal(signal));
        }
    });

    let flags = Flags {
        commands,
        open_settings: has_arg(&args, "--settings"),
        open_wizard: has_arg(&args, "--setup") || app_shared::launcher::is_first_run(),
        background: has_arg(&args, "--background"),
    };
    let settings = cosmic::app::Settings::default().no_main_window(true).transparent(true);
    cosmic::app::run::<app::App>(settings, flags)
}

fn exit_with(result: Result<String, String>, failure: &str) -> ! {
    match result {
        Ok(msg) => {
            println!("{msg}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{failure}: {e}");
            std::process::exit(1);
        }
    }
}
