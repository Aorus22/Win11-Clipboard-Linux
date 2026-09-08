# Phase 3: Pickers parity - Context

**Gathered:** 2026-09-08
**Status:** Ready for planning
**Mode:** Auto (autonomous — full direction discretion granted)

<domain>
## Phase Boundary

Tab emoji / kaomoji / symbols berfungsi penuh dengan tampilan dan perilaku identik:
PickerLayout chrome (search + categories + footer), grid cells, recent strips, category
pills, keyboard nav grid, paste flows. Scope: PICK-01–05. Bukan scope: settings/wizard
(Phase 4), hotkey/tray/packaging (Phase 5).

</domain>

<decisions>
## Implementation Decisions

### Data: vendor emojilib, include_str repo JSONs
- `gpui-app/assets/emojis.json` (1914 entries, generated from `emojilib@4.0.3` via npm —
  dataset identik dengan React). Rust port verbatim `loadEmojis` + `detectCategory`
  (8 kategori keyword + Other). DONE (file committed next).
- `kaomojis.json` + `symbols.json` di-`include_str!` dari `src/data/` (nol duplikasi).
- Custom kaomoji dari settings file (sudah ada di AppSettings).

### Search: ranked-contains, limit 100 (delta Fuse didokumentasikan)
Fuse.js threshold-0.3 tidak di-port 1:1 (fuzzy typo-tolerance = delta yang dinyatakan).
Pengganti: ranking deterministic — name==query > name starts > keyword starts >
contains (name/keywords), stabil, limit 100. Untuk query exact/prefix hasilnya identik.

### Recents
- Emoji: reuse `EmojiManager` (own data dir = gpui config dir, format file sama).
- Symbols: JSON file sendiri `recent_symbols.json` (max 24, LRU depan — port hook).
- Kaomoji: tidak ada recents di React — tidak dibuat.

### Paste flows = mirror `paste_text` command
hide → focus restore → `mark_text_as_pasted` + `set_text_robust` → Ctrl+V simulate.
Emoji: + `record_usage`. Semua via `BackendService::paste_text`.

### Grid rendering: full render, kolom dari lebar window
Kolom emoji/simbol = floor((win_w − 48) / 40) (deterministik; 7 kolom @360px — sama
dengan React pada lebar itu). Kaomoji: ≥768→4, ≥640→3, else 2 (@360px → 2 ✓).
Tanpa virtualisasi dulu (ukur live; React memvirtualisasi emoji/simbol — perf follow-up
bila lambat). Recent strips: emoji 8-kolom, simbol 10-kolom, max 16.

### Keyboard
Grid nav = port verbatim `useKeyboardNavigation` (arrows/Home/End/Ctrl+Home/End/
PgUp×3/PgDn×3/Enter/Space). Di tab picker, arrows milik grid (bukan tab-switch);
tab-switch via klik TabBar atau Ctrl+Left/Right. Printable = type-to-filter per-picker.
Esc/Ctrl+F sama seperti tab clipboard.

### Search editor reuse
`SearchState` dipakai ulang per-picker (field sendiri + FocusHandle sendiri);
`render()` digeneralisasi dengan placeholder + selector field.

</decisions>

<code_context>
## Existing Code Insights

- `EmojiPicker`/`SymbolPicker`: 40px cells (text-2xl emoji / text-xl symbol), hover
  tertiary + scale, ring on focus, recent strip 32px cells (slice 16), footer preview
  (char text-xl + name text-xs) atau hint, empty "No emojis/symbols found".
- `KaomojiPicker`: h-12 buttons text-sm, hover scale-105 + border-subtle, kategori
  + Custom pill bila ada custom, footer preview kategori.
- `CategoryPill`: px-3 py-1 text-xs rounded-full; aktif = accent bg + white text;
  idle = tertiary bg + secondary text.
- `PickerLayout`: header px-3 pt-3 pb-2 / subHeader px-3 pb-2 / grid flex-1 / footer
  h-10 (40px) border-t.
- Paste: `paste_text {text, itemType?}` — emoji record usage; kaomoji/simbol tanpa.

</code>

<specifics>
## Specific Ideas

- Verifikasi: `cargo test` (load counts, kategori spot-check, search ranking, recents
  round-trip) + live run (buka tiap tab, paste emoji/kaomoji/simbol ke editor → teks
  muncul) + perf kasar (waktu render grid penuh di log? tidak perlu — smoke cukup).

</specifics>

<deferred>
## Deferred Ideas

- Virtualisasi grid bila render penuh terbukti lambat (ukur dulu).
- Fuzzy typo-tolerance setara Fuse (butuh riset crate; bukan core value).
- Settings/wizard (Phase 4), hotkey/tray/packaging (Phase 5).

</deferred>
