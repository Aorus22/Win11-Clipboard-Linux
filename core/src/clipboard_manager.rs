//! Clipboard Manager Module
//! Handles clipboard monitoring, history storage, and paste injection

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

// --- Constants ---

pub const DEFAULT_MAX_HISTORY_SIZE: usize = 50;
const PREVIEW_TEXT_MAX_LEN: usize = 100;
const GIF_CACHE_MARKER: &str = "win11-clipboard-history/gifs/";
const FILE_URI_PREFIX: &str = "file://";
/// Cards render image previews at ~96 px tall; a 2x-HiDPI fit box keeps the
/// thumb sharp while staying ~100x smaller on the GPU than a full screenshot.
const THUMB_MAX_W: u32 = 1024;
const THUMB_MAX_H: u32 = 256;
/// Insert-path saves are debounced: the watcher flushes pending changes at
/// most this often (explicit operations still save immediately).
const SAVE_DEBOUNCE: Duration = Duration::from_secs(2);
const CLIPBOARD_HELPER_READY_TIMEOUT: Duration = Duration::from_secs(2);
const CLIPBOARD_HELPER_POLL_INTERVAL: Duration = Duration::from_millis(2);

// --- Helper Functions ---

thread_local! {
    /// One arboard handle per thread. `Clipboard::new()` opens a fresh
    /// connection on X11/Wayland — the watcher polls twice a second and paste
    /// runs on the UI thread, so both reuse their thread's handle instead of
    /// churning connections.
    static THREAD_CLIPBOARD: std::cell::RefCell<Option<Clipboard>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `f` with the calling thread's cached clipboard handle, creating it on
/// first use.
fn with_thread_clipboard<T>(
    f: impl FnOnce(&mut Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, arboard::Error> {
    THREAD_CLIPBOARD.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(Clipboard::new()?);
        }
        f(slot.as_mut().expect("slot just filled"))
    })
}

// Simple FNV-1a implementation for stable hashing across restarts
// This avoids the randomization of DefaultHasher which causes duplicates on restart
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        FnvHasher(FNV_OFFSET_BASIS)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

/// Calculates a stable hash for any hashable data.
pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = FnvHasher::default();
    t.hash(&mut s);
    s.finish()
}

// --- Data Structures ---

/// Content type for clipboard items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ClipboardContent {
    /// Plain text content
    Text(String),
    /// Rich text with HTML formatting (plain text + optional HTML)
    RichText { plain: String, html: String },
    /// Image as PNG. The full-resolution PNG lives out-of-line in the images
    /// directory (`path`, named after the item id); `thumb` is a small base64
    /// PNG the UI renders from. `base64` only carries an inline copy for items
    /// persisted by older versions (migrated to a file on load) or when the
    /// images directory could not be written.
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base64: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thumb: Option<String>,
        width: u32,
        height: u32,
    },
}

impl ClipboardContent {
    /// The path of the full-resolution PNG for an image item, when stored
    /// out-of-line.
    pub fn image_file(&self) -> Option<&str> {
        match self {
            ClipboardContent::Image {
                path: Some(path), ..
            } => Some(path),
            _ => None,
        }
    }

    /// Full-resolution PNG bytes for an image item: reads the out-of-line
    /// file, falling back to the inline legacy copy when the file is gone.
    /// `Ok(None)` means the item keeps its metadata but no pixel data survives.
    pub fn image_png(&self) -> Result<Option<Vec<u8>>, String> {
        match self {
            ClipboardContent::Image { .. } => {
                if let Some(path) = self.image_file() {
                    match fs::read(path) {
                        Ok(bytes) => return Ok(Some(bytes)),
                        Err(e) => eprintln!("[ClipboardManager] image file unreadable: {e}"),
                    }
                }
                match self {
                    ClipboardContent::Image {
                        base64: Some(b64), ..
                    } => BASE64
                        .decode(b64)
                        .map(Some)
                        .map_err(|e| format!("Base64 decode failed: {e}")),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }
}

/// A single clipboard history item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    /// Unique identifier
    pub id: String,
    /// The content
    pub content: ClipboardContent,
    /// When it was copied
    pub timestamp: DateTime<Utc>,
    /// Whether this item is pinned
    pub pinned: bool,
    /// Preview text (for display)
    pub preview: String,
}

impl ClipboardItem {
    pub fn new_text(text: String) -> Self {
        let preview = if text.chars().count() > PREVIEW_TEXT_MAX_LEN {
            format!(
                "{}...",
                text.chars().take(PREVIEW_TEXT_MAX_LEN).collect::<String>()
            )
        } else {
            text.clone()
        };

        Self::create(ClipboardContent::Text(text), preview)
    }

    pub fn new_rich_text(plain: String, html: String) -> Self {
        let preview = if plain.chars().count() > PREVIEW_TEXT_MAX_LEN {
            format!(
                "{}...",
                plain.chars().take(PREVIEW_TEXT_MAX_LEN).collect::<String>()
            )
        } else {
            plain.clone()
        };

        Self::create(ClipboardContent::RichText { plain, html }, preview)
    }

    pub fn new_image(thumb: Option<String>, width: u32, height: u32, hash: u64) -> Self {
        // We store the hash in the preview string to persist it across sessions
        // without breaking the serialization schema of existing data.
        let preview = format!("Image ({}x{}) #{}", width, height, hash);

        Self::create(
            ClipboardContent::Image {
                path: None,
                base64: None,
                thumb,
                width,
                height,
            },
            preview,
        )
    }

    fn create(content: ClipboardContent, preview: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            content,
            timestamp: Utc::now(),
            pinned: false,
            preview,
        }
    }

    /// Attempts to extract the image hash from the preview string.
    /// Returns None if content is not an image or hash is missing.
    pub fn extract_image_hash(&self) -> Option<u64> {
        if !matches!(self.content, ClipboardContent::Image { .. }) {
            return None;
        }
        self.preview
            .split('#')
            .nth(1)
            .and_then(|h| h.parse::<u64>().ok())
    }
}

// --- Manager Logic ---

/// Manages clipboard operations and history
pub struct ClipboardManager {
    history: Vec<ClipboardItem>,
    /// Track the last pasted content to avoid re-adding it to history
    last_pasted_text: Option<String>,
    last_pasted_image_hash: Option<u64>,
    /// Track last added text hash to prevent duplicates from rapid copies
    last_added_text_hash: Option<u64>,
    /// Path to save the history file
    persistence_path: PathBuf,
    /// Maximum number of history items to keep
    max_history_size: usize,
    /// Set by the insert path; [`Self::flush_pending_save`] persists after
    /// [`SAVE_DEBOUNCE`], and Drop persists on shutdown.
    dirty_since: Option<Instant>,
}

impl ClipboardManager {
    fn clamp_max_history_size(size: usize) -> usize {
        match size {
            0 => DEFAULT_MAX_HISTORY_SIZE,
            1..=100_000 => size,
            _ => 100_000,
        }
    }

