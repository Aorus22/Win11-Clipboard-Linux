//! Toolkit-independent parts of the Win11 Clipboard History frontends.
//!
//! Everything that does not draw lives here so the GPUI and iced frontends share
//! one implementation: settings, history/search logic, picker datasets, the
//! clipboard backend service, single-instance IPC, desktop shortcut
//! registration, and the platform backends (x11 / gnome / cosmic) that place and
//! dismiss the popup. A frontend sets its [`identity::AppIdentity`] first.

pub mod backend;
pub mod drag_log;
pub mod geometry;
pub mod history;
pub mod identity;
pub mod instance;
pub mod launcher;
pub mod pickers;
pub mod platform;
pub mod settings;
pub mod test_util;
