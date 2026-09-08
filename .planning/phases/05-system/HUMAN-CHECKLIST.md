# Human Visual Checklist — v0.8.0 GPUI port

Sandboxed automation cannot take screenshots here, so pixel sign-off is yours.
Run both builds side by side (Super+V each — only one should own it at a time;
quit the other first). Check **dark + light** (toggle system theme between runs).

Build & run:
```bash
cargo run --manifest-path gpui-app/Cargo.toml -- --settings   # popup + settings
GPUI_SMOKE_TAB=emoji   cargo run --manifest-path gpui-app/Cargo.toml
GPUI_SMOKE_TAB=symbols  cargo run --manifest-path gpui-app/Cargo.toml
GPUI_SMOKE_TAB=kaomoji  cargo run --manifest-path gpui-app/Cargo.toml
./target/debug/win11-clipboard-history-gpui --setup   # wizard (fresh: rm ~/.config/win11-clipboard-history-gpui/first_run_done)
```

## Popup (CORE/WIND)

- [ ] List, spacing, radii, pin ring + dot match the Tauri window 1:1
- [ ] Card hover reveals pin/delete (+ link/mail on URLs, emails, `#hex` colors)
- [ ] Image item shows thumbnail + `WxH` badge
- [ ] Ctrl+F search, type-to-filter, regex toggle, Esc hierarchy
- [ ] Enter pastes into the focused app; Esc closes; arrows/Home/End navigate
- [ ] Window follows the mouse (X11) / bottom-center (Wayland); always-on-top
- [ ] Acrylic blur + opacity match (try NVIDIA/AppImage machine for opaque fallback)

## Pickers (PICK)

- [ ] Emoji grid (40px cells), recent strip, category pills, footer preview
- [ ] Kaomoji 2-col buttons, Custom pill with your customs, footer category
- [ ] Symbols grid, 14 categories, recents, footer names
- [ ] Clicking/Enter pastes the glyph into the focused app
- [ ] Grid arrows/Home/End/PgUp/PgDn + Ctrl+Left/Right tab switching

## Settings (SET)

- [ ] Personalization header + Saved pill; 3 theme cards switch the popup live
- [ ] Auto-delete value + unit pills + info line
- [ ] Both opacity sliders drag smoothly and change the popup live
- [ ] UI scale slider (window resizes; content zoom is a known delta)
- [ ] Max history number; custom kaomoji add/delete; 2 feature toggles
- [ ] Register Shortkeys button reports success; Reset restores defaults

## Wizard (SET)

- [ ] 5 steps with progress dots; permission fix; conflict auto-fix or manual text
- [ ] Shortcut registers Super+V (check Settings → Keyboard on GNOME)
- [ ] Autostart yes/no; Done opens the popup

## System (SYS/PKG)

- [ ] Tray icon appears (AppIndicator extension needed on GNOME); menu works
- [ ] Super+V toggles after `--register-shortcuts` (log out/in may be needed)
- [ ] Autostart entry launches on login (check `~/.config/autostart/`)
- [ ] `make gpui-install PREFIX=/usr` + first-run wizard on a clean machine

Report mismatches as `adjust: ...` and they become the next milestone's scope.
