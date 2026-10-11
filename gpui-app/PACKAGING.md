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
make gpui-appimage                 # → gpui-app/dist/win11-clipboard-history_<ver>_x86_64.AppImage
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

`--install` copies the AppImage to `~/Applications`, writes a user desktop entry
with an absolute `Exec` (so the `Settings` action works), and installs the
icons. Autostart and the Super+V binding stay with the wizard.

## Platform backends (x11, gnome, cosmic)

One binary carries three window backends (`src/platform/`); `platform::init()`
picks one at startup, before gpui initializes:

| Backend | Chosen when | How the popup works |
|---|---|---|
| `cosmic` | `XDG_CURRENT_DESKTOP` has `cosmic` on a Wayland session | native Wayland, a transparent full-output `wlr-layer-shell` overlay created per show; the card is drawn at the pointer, dragged and dismissed client-side. No dock entry, no title bar, no XWayland, no `input` group needed |
| `gnome` | GNOME on a Wayland session **and** the helper GNOME Shell extension is running | native Wayland window; the extension (embedded in the binary, installed by the wizard or `--install-gnome-extension`) places, shows, hides it, reports the pointer and clicks outside, and keeps the app out of the dock while only the popup is open |
| `x11` | everything else (KDE, Sway, X11 sessions, GNOME without the extension) | legacy: X11 client, run through **XWayland** on Wayland sessions (`WAYLAND_DISPLAY` is dropped on purpose: gpui's X11 backend tags the popup `_NET_WM_WINDOW_TYPE_NOTIFICATION`, which docks hide, and cursor-follow works) |

Notes:

- The GNOME extension is only loaded by GNOME Shell at **login** on Wayland
(Shell cannot be restarted there), so right after the first install the app
keeps using `x11`; the wizard says to log out and in once.
- Supported dock filtering was verified against GNOME Shell 50's stock dash;
Dash to Dock / Ubuntu Dock use the same `Shell.AppSystem.get_running()` list
but have not been tested. Where a dock still shows the app, it behaves as a
normal window entry only while the popup is open.
- The shortcut registration (`Super+V` → `--toggle`) follows the *desktop*, not
the backend (`src/platform/shortcuts/`).

Environment variables:

| Env var | Effect |
|---------|--------|
| `WIN11_CLIPBOARD_BACKEND=x11\|gnome\|cosmic` | force a backend (debug/CI) |
| `WIN11_CLIPBOARD_ALLOW_WAYLAND=1` | with the `x11` backend: keep native Wayland instead of pinning XWayland |
| `WIN11_CLIPBOARD_BLUR=1` | `cosmic`: ask the compositor (`ext-background-effect-v1`) to blur behind the card; opt-in, not yet verified on a real COSMIC |

If no `DISPLAY` (XWayland) is available the `x11` backend stays on Wayland
automatically. Settings and the wizard are ordinary decorated windows and always
show up in the dock — only the clipboard popup is hidden.

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
release-LTO verification beyond `cargo check --profile=release`, shortcut
auto-registration for DEs other than GNOME and COSMIC.
