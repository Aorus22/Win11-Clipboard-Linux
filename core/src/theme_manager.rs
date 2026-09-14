//! Theme Manager Module
//! Detects system color scheme preference via XDG Desktop Portal.
//! This is essential for DEs like COSMIC that use the portal standard
//! instead of GNOME settings.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};
use tokio::sync::RwLock;

/// Cached system theme preference
static SYSTEM_THEME: OnceLock<RwLock<Option<ColorScheme>>> = OnceLock::new();

/// Flag to track if the event listener is running
static EVENT_LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Color scheme values from the XDG Desktop Portal
/// See: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorScheme {
    /// No preference (value 0)
    NoPreference,
    /// Prefer dark appearance (value 1)
    Dark,
    /// Prefer light appearance (value 2)
    Light,
}

impl ColorScheme {
    /// Convert portal value to ColorScheme
    pub fn from_portal_value(value: u32) -> Self {
        match value {
            1 => ColorScheme::Dark,
            2 => ColorScheme::Light,
            _ => ColorScheme::NoPreference,
        }
    }

    /// Whether this scheme represents dark mode
    pub fn is_dark(&self) -> bool {
        matches!(self, ColorScheme::Dark)
    }
}

/// Response from the theme detection
#[derive(Debug, Clone, serde::Serialize)]
pub struct ThemeInfo {
    /// The detected color scheme
    pub color_scheme: ColorScheme,
    /// Whether dark mode is preferred
    pub prefers_dark: bool,
    /// Source of the detection (for debugging)
    pub source: String,
}

/// Query the XDG Desktop Portal for the system color scheme.
/// This works with COSMIC, GNOME, KDE, and other portal-compliant DEs.
pub async fn get_system_color_scheme() -> ThemeInfo {
    // Try to get cached value first
    let cache = SYSTEM_THEME.get_or_init(|| RwLock::new(None));

    // Check cache
    if let Some(scheme) = *cache.read().await {
        return ThemeInfo {
            color_scheme: scheme,
            prefers_dark: scheme.is_dark(),
            source: "cache".to_string(),
        };
    }

    // Query the portal
    match query_portal_color_scheme().await {
        Ok(scheme) => {
            // Cache the result
            *cache.write().await = Some(scheme);
            ThemeInfo {
                color_scheme: scheme,
                prefers_dark: scheme.is_dark(),
                source: "xdg-portal".to_string(),
            }
        }
        Err(e) => {
            eprintln!(
                "[ThemeManager] Portal query failed: {}, trying fallbacks",
                e
            );
            // Try COSMIC config file fallback
            match read_cosmic_theme_file() {
                Ok(is_dark) => {
                    let scheme = if is_dark {
                        ColorScheme::Dark
                    } else {
                        ColorScheme::Light
                    };
                    ThemeInfo {
                        color_scheme: scheme,
                        prefers_dark: is_dark,
                        source: "cosmic-config".to_string(),
                    }
                }
                Err(_) => {
                    // Default to no preference (let frontend handle it)
                    ThemeInfo {
                        color_scheme: ColorScheme::NoPreference,
                        prefers_dark: false,
                        source: "default".to_string(),
                    }
                }
            }
        }
    }
}

/// Query the XDG Desktop Portal via D-Bus
async fn query_portal_color_scheme() -> Result<ColorScheme, Box<dyn std::error::Error + Send + Sync>> {
    use zbus::zvariant::Value;
    use zbus::Connection;

    // Connect to the session bus
    let connection = Connection::session().await?;

    // Call the Settings.Read method
    // Interface: org.freedesktop.portal.Settings
    // Method: Read(namespace: string, key: string) -> variant
    let reply: zbus::zvariant::OwnedValue = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "Read",
            &("org.freedesktop.appearance", "color-scheme"),
        )
        .await?
        .body()
        .deserialize()?;

    // The return value is a variant containing the actual value
    // For color-scheme, it's a uint32 wrapped in a variant (sometimes double-wrapped)
    // Try to extract the u32 value, handling potential variant wrapping
    let value: u32 = match reply.downcast_ref::<u32>() {
        Ok(v) => v,
        Err(_) => {
            // The value might be wrapped in another variant
            if let Value::Value(inner) = &*reply {
                inner.downcast_ref::<u32>()?
            } else {
                return Err("Failed to parse color-scheme value".into());
            }
        }
    };

    Ok(ColorScheme::from_portal_value(value))
}

