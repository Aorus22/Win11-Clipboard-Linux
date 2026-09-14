//! Config Manager Adapter for Tauri
//! Provides window positioning resolution using Tauri's Monitor and PhysicalPosition types,
//! while delegating storage and core calculation to win11-clipboard-core.

pub use win11_clipboard_core::config_manager::{ConfigManager, WindowState};
use tauri::{Monitor, PhysicalPosition, PhysicalSize};

/// Determines where the window should be placed based on saved state and available monitors.
pub fn resolve_window_position(
    state: &WindowState,
    available_monitors: &[Monitor],
    window_size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let bounds: Vec<win11_clipboard_core::config_manager::MonitorBounds> = available_monitors
        .iter()
        .map(|m| win11_clipboard_core::config_manager::MonitorBounds {
            name: m.name().map(|s| s.to_string()),
            position: win11_clipboard_core::config_manager::Position {
                x: m.position().x,
                y: m.position().y,
            },
            size: win11_clipboard_core::config_manager::Size {
                width: m.size().width,
                height: m.size().height,
            },
            scale_factor: m.scale_factor(),
        })
        .collect();

    let size = win11_clipboard_core::config_manager::Size {
        width: window_size.width,
        height: window_size.height,
    };

    let pos = win11_clipboard_core::config_manager::resolve_window_position(state, &bounds, size);
    PhysicalPosition::new(pos.x, pos.y)
}
