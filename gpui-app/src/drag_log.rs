//! Drag diagnostics.
//!
//! The popup is normally started from the tray or a `.desktop` entry, where
//! stderr goes nowhere, so a drag leaves no trace to look at afterwards. This
//! appends every drag step to `<config>/drag-debug.log` (truncated once per
//! process, so the file always describes the newest run) and mirrors it to
//! stderr so a terminal run shows the same thing.
//!
//! Set `WIN11_CLIPBOARD_DRAG_LOG=0` to silence it.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

const LOG_FILE: &str = "drag-debug.log";

pub fn path() -> PathBuf {
    crate::settings::config_dir().join(LOG_FILE)
}

fn enabled() -> bool {
    std::env::var("WIN11_CLIPBOARD_DRAG_LOG")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Truncate the trace once per process so the newest run is always the whole file.
fn prepare() {
    static PREPARED: OnceLock<()> = OnceLock::new();
    PREPARED.get_or_init(|| {
        let _ = std::fs::write(path(), b"");
    });
}

/// Append one trace line, tagged with the wall clock so drag steps can be timed.
pub fn log(message: impl std::fmt::Display) {
    if !enabled() {
        return;
    }
    prepare();
    let line = format!(
        "{} {}",
        chrono::Local::now().format("%H:%M:%S%.3f"),
        message
    );
    eprintln!("[drag] {line}");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path()) {
        let _ = writeln!(file, "{line}");
    }
}
