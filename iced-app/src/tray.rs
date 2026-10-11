//! StatusNotifierItem tray icon (ksni): left click toggles the popup, the menu
//! offers Show / Settings / Quit. Icons are the ones the other builds ship.

use std::sync::{Arc, Mutex};

use ksni::{blocking::TrayMethods, menu::*, Icon, MenuItem, ToolTip, Tray};
use tokio::sync::mpsc::UnboundedSender;

use crate::Command;
use app_shared::instance::AppSignal;

const ICON: &[u8] = include_bytes!("../../src-tauri/icons/icon.png");
const ICON_LIGHT: &[u8] = include_bytes!("../../src-tauri/icons/icon-light.png");
const ICON_DARK: &[u8] = include_bytes!("../../src-tauri/icons/icon-dark.png");

#[derive(Clone, Copy, Default)]
pub struct TrayState {
    /// Settings: swap between the light and dark glyph with the theme.
    pub dynamic_icon: bool,
    pub dark: bool,
}

struct AppTray {
    state: Arc<Mutex<TrayState>>,
    commands: UnboundedSender<Command>,
}

impl AppTray {
    fn send(&self, signal: AppSignal) {
        let _ = self.commands.send(Command::Signal(signal));
    }
}

impl Tray for AppTray {
    fn id(&self) -> String {
        app_shared::identity::get().binary_name.to_string()
    }

    fn title(&self) -> String {
        app_shared::identity::get().shortcut_label.to_string()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let state = *self.state.lock().unwrap();
        let bytes = if state.dynamic_icon {
            if state.dark { ICON_LIGHT } else { ICON_DARK }
        } else {
            ICON
        };
        argb_icon(bytes).into_iter().collect()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: app_shared::identity::get().shortcut_label.to_string(),
            description: "Click to open the clipboard history".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(AppSignal::Toggle);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show Clipboard".into(),
                activate: Box::new(|this: &mut Self| this.send(AppSignal::Toggle)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings".into(),
                icon_name: "preferences-system".into(),
                activate: Box::new(|this: &mut Self| this.send(AppSignal::OpenSettings)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| this.send(AppSignal::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// StatusNotifier pixmaps are ARGB32, network byte order.
fn argb_icon(png: &[u8]) -> Option<Icon> {
    let img = image::load_from_memory(png).ok()?.resize(48, 48, image::imageops::FilterType::Lanczos3);
    let rgba = img.to_rgba8();
    let (width, height) = (rgba.width() as i32, rgba.height() as i32);
    let mut data = Vec::with_capacity(rgba.len());
    for px in rgba.pixels() {
        data.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
    }
    Some(Icon { width, height, data })
}

pub struct TrayControl {
    state: Arc<Mutex<TrayState>>,
    handle: Arc<Mutex<Option<ksni::blocking::Handle<AppTray>>>>,
}

impl TrayControl {
    /// Registers the tray in the background; skipped (with a warning) when no
    /// StatusNotifier host is available.
    pub fn start(commands: UnboundedSender<Command>, state: TrayState) -> Self {
        let state = Arc::new(Mutex::new(state));
        let handle = Arc::new(Mutex::new(None));
        let tray = AppTray { state: state.clone(), commands };
        let slot = handle.clone();
        std::thread::spawn(move || match tray.spawn() {
            Ok(tray_handle) => *slot.lock().unwrap() = Some(tray_handle),
            Err(error) => log::warn!("Tray icon unavailable: {error}"),
        });
        Self { state, handle }
    }

    pub fn set_state(&self, state: TrayState) {
        *self.state.lock().unwrap() = state;
        let handle = self.handle.clone();
        std::thread::spawn(move || {
            if let Some(handle) = handle.lock().unwrap().as_ref() {
                let _ = handle.update(|_| {});
            }
        });
    }
}
