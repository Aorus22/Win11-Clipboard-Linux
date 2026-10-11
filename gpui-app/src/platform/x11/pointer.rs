//! Global cursor position on the X11 backend.

/// Global cursor position. Port of `WindowController::get_cursor_position`
/// (Tauri `window.cursor_position()` has no GPUI equivalent, so we go straight
/// to the two fallbacks: xdotool, then raw X11 query).
pub fn cursor_position() -> Option<(i32, i32)> {
    cursor_xdotool().or_else(cursor_x11)
}

fn cursor_xdotool() -> Option<(i32, i32)> {
    let output = std::process::Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let (mut x, mut y) = (None, None);
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("X=") {
            x = v.parse().ok();
        }
        if let Some(v) = line.strip_prefix("Y=") {
            y = v.parse().ok();
        }
    }
    x.zip(y)
}

fn cursor_x11() -> Option<(i32, i32)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let reply = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((reply.root_x as i32, reply.root_y as i32))
}
