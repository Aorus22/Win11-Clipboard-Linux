//! D-Bus client for the GNOME Shell extension (see `extension/extension.js`).

use std::sync::Mutex;

use zbus::blocking::Connection;
use zbus::zvariant::{DynamicType, OwnedValue};

pub const BUS_NAME: &str = "dev.gustavosett.ClipboardHistoryGpui.Shell";
pub const OBJECT_PATH: &str = "/dev/gustavosett/ClipboardHistoryGpui/Shell";
pub const INTERFACE: &str = "dev.gustavosett.ClipboardHistoryGpui.Shell";

/// Extension API version this build speaks. Bump together with
/// `extension.js` (`Version`) whenever the D-Bus interface changes.
pub const API_VERSION: u32 = 1;

static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

fn connection() -> Result<Connection, String> {
    let mut guard = CONNECTION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(conn) = guard.as_ref() {
        return Ok(conn.clone());
    }
    let conn = Connection::session().map_err(|e| format!("session bus: {e}"))?;
    *guard = Some(conn.clone());
    Ok(conn)
}

fn call<B, R>(method: &str, body: &B) -> Result<R, String>
where
    B: serde::Serialize + DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let reply = connection()?
        .call_method(Some(BUS_NAME), OBJECT_PATH, Some(INTERFACE), method, body)
        .map_err(|e| format!("{method}: {e}"))?;
    reply
        .body()
        .deserialize::<R>()
        .map_err(|e| format!("{method} reply: {e}"))
}

/// Is an extension speaking our API version on the bus right now?
pub fn extension_ready() -> bool {
    let Ok(conn) = Connection::session() else {
        return false;
    };
    let owned: bool = conn
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &(BUS_NAME,),
        )
        .ok()
        .and_then(|m| m.body().deserialize::<bool>().ok())
        .unwrap_or(false);
    if !owned {
        return false;
    }
    let version = conn
        .call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(INTERFACE, "Version"),
        )
        .ok()
        .and_then(|m| m.body().deserialize::<OwnedValue>().ok())
        .and_then(|v| u32::try_from(v).ok());
    match version {
        Some(v) if v == API_VERSION => true,
        Some(v) => {
            eprintln!(
                "[app] GNOME extension API v{v}, app needs v{API_VERSION}; \
                 log out and back in after updating"
            );
            false
        }
        None => false,
    }
}

/// Place the popup's top-left at `(x, y)` (compositor logical px).
pub fn place(x: i32, y: i32) -> Result<(), String> {
    ok_or_missing(call::<_, bool>("Place", &(x, y))?)
}

/// Show (raise + focus) or hide (minimize, focus returns to the previous window).
pub fn set_visible(visible: bool) -> Result<(), String> {
    ok_or_missing(call::<_, bool>("SetVisible", &(visible,))?)
}

pub fn pointer() -> Result<(i32, i32), String> {
    call("GetPointer", &())
}

/// Work area `(x, y, w, h)` of the monitor containing the point.
pub fn monitor_at(x: i32, y: i32) -> Result<(i32, i32, i32, i32), String> {
    call("GetMonitorAt", &(x, y))
}

fn ok_or_missing(found: bool) -> Result<(), String> {
    if found {
        Ok(())
    } else {
        Err("popup window not known to the shell yet".to_string())
    }
}

/// Block, forwarding every `OutsideClick` signal to `on_click`.
pub fn watch_outside_clicks(on_click: impl Fn()) -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("session bus: {e}"))?;
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        // The object path, interface, and member identify this helper signal.
        // GDBus reports the emitting connection with a unique sender name.
        .path(OBJECT_PATH)
        .map_err(|e| e.to_string())?
        .interface(INTERFACE)
        .map_err(|e| e.to_string())?
        .member("OutsideClick")
        .map_err(|e| e.to_string())?
        .build();
    let messages = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, None)
        .map_err(|e| format!("match rule: {e}"))?;
    for message in messages {
        if message.is_ok() {
            on_click();
        }
    }
    Ok(())
}
