//! Config Manager Module
//! Handles persistence of window state (position, monitor) specifically for Wayland usage.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "window_state.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorBounds {
    pub name: Option<String>,
    pub position: Position,
    pub size: Size,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowState {
    pub monitor_name: Option<String>,
    pub x: i32,
    pub y: i32,
}

pub struct ConfigManager {
    data_dir: PathBuf,
    state: WindowState,
    dirty: bool, // Tracks if we have unsaved changes in memory
}

impl ConfigManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let mut manager = Self {
            data_dir,
            state: WindowState::default(),
            dirty: false,
        };

        if let Err(e) = manager.load() {
            eprintln!(
                "[ConfigManager] Warning: Failed to load config: {}. Defaulting to empty state.",
                e
            );
        }

        manager
    }

    pub fn get_state(&self) -> WindowState {
        self.state.clone()
    }

    /// Updates the state in memory only. Use sync_to_disk() to flush.
    pub fn update_state(&mut self, monitor_name: Option<String>, x: i32, y: i32) {
        self.state.monitor_name = monitor_name;
        self.state.x = x;
        self.state.y = y;
        self.dirty = true;
    }

    /// Flushes changes to disk only if there are unsaved changes.
    pub fn sync_to_disk(&mut self) {
        if self.dirty {
            if let Err(e) = self.save_to_disk() {
                eprintln!("[ConfigManager] Failed to save config: {}", e);
            } else {
                self.dirty = false;
            }
        }
    }

    // --- IO ---

    fn config_path(&self) -> PathBuf {
        self.data_dir.join(CONFIG_FILE)
    }

    fn load(&mut self) -> Result<(), String> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.state = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn save_to_disk(&self) -> Result<(), String> {
        if !self.data_dir.exists() {
            fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        }
        let content = serde_json::to_string_pretty(&self.state).map_err(|e| e.to_string())?;
        fs::write(self.config_path(), content).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Calculates a centered position at the bottom of the screen.
pub fn calculate_bottom_center(
    mon_x: i32,
    mon_y: i32,
    mon_width: u32,
    mon_height: u32,
    win_width: u32,
    win_height: u32,
) -> Position {
    const PADDING_BOTTOM: i32 = 45;

    let x = mon_x + (mon_width as i32 / 2) - (win_width as i32 / 2);
    let y = mon_y + mon_height as i32 - win_height as i32 - PADDING_BOTTOM;

    Position { x, y }
}

/// Determines where the window should be placed based on saved state and available monitors.
pub fn resolve_window_position(
    _state: &WindowState,
    available_monitors: &[MonitorBounds],
    window_size: Size,
) -> Position {
    if available_monitors.is_empty() {
        return Position { x: 0, y: 0 };
    }

    let target_monitor = available_monitors
        .iter()
        .find(|m| m.scale_factor > 0.0)
        .unwrap_or(&available_monitors[0]);

    calculate_bottom_center(
        target_monitor.position.x,
        target_monitor.position.y,
        target_monitor.size.width,
        target_monitor.size.height,
        window_size.width,
        window_size.height,
    )
}
