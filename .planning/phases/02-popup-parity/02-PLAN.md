# Plan: Phase 2 — Clipboard popup parity

**Phase:** 2 | **Requirements:** CORE-01–06, WIND-01–04
**Mode:** inline (autonomous, no subagents)

## Goal

Popup GPUI: TabBar + Header + Search + history list (pinned/recent, hover actions, smart
actions, compact) + keyboard nav + paste flow + positioning — visually 1:1 vs React,
data live via reused `ClipboardManager`.

## Tasks (atomic commits)

### T1 — Core modules (no UI): settings, geometry, theme, history store
- `Cargo.toml`: path-dep `win11-clipboard-history-lib = { path = "../src-tauri" }`
  (+ needed transitive crates: serde/serde_json, chrono, uuid — via lib re-export where possible).
- `src/settings.rs`: own config dir `~/.config/win11-clipboard-history-gpui/`, load/save
  `user_settings.json` with identical defaults; `system` theme → gsettings color-scheme probe.
- `src/geometry.rs`: cursor pos (xdotool + x11rb port), clamp-10px, bottom-center fallback (ports).
- `src/theme.rs`: Win11 tokens (dark+light) as `Rgba`, opacity math port
  (secondary=+0.3, tertiary=+0.3, card/tertiary bg fns), radius 8/12.
- `src/history.rs`: store (items, search, regex, focus idx, compact, pinned-expanded),
  filter (substring+regex verbatim, images excluded), timestamp relative, smart-action detect
  (color/link/email port of smartActionService).
- Verify: `cargo check`, unit tests for filter/timestamp/opacity/geometry/settings-defaults.
- Commit: `feat(gpui): phase2 core — settings, geometry, theme, history store`

### T2 — UI views
- `src/ui/icons.rs`: inline minimal SVGs (pin, x, type, image, search, chevron-down, history,
  clipboard-list, smile, omega, layout-list) via `svg()`.
- `src/ui/search.rs`: single-line editor (FocusHandle + actions!, trimmed input.rs pattern).
- `src/ui/tabbar.rs`, `src/ui/header.rs`, `src/ui/history_item.rs`, `src/ui/popup.rs`
  (root: drag handle strip + TabBar + Header + Search + scroll list + pinned/recent sections).
- Non-clipboard tabs: centered placeholder label (replaced Phase 3).
- Keyboard on root: Esc (close-search→hide), Enter/Space (paste focused), arrows/Home/End,
  Ctrl+F, printable type-to-filter — mirrors ClipboardTab semantics.
- Verify: `cargo check` + `cargo test`.
- Commit: `feat(gpui): phase2 clipboard popup UI`

### T3 — App wiring: monitor, paste, show/hide, positioning
- `main.rs`: open positioned window (X11 follow-mouse / Wayland saved-or-bottom-center),
  `HistoryStore` as `Model`, 500ms watcher thread (hash-change, cleanup 30s — mirror main.rs),
  version-bump → refresh; paste flow (hide → focus restore → paste_item → refresh);
  Esc-hide (app hide, process lives).
- Verify: `cargo build`, smoke run on GNOME Wayland (stays alive, no panic).
- Commit: `feat(gpui): phase2 live wiring — monitor, paste, positioning`

### T4 — Verification + evidence
- `cargo test` green; screenshot via gnome-screenshot of running popup (seeded demo items if
  live history empty); structural token audit (colors/radii/spacing vs tailwind.config).
- Screenshots saved to `.planning/phases/02-popup-parity/` (not committed if huge — commit if small).
- Write `02-SUMMARY.md`; update REQUIREMENTS traceability (CORE/WIND → Complete where proven,
  else In Progress with reason); STATE.md + ROADMAP.md status.
- Commit: `docs(phase-2): context, plan, summary`

## Verification (phase gate)

1. `cargo check` + `cargo test` clean.
2. Binary runs on GNOME Wayland ≥10s, no panic; live history visible (or seeded demo).
3. Screenshot/token audit shows 1:1 structure (document deltas, if any, honestly).
4. `src/` + `src-tauri/` untouched.

## Risks

- Path-dep pulls tauri closure (webkit sys crates) → system libs confirmed present; if build
  fails → fallback local store (documented in CONTEXT), UI work unaffected (trait boundary).
- `svg()` icon API friction → fallback: unicode glyphs (document delta).
- Image thumbs API friction → fallback: `Image (WxH)` label (document delta).
