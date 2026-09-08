# Phase 1: Foundation & coexistence - Context

**Gathered:** 2026-09-08
**Status:** Ready for planning
**Mode:** Auto (autonomous — user granted full direction discretion; grey areas auto-decided below)

<domain>
## Phase Boundary

GPUI app boots sebagai biner terpisah yang memakai ulang backend Rust yang ada. Stack (upstream vs fork)
dan kebijakan koeksistensi dikunci SEBELUM UI dibangun. Scope: SYS-06, PKG-03, WIND-05.
Bukan scope: UI clipboard, picker, settings (Phase 2–4), shortcut/tray/packaging final (Phase 5).

</domain>

<decisions>
## Implementation Decisions

### Stack: upstream `gpui 0.2.2` (bukan fork adabraka-gpui)
Bukti: crates.io `gpui` 0.2.2 (Okt 2025) = framework Zed yang dipublish standalone, ~249k downloads,
features `wayland`+`x11` built-in, contoh `window_positioning.rs` membuktikan API yang kita butuhkan
(`WindowKind::PopUp`, `WindowBackgroundAppearance::Transparent`, `titlebar: None`, `display_id`,
`cx.displays()`). Fork `adabraka-gpui` 0.5.1 (Feb 2026, ~1k downloads, single-maintainer) ditolak:
extras-nya (tray/hotkey/autostart) menduplikasi backend kita yang sudah terbukti — fork-risk tanpa gain.

### Layanan sistem: backend reuse + 2 crate kecil
- Clipboard monitor, paste/inject (x11rb+uinput), DE shortcut registration, theme XDG listener,
  settings persistence, autostart, permission check, session/rendering-env detect → modul `src-tauri/`
  yang ada (audit: 11 modul Tauri-free, 6 modul kopling ringan — lihat PLAN).
- Tray icon → crate `tray-icon` (SNI/ksni di Linux, setara perilaku AppIndicator saat ini).
- Global hotkey X11 → crate `global-hotkey`; Wayland → reuse `linux_shortcut_manager`
  (registrasi level DE, pola yang dipakai app saat ini). Tidak ada grab global di Wayland (protocol limit).

### Koeksistensi (SYS-06 / PKG-03 / WIND-05)
- Crate baru `gpui-app/`, biner `win11-clipboard-history-gpui` — `src/` dan `src-tauri/` tak disentuh.
- App-id GPUI: `dev.gustavosett.clipboard-history-gpui` (Tauri: `dev.gustavosett.clipboard-history`).
- Config dir terpisah: `~/.config/win11-clipboard-history-gpui/` (berbagi config = berebut shortcut/history).
- Shortcut default SAMA (Super+V) tapi hanya satu biner yang boleh pegang: GPUI build memakai
  single-instance lock sendiri + conflict-detector reuse untuk menolak start ganda yang konflik.
- Window Phase 1: PopUp 360×480 transparan frameless (replika `tauri.conf.json` main window).

### Pinning & isolasi churn
Versi GPUI di-pin (`=0.2.2`) + Cargo.lock di-commit. Kode yang menyentuh API GPUI langsung diisolasi
di `gpui-app/src/ui/` agar bump tidak melebar.

</decisions>

<code_context>
## Existing Code Insights

- `tauri.conf.json` main window: 360×480, min 300×300, decorations=false, transparent=true,
  visible=false, skipTaskbar, alwaysOnTop, focus=true. Settings 480×520, setup 550×650.
- Audit kopling Tauri per modul (grep `tauri`, 2026-09-08):
  - Tauri-free (reuse langsung sebagai lib): clipboard_manager, emoji_manager, focus_manager,
    gif_manager, input_simulator, linux_shortcut_manager, paste_sync, session,
    shortcut_conflict_detector, user_settings (+ lib.rs re-export).
  - Kopling ringan: config_manager (1: `tauri::{Monitor, PhysicalPosition, PhysicalSize}`),
    rendering_env (1: `#[tauri::command]`), autostart_manager (6: command attrs + komen),
    shortcut_setup (5: command attrs), permission_checker (5: command attrs),
    theme_manager (8: `AppHandle` untuk tray-icon update + emit).
  - Pola reuse: `#[tauri::command]` = atribut saja (fungsi tetap callable biasa);
    `AppHandle` di theme_manager hanya untuk tray update + event emit → diganti channel internal.
- Mesin dev: CachyOS (Arch), GNOME Wayland, GPU + Vulkan loader ada, system libs
  (wayland/xkbcommon/xcb) terinstal, crates.io reachable.

</code_context>

<specifics>
## Specific Ideas

- Scaffold minimal dulu: window transparan PopUp + label versi → buktikan compile + run di
  GNOME Wayland mesin dev sebelum wiring backend (Phase 2).
- Verifikasi run: jalankan biner dengan timeout; killed-by-timeout = hidup tanpa panic (sukses);
  exit non-zero langsung = gagal. Screenshot/pixel check baru di Phase 2.

</specifics>

<deferred>
## Deferred Ideas

- Follow-mouse positioning presisi (Phase 2, butuh mouse global position strategy).
- Data emoji/kaomoji/symbol TS→Rust port (Phase 3).
- Registrasi shortcut DE + tray final + packaging (Phase 5).

</deferred>
