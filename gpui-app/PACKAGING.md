# Packaging the GPUI build (`win11-clipboard-history-gpui`)

Milestone v0.8.0 installs the GPUI frontend **alongside** the Tauri app.
Nothing here overwrites Tauri-owned files; names/ids/paths are all distinct
(see `gpui-app/BACKEND_REUSE.md` and the Phase 1 coexistence policy).

## Dependencies (build machine)

- Rust stable (MSRV 1.77; GPUI requires a recent stable — 1.95 verified)
- System libs (present on any desktop Linux, Arch names):
  `wayland`, `libxkbcommon`, `libxkbcommon-x11`, `libxcb`, `vulkan-icd-loader`,
  `gtk3`, `libayatana-appindicator`, `fontconfig`
- Link-time extras the shared backend pulls in (missing ones fail the release
  link step, not `cargo check`): `libxdo` (tray/`muda`) and `libxkbcommon-x11`
  (X11 backend). Fedora: `libxdo-devel libxkbcommon-x11-devel`; Debian/Ubuntu:
  `libxdo-dev libxkbcommon-x11-dev`.
- Explicitly NOT needed at runtime: `webkit2gtk` (the release link drops it via
  `--as-needed`) and Node/npm — the GPUI binary has no webview. It *is* still
  needed to build, because the shared backend crate references Tauri.

## Build

```bash
make gpui-build        # release binary → gpui-app/target/release/win11-clipboard-history-gpui
```

Pinned via `gpui-app/Cargo.lock` (`gpui =0.2.2`).

## Install (PREFIX-aware, DESTDIR-safe)

```bash
sudo make gpui-install                 # → /usr/local
sudo make gpui-install PREFIX=/usr     # like a distro package
```

Layout:

| Path | Source |
|------|--------|
| `$BINDIR/win11-clipboard-history-gpui` | release binary |
| `$DATADIR/applications/win11-clipboard-history-gpui.desktop` | `gpui-app/dist/` (`__BINDIR__` substituted) |
| `$DATADIR/icons/hicolor/{128x128,256x256}/apps/win11-clipboard-history-gpui.png` | reused Tauri icons |
| `/etc/udev/rules.d/99-win11-clipboard-input.rules` | **shared** with Tauri (identical file, idempotent) |

The udev rule is binary-agnostic (uinput access only), so paste injection works
without new rules. `make gpui-uninstall` removes only GPUI-owned files and keeps
user data (`~/.config/win11-clipboard-history-gpui/`) plus the shared udev rule.

## First run

Launching the binary opens the 5-step setup wizard (own first-run marker):
permissions → DE shortcut (GNOME auto-registers `<exe> --toggle`; elsewhere the
wizard shows manual steps) → autostart (own `.desktop` entry) → done.
`--register-shortcuts` / `--unregister-shortcuts` do the GNOME part headlessly
(useful for scripts).

## Coexistence map

| Concern | Tauri build | GPUI build |
|---|---|---|
| Binary | `win11-clipboard-history[-bin]` | `win11-clipboard-history-gpui` |
| App ID | `dev.gustavosett.clipboard-history` | `dev.gustavosett.clipboard-history-gpui` |
| Config | `~/.config/win11-clipboard-history/` | `~/.config/win11-clipboard-history-gpui/` |
| Autostart | `win11-clipboard-history.desktop` | `win11-clipboard-history-gpui.desktop` |
| GNOME shortcuts | `win11-clipboard-history[-alt/-emoji]` | `win11-clipboard-history-gpui-*` |
| Socket | — | `$XDG_RUNTIME_DIR/win11-clipboard-gpui.sock` |

Only one build should own Super+V at a time; the GPUI wizard's conflict step
still detects DE-level collisions.

## AppImage

A portable single-file bundle, built without sudo:

```bash
make gpui-appimage                 # → gpui-app/dist/win11-clipboard-history-gpui_<ver>_x86_64.AppImage
make gpui-appimage-install         # build, then register it for the current user
```

`scripts/build-gpui-appimage.sh` stages an AppDir, deploys the binary's shared
libraries with `linuxdeploy`, and packs it with `appimagetool` (both are
downloaded to `$XDG_CACHE_HOME/win11-clipboard-appimage` and run with
`--appimage-extract-and-run`, so no FUSE is needed). AppDir contents:

| Path | Source |
|------|--------|
| `usr/bin/win11-clipboard-history-gpui` | release build |
| `usr/share/win11-clipboard-history-gpui/assets/` | `gpui-app/assets/` (`emojis.json`, `icons/`) |
| `usr/share/applications/win11-clipboard-history-gpui.desktop` | `gpui-app/dist/` (`__BINDIR__` dropped → AppRun) |
| `usr/share/icons/hicolor/{128x128,256x256,scalable}/apps/` | reused Tauri icons |

Deliberately **not** bundled: GTK3, libvulkan and the GPU drivers — they must
come from the host so theming and driver matching keep working. The tray
backend (`libayatana-appindicator`) *is* bundled.

`--install` copies the AppImage to `~/.local/bin`, writes a user desktop entry
with an absolute `Exec` (so the `Settings` action works), and installs the
icons. Autostart and the Super+V binding stay with the wizard.

## Dock / taskbar behaviour on Wayland (X11 is pinned on purpose)

On a Wayland session the GPUI build runs its windows through **XWayland**.
`main()` removes `WAYLAND_DISPLAY` (and aligns `XDG_SESSION_TYPE`) before the
platform initializes, because:

- gpui's Wayland backend has no way to keep the clipboard popup out of the
dock — xdg-shell has no window-type/notification hint, so `WindowKind::PopUp`
is an ordinary toplevel and GNOME's Dash-to-Dock lists it while it is open
(the Tauri build hits the same wall: tauri-apps/tauri#9829).
- gpui's **X11** backend tags `WindowKind::PopUp` as
`_NET_WM_WINDOW_TYPE_NOTIFICATION`, which docks and taskbars hide, and it
restores cursor-follow positioning for the popup.

Escape hatches:

| Env var | Effect |
|---------|--------|
| `WIN11_CLIPBOARD_ALLOW_WAYLAND=1` | keep the native Wayland backend (popup will appear in the dock while open) |

If no `DISPLAY` (XWayland) is available the app stays on Wayland automatically.
Settings and the wizard are ordinary decorated windows and always show up in
the dock — only the clipboard popup is hidden.

## First run checklist

1. Launch the AppImage (or the installed binary). The wizard opens.
2. Permissions: `/dev/uinput` must be writable for paste injection. The wizard
   offers *Fix Permissions* (`pkexec setfacl`); the durable alternative is the
   udev rule from the Tauri bundle (`TAG+="uaccess"`) or membership of the
   `input` group.
3. Shortcut: on GNOME the wizard registers `<AppImage> --toggle` on Super+V via
   gsettings. Registration resolves the AppImage path (`$APPIMAGE`), never the
   transient mount path, so the binding survives restarts.
4. Autostart: the wizard writes `~/.config/autostart/win11-clipboard-history-gpui.desktop`
   with `--background`, i.e. tray-only at login — no popup, no dock entry.
5. Tray icons need the AppIndicator GNOME extension; without it the app still
   works through Super+V and `--toggle`.

## Not in this milestone

Full deb/rpm bundles for the GPUI binary (AppImage + install flow only),
release-LTO verification beyond `cargo check --profile=release`, non-GNOME
DE auto-registration.