    pub fn new(persistence_path: PathBuf, max_history_size: usize) -> Self {
        // Normalize the requested max size and avoid huge allocations
        let max_size = Self::clamp_max_history_size(max_history_size);
        let mut manager = Self {
            history: Vec::with_capacity(max_size),
            last_pasted_text: None,
            last_pasted_image_hash: None,
            last_added_text_hash: None,
            persistence_path,
            max_history_size: max_size,
            dirty_since: None,
        };
        manager.load_history();
        manager
    }

    /// Updates the maximum history size and enforces the new limit
    pub fn set_max_history_size(&mut self, new_size: usize) {
        let mut clamped = Self::clamp_max_history_size(new_size);
        // Do not set max less than number of pinned items; we won't delete pins automatically
        let pinned_count = self.history.iter().filter(|i| i.pinned).count();
        if clamped < pinned_count {
            eprintln!(
                "clipboard_manager: requested max history size ({}) is less than the number of pinned items ({}); increasing limit to preserve pinned items.",
                clamped,
                pinned_count
            );
            clamped = pinned_count;
        }
        self.max_history_size = clamped;
        let trimmed = self.enforce_history_limit();
        if trimmed {
            self.save_history();
        }
    }

    /// Gets the current maximum history size
    pub fn get_max_history_size(&self) -> usize {
        self.max_history_size
    }

