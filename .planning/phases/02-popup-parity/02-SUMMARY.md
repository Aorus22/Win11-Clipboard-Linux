# Phase 2 Summary: Clipboard popup parity (code complete, visual sign-off deferred)

**Status:** code complete | **Date:** 2026-09-08
**Requirements:** CORE-01–06, WIND-01–04 → all implemented + committed, status **In Progress**
pending visual sign-off (screenshot blocked by sandbox — see §Verification).

## What was built

- `gpui-app/src/ui/` (7 views): tabbar (4 tabs, arrow switching), header (title + count pill +
  compact toggle + Clear All), search (single-line editor, caret, regex + clear buttons),
  history cards (icon box, 3-line clamp, timestamp, hover-reveal pin/delete/smart actions,
  pinned ring + dot, image thumbs via in-memory `ImageSource`), pinned/recent sections,
  EmptyState, drag strip + close, placeholder tabs.
- `gpui-app/src/` core: settings (own config dir, Tauri-identical defaults), geometry
  (xdotool/x11rb cursor, clamp-10px, bottom-center-45px ports), theme (full Win11 token table),
  history (filter/search/regex/timestamp/smart-action ports + 16 tests), backend
  (real `ClipboardManager` reuse, 500ms watcher, paste flow hide→focus-restore→paste).
- `gpui-app/assets/icons/`: 15 lucide-equivalent SVGs tinted via `text_color`.
- Commits: a4d447a (core), 7f71da8 (UI + wiring).

## Verification (all executable checks pass)

1. `cargo check` clean (32 API errors fixed across 6 iterations — all GPUI APIs now proven).
2. `cargo test`: 16/16 green.
3. `cargo build` clean; binary runs on GNOME Wayland, no panic.
4. **Live probe (real Wayland session):** fresh start loaded persisted history (1 item =
   persistence round-trip ✓); two `wl-copy` writes → watcher detected both
   (`history changed: 2 items`, `3 items`) → snapshot→poll→refresh path proven live.
5. Coexistence intact: Tauri build kept running; `src/` + `src-tauri/` untouched.

## Deferred / deltas (honest list)

1. **Visual sign-off BLOCKED:** `gnome-screenshot` denied by sandbox portal policy
   (`Screenshot is not allowed`), no Xvfb. No side-by-side pixels yet. → Phase 5 task:
   human runs both builds and compares (checklist below). Code asserts parity by
   construction (identical tokens, spacing, radii, copy).
2. Micro-deltas vs React (documented in code): chevron static (no -90° rotation when
   collapsed); focused/pinned indication is 2px accent border instead of ring-1;
   compact image label identical; font falls back to system sans (same fallback chain
   as React on machines without Segoe UI).
3. `ui_scale ≠ 1.0` scales nothing yet (window fixed 360×480); owned by Phase 4 settings UI.
4. Window drag regions not wired (GPUI move API); strip + close button render. Best-effort in Phase 5.
5. Wayland positioning = bottom-center each launch (matches Tauri: saved-state writes are
   commented out upstream, so Tauri also always falls back). Saved-state restore arrives
   with show/hide toggle in Phase 5.

## Human visual checklist (Phase 5)

Run `win11-clipboard-history-bin` and `win11-clipboard-history-gpui` side by side (dark +
light): tab bar, header + count, search open (Ctrl+F), pinned + recent sections, hover
actions on a card, image item, empty state, Esc/Enter/arrows/Ctrl+F/type-to-filter behavior.
