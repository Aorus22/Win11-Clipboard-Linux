# Backend Reuse Audit — gpui-app vs `src-tauri/` modules

**Audited:** 2026-09-08 (Phase 1, T2) via `grep tauri` coupling analysis + targeted reads.
**Policy:** `src-tauri/` is NOT modified. Reuse = path-dependency on `win11-clipboard-history-lib`
for Tauri-free modules; thin adapters inside `gpui-app/` where Tauri types leak.

## Tauri-free — reuse directly as library dependency

| Module | Tauri refs | Reuse verdict |
|--------|-----------|---------------|
| clipboard_manager | 0 | Direct. Arboard monitor + history types feed GPUI views (Phase 2). Event fan-out (`AppHandle.emit` in main.rs, not here) replaced by channel — verify when wiring. |
| emoji_manager | 0 | Direct. Emoji data + usage ranking for picker (Phase 3). |
| focus_manager | 0 | Direct. X11 focus save/restore around paste (Phase 2/5). |
| gif_manager | 0 | Direct, but GIF stays disabled (Out of Scope). |
| input_simulator | 0 | Direct. uinput/X11 paste injection (Phase 2). |
| linux_shortcut_manager | 0 | Direct. DE-level shortcut registration = Wayland hotkey strategy (Phase 5). |
| paste_sync | 0 | Direct. Paste sequencing with focus restore (Phase 2). |
| session | 0 | Direct. Wayland/X11 detection (all phases). |
| shortcut_conflict_detector | 0 | Direct. Detect Super+V grabs by other apps incl. our Tauri build (Phase 1/5 coexistence). |
| user_settings | 0 | Direct, with OWN paths. Same struct, separate file dir `~/.config/win11-clipboard-history-gpui/` (SYS-06). |
| lib.rs | 0 | Re-exports only — reusable as-is. |

## Light coupling — thin adapter in `gpui-app/`, no `src-tauri/` changes

| Module | Tauri refs | Coupling | Adapter plan |
|--------|-----------|----------|--------------|
| config_manager | 1 | `use tauri::{Monitor, PhysicalPosition, PhysicalSize}` for follow-mouse geometry math (`calculate_window_position`, `is_position_valid`, …) | Port the pure math to GPUI `Bounds<Pixels>` + `cx.displays()` in `gpui-app/src/geometry.rs` (Phase 2). Logic identical, types swapped. |
| rendering_env | 1 | `#[tauri::command]` attr on getter | Call function directly; NVIDIA/AppImage detect is pure Rust. |
| autostart_manager | 6 | `#[tauri::command]` attrs + comment about old plugin | Call functions directly; expose via GPUI actions (Phase 5). |
| shortcut_setup | 5 | `#[tauri::command]` attrs | Same — direct calls (Phase 5). |
| permission_checker | 5 | `#[tauri::command]` attrs | Same — direct calls (Phase 4 wizard). |
| theme_manager | 8 | `AppHandle` for (a) Tauri tray icon updates, (b) theme-change event emit. Detection itself is pure zbus. | Split: reuse `get_system_color_scheme` + portal listener core; tray updates move to `tray-icon` crate; events become internal broadcast channel in `gpui-app/src/events.rs` (Phase 5). |

## Not reused

- `main.rs` (31 refs): Tauri app bootstrap (tray setup, plugins, invoke handlers, window mgmt).
  Replaced by `gpui-app/src/main.rs` + per-phase wiring.
- Tauri IPC layer (`invoke`/`listen`, capabilities/): replaced by in-process channels
  (GPUI `Model`/`Entity` updates + `std::sync::mpsc`/async tasks).

## Wiring order (later phases)

- Phase 2: session, user_settings (own paths), clipboard_manager, input_simulator, paste_sync,
  focus_manager, geometry port (config_manager math).
- Phase 3: emoji_manager (+ TS service data port: emoji/kaomoji/symbols datasets).
- Phase 4: permission_checker, user_settings writers.
- Phase 5: linux_shortcut_manager, shortcut_setup, shortcut_conflict_detector,
  autostart_manager, theme_manager core + `tray-icon` + `global-hotkey` (X11).
