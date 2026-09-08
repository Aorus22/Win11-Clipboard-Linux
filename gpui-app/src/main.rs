//! Windows 11 Clipboard History — GPUI frontend entry point.
//!
//! Milestone v0.8.0: pixel-identical port of the Tauri/React frontend.
//! Phase 1: foundation — open the transparent frameless popup shell.
//! Backend wiring (clipboard, shortcut, tray, settings) lands in Phase 2+.

use gpui::{
    App, Application, Bounds, Context, SharedString, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions, div, prelude::*, px, rgb, size,
};

/// Separate app-id from the Tauri build so both can coexist (SYS-06).
const APP_ID: &str = "dev.gustavosett.clipboard-history-gpui";

/// Main popup dimensions mirror `tauri.conf.json` (`main` window).
const POPUP_WIDTH: f32 = 360.0;
const POPUP_HEIGHT: f32 = 480.0;

struct PopupShell {
    title: SharedString,
}

impl Render for PopupShell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .gap_2()
            // Temporary solid fill; acrylic/opacity tokens arrive in Phase 2 (WIND-02).
            .bg(rgb(0x202020))
            .text_color(rgb(0xffffff))
            .child(self.title.clone())
            .child(format!("v{} — GPUI port (Phase 1)", env!("CARGO_PKG_VERSION")))
    }
}

fn popup_options(bounds: Bounds<gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // Frameless: no titlebar, matching `decorations: false`.
        titlebar: None,
        // Transparent: compositor blur + opacity applied in Phase 2.
        window_background: WindowBackgroundAppearance::Transparent,
        show: true,
        // Popup semantics: floating utility window (always-on-top behavior in Phase 2).
        kind: WindowKind::PopUp,
        is_movable: false,
        focus: true,
        app_id: Some(APP_ID.into()),
        ..Default::default()
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(POPUP_WIDTH), px(POPUP_HEIGHT)), cx);
        cx.open_window(popup_options(bounds), |_, cx| {
            cx.new(|_| PopupShell {
                title: "Clipboard History".into(),
            })
        })
        .expect("failed to open GPUI popup window");
        cx.activate(true);
    });
}
