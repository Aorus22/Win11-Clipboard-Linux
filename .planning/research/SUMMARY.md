# Research Summary: v0.8.0 GPUI Frontend Port

**Researched:** 2026-09-08 — inline (subagents unavailable, orchestrator web research)
**Sources:** zed-industries/zed `crates/gpui` README + docs.rs, gpui.rs, adabraka-gpui docs.rs (v0.4, Feb 2026), Ropy (GPUI clipboard manager precedent), Zed Linux blog + issue #5040 / #7015.

## Stack: temuan utama

- **Upstream GPUI** (`zed-industries/zed/crates/gpui`, pre-1.0, breaking changes sering): Linux didukung via `gpui_platform` features `wayland` + `x11`, render Vulkan (Blade). Transparansi window didukung di level framework (issue #5040 closed); **blur akrilik di Linux datang dari compositor, bukan framework** — sama seperti versi Tauri yang mengandalkan CSS/compositor (bukti: proyek Tauri lain pakai "CSS fallback" untuk blur di Linux).
- **Upstream TIDAK punya**: system tray, global hotkey, daemon mode, auto-launch. Ini gap kritis untuk aplikasi background seperti clipboard manager.
- **`adabraka-gpui` (fork, v0.4, Apache-2.0)**: menambah tray (DBus/SNI Linux), global hotkey (**X11 via XGrabKey; Wayland TIDAK BISA — keterbatasan protokol**), overlay always-on-top (Wayland parsial, tanpa layer-shell), auto-launch XDG autostart, single-instance, daemon mode. Kandidat kuat, tapi perlu evaluasi: maintenance fork, drift API vs upstream, kualitas kode.
- **Preseden**: Ropy — clipboard manager Rust+GPUI (teks/rich-text/image, tray, global hotkey, pin/fav, search) — membuktikan pola ini bisa. Catatan: README-nya klaim Linux (X11); Wayland jadi area verifikasi.
- **Strategi shortcut Wayland**: tidak ada framework yang bisa grab key global di Wayland — aplikasi Tauri saat ini mengatasinya via registrasi shortcut level DE (`linux_shortcut_manager`) + plugin global-shortcut. Pola yang sama harus dipakai ulang di port GPUI.

## Keputusan stack yang harus dibuat di plan-phase 1

1. **Upstream GPUI vs adabraka-gpui fork** — evaluasi: apakah tray/hotkey/overlay fork cukup matang, atau upstream + crate kecil sendiri (`ksni`/`tray-icon`, `global-hotkey`, autostart manual). Kriteria: X11+Wayland dua-duanya, maintenance aktif, lisensi cocok (MIT vs Apache-2.0 OK).
2. **Backend reuse**: `clipboard_manager` (arboard), `input_simulator`/`paste_sync` (x11rb+uinput), `theme_manager` (zbus), `user_settings`, `autostart_manager`, `linux_shortcut_manager`, `permission_checker` dipakai ulang apa adanya; IPC Tauri (`invoke`/`listen`) diganti message-passing internal GPUI (entity/model + channel).
3. **Nutrisi build**: Rust stable terbaru (persyaratan GPUI), Vulkan/Mesa di mesin dev; risiko NVIDIA (laporan driver issue di Zed Linux — perlu uji di hardware aktual, fallback opaque sudah ada sebagai jaring pengaman).

## Arsitektur (acuan untuk roadmap)

- Crate baru berdampingan (usulan `gpui-app/`), dependensi ke modul backend yang ada sebagai library. Tidak menyentuh `src/` / `src-tauri/`.
- 3 window: main popup (360×480 frameless transparan), settings (480×520), setup wizard (550×650) — replika 1:1 dari `tauri.conf.json` + `ClipboardApp.tsx` + `SettingsApp.tsx` + `SetupWizard.tsx`.
- Token visual bersumber dari `tailwind.config.js` + `index.css` (opacity vars, glass-effect, scrollbar-win11, font Segoe UI Variable 14px) — diterjemahkan ke style GPUI (`Hsla`, `Corners`, dsb.), diverifikasi berdampingan screenshot-per-screenshot.
- Konflik koeksistensi harus diputus eksplisit: app-id terpisah vs single-instance handoff, direktori config terpisah vs berbagi, siapa yang pegang Super+V bila keduanya jalan.

## Watch out (pitfalls)

- **Blur ≠ transparansi**: GPUI bisa jendela transparan; blur tergantung compositor (GNOME/KDE/Wlroots beda perilaku). Kriteria "pixel-identical" harus diuji per-DE, bukan diasumsikan.
- **Wayland**: tanpa layer-shell = positioning follow-mouse pakai cara standar (bisa, seperti sekarang); global hotkey wajib lewat registrasi DE, bukan grab.
- **Pre-1.0 churn**: pin versi GPUI di Cargo.lock, isolasi kode yang menyentuh API GPUI langsung supaya bump tidak meledak ke mana-mana.
- **Dua aplikasi berebut clipboard/shortcut**: tanpa kebijakan single-instance/koeksistensi, user yang menjalankan keduanya dapat perilaku aneh — jadikan requirement eksplisit (SYS-06).
- **Emoji/kaomoji/symbols data**: service TS (`emojiService`, `kaomojiService`, `symbolService`, `emojilib`) perlu diport ke Rust (data + search), bukan sekadar UI.
