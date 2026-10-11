//! Layer-shell surface for the popup overlay. Nothing here is COSMIC-specific:
//! any compositor with `zwlr_layer_shell_v1` (KDE, Sway, …) could reuse it.

use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{WindowKind, px};

/// gpui window kind of the clipboard popup for the active backend.
pub fn popup_window_kind() -> WindowKind {
    match app_shared::platform::backend().popup_surface() {
        app_shared::platform::PopupSurface::Overlay => popup_kind(),
        app_shared::platform::PopupSurface::Window => WindowKind::PopUp,
    }
}

/// Overlay covering one whole output. Exclusive keyboard focus so typing goes
/// to the search box the moment the popup appears, without a click first.
pub fn popup_kind() -> WindowKind {
    WindowKind::LayerShell(LayerShellOptions {
        namespace: "win11-clipboard-history".to_string(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        margin: Some((px(0.), px(0.), px(0.), px(0.))),
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        ..Default::default()
    })
}
