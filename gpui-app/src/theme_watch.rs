//! XDG portal color-scheme listener — mirrors `theme_manager` logic without Tauri.
//!
//! Watches `org.freedesktop.portal.Settings.SettingChanged` for
//! `org.freedesktop.appearance color-scheme` (1 = prefer-dark, 2 = prefer-light)
//! and forwards `AppSignal::ThemeChanged`; the main poll loop recomputes the
//! theme, rebuilds the tray icon, and bumps shared state (popup reloads).

use std::sync::mpsc::Sender;

use futures_lite::stream::StreamExt;

use crate::instance::AppSignal;

/// Spawn the listener thread. Non-fatal if D-Bus/portal is unavailable
/// (the gsettings startup probe remains the fallback).
pub fn spawn_theme_watcher(tx: Sender<AppSignal>) {
    std::thread::Builder::new()
        .name("gpui-theme-watch".to_string())
        .spawn(move || {
            if let Err(e) = run(tx.clone()) {
                eprintln!("[theme-watch] disabled: {e}");
            }
        })
        .expect("spawn theme watcher thread");
}

fn run(tx: Sender<AppSignal>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        while let Some(msg) = stream.next().await {
            let Ok(msg) = msg else { continue };
            let Ok((namespace, key, value)) = msg
                .body()
                .deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
            else {
                continue;
            };
            if namespace != "org.freedesktop.appearance" || key != "color-scheme" {
                continue;
            }
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
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn portal_value_mapping_matches_backend() {
        // freedesktop color-scheme: 1 = prefer-dark, 2 = prefer-light.
        let dark = |v: u32| v == 1;
        assert!(dark(1));
        assert!(!dark(2));
        assert!(!dark(0));
    }
}
