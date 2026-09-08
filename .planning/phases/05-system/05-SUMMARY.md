# Phase 5 Summary: System integration & packaging (code complete)

**Status:** code complete | **Date:** 2026-09-08
**Requirements:** SYS-01–05, PKG-01–03 (+ SYS-06/PKG-03/WIND-05 closed here)

## What was built

- Tray (`src/tray.rs`): `tray-icon` + `muda` menu (Show/Settings/Quit), reused
  PNGs with the Tauri dynamic-icon policy, left-click toggle, theme rebuild.
- Toggle (`src/instance.rs` + main poll loop): Unix-socket single instance,
  `--toggle`/`--settings-open`/`--quit` clients (~25ms), close+reopen popup
  (fresh position + state every show), resident holder window (survives
  last-window close — diagnosed clean exit=0, fixed, proven over 3 cycles).
- Hotkeys (`src/hotkey.rs`): X11-only `global-hotkey` grab (Super+V, Ctrl+Alt+V);
  Wayland skips by design (DE shortcut → `--toggle` is the path).
- Theme (`src/theme_watch.rs`): zbus portal listener mirroring backend logic;
  live flips observed (`dark=false`/`true`), tray rebuild + shared bump.
- Shortcuts (`src/gnome_shortcut.rs` + `--register/--unregister-shortcuts`):
  own gpui-pathed GNOME entries; full lifecycle proven live with Tauri entries
  untouched. Settings/wizard Register buttons route here on GNOME.
- Packaging: `make gpui-{build,install,uninstall}` (PREFIX/DESTDIR),
  desktop entry + Settings action, PACKAGING.md, install smoke green in /tmp.
- Commits: 1abdd70 (tray/hotkey/IPC), 4c66e27 (gtk/holder/flags),
  58d7114 (test hygiene), 476bc7c (packaging/proof).

## Verification (all pass)

1. `cargo check` clean; `cargo test` 36/36 green ×3 (incl. IPC round-trip,
   autostart live cycle, GNOME parse/alloc units, slider mapping).
2. Toggle: 3 cycles, clients exit 0 in ~25ms, primary resident, 0 panics.
3. Theme: live gsettings flips observed, app survived, scheme restored.
4. GNOME register/unregister: entries correct, Tauri entries intact, system clean.
5. Side-by-side: Tauri + GPUI alive, distinct dirs, toggles undisturbed, 0 panics.
6. Install smoke (/tmp prefix): tree + Exec + icon + identical udev rule.
7. `src/` + `src-tauri/` untouched; Tauri install/config/autostart/shortcuts intact.

## Requirement outcomes (honest bar: Complete = live proof)

- Complete (6): SYS-03 (autostart live cycle), SYS-05 (Phase 2 probe),
  SYS-06/PKG-03/WIND-05 (side-by-side), PKG-02 (generic rule + rw check).
- In Progress (code complete, partial proof): SYS-01 (Wayland IPC proven; X11
  grab untestable on this machine), SYS-02 (tray builds/registers; icon/menu
  need eyes), SYS-04 (flips logged + handled; pixels need eyes),
  PKG-01 (install flow smoked; no deb/rpm bundles), all CORE/WIND/PICK/SET
  (structure proven; pixels per HUMAN-CHECKLIST.md).

## Deferred / deltas (carried forward)

1. Human visual sign-off (HUMAN-CHECKLIST.md) — next milestone's entry gate.
2. Non-GNOME DE auto-registration, deb/rpm/AppImage bundles, release-LTO build.
3. Settings Tab-order keyboard, grid virtualization, Fuse-ranking, ui_scale zoom.
4. X11-only paths unverified here (grab, cursor-follow) — need an X11 session.
5. AppIndicator extension presence decides tray visibility on GNOME.
