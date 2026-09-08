# Phase 2: Clipboard popup parity - Context

**Gathered:** 2026-09-08
**Status:** Ready for planning
**Mode:** Auto (autonomous — full direction discretion granted)

<domain>
## Phase Boundary

Popup utama GPUI tak bisa dibedakan dari React: TabBar (4 tab, clipboard aktif),
Header (title + count + compact toggle + Clear All), SearchBar (Ctrl+F / type-to-filter /
regex toggle / Esc), daftar history (pinned section collapsible + recent, hover actions,
smart actions, compact mode), keyboard nav penuh, paste flow identik, positioning
follow-mouse (X11) / saved-or-bottom-center (Wayland). Scope: CORE-01–06, WIND-01–04.
Bukan scope: picker isi (Phase 3), settings/wizard (Phase 4), hotkey/tray/packaging (Phase 5).

</domain>

<decisions>
## Implementation Decisions

### Backend wiring: path-dep on `win11-clipboard-history-lib`, fallback documented
Pertama coba `gpui-app` path-dependency ke lib (reuse ClipboardManager, paste_sync,
input_simulator, focus_manager, session, user_settings-bentuk, config math). Keputusan
final tergantung system deps build (webkit2gtk/appindicator untuk closure tauri):
cek `pacman -Q webkit2gtk-4.1 libayatana-appindicator`. Jika gagal → fallback:
`HistoryStore` lokal di gpui-app (algoritma sama, monitor arboard sendiri) + wiring lib
penuh di Phase 5. Tidak ada perubahan di `src-tauri/` apa pun yang terjadi.

### Paste flow = replika main.rs
hide window → restore focus (`focus_manager`) → `manager.paste_item()` (mark→write→Ctrl+V→move-to-top)
→ refresh list. Di GPUI hide = sembunyikan window via platform (detail API saat implementasi);
show kembali via hotkey di Phase 5 (Phase 2: window mulai visible untuk pengujian).

### Positioning parity per session
- X11: cursor-follow + clamp 10px (port `get_cursor_xdotool`/`get_cursor_x11` + `clamp_window_to_monitor`).
- Wayland: TIDAK follow-mouse (app Tauri juga tidak — `position_for_wayland` pakai saved/bottom-center).
  Port `resolve_window_position` math ke `Bounds<Pixels>` + `cx.displays()`.
- Wayland sizing: GPUI `WindowBounds::Windowed(bounds)` di-set sebelum open (sama seperti Phase 1).

### Rendering choices (GPUI 0.2.2, API-terbukti)
- List: kolom scroll biasa (bukan uniform_list) — React juga render semua item; tinggi card variabel
  (line-clamp-3 ≈ max-h + overflow-hidden). Virtualisasi = follow-up performa bila perlu.
- Search field: single-line editor custom mengikuti pola `examples/input.rs`
  (FocusHandle + actions! + KeyBinding, dipangkas: ketik/hapus/panah saja).
- Ikon (lucide) → SVG inline minimal via `svg()` (contoh `examples/svg/` ada): pin, x, type,
  image, search, chevron-down, history, clipboard-list, smile, omega, layout-list.
- Hover actions: `group_hover` (ada di `elements/div.rs`).
- Gambar clipboard: usaha via `ImageSource::Render/Image` memori; fallback jujur = label
  `Image (WxH)` gaya compact bila API melawan (dicatat di SUMMARY, bukan silently beda).
- Smart actions (color/link/email): di-port dari `smartActionService` (kecil) — bagian parity card.
- Timestamp relatif: logika sama (Just now / Nm ago / Nh ago / tanggal).
- ui_scale: faktor pengali konstanta layout root (fallback deterministik bila tak ada API zoom window).
- Compact mode + pinned-expanded: persisted di file JSON gpui config dir (ganti localStorage).
- Theme: `theme_mode` dari settings file; `system` → baca `gsettings color-scheme`
  (portal listener penuh di Phase 5). Opacity math = port verbatim `themeUtils.ts`.

### Token visual (sumber: tailwind.config.js + index.css)
Dark: bg #202020/#2d2d2d/#383838, hover card #3d3d3d, accent #0078d4, text
#fff/#c5c5c5/#9e9e9e, border #454545/#3a3a3a, radius 8/12px, font Segoe UI Variable 14px.
Light: bg #f3f3f3/#fff/#e5e5e5, hover #f5f5f5, text #1a1a1a/#5c5c5c, border #e5e5e5.
Card bg = rgba(45,45,45,secondaryOpacity) / rgba(255,255,255,secondaryOpacity);
tertiary = rgba(56,56,56|229,229,229,tertiaryOpacity); solid bila opacity ≥ 1.

</decisions>

<code_context>
## Existing Code Insights

- `ClipboardTab.tsx` (463): search (substring+regex, images excluded), pinned section collapsible
  (persist), flat-list keyboard nav + collapse/expand via arrows, Ctrl+F toggle, type-to-filter,
  reset-on-window-shown, focus-first-on-shown, loading spinner, EmptyState.
- `HistoryItem` (246): card p-3 (p-2 compact), rounded 8px, icon box + text clamp-3 (clamp-1 compact)
  + timestamp, hover-reveal buttons (smart actions + pin + delete), pinned ring + badge dot.
- `TabBar` (117): 4 tab (clipboard/symbols/emoji/kaomoji), arrow/Home/End nav, active=tertiary bg.
- `Header` (110): title + count pill (tertiary bg) + compact toggle (LayoutList) + Clear All.
- Backend: `paste_item` = mark→write(os clipboard)→simulate Ctrl+V→move-to-top; preview trunc 100 chars;
  history max default 50; `get_history` ordering pinned-first (verifikasi saat wiring).
- `user_settings.json` di `~/.config/win11-clipboard-history/` — GPUI pakai dir sendiri (SYS-06).

</code>

<specifics>
## Specific Ideas

- Verifikasi: `cargo check/test/build` + unit tests (geometry clamp, filter incl. regex-invalid,
  timestamp, opacity math, settings defaults) + smoke run di GNOME Wayland + screenshot via
  `gnome-screenshot` bila memungkinkan, bandingkan struktur vs token (vision best-effort).
- Non-tab clipboard (emoji/kaomoji/symbols) di Phase 2 = placeholder label sederhana.

</specifics>

<deferred>
## Deferred Ideas

- Isi picker (Phase 3), settings/wizard UI (Phase 4), global hotkey + tray + packaging (Phase 5).
- Virtualisasi list untuk history raksasa (perf follow-up, React pun tak virtualisasi tab ini).
- Portal theme listener + dynamic tray (Phase 5).

</deferred>
