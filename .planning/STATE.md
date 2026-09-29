---
milestone: v0.8.1
milestone_name: Visual Sign-off & Polish (planned)
status: planning
progress:
  phases_total: 0
  phases_complete: 0
  requirements_total: 24
  requirements_complete: 0
---

# State

## Current Position

Phase: Milestone v0.8.0 complete — 5/5 phases code-complete, 6/30 requirements Complete
Plan: Next — human visual sign-off (HUMAN-CHECKLIST.md) → polish gaps → bundles
Status: Between milestones (v0.8.0 archived in MILESTONES.md)
Last activity: 2026-09-29 — SYS-07 Desktop (GTK) theme card implemented in the GPUI app

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-08)

**Core value:** Super+V yang cepat dan cantik di Linux — popup ala Windows 11 yang instan dan familiar.
**Current focus:** v0.8.1 planning — visual sign-off + polish gaps from v0.8.0 audit.

## Accumulated Context

### Decisions (carried forward)

- v0.8.0 = GPUI-Rust frontend port, React/Tauri tetap hidup (koeksistensi) — DONE, proven live
- Backend Rust dipakai ulang, zero `src/`/`src-tauri/` changes — DONE (verified every phase)
- GIF tab tetap disabled (Tenor API mati)
- Research + roadmap inline (subagents unavailable — billing)
- Upstream `gpui =0.2.2` pinned + Cargo.lock committed
- Theme tokens are functions behind a process-global GTK override: GTK mode returns
  desktop colors from *both* `theme::dark::*` and `theme::light::*`, so every existing
  `if is_dark { dark::x() } else { light::x() }` call site follows the desktop palette
  with no edits — and Win11 modes stay byte-identical (`theme::hex` literals + parity tests)
- GTK palette is read through GTK3 `StyleContext::lookup_color` (gtk-rs already linked for
  the tray), cached by (theme name, prefer-dark); GNOME accent is also read from the XDG
  portal `accent-color` because GTK3 themes do not define `accent_bg_color`
- GTK palette merges two sources, user CSS first: `~/.config/gtk-4.0/{colors,gtk}.css`
  (`@define-color` + libadwaita `:root` vars, e.g. Rewaita output) and then the GTK3
  `StyleContext::lookup_color` fallback. Reason: `gtk-theme` in dconf can be
  WhiteSur-Dark while every libadwaita app renders Tokyo Night from the user CSS —
  reading only `gtk-theme` made the Desktop card show the wrong palette. The cache key
  includes those files' mtimes, so editing them repaints the app within ~1 s; the XDG
  portal `accent-color` is only a last-resort accent source (user CSS wins)
- Settings/Setup windows are now ARGB + rounded (`RADIUS_WINDOW`) with a 1 px outline in
  every mode: on X11 mutter painted no shadow and the square opaque root made the window
  melt into the desktop (user-reported against Nautilus). No `_GTK_FRAME_EXTENTS` from
  gpui, so the outline — not a compositor shadow — is what separates the window
- Window controls moved out of their own 36 px title strip into the corner of the
  Settings header / over the wizard (`titlebar::render_controls`, absolute): the window
  opens on the heading instead of a mostly empty bar (user-reported forehead)
- Popup root now carries the same 1 px `theme::dark/light::border()` outline as
  Settings/Setup (user-reported: the popover had no border while Settings did),
  and its radius uses `theme::RADIUS_WINDOW`
- Window shadow: Settings/Setup reserve a 16 px transparent margin
  (`theme::WINDOW_SHADOW_MARGIN`), paint a gpui `BoxShadow` into it, and advertise
  `_GTK_FRAME_EXTENTS` (device px = logical × scale) via `window_drag::set_frame_extents`
  so mutter drops its square shadow and follows the rounded frame. Verified live:
  mutter grows the client by the extents (client = requested + 2e), so
  `centered_options` asks for the card size and the card lands on the frame;
  sliders subtract the margin. Composited shadow still needs a human eye (GNOME
  blocks programmatic screenshots of the composited output)
- v0.8.0 AppImage rebuilt + reinstalled for the user with `theme_mode: "gtk"` so the
  desktop palette is live
- Next: HUMAN-CHECKLIST.md is the entry gate for v0.8.1 scope

### Open Questions / Blockers

- Human pixel sign-off (dark + light) — awaiting user with a screen (HUMAN-CHECKLIST.md)
- X11 session needed to verify: hotkey grab, cursor-follow, NVIDIA blur behavior
- Non-GNOME DE auto-registration + deb/rpm/AppImage bundles — scoped for v0.8.1+
- Release-LTO build verification (`cargo check --profile=release` noted, full build open)

### Todos

- [ ] User runs HUMAN-CHECKLIST.md (dark + light + Desktop/GTK) and reports deltas
- [ ] Scope v0.8.1 from checklist findings (`/gsd-new-milestone`)
- [ ] X11-session verification pass
- [ ] Bundle + publish decision
