# Phase 4: Settings & setup wizard - Context

**Gathered:** 2026-09-08
**Status:** Ready for planning
**Mode:** Auto (autonomous — full direction discretion granted)

<domain>
## Phase Boundary

Settings window + setup wizard berparitas: semua section settings berfungsi dan
berdampak live ke popup; wizard first-run 5 langkah bekerja ujung-ke-ujung.
Scope: SET-01–05. Bukan scope: hotkey/tray/packaging final (Phase 5).

</domain>

<decisions>
## Implementation Decisions

### Arsitektur: second window + shared state
- Settings = window GPUI kedua (480×520, decorated, centered) dibuka via `--settings`.
  Tray menu menyusul Phase 5.
- Shared `AppStateCore` (`Arc<Mutex<...>>` settings + version): settings window menulis
  + save file + bump; popup mem-poll version di loop 300ms dan me-reload (live preview
  parity — React memakai event `app-settings-changed`; GPUI tanpa event bus pusat).
- Wizard: first-run marker SENDIRI (`first_run` di config dir GPUI — jangan sentuh
  marker Tauri). First run → buka window wizard 550×650; complete → tutup + buka popup.
  `--setup` memaksa wizard. Popup `--settings` + first-run ordering: setup menang.

### Kontrol custom (belum ada di codebase)
- Slider: track + knob, click-to-set + drag (mouse down/move/up) + arrows saat fokus,
  label % live. Commit ke disk on release (parity: visual-only sampai mouseUp).
- NumberField: reuse pola SearchState (ketik, Enter/blur commit parse-or-revert).
- Switch: w-11 h-6 pill + knob translate, persis Switch.tsx.
- Theme cards (3), unit pills (4), kaomoji add/list/delete: replika langsung.

### Backend reuse (semua Tauri-free / attr-only)
- permission_checker::{check_permissions, fix_permissions_now} (own marker file!).
- shortcut_setup::{check_shortcut_tools, detect_conflicts, resolve_conflicts,
  register_de_shortcut}, linux_shortcut_manager (register path — cek signature).
- autostart_manager::{enable, is_enabled} untuk wizard step (own binary path!
  pastikan entry autostart menunjuk biner GPUI, bukan Tauri — verifikasi argumen).
- rendering_env detect untuk warning transparansi (cek signature).
- reset_first_run / mark_first_run_complete versi GPUI-sendiri (marker file sendiri).

### ui_scale: cek API zoom dulu, fallback window-size multiplier
Cari `scale_factor|zoom` di gpui window API. Bila ada one-liner → pakai. Bila tidak:
window popup dibuka 360s×480s + font root diskala proporsional via helper terpusat
(minimal, terdokumentasi). Bukan blocker Phase 4.

</decisions>

<code_context>
## Existing Code Insights

- Settings sections: header (Personalization + Saved pill), Appearance (3 theme cards
  + dynamic-tray Switch), Auto-delete (number + 4 unit pills + info box), Transparency
  (2 sliders 0–1 step .01 + NVIDIA/AppImage warning), UI Scale (slider .5–2),
  History (max number 1–100000), Custom Kaomoji (input + Add + grid + hover delete),
  Features (2 Switches), Shortcuts (list + Register button + status), Reset
  (defaults + reset_first_run + show wizard), footer Done (accent).
- Save: tiap perubahan → save file + (Tauri) emit event + max_history sync. GPUI:
  save file + version bump + `set_max_history_size` sync.
- Opacity sliders: visual-only sampai mouseUp (commit). ui_scale sama.
- Wizard 5 steps: welcome → permissions (check/fix/skip) → shortcut (DE detect,
  conflicts + auto-fix/manual + register + skip) → autostart (yes/no) → done.
  Progress dots clickable-back. Tertiary opacity fixed 0.85. System theme for chrome.
- Autostart toggle TIDAK ada di settings (hanya wizard) — parity: tidak dibuat.

</code>

<specifics>
## Specific Ideas

- Verifikasi: `cargo test` (settings save/load round-trip own dir, kaomoji add/remove
  logic, autostart entry menunjuk biner GPUI — cutoff: minimal argumen diperiksa) +
  smoke `--settings` dan `--setup` (no panic, render paths) + live edit satu setting
  via file? (pol loop reload — uji dengan ubah file lalu lihat versi bump di log?
  tambah eprintln debug sementara seperti Phase 2). Screenshot tetap blocked.

</specifics>

<deferred>
## Deferred Ideas

- Tray menu entries membuka settings (Phase 5).
- Global hotkey membuka popup (Phase 5) — penutup verifikasi show/hide penuh.
- Virtualisasi grid (perf follow-up bila perlu).

</deferred>
