---
milestone: v0.8.0
milestone_name: GPUI Frontend Port
status: planning
progress:
  phases_total: 5
  phases_complete: 1
  requirements_total: 30
  requirements_complete: 0
---

# State

## Current Position

Phase: 1 complete (foundation & coexistence) → next: Phase 2 clipboard popup parity
Plan: `.planning/phases/01-foundation/01-PLAN.md` executed (T1–T4 all pass)
Status: Phase 2 context gathering
Last activity: 2026-09-08 — Phase 1 complete (gpui-app boots on GNOME Wayland, 12s smoke pass)

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-08)

**Core value:** Super+V yang cepat dan cantik di Linux — popup ala Windows 11 yang instan dan familiar.
**Current focus:** Milestone v0.8.0 planning — GPUI frontend port, paritas visual 1:1.

## Accumulated Context

### Decisions

- v0.8.0 = GPUI-Rust frontend port, React/Tauri tetap hidup (koeksistensi, crate/direktori baru)
- Paritas penuh: clipboard + emoji/kaomoji/symbols + settings + setup wizard + tray/shortcut/autostart
- Backend Rust dipakai ulang; hanya frontend engine yang pindah
- GIF tab tetap disabled (Tenor API mati) — bukan bagian milestone ini
- Research + roadmap dikerjakan inline (subagents unavailable — billing)

### Open Questions / Blockers

- Kemampuan GPUI di Linux untuk: blur/transparansi akrilik, frameless always-on-top follow-mouse window, tray icon, global shortcut — perlu riset sebelum roadmap final (risiko terbesar milestone)
- DE + distro mesin dev user untuk verifikasi visual (ditanya saat plan-phase)
- Lokasi crate GPUI final (`gpui-app/` diusulkan, belum diputus)

### Todos

- [x] Riset ekosistem GPUI (inline, ringan)
- [x] REQUIREMENTS.md dengan REQ-IDs (30) — menunggu konfirmasi
- [x] ROADMAP.md draft (5 phases) — menunggu approval
- [ ] Commit REQUIREMENTS.md + ROADMAP.md + STATE.md setelah approval
