# Phase 4 Summary: Settings & setup wizard (code complete, visual sign-off deferred)

**Status:** code complete | **Date:** 2026-09-08
**Requirements:** SET-01–05 → implemented + committed, status **In Progress**
(visual sign-off joins the Phase 5 human checklist).

## What was built

- Settings window (`src/ui/settings.rs`, second GPUI window 480×520 decorated):
  header + Saved pill, Appearance (3 theme cards + dynamic-tray Switch), Auto-delete
  (number + 4 unit pills + info box), Transparency (2 drag sliders + env warning),
  UI Scale (slider), History (max number), Custom Kaomoji (add/list/hover-delete),
  Features (2 Switches), Shortcuts (list + register + status), Reset, Done footer.
- Controls (`src/ui/controls.rs`): Switch, Slider (click+drag+arrows, resize-safe
  mapping, commit-on-release), TextField (shared `edit_text` semantics).
- Wizard (`src/ui/wizard.rs`): 5 steps (welcome → permissions → shortcut →
  autostart → done) + progress dots + all backend calls, own first-run marker.
- Shared state (`src/app_state.rs`): version-counter live-apply same-process +
  **mtime-watch cross-process reload (proven live)**; own autostart entry
  (never the Tauri-pathed one); own first-run marker.
- main.rs: `--settings` / `--setup` / `--version`, first-run → wizard,
  deferred window-open flags, ui_scale window-size multiplier.
- Commits: 166ab54 (state+shims), 5c2c6c4 (windows+controls), 638f813 (mtime reload).

## Verification (all pass)

1. `cargo check` clean (after ~15 API/borrow fix rounds); `cargo test` 29/29 green
   (incl. settings save round-trip, marker lifecycle, slider mapping, autostart template).
2. Smoke `--settings` / `--setup` / `--version`: alive, 0 panics.
3. **Cross-process live reload proven:** popup run + external settings save →
   `settings reloaded from disk (was None, now Some(...))`.
4. **First-run routing confirmed:** no-flag runs open the wizard (own marker absent).
5. `src/` + `src-tauri/` untouched; Tauri autostart/marker paths never touched.

## Debugging notes (for the record)

- Silent UI poll mystery → root causes found: (a) first-run marker absent meant the
  wizard opened instead of the popup (correct behavior!); (b) one probe sequenced
  `timeout; create-file` so the app was dead before the file appeared. Lesson: always
  verify WHICH window opens + sequence background runs with `&` + pidfile, and kill
  strays by PID with SIGKILL (`timeout -s KILL` for foreground runs) — SIGTERM is
  ignored and wedges the shell waiting for the child.

## Deferred / deltas (honest list)

1. **Visual sign-off** joins the Phase 5 human checklist.
2. Settings keyboard: text fields + slider arrows + Esc work; full Tab-order nav missing.
3. Register/fix calls are synchronous (no mid-call spinner; statuses render after).
4. Wizard register advances immediately (no 1.5s beat); "Copied!" sticks.
5. ui_scale = window-size multiplier only (no content zoom API in GPUI).
6. Settings window resizable (React fixed 480×520); slider mapping is resize-safe.
7. Autostart entry launches visibly until Phase 5 hidden-start lands.
8. Kaomoji list has no inner max-h scroll (page scrolls instead).
9. Chevron static, focus ring = accent border (carried from Phase 2).
