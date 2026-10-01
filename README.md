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

## Installation Options

<details>
<summary><b>AppImage — any distro (recommended)</b></summary>

The AppImage is built by the
[`build-appimage`](https://github.com/Aorus22/Win11-Clipboard-Linux/actions/workflows/build-appimage.yml)
workflow, which only runs manually: open it from the Actions tab with a tag
(`latest` for the rolling release, `vX.Y.Z` for a versioned one) and tick
publish. Versioned tags (e.g. `v0.9.0`) get their own release page with the same assets.

```bash
chmod +x win11-clipboard-history_*.AppImage
sudo setfacl -m u:$USER:rw /dev/uinput  # required for paste simulation
./win11-clipboard-history_*.AppImage
```

</details>

<details>
<summary><b>Build from source</b></summary>

```bash
# 1. Clone
git clone https://github.com/Aorus22/Win11-Clipboard-Linux.git
cd Win11-Clipboard-Linux

# 2. Install dependencies
make deps && make rust && make node
source ~/.cargo/env

# 3. Run in dev mode (hot reload)
make dev

# 4. Or build a production release
make build
```

GPUI frontend only:

```bash
make gpui-appimage   # -> gpui-app/dist/*.AppImage
```

</details>

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

## For Developers

**Tech Stack:** `Rust` + `GPUI` + `Linux`

```bash
# 1. Clone
git clone https://github.com/Aorus22/Win11-Clipboard-Linux.git
cd Win11-Clipboard-Linux

# 2. Install Deps
make deps && make rust && make node
source ~/.cargo/env

# 3. Run Dev Mode
make dev
```

Useful commands: `make lint`, `make format`, `make test`, `make clean`.

---

## Credits

Forked from [gustavosett/Windows-11-Clipboard-History-For-Linux](https://github.com/gustavosett/Windows-11-Clipboard-History-For-Linux)
— all credit for the original design and implementation goes upstream.
This fork focuses on GPUI development.

Like this project? Give it a star on [GitHub](https://github.com/Aorus22/Win11-Clipboard-Linux).
