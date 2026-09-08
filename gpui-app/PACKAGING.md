# Packaging the GPUI build (`win11-clipboard-history-gpui`)

Milestone v0.8.0 installs the GPUI frontend **alongside** the Tauri app.
Nothing here overwrites Tauri-owned files; names/ids/paths are all distinct
(see `gpui-app/BACKEND_REUSE.md` and the Phase 1 coexistence policy).

## Dependencies (build machine)

- Rust stable (MSRV 1.77; GPUI requires a recent stable — 1.95 verified)
- System libs (present on any desktop Linux, Arch names):
  `wayland`, `libxkbcommon`, `libxcb`, `vulkan-icd-loader`,
  `gtk3`, `libayatana-appindicator`, `fontconfig`
- Explicitly NOT needed: `webkit2gtk`, Node/npm — the GPUI binary has no webview.

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

## Not in this milestone

Full deb/rpm/AppImage bundles for the GPUI binary (install flow + docs only),
release-LTO verification beyond `cargo check --profile=release`, non-GNOME
DE auto-registration.
