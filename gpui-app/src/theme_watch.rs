//! XDG portal appearance listener — mirrors `theme_manager` logic without Tauri.
//!
//! Watches `org.freedesktop.portal.Settings.SettingChanged` for
//! `org.freedesktop.appearance color-scheme` (1 = prefer-dark, 2 = prefer-light)
//! and `accent-color` (GNOME 47+), then forwards `AppSignal::ThemeChanged`; the
//! main poll loop recomputes the theme, rebuilds the tray icon, and bumps shared
//! state (popup reloads). The accent is kept in `theme::set_portal_accent` as the
//! last-resort accent for the GTK palette.

use futures_lite::stream::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

use crate::instance::AppSignal;

/// 0.0-1.0 portal component → 0-255.
fn to_byte(component: f64) -> u8 {
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Spawn the listener thread. Non-fatal if D-Bus/portal is unavailable
/// (the gsettings startup probe remains the fallback).
pub fn spawn_theme_watcher(tx: UnboundedSender<AppSignal>) {
    std::thread::Builder::new()
        .name("gpui-theme-watch".to_string())
        .spawn(move || {
            if let Err(e) = run(tx.clone()) {
                eprintln!("[theme-watch] disabled: {e}");
            }
        })
        .expect("spawn theme watcher thread");
}

fn run(tx: UnboundedSender<AppSignal>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        use zbus::{Connection, MatchRule, MessageStream};

        let connection = Connection::session().await?;
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender("org.freedesktop.portal.Desktop")?
            .interface("org.freedesktop.portal.Settings")?
            .member("SettingChanged")?
            .build();
        let mut stream = MessageStream::for_match_rule(rule, &connection, None).await?;
        eprintln!("[theme-watch] listening for color-scheme signals");

        let mut last_dark: Option<bool> = None;
        let mut last_accent: Option<(u8, u8, u8)> = None;
        while let Some(msg) = stream.next().await {
            let Ok(msg) = msg else { continue };
            let Ok((namespace, key, value)) = msg
                .body()
                .deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
            else {
                continue;
            };
            if namespace != "org.freedesktop.appearance" {
                continue;
            }
            match key.as_str() {
                "color-scheme" => {
                    let Ok(scheme) = value.downcast_ref::<u32>() else {
                        continue;
                    };
                    // Portal: 1 = prefer-dark, 2 = prefer-light, else no preference.
                    let dark = scheme == 1;
                    if last_dark != Some(dark) {
                        last_dark = Some(dark);
                        eprintln!("[theme-watch] color-scheme changed (dark={dark})");
                        if tx.send(AppSignal::ThemeChanged).is_err() {
                            break;
                        }
                    }
                }
                // GNOME 47+ system accent, published as a (ddd) RGB tuple.
                "accent-color" => {
                    let Ok((r, g, b)) = value.downcast_ref::<(f64, f64, f64)>() else {
                        continue;
                    };
                    let rgb = (to_byte(r), to_byte(g), to_byte(b));
                    if last_accent != Some(rgb) {
                        last_accent = Some(rgb);
                        crate::theme::set_portal_accent(Some(crate::theme::rgb8(
                            rgb.0, rgb.1, rgb.2,
                        )));
                        eprintln!(
                            "[theme-watch] accent-color changed (#{:02x}{:02x}{:02x})",
                            rgb.0, rgb.1, rgb.2
                        );
                        if tx.send(AppSignal::ThemeChanged).is_err() {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_value_mapping_matches_backend() {
        // freedesktop color-scheme: 1 = prefer-dark, 2 = prefer-light.
        let dark = |v: u32| v == 1;
        assert!(dark(1));
        assert!(!dark(2));
        assert!(!dark(0));
    }

    #[test]
    fn portal_accent_tuple_to_rgb8() {
        assert_eq!(to_byte(0.0), 0);
        assert_eq!(to_byte(1.0), 255);
        assert_eq!(to_byte(1.5), 255);
        assert_eq!(to_byte(-1.0), 0);
        // The tuple GNOME publishes for its red accent.
        assert_eq!(
            (to_byte(0.90196079015731812), to_byte(0.17647059261798859), to_byte(0.25882354378700256)),
            (0xe6, 0x2d, 0x42)
        );
        assert_eq!(
            crate::theme::rgb8(0xe6, 0x2d, 0x42),
            gpui::rgb(0xe62d42)
        );
    }
}
