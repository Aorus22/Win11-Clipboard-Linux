# Roadmap: v0.8.0 GPUI Frontend Port

**Milestone:** v0.8.0 GPUI Frontend Port
**Status:** Approved [auto-approved — full autonomy granted] | inline (roadmapper subagent unavailable)
**Phases:** 5 | **Requirements mapped:** 30/30 ✓

| # | Phase | Goal | Requirements | Success Criteria | Status |
|---|-------|------|--------------|------------------|--------|
| 1 | Foundation & coexistence | GPUI app boots sebagai biner terpisah reuse backend; keputusan stack + koeksistensi dikunci | SYS-06, PKG-03, WIND-05 | 4 | ✓ complete |
| 2 | Clipboard popup parity | Popup utama identik 1:1, history end-to-end | CORE-01–06, WIND-01–04 | 4 | ○ pending |
| 3 | Pickers parity | Semua tab picker + data identik | PICK-01–05 | 3 | ○ pending |
| 4 | Settings & setup wizard | Settings + wizard paritas perilaku | SET-01–05 | 2 | ○ pending |
| 5 | System integration & packaging | Terinstal, terintegrasi, terverifikasi berdampingan | SYS-01–05, PKG-01–02 | 3 | ○ pending |

## Phase Details

### Phase 1: Foundation & coexistence

Goal: GPUI app boots sebagai biner terpisah yang memakai ulang backend; keputusan stack (upstream vs fork) dan kebijakan koeksistensi dikunci sebelum UI dibangun.
Requirements: SYS-06, PKG-03, WIND-05
Success criteria:
1. `cargo run -p gpui-app` membuka window kosong 360×480 frameless di X11 dan Wayland
2. Keputusan GPUI upstream vs `adabraka-gpui` tercatat di PROJECT.md beserta rationale
3. GPUI + Tauri dijalankan berdampingan tanpa konflik shortcut, clipboard, atau config
4. App-id, nama biner, dan path config GPUI terpisah dan terdokumentasi dari build Tauri

### Phase 2: Clipboard popup parity

Goal: Popup utama tak bisa dibedakan dari versi React; riwayat clipboard bekerja end-to-end.
Requirements: CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, CORE-06, WIND-01, WIND-02, WIND-03, WIND-04
Success criteria:
1. Screenshot berdampingan (dark + light) cocok 1:1 dengan React (list, pin, search, spacing, radius, opacity)
2. Search/filter, pin/unpin, delete/clear berperilaku identik
3. Enter paste via backend reuse di X11 dan Wayland; Esc hide tanpa quit
4. Window follow-mouse lintas monitor, always-on-top; fallback opaque NVIDIA/AppImage

### Phase 3: Pickers parity

Goal: Semua tab picker + data (termasuk port service TS ke Rust) identik.
Requirements: PICK-01, PICK-02, PICK-03, PICK-04, PICK-05
Success criteria:
1. Tab bar identik; browse + search + paste emoji/kaomoji/symbols cocok dengan React
2. Custom kaomoji dari settings muncul di picker GPUI
3. Operasi keyboard penuh (arrows/Enter/Esc) terverifikasi per tab

### Phase 4: Settings & setup wizard

Goal: Settings window + setup wizard paritas perilaku dengan versi Tauri.
Requirements: SET-01, SET-02, SET-03, SET-04, SET-05
Success criteria:
1. Semua kontrol settings ada dan berdampak live ke popup utama (theme, opacity, scale, limit, toggle, custom kaomoji)
2. Alur setup wizard fresh-run selesai (permissions, shortcut, autostart)

### Phase 5: System integration & packaging

Goal: Dapat diinstal, terintegrasi sistem, terverifikasi berdampingan dengan build Tauri.
Requirements: SYS-01, SYS-02, SYS-03, SYS-04, SYS-05, PKG-01, PKG-02
Success criteria:
1. Super+V / Ctrl+Alt+V toggle window GPUI di X11+Wayland; tray icon + menu + autostart bekerja
2. Theme ikut sistem; monitoring menangkap teks/rich-text/image
3. deb/rpm/appimage terinstal bersih dengan flow uinput tak berubah; instalasi GPUI tak mengganggu instalasi Tauri

## Coverage

- v0.8.0 requirements: 30 total (koreksi dari 29 — SYS ada 6 item)
- Mapped to phases: 30
- Unmapped: 0 ✓

---
*Roadmap drafted: 2026-09-08 (inline, menunggu approval sebelum commit)*
