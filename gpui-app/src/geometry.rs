//! Window positioning math — port of `WindowController` + `config_manager`
//! geometry from the Tauri app (`src-tauri/src/main.rs`, `config_manager.rs`).
//!
//! Pure math only; reading the cursor is backend work (`platform`).
//!
//! Behavior parity contract:
//! - Backends that can report the cursor: popup follows it, clamped into the
//!   containing monitor.
//! - Otherwise (native Wayland): NO cursor follow (Tauri app doesn't either) —
//!   bottom-center of the primary/first display.
//!
//! Frame contract: every value in here is **physical** pixels. The cursor query,
//! the X11 `ConfigureWindow` move and the popup's real size are all physical
//! (gpui scales the logical window bounds it is handed). gpui's
//! `PlatformDisplay::bounds()` is the exception — the X11 backend divides the
//! screen by the scale factor — so callers run it through [`MonitorRect::scaled`]
//! first. Clamping a physical cursor against a logical monitor is what pinned
//! the popup to `logical_w - popup_w` (the middle of the screen on HiDPI)
//! instead of letting it follow the cursor to the right edge.

/// Display-independent monitor rectangle (physical pixels).
#[derive(Debug, Clone, Copy)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl MonitorRect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    /// Logical rect → physical rect (`scale` = display scale factor).
    ///
    /// gpui reports a 3072x1728 screen at scale 2 as 1536x864; the cursor, the
    /// popup size and the X move are all in the 3072x1728 frame.
    pub fn scaled(&self, scale: f32) -> MonitorRect {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        MonitorRect {
            x: (self.x as f32 * scale).round() as i32,
            y: (self.y as f32 * scale).round() as i32,
            w: (self.w as f32 * scale).round() as i32,
            h: (self.h as f32 * scale).round() as i32,
        }
    }
}

pub const POPUP_W: i32 = 360;
pub const POPUP_H: i32 = 480;
const CLAMP_PADDING: i32 = 10;
const BOTTOM_PADDING: i32 = 45;

/// Follow-mouse position clamped into the monitor (port of `clamp_window_to_monitor`).
pub fn clamp_to_monitor(mon: &MonitorRect, win_w: i32, win_h: i32, x: i32, y: i32) -> (i32, i32) {
    // A popup taller/wider than its monitor inverts the range, and `clamp`
    // panics on `min > max` — with `panic = "abort"` that would take the whole
    // app down, so pin to the padded origin instead.
    let min_x = mon.x + CLAMP_PADDING;
    let min_y = mon.y + CLAMP_PADDING;
    let max_x = mon.x + mon.w - win_w - CLAMP_PADDING;
    let max_y = mon.y + mon.h - win_h - CLAMP_PADDING;
    let safe_x = if max_x < min_x { min_x } else { x.clamp(min_x, max_x) };
    let safe_y = if max_y < min_y { min_y } else { y.clamp(min_y, max_y) };
    (safe_x, safe_y)
}

/// Bottom-center fallback (port of `calculate_bottom_center`).
pub fn bottom_center(mon: &MonitorRect, win_w: i32, win_h: i32) -> (i32, i32) {
    let x = mon.x + (mon.w / 2) - (win_w / 2);
    let y = mon.y + mon.h - win_h - BOTTOM_PADDING;
    (x, y)
}

/// Saved-position validity (port of `is_position_valid`).
pub fn position_valid(mon: &MonitorRect, win_h: i32, x: i32, y: i32) -> bool {
    mon.contains(x, y) && y < (mon.y + mon.h - (win_h / 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon() -> MonitorRect {
        MonitorRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        }
    }

    #[test]
    fn clamp_keeps_popup_inside_with_padding() {
        // Cursor near bottom-right corner → clamped back with 10px padding.
        let (x, y) = clamp_to_monitor(&mon(), POPUP_W, POPUP_H, 1900, 1060);
        assert_eq!((x, y), (1920 - 360 - 10, 1080 - 480 - 10));
        // Cursor near top-left → pushed forward with 10px padding.
        let (x, y) = clamp_to_monitor(&mon(), POPUP_W, POPUP_H, 0, 0);
        assert_eq!((x, y), (10, 10));
        // Center cursor passes through.
        let (x, y) = clamp_to_monitor(&mon(), POPUP_W, POPUP_H, 960, 540);
        assert_eq!((x, y), (960, 540));
    }

    #[test]
    fn bottom_center_matches_tauri_math() {
        let (x, y) = bottom_center(&mon(), POPUP_W, POPUP_H);
        assert_eq!(x, 960 - 180);
        assert_eq!(y, 1080 - 480 - 45);
    }

    #[test]
    fn saved_position_validity_matches_tauri_heuristics() {
        assert!(position_valid(&mon(), POPUP_H, 100, 100));
        // Top-left outside monitor → invalid.
        assert!(!position_valid(&mon(), POPUP_H, -50, 100));
        // Window hanging entirely below the screen → invalid.
        assert!(!position_valid(&mon(), POPUP_H, 100, 1080));
    }

    #[test]
    fn hidpi_cursor_on_the_right_still_reaches_the_right_edge() {
        // Reported bug: a 1536x864 logical monitor at scale 2 is 3072x1728
        // physical, so a 720x960 window must be allowed out to 3072-720-10.
        let mon = MonitorRect {
            x: 0,
            y: 0,
            w: 1536,
            h: 864,
        }
        .scaled(2.0);
        assert_eq!((mon.w, mon.h), (3072, 1728));
        let (x, y) = clamp_to_monitor(&mon, 720, 960, 2600, 900);
        // Bottom-right corner of the cursor is clamped back inside both axes.
        assert_eq!((x, y), (3072 - 720 - 10, 1728 - 960 - 10));
        // A cursor inside the reachable area is followed through untouched.
        assert_eq!(clamp_to_monitor(&mon, 720, 960, 2000, 700), (2000, 700));
        // …while the same cursor clamped against the *logical* rect is the old,
        // broken answer: 1536-720-10, i.e. the middle of the screen.
        let logical = MonitorRect {
            x: 0,
            y: 0,
            w: 1536,
            h: 864,
        };
        assert_eq!(clamp_to_monitor(&logical, 720, 960, 2600, 900).0, 806);
    }

    #[test]
    fn clamp_never_panics_when_the_popup_is_larger_than_the_monitor() {
        let small = MonitorRect {
            x: 0,
            y: 0,
            w: 300,
            h: 400,
        };
        // Would be `clamp(250, 10, -430)` — a panic — without the guard.
        assert_eq!(clamp_to_monitor(&small, 720, 960, 250, 380), (10, 10));
    }

    #[test]
    fn multi_monitor_offset_math() {
        let right = MonitorRect {
            x: 1920,
            y: 0,
            w: 1920,
            h: 1080,
        };
        assert!(right.contains(2000, 500));
        let (x, y) = clamp_to_monitor(&right, POPUP_W, POPUP_H, 2000, 500);
        assert_eq!((x, y), (2000, 500));
    }
}
