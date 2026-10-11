//! Desktop-level shortcut registration (Super+V → `<exe> --toggle`).
//!
//! This is keyed on the *desktop* (`XDG_CURRENT_DESKTOP`), not on the window
//! backend: the `x11` backend running under GNOME via XWayland still has to
//! register its shortcut with GNOME, so these modules belong to no backend.

pub mod cosmic;
pub mod gnome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Cosmic,
    Gnome,
    Other,
}

/// The desktop environment this session runs, from `XDG_CURRENT_DESKTOP`.
pub fn current_desktop() -> Desktop {
    if cosmic::is_cosmic() {
        Desktop::Cosmic
    } else if gnome::is_gnome() {
        Desktop::Gnome
    } else {
        Desktop::Other
    }
}

/// Register the shortcuts with the current desktop. `None` when the desktop has
/// no automatic registration (callers show manual instructions).
pub fn register() -> Option<Result<String, String>> {
    match current_desktop() {
        Desktop::Cosmic => Some(cosmic::register()),
        Desktop::Gnome => Some(gnome::register()),
        Desktop::Other => None,
    }
}

/// Headless `--register-shortcuts`: COSMIC gets its own writer, every other
/// desktop goes through the GNOME one (the long-standing packaging behaviour).
pub fn register_headless() -> Result<String, String> {
    match current_desktop() {
        Desktop::Cosmic => cosmic::register(),
        _ => gnome::register(),
    }
}

/// Headless `--unregister-shortcuts`, mirroring [`register_headless`].
pub fn unregister_headless() -> Result<String, String> {
    match current_desktop() {
        Desktop::Cosmic => cosmic::unregister(),
        _ => gnome::unregister(),
    }
}
