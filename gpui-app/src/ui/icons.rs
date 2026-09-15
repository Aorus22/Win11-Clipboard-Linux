//! Lucide-equivalent icons as SVG assets, tinted with the inherited text color.
//!
//! Files live in `gpui-app/assets/icons/` and are served through the app
//! `AssetSource` rooted at `gpui-app/assets/`.
//!
//! Why a `canvas` and not `svg()`: gpui's `Svg` element resolves its style from
//! `Style::default()`, whose text color is **black**, and it never inherits
//! `text_color` from its parent — so an icon drawn as `svg()` is painted black
//! (invisible on the dark popup) unless every single call site sets a color.
//! `Window::text_style()` *does* compose every ancestor's text-style refinement
//! (it is how `Text` inherits), and a `canvas` paints inside that scope, so an
//! icon simply takes the color of the container it sits in — the same contract
//! the React build has via `text-win11-text-*` on the root.

use gpui::{Canvas, Pixels, SharedString, canvas, prelude::*};

/// An icon, tinted with the text color inherited from its container.
pub fn icon(name: &str, size: Pixels) -> Canvas<()> {
    let path: SharedString = format!("icons/{name}.svg").into();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let color = window.text_style().color;
            // gpui's own `svg()` element swallows this error and paints nothing,
            // which reads as a silently missing icon — surface it instead.
            if let Err(error) = window.paint_svg(bounds, path.clone(), Default::default(), color, cx)
            {
                eprintln!("[icon] {}: {error}", path);
            }
        },
    )
    .w(size)
    .h(size)
}

pub const PIN: &str = "pin";
pub const PIN_FILLED: &str = "pin-filled";
pub const X: &str = "x";
pub const TYPE_TEXT: &str = "type";
pub const IMAGE: &str = "image";
pub const SEARCH: &str = "search";
pub const CHEVRON_DOWN: &str = "chevron-down";
pub const CHEVRON_LEFT: &str = "chevron-left";
pub const CHEVRON_RIGHT: &str = "chevron-right";
pub const HISTORY: &str = "history";
pub const CLIPBOARD_LIST: &str = "clipboard-list";
pub const SMILE: &str = "smile";
pub const OMEGA: &str = "omega";
pub const LAYOUT_LIST: &str = "layout-list";
pub const REGEX: &str = "regex";
pub const LINK: &str = "link";
pub const MAIL: &str = "mail";
pub const CLOCK: &str = "clock";
pub const MONITOR: &str = "monitor";
pub const MOON: &str = "moon";
pub const SUN: &str = "sun";
pub const KEYBOARD: &str = "keyboard";
pub const TRASH: &str = "trash";
pub const CHECK: &str = "check";
pub const RESET: &str = "reset";
pub const PLUS: &str = "plus";
pub const ROCKET: &str = "rocket";
pub const SHIELD: &str = "shield";
pub const CHECK_CIRCLE: &str = "check-circle";
pub const ALERT_TRIANGLE: &str = "alert-triangle";
pub const ALERT_CIRCLE: &str = "alert-circle";
pub const COPY: &str = "copy";
pub const ZAP: &str = "zap";
pub const SETTINGS_GEAR: &str = "settings-gear";

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `pub const` below names a file in `gpui-app/assets/icons/`.
    ///
    /// A name with no asset is the one icon failure gpui reports nowhere: the
    /// `canvas` in [`icon`] gets an error from `paint_svg` and the element stays
    /// empty, which looks exactly like a missing glyph.
    const NAMES: &[&str] = &[
        PIN,
        PIN_FILLED,
        X,
        TYPE_TEXT,
        IMAGE,
        SEARCH,
        CHEVRON_DOWN,
        CHEVRON_LEFT,
        CHEVRON_RIGHT,
        HISTORY,
        CLIPBOARD_LIST,
        SMILE,
        OMEGA,
        LAYOUT_LIST,
        REGEX,
        LINK,
        MAIL,
        CLOCK,
        MONITOR,
        MOON,
        SUN,
        KEYBOARD,
        TRASH,
        CHECK,
        RESET,
        PLUS,
        ROCKET,
        SHIELD,
        CHECK_CIRCLE,
        ALERT_TRIANGLE,
        ALERT_CIRCLE,
        COPY,
        ZAP,
        SETTINGS_GEAR,
    ];

    fn icons_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons")
    }

    #[test]
    fn every_icon_constant_has_an_asset() {
        for name in NAMES {
            let path = icons_dir().join(format!("{name}.svg"));
            assert!(path.is_file(), "{} has no icon asset", path.display());
        }
    }

    /// An icon that draws nothing is invisible, so each file must carry at least
    /// one shape and take its ink from `currentColor` — the color gpui paints it
    /// with (see [`icon`]).
    #[test]
    fn every_asset_draws_a_tinted_shape() {
        for name in NAMES {
            let path = icons_dir().join(format!("{name}.svg"));
            let svg = std::fs::read_to_string(&path).expect("read icon");
            assert!(
                svg.contains("currentColor"),
                "{}: no currentColor — the icon would ignore its text color",
                path.display()
            );
            assert!(
                ["<path", "<circle", "<rect", "<line", "<polyline", "<polygon"]
                    .iter()
                    .any(|shape| svg.contains(shape)),
                "{}: no shape — the icon would be blank",
                path.display()
            );
        }
    }
}
