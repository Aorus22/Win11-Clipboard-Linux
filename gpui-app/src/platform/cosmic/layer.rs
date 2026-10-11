//! Layer-shell surface for the popup overlay. Nothing here is COSMIC-specific:
//! any compositor with `zwlr_layer_shell_v1` (KDE, Sway, …) could reuse it.

use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{WindowKind, px};

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