    fn load_history(&mut self) {
        if !self.persistence_path.exists() {
            return;
        }

        match fs::read_to_string(&self.persistence_path) {
            Ok(content) => {
                match serde_json::from_str::<Vec<ClipboardItem>>(&content) {
                    Ok(items) => {
                        // Reorder items so pinned come first while preserving order within each group
                        let mut pinned_items = Vec::new();
                        let mut unpinned_items = Vec::new();

                        for item in items {
                            if item.pinned {
                                pinned_items.push(item);
                            } else {
                                unpinned_items.push(item);
                            }
                        }

                        pinned_items.extend(unpinned_items);
                        self.history = pinned_items;
                        // Trim first so migrated storage is only written for
                        // items that actually stay in history.
                        let history_trimmed = self.enforce_history_limit();
                        // Bring pre-thumbnail / inline-base64 items to the
                        // current out-of-line storage.
                        let storage_migrated = self.migrate_image_storage();
                        // If anything changed, persist so disk stays in sync.
                        if history_trimmed || storage_migrated {
                            self.save_history();
                        }
                        self.gc_image_files();
                        // Initialize last_added_text_hash from the most recent item (even if pinned)
                        // This prevents duplication on startup if the clipboard content matches the top item
                        if let Some(first) = self.history.first() {
                            match &first.content {
                                ClipboardContent::Text(text) => {
                                    self.last_added_text_hash = Some(calculate_hash(text));
                                }
                                ClipboardContent::RichText { plain, .. } => {
                                    self.last_added_text_hash = Some(calculate_hash(plain));
                                }
                                ClipboardContent::Image { .. } => {
                                    if let Some(_hash) = first.extract_image_hash() {
                                        // We don't have a separate last_added_image_hash,
                                        // but we can at least avoid text hash collision
                                        self.last_added_text_hash = None;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to parse history: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to read history file: {}", e),
        }
    }

    pub fn save_history(&mut self) {
        match serde_json::to_string_pretty(&self.history) {
            Ok(content) => {
                if let Some(parent) = self.persistence_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                // Atomic replace: a crash mid-write must not take the whole
                // history with it.
                let tmp = self.persistence_path.with_extension("json.tmp");
                if let Err(e) = fs::write(&tmp, &content) {
                    eprintln!("Failed to save history: {}", e);
                    return;
                }
                if let Err(e) = fs::rename(&tmp, &self.persistence_path) {
                    eprintln!("Failed to save history: {}", e);
                }
                self.dirty_since = None;
            }
            Err(e) => eprintln!("Failed to serialize history: {}", e),
        }
    }

    /// Persist pending insert-path changes once [`SAVE_DEBOUNCE`] has elapsed.
    /// Called from the watcher tick; explicit operations save immediately.
    pub fn flush_pending_save(&mut self) {
        match self.dirty_since {
            Some(when) if when.elapsed() >= SAVE_DEBOUNCE => self.save_history(),
            _ => {}
        }
    }

    // --- Monitoring / Reading ---

    pub fn get_current_text(&mut self) -> Result<String, arboard::Error> {
        // We unwrap internal map error because arboard::Error is the expected return type here
        // for the monitoring loop in main.rs
        with_thread_clipboard(|clipboard| clipboard.get_text())
    }

    /// Try to get HTML content from clipboard. Returns None if not available.
    pub fn get_current_html(&self) -> Option<String> {
        with_thread_clipboard(|clipboard| Ok(clipboard.get().html().ok()))
            .ok()
            .flatten()
    }

    pub fn get_current_image(
        &mut self,
    ) -> Result<Option<(ImageData<'static>, u64)>, arboard::Error> {
        with_thread_clipboard(|clipboard| {
            match clipboard.get_image() {
                Ok(image) => {
                    let hash = calculate_hash(&image.bytes);
                    let owned = ImageData {
                        width: image.width,
                        height: image.height,
                        bytes: image.bytes.into_owned().into(),
                    };
                    Ok(Some((owned, hash)))
                }
                Err(arboard::Error::ContentNotAvailable) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    // --- Adding Items ---

    /// Add text content to history, with optional HTML for rich text
    pub fn add_text(&mut self, text: String, html: Option<String>) -> Option<ClipboardItem> {
        if self.should_skip_text(&text) {
            return None;
        }

        let text_hash = calculate_hash(&text);

        // Rapid copy detection
        if Some(text_hash) == self.last_added_text_hash {
            return None;
        }

        // Check if this exact text is already the most recent non-pinned item
        // If so, skip entirely - no need to add or move
        if self.is_duplicate_text(&text) {
            self.last_added_text_hash = Some(text_hash);
            return None;
        }

        // Check if this text exists elsewhere in history (not at top)
        // If so, remove the old entry so we can add fresh at top
        self.remove_duplicate_text_from_history(&text);

        // Create new item - use RichText if HTML is available, otherwise plain Text
        let item = match html {
            Some(html_content) if !html_content.trim().is_empty() => {
                ClipboardItem::new_rich_text(text, html_content)
            }
            _ => ClipboardItem::new_text(text),
        };
        self.insert_item(item.clone());

        self.last_added_text_hash = Some(text_hash);

        Some(item)
    }

    pub fn add_image(&mut self, image_data: ImageData<'_>, hash: u64) -> Option<ClipboardItem> {
        if self.should_skip_image(hash) {
            return None;
        }

        // Encode once: the full PNG goes to the images dir, a small preview
        // travels with the item.
        let png = Self::encode_image_png(&image_data)?;
        let thumb = Self::make_thumb_base64(&image_data);
        let width = image_data.width as u32;
        let height = image_data.height as u32;

        let mut item = ClipboardItem::new_image(thumb, width, height, hash);
        self.attach_full_png(&mut item, &png);
        self.insert_item(item.clone());
        Some(item)
    }

    // --- Image storage (out-of-line full PNGs + inline thumbs) ---

    /// Directory holding the full-resolution PNGs, next to history.json so
    /// each frontend's data stays self-contained.
    fn images_dir(&self) -> PathBuf {
        self.persistence_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("images")
    }

    fn encode_image_png(image_data: &ImageData<'_>) -> Option<Vec<u8>> {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
            image_data.width as u32,
            image_data.height as u32,
            image_data.bytes.to_vec(),
        )?); // Returns None if dimensions don't match bytes

        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, ImageFormat::Png).ok()?;
        Some(buffer.into_inner())
    }

    /// Downscale to the preview fit box (never upscaled) and base64-encode.
    fn make_thumb_base64(image_data: &ImageData<'_>) -> Option<String> {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
            image_data.width as u32,
            image_data.height as u32,
            image_data.bytes.to_vec(),
        )?);
        Self::thumb_png_base64(&img)
    }

    /// Backfill path: build a thumb from already-encoded PNG bytes.
    fn make_thumb_png_base64(png: &[u8]) -> Option<String> {
        let img = image::load_from_memory(png).ok()?;
        Self::thumb_png_base64(&img)
    }

    fn thumb_png_base64(img: &DynamicImage) -> Option<String> {
        let thumb = if img.width() > THUMB_MAX_W || img.height() > THUMB_MAX_H {
            img.thumbnail(THUMB_MAX_W, THUMB_MAX_H)
        } else {
            img.clone()
        };
        let mut buffer = Cursor::new(Vec::new());
        thumb.write_to(&mut buffer, ImageFormat::Png).ok()?;
        Some(BASE64.encode(buffer.into_inner()))
    }

    /// Store the full-resolution PNG for a freshly created image item.
    /// Prefer the out-of-line file; if the images dir cannot be written, keep
    /// the legacy inline base64 so the item still works.
    fn attach_full_png(&self, item: &mut ClipboardItem, png: &[u8]) {
        if let ClipboardContent::Image { path, base64, .. } = &mut item.content {
            match Self::write_image_file(&self.images_dir(), &item.id, png) {
                Ok(stored) => *path = Some(stored),
                Err(e) => {
                    eprintln!("[ClipboardManager] image file store failed, keeping inline: {e}");
                    *base64 = Some(BASE64.encode(png));
                }
            }
        }
    }

    /// Write an image file atomically (temp + rename) and return its path.
    fn write_image_file(images_dir: &Path, id: &str, png: &[u8]) -> Result<String, String> {
        fs::create_dir_all(images_dir).map_err(|e| format!("create images dir: {e}"))?;
        let file = images_dir.join(format!("{id}.png"));
        let tmp = images_dir.join(format!("{id}.png.tmp"));
        fs::write(&tmp, png).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        fs::rename(&tmp, &file).map_err(|e| format!("rename {}: {e}", file.display()))?;
        Ok(file.to_string_lossy().into_owned())
    }

    /// Best-effort removal of the full-resolution PNG when an item leaves
    /// history.
    fn delete_image_file(item: &ClipboardItem) {
        if let Some(path) = item.content.image_file() {
            let _ = fs::remove_file(path);
        }
    }

    /// Bring items persisted by older versions to the current storage: move
    /// inline base64 PNGs into the images dir, restore files that vanished
    /// while an inline copy still exists, drop redundant inline copies, and
    /// backfill missing thumbnails. Returns true if anything changed.
    fn migrate_image_storage(&mut self) -> bool {
        let images_dir = self.images_dir();
        let mut changed = false;
        for item in &mut self.history {
            let ClipboardContent::Image {
                path,
                base64,
                thumb,
                ..
            } = &mut item.content
            else {
                continue;
            };

            // Inline legacy data -> out-of-line file.
            if path.is_none() {
                if let Some(b64) = base64.as_ref() {
                    match BASE64.decode(b64) {
                        Ok(png) => match Self::write_image_file(&images_dir, &item.id, &png) {
                            Ok(stored) => {
                                *path = Some(stored);
                                *base64 = None;
                                changed = true;
                            }
                            Err(e) => {
                                eprintln!(
                                    "[ClipboardManager] image migration store failed, keeping inline: {e}"
                                )
                            }
                        },
                        Err(e) => eprintln!("[ClipboardManager] undecodable inline image: {e}"),
                    }
                }
            }

            if let Some(stored) = path.as_ref() {
                if Path::new(stored).exists() {
                    // Healthy: the inline copy (if any) is redundant.
                    if base64.is_some() {
                        *base64 = None;
                        changed = true;
                    }
                } else if let Some(b64) = base64.as_ref() {
                    // File vanished (crash between insert and save, manual
                    // deletion) — restore it from the inline copy.
                    if let Ok(png) = BASE64.decode(b64) {
                        match Self::write_image_file(&images_dir, &item.id, &png) {
                            Ok(restored) => {
                                *path = Some(restored);
                                changed = true;
                            }
                            Err(e) => eprintln!("[ClipboardManager] image restore failed: {e}"),
                        }
                    }
                }
            }

            // Backfill the thumbnail for items persisted before thumbnails.
            if thumb.is_none() {
                let png = match (path.as_ref(), base64.as_ref()) {
                    (Some(p), _) => fs::read(p).ok(),
                    (None, Some(b64)) => BASE64.decode(b64).ok(),
                    (None, None) => None,
                };
                if let Some(png) = png {
                    if let Some(backfilled) = Self::make_thumb_png_base64(&png) {
                        *thumb = Some(backfilled);
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// Delete image files no history item references (leftovers from crashes
    /// between insert and save, or from items lost to a failed save).
    fn gc_image_files(&self) {
        let dir = self.images_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };
        let referenced: HashSet<String> = self
            .history
            .iter()
            .filter_map(|item| item.content.image_file())
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let orphan = name.ends_with(".tmp")
                || (name.ends_with(".png") && !referenced.contains(name.as_ref()));
            if orphan {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    // --- State Management Helpers ---

    fn should_skip_text(&mut self, text: &str) -> bool {
        if text.trim().is_empty() {
            return true;
        }

        // Skip internal GIF cache URIs
        if text.contains(FILE_URI_PREFIX) && text.contains(GIF_CACHE_MARKER) {
            eprintln!("[ClipboardManager] Skipping GIF cache URI");
            return true;
        }

        // Skip self-pasted content
        if let Some(ref pasted) = self.last_pasted_text {
            if pasted == text {
                self.last_pasted_text = None;
                return true;
            }
            // Clipboard has changed to something else; the paste echo window has passed.
            self.last_pasted_text = None;
        }

        false
    }

    fn should_skip_image(&mut self, hash: u64) -> bool {
        // Check if just pasted
        if let Some(pasted_hash) = self.last_pasted_image_hash {
            if pasted_hash == hash {
                self.last_pasted_image_hash = None;
                return true;
            }
        }

        // Check if it's the exact same image as the most recent non-pinned item
        if let Some(item) = self.history.iter().find(|item| !item.pinned) {
            if let Some(item_hash) = item.extract_image_hash() {
                if item_hash == hash {
                    return true;
                }
            }
        }

        false
    }

    fn is_duplicate_text(&self, text: &str) -> bool {
        // Check only the very first non-pinned item for exact match logic
        // used in rapid detection
        if let Some(item) = self.history.iter().find(|item| !item.pinned) {
            match &item.content {
                ClipboardContent::Text(t) if t == text => return true,
                ClipboardContent::RichText { plain, .. } if plain == text => return true,
                _ => {}
            }
        }
        false
    }

    fn remove_duplicate_text_from_history(&mut self, text: &str) {
        if let Some(pos) = self.history.iter().position(|item| {
            if item.pinned {
                return false;
            }
            match &item.content {
                ClipboardContent::Text(t) => t == text,
                ClipboardContent::RichText { plain, .. } => plain == text,
                _ => false,
            }
        }) {
            self.history.remove(pos);
        }
    }

    fn insert_item(&mut self, item: ClipboardItem) {
        // Insert after pinned items (first non-pinned slot)
        // If all items are pinned, insert at the end to preserve pinned ordering
        let insert_pos = self
            .history
            .iter()
            .position(|i| !i.pinned)
            .unwrap_or(self.history.len());
        self.history.insert(insert_pos, item);

        // Trim history, then defer the disk write to the debounce window —
        // the watcher flushes it, and Drop persists on shutdown.
        self.enforce_history_limit();
        self.dirty_since = Some(Instant::now());
    }

    /// Enforce the configured history size. Returns true if trimming occurred.
    fn enforce_history_limit(&mut self) -> bool {
        let before = self.history.len();
        while self.history.len() > self.max_history_size {
            // Remove from the end, skipping pinned items if possible
            if let Some(pos) = self.history.iter().rposition(|i| !i.pinned) {
                let removed = self.history.remove(pos);
                Self::delete_image_file(&removed);
            } else {
                // All items are pinned. We stopped removing to avoid deleting pins.
                break;
            }
        }
        self.history.len() != before
    }

    // --- Accessors ---

    pub fn get_history(&self) -> Vec<ClipboardItem> {
        self.history.clone()
    }

    pub fn get_item(&self, id: &str) -> Option<&ClipboardItem> {
        self.history.iter().find(|item| item.id == id)
    }

    pub fn clear(&mut self) {
        self.history.retain(|item| {
            if item.pinned {
                true
            } else {
                Self::delete_image_file(item);
                false
            }
        });
        self.save_history();
    }

    pub fn remove_item(&mut self, id: &str) {
        if let Some(pos) = self.history.iter().position(|item| item.id == id) {
            let removed = self.history.remove(pos);
            Self::delete_image_file(&removed);
            self.save_history();
        }
    }

    pub fn toggle_pin(&mut self, id: &str) -> Option<ClipboardItem> {
        // Find the item and toggle its pin status
        let pos = self.history.iter().position(|i| i.id == id)?;
        self.history[pos].pinned = !self.history[pos].pinned;

        // Reposition the item so the invariant
        let item = self.history.remove(pos);
        let insert_pos = self
            .history
            .iter()
            .position(|i| !i.pinned)
            .unwrap_or(self.history.len());
        self.history.insert(insert_pos, item);

        let item_clone = self.history[insert_pos].clone();
        self.save_history();
        Some(item_clone)
    }

    /// Move an item to the top of the history (respecting pinned items)
    /// If the item is pinned, it moves to the top of pinned items
    /// If not pinned, it moves to the first non-pinned position
    pub fn move_item_to_top(&mut self, id: &str) -> bool {
        // Find the item's current position
        let current_pos = match self.history.iter().position(|i| i.id == id) {
            Some(pos) => pos,
            None => return false, // Item not found
        };
        // Determine where we *would* insert based on pinned status, without mutating yet
        let item_pinned = self.history[current_pos].pinned;
        let insert_pos = if item_pinned {
            // Move to top of pinned items (position 0)
            0
        } else {
            // Move to first non-pinned position (right after all pinned items)
            self.history
                .iter()
                .position(|i| !i.pinned)
                .unwrap_or(self.history.len())
        };
        // If the item is already at the correct position, avoid unnecessary mutation and I/O
        if insert_pos == current_pos {
            return true;
        }
        // Now actually move the item
        let item = self.history.remove(current_pos);
        self.history.insert(insert_pos, item);
        self.save_history();
        true
    }

    pub fn cleanup_old_items(&mut self, interval_minutes: u64) -> bool {
        if interval_minutes == 0 {
            return false;
        }

        let now = Utc::now();
        let mut changed = false;

        // Use a more robust time comparison
        self.history.retain(|item| {
            if item.pinned {
                return true;
            }

            let age_seconds = now.signed_duration_since(item.timestamp).num_seconds();
            let interval_seconds = (interval_minutes * 60) as i64;
            let keep = age_seconds < interval_seconds;

            if !keep {
                changed = true;
                Self::delete_image_file(item);
                println!(
                    "[ClipboardManager] Auto-deleting old item: {} (age: {}s, limit: {}s)",
                    item.id, age_seconds, interval_seconds
                );
            }
            keep
        });

        if changed {
            self.save_history();
        }

        changed
    }

    // --- Paste Logic ---

    pub fn mark_as_pasted(&mut self, item: &ClipboardItem) {
        match &item.content {
            ClipboardContent::Text(text) => {
                self.last_pasted_text = Some(text.clone());
                self.last_pasted_image_hash = None;
            }
            ClipboardContent::RichText { plain, html: _ } => {
                self.last_pasted_text = Some(plain.clone());
                self.last_pasted_image_hash = None;
            }
            ClipboardContent::Image { .. } => {
                if let Some(hash) = item.extract_image_hash() {
                    self.last_pasted_image_hash = Some(hash);
                }
                self.last_pasted_text = None;
            }
        }
    }

    /// Mark a specific text as pasted (to prevent it from appearing in history)
    /// Used for emojis/special insertions
    pub fn mark_text_as_pasted(&mut self, text: &str) {
        self.last_pasted_text = Some(text.to_string());
        self.last_added_text_hash = Some(calculate_hash(&text));
    }

    pub fn paste_item(&mut self, item: &ClipboardItem) -> Result<(), String> {
        // 1. Prevent loop: Mark as pasted before OS action
        self.mark_as_pasted(item);

        // 2. Write content to OS clipboard
        match &item.content {
            ClipboardContent::Text(text) => {
                self.set_text_robust(text)?;
            }
            ClipboardContent::RichText { plain, html } => {
                // Set HTML with plain text as fallback - this preserves formatting
                self.set_html_robust(html, plain)?;
            }
            ClipboardContent::Image { width, height, .. } => {
                let png = item
                    .content
                    .image_png()?
                    .ok_or_else(|| "Image data is missing".to_string())?;
                self.set_image_robust(&png, *width, *height)?;
            }
        }

        // 3. Simulate User Input
        self.simulate_paste_action()?;

        // 4. Move item to top of history so it's easily accessible for repeated use
        self.move_item_to_top(&item.id);

        Ok(())
    }

    fn set_image_robust(&self, png: &[u8], width: u32, height: u32) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            if crate::session::is_wayland() {
                if self
                    .set_clipboard_external("wl-copy", &["--type", "image/png"], png)
                    .is_ok()
                {
                    return Ok(());
                }
            } else if self
                .set_clipboard_external(
                    "xclip",
                    &["-selection", "clipboard", "-t", "image/png"],
                    png,
                )
                .is_ok()
            {
                return Ok(());
            }
        }

        let img = image::load_from_memory(png).map_err(|e| format!("Image load failed: {}", e))?;
        let rgba = img.to_rgba8();
        let expected_bytes = rgba.into_raw();

        // The closure must surface arboard errors itself; the String error the
        // callers see travels as the inner result.
        with_thread_clipboard(|clipboard| {
            let image_data = ImageData {
                width: width as usize,
                height: height as usize,
                bytes: expected_bytes.clone().into(),
            };

            let () = clipboard.set_image(image_data)?;

            // Reading through the same arboard instance performs the backend
            // round trip that confirms our provider owns and serves the selection.
            let observed = clipboard.get_image()?;
            if observed.width != width as usize
                || observed.height != height as usize
                || observed.bytes.as_ref() != expected_bytes.as_slice()
            {
                return Ok(Err(
                    "Clipboard image verification returned different data".to_string()
                ));
            }

            Ok(Ok(()))
        })
    }

    fn simulate_paste_action(&self) -> Result<(), String> {
        // Clipboard writers return only after their platform-specific
        // readiness barrier. The serving process (or arboard's verified global
        // worker) outlives this call, so no fixed post-paste retention is needed.
        crate::input_simulator::simulate_paste_keystroke()
    }

    /// Robustly set text to clipboard using xclip/wl-copy on Linux if available,
    /// falling back to arboard. This fixes issues on distros like Kali Linux.
    pub fn set_text_robust(&self, text: &str) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            if crate::session::is_wayland() {
                if let Ok(()) = self.set_clipboard_external(
                    "wl-copy",
                    &["--type", "text/plain;charset=utf-8"],
                    text.as_bytes(),
                ) {
                    return Ok(());
                }
            } else if let Ok(()) = self.set_clipboard_external(
                "xclip",
                &["-selection", "clipboard", "-t", "UTF8_STRING"],
                text.as_bytes(),
            ) {
                return Ok(());
            }
        }

        // Fallback to arboard
        with_thread_clipboard(|clipboard| {
            clipboard.set_text(text)?;
            let observed = clipboard.get_text()?;
            if observed != text {
                return Ok(Err(
                    "Clipboard text verification returned different data".to_string()
                ));
            }
            Ok(Ok(()))
        })
    }

    /// Robustly set HTML to clipboard using xclip/wl-copy on Linux if available,
    /// falling back to arboard.
    pub fn set_html_robust(&self, html: &str, plain: &str) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            if crate::session::is_wayland() {
                if let Ok(()) = self.set_clipboard_external(
                    "wl-copy",
                    &["--type", "text/html"],
                    html.as_bytes(),
                ) {
                    let _ = self.set_text_robust(plain);
                    return Ok(());
                }
            } else if let Ok(()) = self.set_clipboard_external(
                "xclip",
                &["-selection", "clipboard", "-t", "text/html"],
                html.as_bytes(),
            ) {
                let _ = self.set_text_robust(plain);
                return Ok(());
            }
        }

        // Fallback to arboard (which handles multiple MIME types correctly)
        with_thread_clipboard(|clipboard| {
            clipboard.set_html(html, Some(plain))?;
            let observed = clipboard.get().html()?;
            if observed != html {
                return Ok(Err(
                    "Clipboard HTML verification returned different data".to_string()
                ));
            }
            Ok(Ok(()))
        })
    }

    fn set_clipboard_external(&self, cmd: &str, args: &[&str], data: &[u8]) -> Result<(), String> {
        use std::io::{Read, Write};
        use std::process::{Command, Stdio};

        // Snapshot the current selection owner so we can detect the handoff.
        let owner_before = crate::paste_sync::clipboard_owner();

        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", cmd, e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(data)
                .map_err(|e| format!("Pipe write error: {}", e))?;
        }

        if cmd == "wl-copy" {
            // wl-copy's foreground parent exits only after the Wayland
            // selection was installed. Waiting for that exit is an adaptive
            // readiness acknowledgement: immediate on a fast compositor and
            // longer only while a constrained compositor is still working.
            return wait_for_clipboard_helper_ready(&mut child, cmd);
        }

        // On X11, selection ownership is directly observable. A timeout is a
        // failed precondition, not permission to emit an unverified Ctrl+V.
        let handoff_confirmed = crate::paste_sync::settle_clipboard_handoff(
            owner_before,
            CLIPBOARD_HELPER_READY_TIMEOUT,
        );
        if !handoff_confirmed {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "{} did not acquire the clipboard selection within {:?}",
                cmd, CLIPBOARD_HELPER_READY_TIMEOUT
            ));
        }

        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                let mut stderr = String::new();
                if let Some(mut stderr_pipe) = child.stderr.take() {
                    let _ = stderr_pipe.read_to_string(&mut stderr);
                }
                Err(format!(
                    "{} exited with status {}. Stderr: {}",
                    cmd,
                    status,
                    stderr.trim()
                ))
            }
            Ok(_) => {
                // xclip remains alive to serve selection requests. Reap it in
                // the background after another application takes ownership.
                thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => Err(format!("Process status check failed: {}", e)),
        }
    }
}

/// Persist debounced insert-path changes on shutdown.
impl Drop for ClipboardManager {
    fn drop(&mut self) {
        if self.dirty_since.is_some() {
            self.save_history();
        }
    }
}

/// Waits for helpers such as wl-copy whose parent process exits after the
/// clipboard selection is ready. The child serving the selection has already
/// forked by then, so waiting for the parent does not shorten clipboard life.
fn wait_for_clipboard_helper_ready(
    child: &mut std::process::Child,
    command: &str,
) -> Result<(), String> {
    use std::io::Read;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return Err(format!(
                    "{} exited with status {}. Stderr: {}",
                    command,
                    status,
                    stderr.trim()
                ));
            }
            Ok(None) if start.elapsed() < CLIPBOARD_HELPER_READY_TIMEOUT => {
                thread::sleep(CLIPBOARD_HELPER_POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{} did not confirm clipboard readiness within {:?}",
                    command, CLIPBOARD_HELPER_READY_TIMEOUT
                ));
            }
            Err(error) => {
                return Err(format!("Failed to inspect {} status: {}", command, error));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png_base64() -> String {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([255, 0, 0, 255]),
        ));
        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, ImageFormat::Png).unwrap();
        BASE64.encode(buffer.into_inner())
    }

    fn solid_image(w: usize, h: usize) -> ImageData<'static> {
        ImageData {
            width: w,
            height: h,
            bytes: vec![90u8; w * h * 4].into(),
        }
    }

    fn manager_in(dir: &tempfile::TempDir) -> ClipboardManager {
        ClipboardManager::new(dir.path().join("history.json"), 10)
    }

    fn png_dims(png: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(png).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn legacy_inline_image_migrates_to_file_and_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = format!(
            r#"[{{
                "id": "legacy-1",
                "content": {{ "type": "Image", "data": {{
                    "base64": "{}", "width": 2, "height": 2 }} }},
                "timestamp": "2026-01-01T00:00:00Z",
                "pinned": false,
                "preview": "Image (2x2) #1"
            }}]"#,
            tiny_png_base64()
        );
        fs::write(dir.path().join("history.json"), legacy).unwrap();

        let manager = manager_in(&dir);
        let item = &manager.history[0];
        let path = item.content.image_file().expect("migrated to a file");
        assert!(Path::new(path).exists());
        let ClipboardContent::Image { base64, thumb, .. } = &item.content else {
            panic!("not an image")
        };
        assert!(base64.is_none(), "inline copy moved out of the JSON");
        assert!(thumb.is_some(), "thumbnail backfilled");

        let png = item.content.image_png().unwrap().unwrap();
        assert_eq!(png_dims(&png), (2, 2));
    }

    #[test]
    fn new_items_store_full_png_out_of_line() {
        let dir = tempfile::tempdir().unwrap();
        let (thumb, path, full_len) = {
            let mut manager = manager_in(&dir);
            let item = manager.add_image(solid_image(64, 32), 1).unwrap();
            let ClipboardContent::Image {
                path,
                base64,
                thumb,
                ..
            } = &item.content
            else {
                panic!("not an image")
            };
            let stored = path.clone().expect("file path set");
            assert!(Path::new(&stored).exists());
            assert!(base64.is_none(), "healthy disk write needs no inline copy");
            let full_len = item.content.image_png().unwrap().unwrap().len();
            (thumb.clone(), stored, full_len)
        };

        // Reload: storage and thumb survive verbatim, no re-encode.
        let manager = manager_in(&dir);
        let item = &manager.history[0];
        assert_eq!(item.content.image_file(), Some(path.as_str()));
        let ClipboardContent::Image {
            thumb: reloaded_thumb,
            ..
        } = &item.content
        else {
            panic!("not an image")
        };
        assert_eq!(*reloaded_thumb, thumb);
        assert_eq!(item.content.image_png().unwrap().unwrap().len(), full_len);
    }

    #[test]
    fn thumb_is_downscaled_but_full_size_kept() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = manager_in(&dir);
        let item = manager.add_image(solid_image(1920, 1080), 2).unwrap();
        assert_eq!(item_width(&item), 1920, "item keeps full dimensions");
        assert_eq!(item_height(&item), 1080);

        let thumb = item_thumb(&item).unwrap();
        let png = BASE64.decode(thumb).unwrap();
        let (tw, th) = png_dims(&png);
        assert!(th <= 256 && tw <= 1024, "thumb fits the box: {tw}x{th}");
        assert!(tw < 1920, "large image must be downscaled");
        let aspect = tw as f32 / th as f32;
        assert!(
            (aspect - 1920.0 / 1080.0).abs() < 0.05,
            "aspect preserved: {aspect}"
        );
    }

    #[test]
    fn small_images_are_not_upscaled() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = manager_in(&dir);
        let item = manager.add_image(solid_image(64, 32), 3).unwrap();
        let thumb = item_thumb(&item).unwrap();
        let png = BASE64.decode(thumb).unwrap();
        assert_eq!(png_dims(&png), (64, 32));
    }

    #[test]
    fn remove_item_deletes_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = manager_in(&dir);
        let item = manager.add_image(solid_image(16, 16), 4).unwrap();
        let path = item.content.image_file().unwrap().to_string();
        manager.remove_item(&item.id);
        assert!(!Path::new(&path).exists());
        assert!(manager.history.is_empty());
    }

    #[test]
    fn clear_keeps_pinned_file_and_deletes_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = manager_in(&dir);
        let kept = manager.add_image(solid_image(16, 16), 5).unwrap();
        let dropped = manager.add_image(solid_image(16, 16), 6).unwrap();
        manager.toggle_pin(&kept.id);
        manager.clear();

        assert!(Path::new(kept.content.image_file().unwrap()).exists());
        assert!(!Path::new(dropped.content.image_file().unwrap()).exists());
        assert_eq!(manager.history.len(), 1);
    }

    #[test]
    fn enforce_limit_trims_and_deletes_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut manager = ClipboardManager::new(path, 1);
        let first = manager.add_image(solid_image(16, 16), 7).unwrap();
        let second = manager.add_image(solid_image(16, 16), 8).unwrap();

        assert_eq!(manager.history.len(), 1);
        assert!(!Path::new(first.content.image_file().unwrap()).exists());
        assert!(Path::new(second.content.image_file().unwrap()).exists());
    }

    #[test]
    fn gc_removes_unreferenced_and_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = manager_in(&dir);
        let item = manager.add_image(solid_image(16, 16), 9).unwrap();
        // The insert path debounces its save; persist so the reload below
        // sees the item and keeps its file.
        manager.save_history();
        let images = dir.path().join("images");
        fs::write(images.join("orphan.png"), b"not a real png").unwrap();
        fs::write(images.join("half-written.png.tmp"), b"junk").unwrap();

        // Reload runs the GC pass.
        let manager = manager_in(&dir);
        assert!(Path::new(item.content.image_file().unwrap()).exists());
        assert!(!images.join("orphan.png").exists());
        assert!(!images.join("half-written.png.tmp").exists());
    }

    #[test]
    fn missing_file_is_restored_from_inline_copy() {
        let dir = tempfile::tempdir().unwrap();
        let new_format = format!(
            r#"[{{
                "id": "victim-1",
                "content": {{ "type": "Image", "data": {{
                    "path": "/nonexistent/images/victim-1.png",
                    "base64": "{}", "width": 2, "height": 2 }} }},
                "timestamp": "2026-01-01T00:00:00Z",
                "pinned": false,
                "preview": "Image (2x2) #1"
            }}]"#,
            tiny_png_base64()
        );
        fs::write(dir.path().join("history.json"), new_format).unwrap();

        let manager = manager_in(&dir);
        let item = &manager.history[0];
        let restored = item.content.image_file().unwrap();
        assert!(
            restored.ends_with("victim-1.png") && Path::new(restored).exists(),
            "file restored at {restored}"
        );
    }

    #[test]
    fn image_png_falls_back_to_inline_when_file_missing() {
        let content = ClipboardContent::Image {
            path: Some("/nonexistent/images/x.png".to_string()),
            base64: Some(tiny_png_base64()),
            thumb: None,
            width: 2,
            height: 2,
        };
        let png = content.image_png().unwrap().expect("inline fallback");
        assert_eq!(png_dims(&png), (2, 2));
    }

    #[test]
    fn image_with_no_surviving_data_stays_displayable() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"[{
                "id": "gone-1",
                "content": { "type": "Image", "data": {
                    "path": "/nonexistent/images/gone-1.png",
                    "thumb": "aGk=", "width": 2, "height": 2 } },
                "timestamp": "2026-01-01T00:00:00Z",
                "pinned": false,
                "preview": "Image (2x2) #1"
            }]"#;
        fs::write(dir.path().join("history.json"), json).unwrap();

        let manager = manager_in(&dir);
        assert_eq!(manager.history.len(), 1);
        let item = &manager.history[0];
        assert_eq!(item.content.image_png().unwrap(), None);
        let ClipboardContent::Image { thumb, .. } = &item.content else {
            panic!("not an image")
        };
        assert_eq!(thumb.as_deref(), Some("aGk="));
    }

    fn item_width(item: &ClipboardItem) -> u32 {
        match &item.content {
            ClipboardContent::Image { width, .. } => *width,
            _ => panic!("not an image"),
        }
    }

    fn item_height(item: &ClipboardItem) -> u32 {
        match &item.content {
            ClipboardContent::Image { height, .. } => *height,
            _ => panic!("not an image"),
        }
    }

    fn item_thumb(item: &ClipboardItem) -> Option<String> {
        match &item.content {
            ClipboardContent::Image { thumb, .. } => thumb.clone(),
            _ => panic!("not an image"),
        }
    }
}
