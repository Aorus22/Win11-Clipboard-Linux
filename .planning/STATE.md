---
milestone: v0.8.0
milestone_name: GPUI Frontend Port
status: planning
progress:
  phases_total: 5
  phases_complete: 0
  requirements_total: 30
  requirements_complete: 0
---

# State

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-09-08 — Requirements (30) + roadmap draft (5 phases) proposed, awaiting approval

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
