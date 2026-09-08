# Phase 3 Summary: Pickers parity (code complete, visual sign-off deferred)

**Status:** code complete | **Date:** 2026-09-08
**Requirements:** PICK-01–05 → implemented + committed, status **In Progress**
(visual sign-off joins the Phase 5 human checklist).

## What was built

- Data (`src/pickers.rs`): emojilib@4 vendored (`assets/emojis.json`, 1914 — dataset
  identik React), kaomoji/symbols via `include_str!` (972 / 2484, nol duplikasi),
  ported category algorithm (incl. its quirks — asserted in tests), ranked-contains
  search limit 100, symbol recents file (max 24 LRU), custom kaomoji support.
- Backend: `EmojiManager` reuse (own data dir) + `paste_text` mirror
  (mark → set_text_robust → Ctrl+V; + usage record untuk emoji).
- Views (`src/ui/pickers.rs`): PickerLayout chrome, wrapping category pills
  (All/[Custom]/cats), 40px glyph grids (emoji 24px / symbol 20px), kaomoji h-12
  buttons, recent strips (8/10-col, max 16), hover footers, empty states.
- Wiring: per-picker search state + focus, grid keyboard nav port
  (arrows/Home/End/Ctrl variants/PgUp/PgDn/Enter/Space), Ctrl+Left/Right tab switch,
  type-to-filter per picker, `GPUI_SMOKE_TAB` render-path aid.
- Commits: f3a4155 (data), e238be7 (UI + wiring).

## Verification (all pass)

1. `cargo check` clean; `cargo test` 22→24 green (dataset counts, quirk parity,
   search ranking, recents round-trip, **live clipboard write round-trip**,
   scratch add/remove).
2. Smoke all 4 tabs 6s each on GNOME Wayland: alive, 0 panics (full 1914/972/2484
   grids render without complaint).
3. `src/` + `src-tauri/` untouched.

## Deferred / deltas (honest list)

1. **Visual sign-off** joins the Phase 5 human checklist (sandbox blocks screenshots).
2. Fuse.js fuzzy typo-tolerance → ranked-contains (exact/prefix queries identical).
3. Category pills wrap instead of horizontal-scroll strip (all visible, no chevrons).
4. Recent strips keyboard-navigable in React; GPUI: clickable + hover only (main grid
   fully keyboard-operable). Category pills clickable only.
5. No grid virtualization yet (full render measured fine in smoke; revisit if slow
   on weaker GPUs).
6. Emoji glyphs render via system emoji font (same as WebKit — same glyphs).
