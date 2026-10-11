# iced frontend — manual test matrix

Verified headless (see `iced-app/SPIKE-RESULTS.md` and the commit log): COSMIC overlay (nested
cosmic-comp: position, drag, outside click, Escape, type-to-filter, tabs, UI scale 1.5), GNOME window
path (headless gnome-shell + extension: place/hide/show, outside click), X11 (Xvfb + openbox:
cursor-follow, hide/show, WM hints), Settings and wizard rendering.

Needs a human on a real session:

| Session | Check |
|---|---|
| COSMIC (real, HiDPI/fractional) | overlay opens at the pointer once it moves, bottom-center otherwise; per-show creation latency; blur (`WIN11_CLIPBOARD_BLUR=1`); keyboard focus without a click; theme follows COSMIC dark/light live; paste lands in the previous window; tray icon + left click toggles |
| COSMIC multi-monitor | overlay appears on the output of the pointer; dismiss works on all outputs |
| GNOME Wayland + extension | popup at the pointer, hidden from dash, focus returns after hide, paste; drag via `window::drag`; wizard installs the extension (log out/in) |
| GNOME without extension, KDE, Sway | falls back to the x11 backend (XWayland) |
| X11 real WM | popup follows the cursor on every show, evdev outside-click, global hotkey grab |
| All | emoji tab colours, kaomoji/symbol insertion, Settings changes apply live, UI scale slider (re-creates the popup), wizard full run incl. `pkexec` permission fix, autostart toggle, Super+V shortcut registration |

Known gaps: no fade-in; history list renders at most the rows near the viewport; Settings/wizard
are not scaled by UI scale; `iced_test` UI simulations cover only the emoji grid so far.