/// Fallback: Read COSMIC's theme config file directly
/// Path: ~/.config/cosmic/com.system76.CosmicTheme.Mode/v1/is_dark
fn read_cosmic_theme_file() -> Result<bool, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_path = home.join(".config/cosmic/com.system76.CosmicTheme.Mode/v1/is_dark");

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let trimmed = content.trim();
        let is_dark = match trimmed {
            "true" => true,
            "false" => false,
            _ => {
                return Err(format!(
                    "Invalid COSMIC theme value '{}' (expected 'true' or 'false')",
                    trimmed
                )
                .into());
            }
        };
        eprintln!(
            "[ThemeManager] Read COSMIC config file: is_dark={}",
            is_dark
        );
        return Ok(is_dark);
    }

    Err("COSMIC config file not found".into())
}

/// Clear the cached theme value (useful when system theme changes)
pub async fn clear_theme_cache() {
    if let Some(cache) = SYSTEM_THEME.get() {
        *cache.write().await = None;
    }
}

/// Check if the event listener is running
pub fn is_event_listener_running() -> bool {
    EVENT_LISTENER_RUNNING.load(Ordering::SeqCst)
}

/// Start listening for theme changes via D-Bus signals
pub async fn start_theme_listener<F>(
    on_change: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn(ColorScheme) + Send + Sync + 'static,
{
    if EVENT_LISTENER_RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    tokio::spawn(async move {
        eprintln!("[ThemeManager] Starting D-Bus event listener for theme changes");

        match listen_for_theme_changes(on_change).await {
            Ok(_) => {
                eprintln!("[ThemeManager] Theme listener ended gracefully");
                EVENT_LISTENER_RUNNING.store(false, Ordering::SeqCst);
            }
            Err(e) => {
                eprintln!("[ThemeManager] Theme listener error: {}", e);
                EVENT_LISTENER_RUNNING.store(false, Ordering::SeqCst);
            }
        }
    });

    Ok(())
}

/// Listen for SettingChanged signals from the XDG Desktop Portal
async fn listen_for_theme_changes<F>(
    on_change: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn(ColorScheme) + Send + Sync + 'static,
{
    use futures_lite::stream::StreamExt;
    use zbus::{Connection, MatchRule, MessageStream};

    let connection = Connection::session().await?;

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.portal.Desktop")?
        .interface("org.freedesktop.portal.Settings")?
        .member("SettingChanged")?
        .build();

    let mut stream = MessageStream::for_match_rule(rule, &connection, None).await?;

    eprintln!("[ThemeManager] Listening for theme change signals...");

    while let Some(msg) = stream.next().await {
        if let Ok(msg) = msg {
            let body = msg.body();
            if let Ok((namespace, key, value)) =
                body.deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
            {
                if namespace == "org.freedesktop.appearance" && key == "color-scheme" {
                    if let Ok(color_value) = value.downcast_ref::<u32>() {
                        let scheme = ColorScheme::from_portal_value(color_value);
                        let cache = SYSTEM_THEME.get_or_init(|| RwLock::new(None));
                        let mut cache_guard = cache.write().await;
                        let previous_scheme = *cache_guard;

                        let new_cache_value = if scheme == ColorScheme::NoPreference {
                            None
                        } else {
                            Some(scheme)
                        };

                        if previous_scheme != new_cache_value {
                            *cache_guard = new_cache_value;
                            drop(cache_guard);
                            on_change(scheme);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
