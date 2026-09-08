# Plan: Phase 5 — System integration & packaging

**Phase:** 5 | **Requirements:** SYS-01–05, PKG-01–03 (+ close SYS-06/PKG-03/WIND-05)
**Mode:** inline (autonomous, no subagents)

## Goal

GPUI build runs as a complete background app (toggle, tray, autostart, theme-follow)
with packaging basics + coexistence proof + human visual checklist.

## Tasks (atomic commits)

### T1 — Tray + hotkey + single-instance + toggle + `--background`
- Deps: `tray-icon 0.24`, `muda` (menu), `global-hotkey 0.8`.
- `src/tray.rs`: icon build (reuse PNGs + picker logic), menu Show/Settings/Quit,
  event channel → main poll loop. Dynamic rebuild on theme change.
- `src/instance.rs`: Unix socket lock + `--toggle`/`--settings-open` IPC
  (newline words), second-instance exits fast. Socket path override via env for tests.
- `src/hotkey.rs`: X11-only `global-hotkey` grab (Super+V, Ctrl+Alt+V) → toggle
  signal; Wayland: log + skip.
- main.rs: `--background` (tray-only), `--toggle`/`--settings-open` client mode,
  poll loop acts on tray/menu/hotkey/IPC signals (show/hide with reposition,
  open settings, quit).
- Tests: socket IPC round-trip (temp path), tray icon bytes decode, toggle-word parse.
- Verify: `cargo test`, tray-only start alive, `--toggle` second-process exits ~instantly.
- Commit: `feat(gpui): phase5 tray, hotkey, single-instance toggle`

### T2 — Theme portal listener + GNOME shortcut registration
- `src/theme_watch.rs`: zbus SettingChanged listener (mirror backend rule) →
  shared theme bump + tray rebuild. gsettings probe stays fallback.
- `src/gnome_shortcut.rs`: register/unregister/list Super+V + Ctrl+Alt+V →
  current_exe `--toggle` via gsettings custom-keybindings (own entries only).
- Wizard shortcut step + settings Register button route to GNOME registrant on
  GNOME (`XDG_CURRENT_DESKTOP`), manual (gpui path) elsewhere.
- Verify: `cargo test`, live gsettings color-scheme flip → theme change logged;
  live GNOME registration creates entries pointing at gpui binary (then unregister).
- Commit: `feat(gpui): phase5 theme listener + GNOME shortcuts`

### T3 — Packaging + coexistence proof + human checklist
- `Makefile` targets `gpui-build/install/uninstall` (PREFIX-aware), `.desktop`
  template, PACKAGING.md (deps, layout, udev reuse, coexistence).
- Live side-by-side: Tauri bin + gpui bin both alive, distinct dirs/ids → evidence.
- `HUMAN-CHECKLIST.md`: dark+light side-by-side steps (popup/pickers/settings/wizard).
- Requirement statuses: SYS-05/PKG-02/SYS-06/PKG-03/WIND-05/SYS-03(+toggle/theme/tray
  as proven) → Complete where live-proven; rest In Progress with reasons.
- Verify: `cargo test`, install/uninstall smoke with PREFIX=/tmp.
- Commit: `feat(gpui): phase5 packaging + proof`

### T4 — Close-out + lifecycle
- `05-SUMMARY.md`; STATE + ROADMAP final; commit docs.
- Lifecycle: audit (inline checklist vs ROADMAP/success criteria) → MILESTONES.md
  (v0.8.0 entry) → STATE reset for next milestone → cleanup phase dirs? (GSD cleanup
  archives phases — keep .planning/phases + commit; note next milestone).
- Final commit + report to user with human checklist pointer.

## Verification (phase gate)

1. `cargo check` + `cargo test` clean.
2. Tray-only start alive; toggle IPC live; theme flip live; GNOME registration live.
3. `src/` + `src-tauri/` untouched; Tauri install undisturbed.
4. Every requirement has an honest status + evidence pointer.

## Risks

- tray-icon SNI on GNOME needs AppIndicator ext (may be absent → log + continue;
  toggle still works via `--toggle`/hotkey-X11).
- global-hotkey on Wayland: skip by design (document).
- Portal listener zbus API drift vs backend's version — mirror + compile.
