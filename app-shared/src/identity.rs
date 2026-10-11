//! Names that tell the frontends apart so they can be installed side by side
//! (config dir, IPC socket, autostart entry, desktop shortcuts, GNOME extension
//! window match). Set once at startup with [`init`]; an uninitialised process
//! behaves as the GPUI frontend.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppIdentity {
    /// Wayland app id / X11 class the compositor and the GNOME extension see.
    pub app_id: &'static str,
    /// What the X11 `WM_CLASS` of the popup contains (gpui sets the app id, winit the binary name).
    pub window_class: &'static str,
    /// Binary / desktop-entry / icon name, also the prefix of GNOME keybinding ids.
    pub binary_name: &'static str,
    /// `~/.config/<this>/`.
    pub config_dir_name: &'static str,
    /// `$XDG_RUNTIME_DIR/<this>` single-instance socket.
    pub socket_name: &'static str,
    /// Human name (autostart entry).
    pub display_name: &'static str,
    /// Names of the two desktop shortcuts (Super+V and Ctrl+Alt+V).
    pub shortcut_label: &'static str,
    pub shortcut_label_alt: &'static str,
    /// Title of the popup window; the GNOME extension finds the popup by it.
    pub popup_title: &'static str,
}

impl AppIdentity {
    pub const GPUI: AppIdentity = AppIdentity {
        app_id: "dev.gustavosett.clipboard-history-gpui",
        window_class: "dev.gustavosett.clipboard-history-gpui",
        binary_name: "win11-clipboard-history-gpui",
        config_dir_name: "win11-clipboard-history-gpui",
        socket_name: "win11-clipboard-gpui.sock",
        display_name: "Win11 Clipboard History (GPUI)",
        shortcut_label: "Clipboard History (GPUI)",
        shortcut_label_alt: "Clipboard History (GPUI Alt)",
        popup_title: "Clipboard History Popup",
    };

    pub const ICED: AppIdentity = AppIdentity {
        app_id: "dev.gustavosett.clipboard-history-iced",
        window_class: "win11-clipboard-history-iced",
        binary_name: "win11-clipboard-history-iced",
        config_dir_name: "win11-clipboard-history-iced",
        socket_name: "win11-clipboard-iced.sock",
        display_name: "Win11 Clipboard History (iced)",
        shortcut_label: "Clipboard History (iced)",
        shortcut_label_alt: "Clipboard History (iced Alt)",
        popup_title: "Clipboard History Popup",
    };
}

static IDENTITY: OnceLock<AppIdentity> = OnceLock::new();

/// Choose this process's identity. The first call wins; later calls are ignored.
pub fn init(identity: AppIdentity) {
    let _ = IDENTITY.set(identity);
}

pub fn get() -> &'static AppIdentity {
    IDENTITY.get_or_init(|| AppIdentity::GPUI)
}
