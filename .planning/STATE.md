---
milestone: v0.8.1
milestone_name: Visual Sign-off & Polish (planned)
status: planning
progress:
  phases_total: 0
  phases_complete: 0
  requirements_total: 24
  requirements_complete: 0
---

# State

## Current Position

Phase: Milestone v0.8.0 complete — 5/5 phases code-complete, 6/30 requirements Complete
Plan: Next — human visual sign-off (HUMAN-CHECKLIST.md) → polish gaps → bundles
Status: Between milestones (v0.8.0 archived in MILESTONES.md)
Last activity: 2026-09-08 — Milestone v0.8.0 completed (lifecycle: audit → complete)

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-08)

**Core value:** Super+V yang cepat dan cantik di Linux — popup ala Windows 11 yang instan dan familiar.
**Current focus:** v0.8.1 planning — visual sign-off + polish gaps from v0.8.0 audit.

## Accumulated Context

### Decisions (carried forward)

- v0.8.0 = GPUI-Rust frontend port, React/Tauri tetap hidup (koeksistensi) — DONE, proven live
- Backend Rust dipakai ulang, zero `src/`/`src-tauri/` changes — DONE (verified every phase)
- GIF tab tetap disabled (Tenor API mati)
- Research + roadmap inline (subagents unavailable — billing)
- Upstream `gpui =0.2.2` pinned + Cargo.lock committed
- Next: HUMAN-CHECKLIST.md is the entry gate for v0.8.1 scope

### Open Questions / Blockers

- Human pixel sign-off (dark + light) — awaiting user with a screen (HUMAN-CHECKLIST.md)
- X11 session needed to verify: hotkey grab, cursor-follow, NVIDIA blur behavior
- Non-GNOME DE auto-registration + deb/rpm/AppImage bundles — scoped for v0.8.1+
- Release-LTO build verification (`cargo check --profile=release` noted, full build open)

### Todos

- [ ] User runs HUMAN-CHECKLIST.md (dark + light) and reports deltas
- [ ] Scope v0.8.1 from checklist findings (`/gsd-new-milestone`)
- [ ] X11-session verification pass
- [ ] Bundle + publish decision
