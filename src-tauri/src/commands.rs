//! Tauri command wrappers for win11-clipboard-core modules

pub mod permission_checker {
    use win11_clipboard_core::permission_checker as core;
    pub use core::PermissionStatus;

    #[tauri::command]
    pub fn check_permissions() -> PermissionStatus {
        core::check_permissions()
    }

    #[tauri::command]
    pub fn fix_permissions_now() -> Result<String, String> {
        core::fix_permissions_now()
    }

    #[tauri::command]
    pub fn is_first_run() -> bool {
        core::is_first_run()
    }

    #[tauri::command]
    pub fn mark_first_run_complete() -> Result<(), String> {
        core::mark_first_run_complete()
    }

    #[tauri::command]
    pub fn reset_first_run() -> Result<(), String> {
        core::reset_first_run()
    }
}

pub mod shortcut_setup {
    use win11_clipboard_core::shortcut_conflict_detector::ConflictDetectionResult;
    use win11_clipboard_core::shortcut_setup as core;
    pub use core::ShortcutToolsStatus;

    #[tauri::command]
    pub fn get_desktop_environment() -> String {
        core::get_desktop_environment()
    }

    #[tauri::command]
    pub fn detect_conflicts() -> ConflictDetectionResult {
        core::detect_conflicts()
    }

    #[tauri::command]
    pub fn resolve_conflicts() -> Result<Vec<String>, String> {
        core::resolve_conflicts()
    }

    #[tauri::command]
    pub fn register_de_shortcut() -> Result<String, String> {
        core::register_de_shortcut()
    }

    #[tauri::command]
    pub fn check_shortcut_tools() -> ShortcutToolsStatus {
        core::check_shortcut_tools()
    }
}

pub mod autostart_manager {
    use win11_clipboard_core::autostart_manager as core;

    #[tauri::command]
    pub fn autostart_enable() -> Result<(), String> {
        core::autostart_enable()
    }

    #[tauri::command]
    pub fn autostart_disable() -> Result<(), String> {
        core::autostart_disable()
    }

    #[tauri::command]
    pub fn autostart_is_enabled() -> Result<bool, String> {
        core::autostart_is_enabled()
    }

    #[tauri::command]
    pub fn autostart_migrate() -> Result<bool, String> {
        core::autostart_migrate()
    }
}

pub mod rendering_env {
    use win11_clipboard_core::rendering_env as core;
    pub use core::{init, RenderingEnv};

    #[tauri::command]
    pub fn get_rendering_environment() -> RenderingEnv {
        core::get_rendering_environment()
    }
}
