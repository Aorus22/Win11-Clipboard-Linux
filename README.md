# Win11 Clipboard for Linux

A GPUI-focused fork of
[gustavosett/Windows-11-Clipboard-History-For-Linux](https://github.com/gustavosett/Windows-11-Clipboard-History-For-Linux).

[Releases](https://github.com/Aorus22/Win11-Clipboard-Linux/releases) • [Report Bug](https://github.com/Aorus22/Win11-Clipboard-Linux/issues) • [Actions](https://github.com/Aorus22/Win11-Clipboard-Linux/actions)

**The aesthetic, feature-rich clipboard manager your Linux desktop deserves — now with a lightweight native GPUI frontend.**

MIT licensed. Built with Rust and GPUI.

---

## Quick Start

No install needed. Grab the AppImage built fresh from `main` by CI and run it.

```bash
# 1. Download the latest AppImage (GPUI frontend)
gh release download latest --repo Aorus22/Win11-Clipboard-Linux --pattern '*_x86_64.AppImage'

# 2. Make it executable and run it
chmod +x win11-clipboard-history_*.AppImage
./win11-clipboard-history_*.AppImage
```

No `gh` CLI? Download it from the browser instead:
**[Latest AppImage](https://github.com/Aorus22/Win11-Clipboard-Linux/releases/tag/latest)**,
then `chmod +x` and run.

> **Paste permission (one-time):** paste simulation needs access to `/dev/uinput`.
> `sudo setfacl -m u:$USER:rw /dev/uinput` grants it immediately (resets on reboot —
> for a permanent rule see the upstream install script or your distro's udev docs).

The AppImage is the native GPUI client (`win11-clipboard-history_*_x86_64.AppImage`):
no webview, no install needed.

Verify your download with the checksums published next to every release:

```bash
sha256sum -c SHA256SUMS.txt
```

Register `Super+V` to launch it:
`KEYBOARD SETTINGS -> SHORTCUTS -> NEW SHORTCUT -> Super+V -> /path/to/win11-clipboard-history_*.AppImage`

---

## About this fork

This is a fork of
[gustavosett/Windows-11-Clipboard-History-For-Linux](https://github.com/gustavosett/Windows-11-Clipboard-History-For-Linux)
focused on the **GPUI frontend**: a native Rust client with no webview,
smaller footprint, and faster startup.

What this fork adds on top of upstream:

- A portable **AppImage** of the GPUI client, built by a manually-triggered CI
  workflow and published to a rolling `latest` release.
- A reproducible **Docker builder image** so local builds use the exact same
  toolchain as CI.

---

## Why use this?

Most Linux clipboard managers are purely functional but lack visual appeal. This project brings the **modern, fluid design of Windows 11's clipboard history** to the Linux ecosystem, backed by the blazing speed of Rust.

| Feature | Description |
| --- | --- |
| **Universal Support** | Works on both **Wayland** and **X11**. The AppImage runs on any distro. |
| **Instant Access** | Opens instantly with `Super+V` or `Ctrl+Alt+V`. |
| **Smart Positioning** | The window follows your mouse cursor across multiple monitors. |
| **Pin and Sync** | Pin important snippets to keep them at the top. |
| **Emoji Picker** | A built-in, searchable emoji keyboard. |
| **Privacy First** | Your history is stored locally. No data leaves your machine. |

> **Note on GIFs:** the GIF tab is currently disabled because Google killed the Tenor API
> ([background](https://arstechnica.com/gadgets/2026/06/google-kills-tenor-gif-api-forcing-changes-at-x-discord-and-more/)).
> The implementation remains in the source tree so a sustainable provider can be integrated later.

---

## Shortcuts and Usage

| Key | Action |
| --- | --- |
| <kbd>Super</kbd> + <kbd>V</kbd> | **Open Clipboard History** |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>V</kbd> | Alternative Shortcut |
| <kbd>Enter</kbd> | Paste Selected Item |
| <kbd>Esc</kbd> | Close Window |

On first run the app shows a Setup Wizard to configure shortcuts and permissions.

---

## Build

Three ways to get a binary: build locally, trigger CI, or run from source in dev mode.

### 1. Local build

Prerequisites: Rust plus the system headers (`make deps` installs them, distro auto-detected).

```bash
# 1. Clone
git clone https://github.com/Aorus22/Win11-Clipboard-Linux.git
cd Win11-Clipboard-Linux

# 2. System deps + Rust
make deps && make rust
source ~/.cargo/env

# 3a. Release binary         -> gpui-app/target/release/win11-clipboard-history-gpui
make gpui-build

# 3b. Portable AppImage      -> gpui-app/dist/win11-clipboard-history_*_x86_64.AppImage
make gpui-appimage

# 3c. Install to this system (needs sudo)
sudo make gpui-install
```

Prefer the exact CI toolchain instead? Build inside the Docker builder image —
byte-identical to what CI produces, no host deps needed except Docker:

```bash
docker build -f docker/Dockerfile.build -t winclip-build .
docker run --rm -v "$PWD:/src" -w /src winclip-build bash docker/build-appimage.sh
# -> dist/win11-clipboard-history_*_x86_64.AppImage
#    dist/SHA256SUMS.txt
```

### 2. CI build

The [`build-appimage`](https://github.com/Aorus22/Win11-Clipboard-Linux/actions/workflows/build-appimage.yml)
workflow never runs on its own — trigger it explicitly. From the Actions tab
open "Build AppImage", click "Run workflow", set the tag (`latest` for the
rolling release, `vX.Y.Z` for a versioned one) and tick `publish` to land the
result in Releases. Or from the terminal:

```bash
# Build + publish v0.9.0 to Releases
gh workflow run build-appimage.yml --repo Aorus22/Win11-Clipboard-Linux --ref main -f tag=v0.9.0

# Build only (artifact, no release): untick publish in the Actions tab
```

Every run uploads the AppImage + `SHA256SUMS.txt` as an Actions artifact
(kept 14 days), so even a build-only run stays downloadable.

### 3. Dev run

```bash
# System deps + Rust (once)
make deps && make rust
source ~/.cargo/env

# Run the GPUI client from source (debug build, logging on)
cargo run --manifest-path gpui-app/Cargo.toml
```

Useful commands: `make lint`, `make format`, `make clean`.

---

## Troubleshooting

<details>
<summary><b>Shortcut (Super+V) isn't working</b></summary>

1. Ensure the app is running: `pgrep -f win11-clipboard-history`
2. If running, try resetting the config:
```bash
rm ~/.config/win11-clipboard-history/setup.json
win11-clipboard-history
```
3. **Conflicts:** GNOME and other DEs often reserve `Super+V`. The app's **Setup Wizard** usually fixes this, but you can manually unbind `Super+V` in your system keyboard settings.

</details>

<details>
<summary><b>Transparency Issues (NVIDIA / AppImage)</b></summary>

If you see a black background or flickering, use the compatibility mode:

```bash
# Force NVIDIA workaround
IS_NVIDIA=1 win11-clipboard-history

# Force AppImage workaround
IS_APPIMAGE=1 win11-clipboard-history
```

</details>

---

## Credits

Forked from [gustavosett/Windows-11-Clipboard-History-For-Linux](https://github.com/gustavosett/Windows-11-Clipboard-History-For-Linux)
— all credit for the original design and implementation goes upstream.
This fork focuses on GPUI development.

Like this project? Give it a star on [GitHub](https://github.com/Aorus22/Win11-Clipboard-Linux).
