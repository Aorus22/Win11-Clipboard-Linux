# Clipboard History — iced / libcosmic frontend

The third frontend of this repo (after the Tauri app and the GPUI build): same clipboard
engine (`core/`) and shared logic (`app-shared/`), UI written with
[libcosmic](https://github.com/pop-os/libcosmic) (COSMIC's iced), so it follows the live
COSMIC theme and uses COSMIC widgets. It installs **next to** the GPUI build: own binary
`win11-clipboard-history-iced`, config `~/.config/win11-clipboard-history-iced/`, socket,
autostart entry, shortcut names and Wayland app id (`dev.gustavosett.clipboard-history-iced`).

## Build / install

```bash
make iced-build              # iced-app/target/release/win11-clipboard-history-iced
make iced-install            # PREFIX-aware install (needs sudo for /usr/local, like gpui-install)
make iced-appimage           # iced-app/dist/win11-clipboard-history-iced_<ver>_x86_64.AppImage
make iced-appimage-install   # AppImage for this user only (~/Applications); no autostart
```

libcosmic is pinned to a git revision (`Cargo.toml`); `Cargo.lock` is committed. System
libraries: Wayland / xkbcommon / fontconfig / Vulkan loader (no GTK, no appindicator: the
tray is a pure D-Bus StatusNotifierItem).

## Window backends

Chosen at startup like the GPUI build (`WIN11_CLIPBOARD_BACKEND=x11|gnome|cosmic`, see
[`gpui-app/PACKAGING.md`](../gpui-app/PACKAGING.md#platform-backends-x11-gnome-cosmic)):

| Desktop | Popup |
|---|---|
| COSMIC (Wayland) | transparent full-output `wlr-layer-shell` overlay, created per show; card at the pointer (bottom-center if the pointer has not moved), drag and click-outside handled in-surface; blur behind the card follows COSMIC's frosted-glass setting (`WIN11_CLIPBOARD_BLUR=1|0` forces it) |
| GNOME (Wayland) + extension | native window placed/shown/hidden by the shared GNOME Shell extension (`--install-gnome-extension`, log out/in once) |
| X11 / everything else | X11 window moved/mapped through `app-shared`'s x11 backend |

## What differs from the GPUI build

- Colours come from the COSMIC theme (Settings → Theme: System / Light / Dark); the GTK
  theme mode and the Win11 palette are not ported. Without COSMIC settings (GNOME/X11) the
  theme falls back to libcosmic's dark/light default.
- Colour emoji are drawn as bitmaps taken from the bundled `NotoColorEmoji-CBDT.ttf`
  (iced's text renderer draws no colour glyphs); see `src/emoji_image.rs`.
- libcosmic does not apply its scale factor, so **UI scale** is implemented by the popup
  itself (`src/scale.rs`); Settings and the wizard are not scaled.
- Lists are virtualised with fixed row heights; very long histories render only the rows near
  the viewport. No fade-in animation on show.
- Outside-click on X11 still needs read access to `/dev/input` (evdev), as in the GPUI build.

## Tests

`cargo test --manifest-path iced-app/Cargo.toml` — popup / picker / settings / wizard logic and
the emoji bitmap lookup (every emoji of the dataset must have one). Interaction was exercised
headless: nested `cosmic-comp` (overlay), `gnome-shell --headless` + the extension (window),
Xvfb + openbox (X11). See `.planning/phases/07-iced-app/HUMAN-CHECKLIST.md` for what still needs
a human on a real session.
