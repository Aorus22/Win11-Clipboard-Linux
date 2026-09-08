# Plan: Phase 3 — Pickers parity

**Phase:** 3 | **Requirements:** PICK-01–05
**Mode:** inline (autonomous, no subagents)

## Goal

Tab emoji / kaomoji / symbols berfungsi penuh: data + search + kategori + recents +
grid + keyboard + paste — paritas perilaku dan tampilan vs React.

## Tasks (atomic commits)

### T1 — Data layer: `src/pickers.rs` + backend additions
- Emoji: load `assets/emojis.json` (include_str), port `loadEmojis` + `detectCategory`
  (8 kategori + Other), `categories()` sorted; ranked-contains search limit 100.
- Kaomoji: include_str `src/data/kaomojis.json` + custom dari settings; port `getKaomojis`
  (custom dulu, filter kategori, search text/keywords/category).
- Symbols: include_str `src/data/symbols.json`; port `getSymbols`; recents file
  `recent_symbols.json` max 24 LRU (port hook).
- `BackendService`: `EmojiManager` (own data dir) + `recent_emojis()` +
  `paste_text(text, record_emoji: bool)` (mark + set_text_robust + Ctrl+V simulate).
- Tests: emoji count 1914 + spot 😀/❤️, kategori spot-checks, kaomoji 972, symbols 2484,
  search ranking sanity, symbol-recents round-trip, emoji usage record round-trip (tmp dir).
- Verify: `cargo test`.
- Commit: `feat(gpui): phase3 picker data layer`

### T2 — Views + wiring
- `search.rs`: generalisasi `render_for(placeholder, which)` (clipboard render delegates).
- `src/ui/pickers.rs`: PickerLayout chrome, category pills + strip (All/[Custom]/cats,
  scroll buttons), 40px cells (emoji 24px glyph / symbol 20px), kaomoji h-12 buttons,
  recent strips (8/10 col, max 16), footers, empty states.
- `popup.rs`: state per-picker (search ×3 + focus handles, focused idx main/recent/category,
  selected category, hovered preview), `render_body` ganti placeholder, keyboard routing
  (grid nav port; Ctrl+Left/Right switch tabs; printable → picker filter).
- `main.rs`: `GPUI_SMOKE_TAB` env (emoji|kaomoji|symbols) untuk verifikasi render path.
- Verify: `cargo check` + `cargo test`.
- Commit: `feat(gpui): phase3 picker UI + wiring`

### T3 — Live verification
- Smoke tiap tab 5s (no panic) + paste emoji ke `wl-copy`-observable target? Paste butuh
  focus target — verifikasi paste via: fokus terminal, trigger paste lewat... headless tak
  bisa klik. Bukti: (a) render tiap tab tanpa panic, (b) unit-tested paste path
  (`paste_text` diuji terhadap clipboard asli: paste "gpui-pick-probe" lalu baca balik
  via arboard dalam test? integrasi nyata tanpa GUI). Tulis 1 integration test untuk itu.
- Commit hasil bila perlu (test file ikut T1).

### T4 — Close-out
- `03-SUMMARY.md`; traceability PICK → In Progress (visual sign-off → Phase 5);
  STATE.md + ROADMAP.md; commit docs.

## Verification (phase gate)

1. `cargo check` + `cargo test` clean (incl. paste round-trip integration test).
2. Smoke run tiap tab tanpa panic.
3. `src/` + `src-tauri/` untouched.

## Risks

- Grid penuh (1914 emoji) lambat → ukur smoke; fallback virtualisasi documented.
- Fuse-ranking delta → dinyatakan di SUMMARY.
