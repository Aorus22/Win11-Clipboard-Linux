# Plan: Phase 4 — Settings & setup wizard

**Phase:** 4 | **Requirements:** SET-01–05
**Mode:** inline (autonomous, no subagents)

## Goal

Settings window (second GPUI window, all sections, live-apply) + 5-step setup wizard
(first-run + `--setup`) — paritas perilaku vs React. Coexistence-safe: own marker,
own autostart entry, own config dir.

## Tasks (atomic commits)

### T1 — Shared state + own system-integration shims
- `src/app_state.rs`: `SharedConfig { settings, version }` (Arc<Mutex>), `save_now()`
  (write file + backend max_history sync + bump), first-run marker fns (own file),
  own autostart enable/disable/is_enabled (`win11-clipboard-history-gpui.desktop`,
  current_exe — backend's hardcoded Tauri paths must NOT be reused).
- `Popup`: hold `Arc<Mutex<SharedConfig>>`; poll loop reloads settings/is_dark on bump.
- Tests: save/load round-trip own dir, version bump, marker lifecycle (tmp), autostart
  entry content points at a fake exe path (unit-test the template fn, not the FS write…
  actually test enable/disable against temp HOME? keep to template unit test).
- Verify: `cargo test`.
- Commit: `feat(gpui): phase4 shared settings state + shims`

### T2 — Controls: Switch, Slider, NumberField, cards, pills
- `src/ui/controls.rs`: Switch (w-11 h-6 + knob), Slider (click+drag+arrows, % label,
  commit-on-release), NumberField (SearchState-based, Enter/blur commit-or-revert),
  theme cards (3 + radio dot), unit pills (4).
- Verify: `cargo check`.
- Commit: `feat(gpui): phase4 settings controls`

### T3 — Settings window + wizard views
- `src/ui/settings.rs`: all sections (header+Saved pill, Appearance, Auto-delete,
  Transparency + env warning, UI Scale, History, Custom Kaomoji, Features, Shortcuts,
  Reset, Done footer). Live-apply; opacity sliders visual-until-release.
- `src/ui/wizard.rs`: 5 steps + progress dots + all backend calls
  (permissions fix, conflicts detect/resolve, shortcut register/manual, autostart,
  mark complete).
- Verify: `cargo check` + `cargo test`.
- Commit: `feat(gpui): phase4 settings + wizard UI`

### T4 — main.rs wiring + verification
- Flags `--settings` / `--setup`; first-run → wizard window (550×650 centered) else
  popup (360s×480s, ui_scale multiplies size); settings window 480×520 decorated
  centered; Done/wizard-complete closes window (`remove_window`); settings saved →
  popup live-reloads (temp eprintln probe like Phase 2, then revert).
- Smoke: `--settings`, `--setup` (no panic); settings save probe (edit opacity via…
  headless can't click — probe via file write + poll reload log).
- `04-SUMMARY.md`; traceability SET → In Progress; STATE + ROADMAP; commit docs.

## Verification (phase gate)

1. `cargo check` + `cargo test` clean.
2. Settings + wizard windows render without panic (smoke).
3. Settings save → file updated → popup reloads (probe log).
4. `src/` + `src-tauri/` untouched; no Tauri autostart/marker paths touched
   (verify: gpui config dir only).

## Risks

- Second-window event loops / close semantics — verify at runtime (smoke).
- Slider drag event API friction → fallback click+arrows (document).
- ui_scale: window-size multiplier only (documented delta from Phase 2).
