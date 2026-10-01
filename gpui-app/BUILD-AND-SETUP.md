# Build, Install & Setup — GPUI build (step by step)

A hands-on walkthrough for compiling the GPUI frontend in release mode,
installing it, and completing first-run setup. For architecture/packaging
reference details (coexistence map, AppDir layout, Wayland notes), see
[PACKAGING.md](PACKAGING.md).

Tested on Fedora 44 · Rust 1.98 · gpui 0.2.2 (pinned via `gpui-app/Cargo.lock`).

---

## 1. Prerequisites

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustc --version   # 1.95+ verified; older stable may fail on gpui
```

### System packages

The gpui app reuses the Tauri crate as a library (`src-tauri/`), so a few
"frontend" libs are needed at link time even though no webview runs.

**Fedora:**

```bash
sudo dnf install gtk3-devel libayatana-appindicator-gtk3-devel \
    libxkbcommon-devel libxkbcommon-x11-devel libxcb-devel wayland-devel \
    fontconfig-devel vulkan-loader libxdo-devel openssl-devel pkg-config
```

| Package | Why |
|---|---|
| `gtk3-devel`, `libayatana-appindicator-gtk3-devel` | tray icon (StatusNotifier) |
| `libxkbcommon-x11-devel` | gpui X11 backend — **missing this fails the release *link*, not `cargo check`** |
| `libxdo-devel` | tray/`muda` hotkeys — same link-time surprise |
| `vulkan-loader` | gpui renders via Vulkan (note: Fedora has no `vulkan-icd-loader` package) |
| `openssl-devel`, `pkg-config` | shared backend crates |

**Debian/Ubuntu equivalents:** `libgtk-3-dev libayatana-appindicator3-dev
libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libwayland-dev
libfontconfig-dev libvulkan-dev libxdo-dev libssl-dev pkg-config`.

**Node/npm:** not needed at runtime (the GPUI binary has no webview) but the
shared backend crate still references Tauri at build time, so have Node
available (see `make node`).

---

## 2. Build (release)

From the repo root:

```bash
make gpui-build
```

or directly:

```bash
cargo build --release --manifest-path gpui-app/Cargo.toml
```

- Binary lands in `gpui-app/target/release/win11-clipboard-history-gpui`.
- First build takes several minutes (gpui is large); later ones are incremental.
- Quick sanity check without producing a binary: `cargo check --manifest-path
  gpui-app/Cargo.toml --all-targets`.
- Run the unit tests: `cargo test --manifest-path gpui-app/Cargo.toml`.

Smoke-test the binary before installing:

```bash
./gpui-app/target/release/win11-clipboard-history-gpui --version
```

---

## 3. Install

Pick **one** of the two options. Both coexist safely with the Tauri build —
names, ids, config dirs and sockets are all distinct.

### Option A — AppImage (recommended, no sudo)

```bash
make gpui-appimage            # → gpui-app/dist/win11-clipboard-history_<ver>_x86_64.AppImage
make gpui-appimage-install    # build + register for the current user
```

or drive the script directly:

```bash
./scripts/build-gpui-appimage.sh --install
```

What `--install` does:

| Item | Location |
|---|---|
| AppImage | `~/Applications/win11-clipboard-history_<ver>_x86_64.AppImage` |
| Desktop entry | `~/.local/share/applications/win11-clipboard-history-gpui.desktop` (absolute `Exec`, so the Settings action works) |
| Icons | `~/.local/share/icons/hicolor/...` |

No FUSE is needed to *build* (the bundled `linuxdeploy`/`appimagetool` run
with `--appimage-extract-and-run`); to *run* the result, either FUSE or the
same extract-and-run trick works. To update later: rebuild, then overwrite the
AppImage in `~/Applications` and restart the app.

### Option B — system install (PREFIX-aware)

```bash
sudo make gpui-install                # → /usr/local
sudo make gpui-install PREFIX=/usr    # distro-style layout
```

This installs the bare binary, desktop entry, icons, and the shared
`/etc/udev/rules.d/99-win11-clipboard-input.rules` udev rule (idempotent with
the Tauri build). Uninstall with `sudo make gpui-uninstall` (keeps user data).

---

## 4. First-run setup

Launch the app:

```bash
~/Applications/win11-clipboard-history_*_x86_64.AppImage
```

The 5-step setup wizard opens automatically:

1. **Permissions** — paste injection needs write access to `/dev/uinput`.
   Use the wizard's *Fix Permissions* (`pkexec setfacl`), or make it durable
   with the udev rule (`TAG+="uaccess"`) or `input` group membership.
   Check with: `getfacl /dev/uinput`.
2. **Shortcut (GNOME)** — the wizard registers `<executable> --toggle` on
   Super+V via gsettings, pointing at the AppImage path (never the transient
   `/tmp/.mount_...` path), so the binding survives reboots. Headless variant
   for scripts: `--register-shortcuts` / `--unregister-shortcuts`.
   Verify:

   ```bash
   gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings
   ```
3. **Autostart** — optional; writes
   `~/.config/autostart/win11-clipboard-history-gpui.desktop` with
   `--background` (tray-only at login).
4. Done → the popup closes, the app stays resident in the tray.

### Tray icon on GNOME

GNOME needs the AppIndicator extension, and it must be *enabled* (installed ≠
enabled):

```bash
gnome-extensions info appindicatorsupport@rgcjonas.gmail.com
gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com
```

(Enable once, then log out/in if it doesn't appear.) Left-click opens the
popup; right-click gives Show / Settings / Quit.

### Verify end to end

```bash
# popup toggles and holds window type NOTIFICATION (hidden from dock):
xdotool key super+v
```

Expected: clipboard popup appears near the cursor, nothing new in the dock,
Super+V again (or click elsewhere) hides it.

---

## 5. Everyday operations & troubleshooting

| Task / Symptom | Fix |
|---|---|
| Rebuild after code changes | `make gpui-build` (or `make gpui-appimage-install` for the AppImage flow) — restart the running instance first, the file can't be overwritten while it runs |
| Link error `-lxdo` or `-lxkbcommon-x11` | install `libxdo-devel` / `libxkbcommon-x11-devel` (section 1) |
| `cargo check` passes, release *link* fails | same as above — only the link step pulls those libs |
| Tray icon missing | AppIndicator extension not enabled (section 4); also check `gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems` |
| App appears in the dock | you forced Wayland (`WIN11_CLIPBOARD_ALLOW_WAYLAND=1`) or no XWayland is available — the dock-hidden behaviour relies on the X11 backend |
| Popup won't close on outside click / drag misbehaves | inspect `~/.config/win11-clipboard-history-gpui/drag-debug.log` (disable with `WIN11_CLIPBOARD_DRAG_LOG=0`) |
| Settings window: transparency disabled | expected on NVIDIA + AppImage (WebKit DMABUF workaround); the Settings window says so |
| Reset everything (fresh wizard) | `rm -rf ~/.config/win11-clipboard-history-gpui/` |
| Uninstall (AppImage) | remove `~/Applications/win11-clipboard-history_*.AppImage`, `~/.local/share/applications/win11-clipboard-history-gpui.desktop`, hicolor icons; unbind Super+V via Settings → keyboard |

### Themes (System / Light / Dark / Desktop-GTK)

Settings → Appearance shows four cards; the choice is stored as `theme_mode`
(`system` / `dark` / `light` / `gtk`) in `user_settings.json`.

| Card | Colors come from |
|---|---|
| System | Win11 palette, dark or light following the XDG portal `color-scheme` |
| Light / Dark | Win11 palette, pinned |
| Desktop (GTK) | the active GTK/GNOME theme — full palette, not just dark/light |

The Desktop card merges two sources, in this order of authority:

1. **Your own GTK config** — `colors.css` / `gtk.css` in `gtk-4.0` (then `gtk-3.0`).
   On GNOME the `gtk-theme` setting is only half the story: libadwaita 1.6+ apps
   take their palette from `:root` custom properties in `~/.config/gtk-4.0/gtk.css`
   (what palette tools such as Rewaita write), plus `@define-color` entries in the
   `colors.css` next to it. If your `gtk-theme` still says `WhiteSur-Dark` while
   every other app is Tokyo Night, it is these files that win.
2. **The GTK3 style engine** — `gtk::StyleContext::lookup_color`
   (`theme_bg_color`, `theme_base_color`, `theme_fg_color`,
   `theme_selected_bg_color`, `borders`, `error_color`, `warning_color`,
   `success_color`, `insensitive_fg_color`, …), the fallback for users with no
   user CSS.

Either way the result is mapped onto every app token, so surfaces, text,
borders, accents and the semantic error/warning/success colors all follow the
desktop palette. A caption under the cards names the detected GTK theme.

- Changes are picked up live: the portal `color-scheme`/`accent-color` signal,
  plus a ~1 s check of the GTK theme name **and the mtimes of your CSS files**, so
  editing `gtk.css` repaints the app without a restart.
- GTK unavailable (headless session, no GTK theme, no tray)? The card falls back
  to the system light/dark preference and the grid says "GTK theme not detected".
- System / Light / Dark are untouched by this: their colors are Win11 literals
  (`theme::hex`) and are pinned by parity tests (`cargo test --manifest-path
  gpui-app/Cargo.toml`).
- Fonts, cursor and icon themes still come from the desktop independently — only
  colors are affected.

### Window chrome (rounded corners + outline)

The Settings and Setup windows are created with an ARGB surface
(`WindowBackgroundAppearance::Transparent`) and their root element is rounded by
`theme::RADIUS_WINDOW` with a 1 px `border` outline, exactly like the popup. That
makes them read as native GNOME windows (rounded, outlined) instead of square
slabs that melt into the wallpaper, in every theme mode — Win11 included, whose
own windows are rounded too.

Chromeless sessions (gpui asked for client-side decorations) get the
minimise/maximise/close buttons from `ui::titlebar::render_controls`, drawn
absolutely into the top-right *corner* of the Settings header and over the
wizard's corner. There is no separate 36 px title strip: the window opens on the
"Personalization" heading, the subtitle keeps the full width, and the save pill
floats in the same corner row so nothing squeezes the text.

Shadow: X clients get theirs from the compositor, and mutter paints it around the
window *rectangle* — with transparent rounded corners that square shadow shows
through as a dark wedge behind each corner. So the windows do what GTK CSD does:

- the root is wrapped in a `theme::WINDOW_SHADOW_MARGIN` (16 px) transparent
  frame and paints its own shadow into it (`theme::window_shadow()`, a gpui box
  shadow that follows the rounded corners);
- `window_drag::set_frame_extents` sets `_GTK_FRAME_EXTENTS` on the X window
  (found by title, retried in the background because the property may land after
  the map). Mutter then treats the inset frame as the window body and drops its
  own square shadow.

`_GTK_FRAME_EXTENTS` is in **device pixels** (GTK scales it too), so the logical
margin is multiplied by the window's scale factor. Mutter also *grows* the client
by the advertised extents (verified live: extents `e` ⇒ client = requested +
2e), which is where the margin comes from — so `centered_options` asks for the
card size alone and the painted card lands exactly on the frame. Anything that
maps window coordinates to view-local ones (the sliders) subtracts the margin.
The popup needs none of this: it is an override-redirect window, so no
compositor shadow or frame exists around it — it only shares the 1 px outline.

### Runtime env vars

| Var | Effect |
|---|---|
| `WIN11_CLIPBOARD_DRAG_LOG=0` | disable the drag trace log |
| `WIN11_CLIPBOARD_ALLOW_WAYLAND=1` | keep the native Wayland backend (popup shows in dock while open) |

### Useful CLI flags

| Flag | Effect |
|---|---|
| `--toggle` | open/close the popup (what Super+V runs) |
| `--background` | start tray-only (used by autostart) |
| `--settings` | open Settings on startup |
| `--settings-open` | tell the running instance to open Settings, then exit |
| `--setup` | force the setup wizard even if not first run |
| `--quit` | tell the running instance to quit, then exit |
| `--version` / `-v` | print version |
| `--register-shortcuts` / `--unregister-shortcuts` | manage GNOME keybindings headlessly |

### Files created at runtime

| Path | Content |
|---|---|
| `~/.config/win11-clipboard-history-gpui/` | settings, history, first-run marker, UI state |
| `~/.config/win11-clipboard-history-gpui/drag-debug.log` | drag/focus trace (reset per launch) |
| `~/.config/autostart/win11-clipboard-history-gpui.desktop` | optional autostart |
| `$XDG_RUNTIME_DIR/win11-clipboard-gpui.sock` | single-instance socket |
