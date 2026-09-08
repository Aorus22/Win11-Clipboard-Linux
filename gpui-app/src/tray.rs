//! System tray — `tray-icon` + `muda` menu, icons reused from `src-tauri/icons/`.
//!
//! Same icon policy as the Tauri build (`get_icon_bytes`): dynamic mode shows
//! `icon-light` on dark themes and `icon-dark` on light ones, else `icon.png`.
//! Left-click toggles the popup; the menu offers Show / Settings / Quit.
//! Events are drained by the main poll loop (no extra threads).

use muda::{Menu, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::instance::AppSignal;

fn icon_rgba(dynamic: bool, dark: bool) -> Icon {
    let bytes: &[u8] = if dynamic {
        if dark {
            include_bytes!("../../src-tauri/icons/icon-light.png")
        } else {
            include_bytes!("../../src-tauri/icons/icon-dark.png")
        }
    } else {
        include_bytes!("../../src-tauri/icons/icon.png")
    };
    let img = image::load_from_memory(bytes).expect("tray icon decodes");
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    Icon::from_rgba(rgba.into_raw(), w, h).expect("tray icon builds")
}

pub struct Tray {
    icon: TrayIcon,
    show: MenuItem,
    settings: MenuItem,
    quit: MenuItem,
}

impl Tray {
    pub fn build(dynamic: bool, dark: bool) -> Result<Self, String> {
        let menu = Menu::new();
        let show = MenuItem::new("Show Clipboard", true, None);
        let settings = MenuItem::new("Settings", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append_items(&[&show, &settings, &quit])
            .map_err(|e| format!("tray menu: {e}"))?;
        let icon = TrayIconBuilder::new()
            .with_menu_on_left_click(false)
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("Clipboard History (GPUI)")
            .with_title("Clipboard History (GPUI)")
            .with_icon(icon_rgba(dynamic, dark))
            .build()
            .map_err(|e| format!("tray build: {e}"))?;
        Ok(Self {
            icon,
            show,
            settings,
            quit,
        })
    }

    /// Swap the icon after a theme change (parity: dynamic tray icon).
    pub fn rebuild_icon(&mut self, dynamic: bool, dark: bool) {
        if self.icon.set_icon(Some(icon_rgba(dynamic, dark))).is_err() {
            eprintln!("[tray] failed to update icon");
        }
    }

    /// Drain menu selections into signals. Call from the main poll loop.
    pub fn poll_menu(&self) -> Vec<AppSignal> {
        let mut out = Vec::new();
        while let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            if event.id() == self.show.id() {
                out.push(AppSignal::Toggle);
            } else if event.id() == self.settings.id() {
                out.push(AppSignal::OpenSettings);
            } else if event.id() == self.quit.id() {
                out.push(AppSignal::Quit);
            }
        }
        out
    }

    /// Drain tray-icon clicks into signals. Left-click release toggles.
    pub fn poll_clicks(&self) -> Vec<AppSignal> {
        let mut out = Vec::new();
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                out.push(AppSignal::Toggle);
            }
        }
        out
    }
}
