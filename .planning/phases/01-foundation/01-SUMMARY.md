# Phase 1 Summary: Foundation & coexistence ✓

**Status:** complete | **Date:** 2026-09-08 | **Requirements:** SYS-06, PKG-03, WIND-05 (decisions locked, full side-by-side proof in Phase 5)

## What was built

- `gpui-app/` crate (`win11-clipboard-history-gpui` v0.8.0, `gpui =0.2.2` pinned + Cargo.lock):
  transparent frameless `WindowKind::PopUp` 360×480, `titlebar: None`, app-id
  `dev.gustavosett.clipboard-history-gpui`.
- `gpui-app/BACKEND_REUSE.md`: all 18 `src-tauri/` modules audited —
  11 Tauri-free (direct reuse), 6 light adapters (config_manager geometry→`Bounds<Pixels>`,
  theme_manager AppHandle→channel+tray-icon, 4× `#[tauri::command]` direct calls),
  main.rs/IPC replaced by in-process channels.
- PROJECT.md Key Decisions locked (upstream gpui, tray-icon/global-hotkey plan,
  coexistence naming, pinning policy).

## Verification (all pass)

1. `cargo check` clean ✓ (first build 3m17s, deps from crates.io reachable)
2. `cargo build` clean ✓ (4m10s)
3. Runtime smoke: binary alive 12s on GNOME Wayland, no panic, empty stderr ✓ (exit=124 timeout-kill)
4. `src/` + `src-tauri/` untouched ✓ (`git diff` empty, no untracked files)
5. **Live coexistence:** installed Tauri build (`/usr/bin/win11-clipboard-history-bin --background`,
   PID 12095) kept running undisturbed while GPUI binary ran — first WIND-05 evidence ✓

## Commits

- eb26d0d feat(gpui): scaffold gpui-app with transparent popup window
- 695f02a docs(gpui): backend reuse audit per module
- 9f8e2fb docs: lock phase 1 stack + coexistence decisions

## Deferred to later phases

- Follow-mouse positioning (Phase 2, geometry port), backend wiring (Phase 2+),
  tray/hotkey/packaging final (Phase 5), formal side-by-side conflict test incl.
  shortcut ownership (Phase 5 → completes SYS-06/PKG-03/WIND-05).
