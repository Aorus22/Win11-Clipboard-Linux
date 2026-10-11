# Platform backends — manual test matrix

What automated/headless testing already covered (GNOME Shell 50.4 `--headless`, nested
`cosmic-comp`, Xvfb + openbox) and what still needs a human on a real session.

Common checks per backend: toggle (Super+V / `--toggle`), popup at the cursor and clamped
to the monitor, header drag, click outside closes, Escape closes, typing reaches the search
box immediately, paste lands in the previously focused window, popup absent from dock /
alt-tab, Settings and Setup windows have the rounded card + shadow (no square slab, no
double margin), multi-monitor follow.

| Session | Backend | Verified headless | Needs a human |
|---|---|---|---|
| GNOME Wayland + extension | `gnome` | extension loads (Shell 50.4), place/show/hide, pointer, monitor work area, outside click, focus returns to previous window, stock dash hides the app, Settings frame = card size | real pointer/keyboard feel, drag via `xdg_toplevel.move`, paste, Dash to Dock / Ubuntu Dock / Dash to Panel filtering, GNOME 45–49, minimize animation, HiDPI/fractional scale |
| GNOME Wayland, no extension | `x11` | selection falls back | wizard install message, log out/in activation |
| GNOME on X11 | `x11` | — | full matrix |
| COSMIC (real) | `cosmic` | nested cosmic-comp: overlay opens at pointer, clamp, drag, outside click, Escape, keyboard focus (Exclusive) | per-show window creation latency on a real GPU, output choice on multi-monitor, IME, blur (`WIN11_CLIPBOARD_BLUR=1`), paste focus return, no `input` group needed |
| X11 (Xvfb + openbox) | `x11` | show at cursor, hide = unmap, WM hints, gpui-pre X11 | real WMs, evdev outside click, global hotkey |
| KDE / Sway | `x11` | — | full matrix (fallback path) |

Known open items:
- `gpui-app/Cargo.toml` patches `gpui-pre` with a local path; CI/AppImage builds need the
  `gpui-wayland-patches` repo pushed and the `git = …, rev = …` form.
- XWayland inside `gnome-shell --headless` hung in the test harness, so the `x11` backend was
  checked on Xvfb instead.
