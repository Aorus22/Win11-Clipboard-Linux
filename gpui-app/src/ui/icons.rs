//! Lucide-equivalent icons as SVG assets, tinted via `text_color`
//! (GPUI paints `svg()` with `style.text.color`).
//!
//! Files live in `gpui-app/assets/icons/` and are served through the app
//! `AssetSource` rooted at `gpui-app/assets/`.

use gpui::{Pixels, Svg, prelude::*, svg};

pub fn icon(name: &str, size: Pixels) -> Svg {
    svg().path(format!("icons/{name}.svg")).w(size).h(size)
}

pub const PIN: &str = "pin";
pub const PIN_FILLED: &str = "pin-filled";
pub const X: &str = "x";
pub const TYPE_TEXT: &str = "type";
pub const IMAGE: &str = "image";
pub const SEARCH: &str = "search";
pub const CHEVRON_DOWN: &str = "chevron-down";
pub const HISTORY: &str = "history";
pub const CLIPBOARD_LIST: &str = "clipboard-list";
pub const SMILE: &str = "smile";
pub const OMEGA: &str = "omega";
pub const LAYOUT_LIST: &str = "layout-list";
pub const REGEX: &str = "regex";
pub const LINK: &str = "link";
pub const MAIL: &str = "mail";
