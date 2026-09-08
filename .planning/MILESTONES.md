# Milestones

## v0.8.0 GPUI Frontend Port — code complete (2026-09-08)

**Goal:** Rebuild the Tauri/React frontend in GPUI Rust, pixel-identical, backend reused.
**Outcome:** All 5 phases code-complete, committed, and live-probed on GNOME Wayland.
6/30 requirements Complete (live proof); 24 In Progress pending human visual
sign-off + X11-session verification (see `.planning/phases/05-system/HUMAN-CHECKLIST.md`).

**Shipped:**
- `gpui-app/` (~9k lines Rust): popup (clipboard history + search + pin + paste),
  emoji/kaomoji/symbol pickers, settings window, 5-step setup wizard, tray,
  single-instance toggle IPC, X11 hotkeys, portal theme listener, GNOME shortcut
  registration, `make gpui-install/uninstall`, PACKAGING.md.
- Planning artifacts: PROJECT.md, REQUIREMENTS.md (30 REQ-IDs), ROADMAP.md (5 phases),
  research summary, per-phase CONTEXT/PLAN/SUMMARY, HUMAN-CHECKLIST.md.

**Complete (live proof):** SYS-03, SYS-05, SYS-06, PKG-02, PKG-03, WIND-05.

**Known gaps (next milestone):** human pixel sign-off (all UI reqs), X11-only paths
(hotkey grab, cursor-follow), non-GNOME DE registration, deb/rpm bundles,
release-LTO build, settings Tab-order, grid virtualization, Fuse ranking, ui_scale zoom.

**Coexistence:** React/Tauri build untouched and running throughout; separate binary,
app-id, config, autostart, shortcuts, socket — proven side by side.

---
*Teams*: single autonomous session (no subagents — billing), ~1 day, 30+ atomic commits.
