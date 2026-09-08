//! Backend service — owns the reused `ClipboardManager` and mirrors the Tauri
//! app's watcher + paste semantics (`src-tauri/src/main.rs`).
//!
//! - Watcher thread: 500ms poll, hash-change detection, 30s auto-delete cleanup.
//! - Paste flow: caller hides the window + restores focus first (main.rs),
//!   then `paste()` = `manager.paste_item()` (mark → write → Ctrl+V → move-to-top).
//! - UI reads via `snapshot()`; change detection via `version()` counter.

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use win11_clipboard_history_lib::clipboard_manager::{self, ClipboardItem, ClipboardManager};

use crate::settings::{AppSettings, config_dir};

pub fn history_path() -> PathBuf {
    config_dir().join("history.json")
}

fn cleanup_interval_minutes(settings: &AppSettings) -> u64 {
    if settings.auto_delete_interval == 0 {
        return 0;
    }
    let base = settings.auto_delete_interval;
    match settings.auto_delete_unit.as_str() {
        "minutes" => base,
        "hours" => base.saturating_mul(60),
        "days" => base.saturating_mul(60).saturating_mul(24),
        "weeks" => base.saturating_mul(60).saturating_mul(24).saturating_mul(7),
        _ => 0,
    }
}

pub struct BackendService {
    manager: Mutex<ClipboardManager>,
    version: AtomicU64,
    max_history_size: usize,
    cleanup_minutes: u64,
}

impl BackendService {
    pub fn new(settings: &AppSettings) -> Arc<Self> {
        let service = Arc::new(Self {
            manager: Mutex::new(ClipboardManager::new(
                history_path(),
                settings.max_history_size,
            )),
            version: AtomicU64::new(1),
            max_history_size: settings.max_history_size,
            cleanup_minutes: cleanup_interval_minutes(settings),
        });
        service.clone().start_watcher();
        service
    }

    fn bump(&self) {
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> Vec<ClipboardItem> {
        self.manager.lock().get_history()
    }

    pub fn toggle_pin(&self, id: &str) {
        if self.manager.lock().toggle_pin(id).is_some() {
            self.bump();
        }
    }

    pub fn remove_item(&self, id: &str) {
        self.manager.lock().remove_item(id);
        self.bump();
    }

    pub fn clear(&self) {
        self.manager.lock().clear();
        self.bump();
    }

    pub fn set_max_history_size(&self, size: usize) {
        self.manager.lock().set_max_history_size(size);
        self.bump();
    }

    /// Paste after the caller hid the window and restored focus (parity with
    /// the Tauri `paste_item` command ordering).
    pub fn paste(&self, item: &ClipboardItem) -> Result<(), String> {
        self.manager.lock().paste_item(item)?;
        self.bump();
        Ok(())
    }

    /// Mark pasted text so picker insertions don't re-enter history
    /// (mirrors `mark_text_as_pasted`, used by Phase 3 pickers).
    pub fn mark_text_as_pasted(&self, text: &str) {
        self.manager.lock().mark_text_as_pasted(text);
    }

    /// Background watcher — port of `start_clipboard_watcher` (500ms cadence,
    /// hash-change detection, cleanup every 60 ticks ≈ 30s).
    fn start_watcher(self: Arc<Self>) {
        std::thread::Builder::new()
            .name("gpui-clipboard-watcher".to_string())
            .spawn(move || {
                let mut last_text_hash: Option<u64> = None;
                let mut last_image_hash: Option<u64> = None;
                let mut cleanup_counter = 0u32;

                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    cleanup_counter += 1;

                    let mut manager = self.manager.lock();

                    if cleanup_counter >= 60 {
                        cleanup_counter = 0;
                        if self.cleanup_minutes > 0
                            && manager.cleanup_old_items(self.cleanup_minutes)
                        {
                            drop(manager);
                            self.bump();
                            continue;
                        }
                    }

                    if let Ok(text) = manager.get_current_text() {
                        if !text.is_empty() {
                            let hash = clipboard_manager::calculate_hash(&text);
                            if Some(hash) != last_text_hash {
                                last_text_hash = Some(hash);
                                last_image_hash = None;
                                let html = manager.get_current_html();
                                if manager.add_text(text, html).is_some() {
                                    drop(manager);
                                    self.bump();
                                    continue;
                                }
                            }
                        }
                    }

                    if let Ok(Some((image_data, hash))) = manager.get_current_image() {
                        if Some(hash) != last_image_hash {
                            last_image_hash = Some(hash);
                            last_text_hash = None;
                            if manager.add_image(image_data, hash).is_some() {
                                drop(manager);
                                self.bump();
                            }
                        }
                    }
                }
            })
            .expect("spawn clipboard watcher");
    }

    #[allow(dead_code)]
    pub fn max_history_size(&self) -> usize {
        self.max_history_size
    }
}
