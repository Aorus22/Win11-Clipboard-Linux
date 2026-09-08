# Windows 11 Clipboard History For Linux

## What This Is

A Windows 11-style clipboard history manager for Linux (Super+V popup, emoji/kaomoji/symbol pickers, settings + setup wizard). v0.7.1 ships as Rust + Tauri v2 + React + Tailwind. Milestone v0.8.0 builds a GPUI-Rust frontend in parallel with pixel-identical UI, reusing the existing Rust backend modules.

## Current Milestone: v0.8.0 GPUI Frontend Port

**Goal:** Pindahkan frontend engine dari React/Tauri-Webview ke GPUI Rust murni dengan tampilan 1:1 identik, backend Rust dipakai ulang semaksimal mungkin.

**Target features:**
- Clipboard history UI di GPUI (teks / rich-text / image, pin, search, paste, clear/delete)
- Picker di GPUI (emoji, kaomoji + custom, symbols) + tab bar + keyboard nav identik
- Settings window + Setup Wizard di GPUI
- Window behavior identik (frameless transparan, always-on-top, follow-mouse, fallback opaque)
- System integration via backend reuse (shortcut Super+V, tray, autostart, theme, uinput paste)
- Packaging reuse (deb/rpm/appimage + wrapper/udev/postinst yang ada)

## Core Value

Super+V yang cepat dan cantik di Linux — popup riwayat clipboard ala Windows 11 yang selalu terasa instan dan familiar.

## Requirements

### Validated

<!-- Shipped in v0.7.1 Tauri app, confirmed working. Locked unless explicitly discussed. -->

- ✓ Clipboard history teks/rich-text/image dengan pin & search — v0.7.1
- ✓ Paste via Enter/uinput simulation (X11 + Wayland) — v0.7.1
- ✓ Global shortcut Super+V / Ctrl+Alt+V + tray icon + autostart — v0.7.1
- ✓ Emoji / kaomoji (+ custom) / symbols picker — v0.7.1
- ✓ Settings (theme, opacity, ui_scale, max_history, auto-delete) + Setup Wizard — v0.7.1
- ✓ System/light/dark theme via XDG portal + NVIDIA/AppImage opaque fallback — v0.7.1

### Active

<!-- Milestone v0.8.0 scope. Hypotheses until shipped in GPUI. -->

- [ ] GPUI app menampilkan clipboard history dengan paritas visual 1:1 vs React
- [ ] GPUI picker (emoji/kaomoji/symbols) + tab bar + keyboard nav identik
- [ ] GPUI settings + setup wizard dengan paritas perilaku
- [ ] Window behavior identik (frameless, transparan, follow-mouse, always-on-top, fallback)
- [ ] Integrasi sistem reuse backend (shortcut, tray, autostart, theme, paste)
- [ ] Packaging Linux reuse untuk biner GPUI (deb/rpm/appimage)

### Out of Scope

- GIF tab/provider baru — Tenor API mati, implementasi lama tetap disabled, bukan bagian port ini
- Menghapus/mengganti aplikasi React/Tauri — tetap hidup berdampingan selama milestone ini
- Mobile app — Linux desktop only, seperti sebelumnya
- Rewrite backend clipboard/paste/shortcut — dipakai ulang, bukan ditulis ulang

## Context

- Tech stack berjalan: Rust + Tauri v2.11 + React 19 + Tailwind v4, backend 18 modul di `src-tauri/src/` (clipboard_manager via arboard wayland-data-control, input_simulator/paste_sync via x11rb+uinput, global-shortcut plugin, tray, theme_manager via zbus XDG portal)
- Acuan visual: `src/ClipboardApp.tsx` (360×480), `src/index.css` + `tailwind.config.js` (sistem opacity `--win11-*-bg-alpha`, glass-effect, scrollbar-win11, font Segoe UI Variable 14px)
- Window utama: frameless, transparent, skipTaskbar, alwaysOnTop, visible=false sampai dipanggil shortcut
- GSD subagents (researcher/roadmapper) tidak tersedia saat milestone ini didefinisikan (billing habis) — research & roadmap dikerjakan inline oleh orchestrator
- User dev machine + DE akan dikonfirmasi saat plan-phase (relevan untuk verifikasi blur/transparansi GPUI)

## Constraints

- **[Tech]**: Linux X11 + Wayland dua-duanya harus jalan — mengapa: user base mencakup keduanya, backend sudah mendukung
- **[Tech]**: Koeksistensi — kode GPUI di direktori/crate baru (mis. `gpui-app/`), tidak menghapus `src/` / `src-tauri/` — mengapa: versi React tetap rilis stabil selama port berjalan
- **[Tech]**: Backend reuse maksimal, hanya frontend engine yang pindah — mengapa: keputusan user eksplisit, menghindari regresi paste/shortcut/izin
- **[Compat]**: Packaging reuse (deb/rpm/appimage, wrapper.sh, udev rules, postinst/postrm) — mengapa: instalasi + permission flow sudah terbukti
- **[Quality]**: Pixel-identical 1:1 (layout, spacing, radius, blur akrilik, opacity, dark/light) — mengapa: definisi sukses milestone menurut user

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Port ke GPUI Rust, React dibiarkan hidup (koeksistensi) | Stabilitas rilis + eksperimen paralel tanpa risiko | ✓ Locked Phase 1 |
| Paritas penuh (clipboard + semua picker + settings + wizard) | Tanggung kalau setengah; definisi "sama persis" | — Pending |
| Backend Rust dipakai ulang, bukan rewrite | Paste/shortcut/izin uinput rawan regresi | ✓ Locked Phase 1 |
| Research + roadmap inline (tanpa subagents) | Billing subagent habis saat milestone dimulai | ✓ Done |
| UI framework: upstream `gpui =0.2.2` (tolak fork adabraka-gpui) | 249k downloads vs ~1k; extras fork diduplikasi backend sendiri; contoh window_positioning membuktikan API PopUp transparan | ✓ Locked Phase 1 |
| Tray: crate `tray-icon` (SNI); hotkey X11: `global-hotkey`, Wayland: reuse `linux_shortcut_manager` | Upstream GPUI tak punya tray/hotkey; pola app saat ini sudah terbukti di kedua display server | ✓ Locked Phase 1 |
| Koeksistensi: biner `win11-clipboard-history-gpui`, app-id `...clipboard-history-gpui`, config `~/.config/win11-clipboard-history-gpui/` | Berbagi config/shortcut = konflik; satu pemegang Super+V + single-instance lock sendiri | ✓ Locked Phase 1 |
| Pin `gpui =0.2.2` + commit Cargo.lock; isolasi API GPUI di `gpui-app/src/ui/` | Pre-1.0 churn; bump tak melebar | ✓ Locked Phase 1 |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-09-08 after milestone v0.8.0 start*
