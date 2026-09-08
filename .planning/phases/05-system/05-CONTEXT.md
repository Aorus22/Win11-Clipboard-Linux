# Phase 5: System integration & packaging - Context

**Gathered:** 2026-09-08
**Status:** Ready for planning
**Mode:** Auto (autonomous — full direction discretion granted)

<domain>
## Phase Boundary

GPUI build menjadi aplikasi background utuh: toggle global, tray, autostart +
hidden-start, theme mengikuti sistem, packaging dasar, bukti koeksistensi, dan
checklist visual untuk manusia. Scope: SYS-01–05, PKG-01–03 + penutup
SYS-06/PKG-03/WIND-05 + HUMAN-CHECKLIST.md. Bukan scope: registrasi DE di luar
GNOME (terdokumentasi), rilis publish (build artefak di mesin dev saja).

</domain>

<decisions>
## Implementation Decisions

### Toggle architecture: single-instance socket + `--toggle` + X11 grab
- First instance holds `$XDG_RUNTIME_DIR/win11-clipboard-gpui.sock`; second invocation
  (`--toggle`, `--settings-open`) sends a word + exits. Background thread → mpsc →
  main poll loop acts (poll task already exists).
- Toggle: visible → `cx.hide()`; hidden → reposition (follow-mouse X11 /
  bottom-center Wayland, same math) + `save_focused_window` (X11) + activate.
- X11: `global-hotkey` 0.8 (Super+V, Ctrl+Alt+V) → same toggle path. Wayland: no grab
  (protocol limit) — DE shortcut Exec = gpui binary `--toggle` is the Wayland path.
- `--background`: start tray-only (no popup). Requires tray (else invisible).

### Tray: `tray-icon` 0.24 + `muda` menu, ikon reuse
- Ikon: `icon-light/dark.png` (dynamic, same picker logic) + `icon.png` fallback,
  via `include_bytes!` + `image` decode → `Icon::from_rgba`.
- Menu: Show Clipboard / Settings / Quit. Tooltip + title updates on theme change.
- GNOME AppIndicator caveat == Tauri build (parity, documented).

### Theme: own zbus portal listener (mirror backend logic)
- `zbus` MatchRule on `org.freedesktop.portal.Desktop` SettingChanged color-scheme
  (copy the exact rule/signal parsing from `theme_manager::listen_for_theme_changes`).
- On change: recompute dark + rebuild tray icon + bump shared version (popup reloads).
- Startup + gsettings probe stays as fallback.

### GNOME shortcut registration (own, gpui-pathed)
- Backend `register_global_shortcut()` prefers the installed Tauri wrapper — MUST NOT
  be reused (would point Super+V at the Tauri app). Own minimal GNOME registrant
  (gsettings custom-keybindings → current_exe + `--toggle`); wizard + settings
  Register buttons use it on GNOME, manual instructions (gpui path) elsewhere.
- Unregister path symmetric (remove only own entries, match by command).

### Packaging: Makefile + desktop entry + docs (no Tauri bundler coupling)
- `make gpui-{build,install,uninstall}`: binary → /usr/local/bin (PREFIX-aware),
  desktop file → applications, icon → hicolor, autostart template reuse.
- udev rule: generic (no binary name) + already installed on dev machine → PKG-02
  satisfied by documentation + live check (`test -r /dev/uinput`).
- PACKAGING.md: deps (wayland/xkb/vulkan — no webkit!), layout, coexistence notes.
- No release-LTO build in milestone (debug proven; release noted as follow-up).

### Requirement completion bar (honest)
- Complete only with live proof: SYS-05 (Phase 2 probe), PKG-02 (generic rule +
  live check), SYS-06/PKG-03/WIND-05 (side-by-side run test this phase),
  SYS-03 (entry content unit-tested + hidden-start smoke), SYS-01 Wayland path
  (--toggle IPC live), SYS-02 (tray registers without panic + menu actions smoke),
  SYS-04 (gsettings flip → theme change observed in log).
- X11-only paths (global-hotkey grab, cursor-follow) untestable on this Wayland
  machine → stay In Progress with reason. Visual pixel requirements stay In Progress
  pending HUMAN-CHECKLIST.md.

</decisions>

<code_context>
## Existing Code Insights

- Tray icons: `icon-light.png` (used when dark), `icon-dark.png` (when light),
  `icon.png` fallback — `get_icon_bytes(enable_dynamic, is_dark)`, same files reused.
- Portal listener: zbus `Connection::session`, MatchRule on Settings SettingChanged,
  `color-scheme` key parse (mirror exact code from theme_manager.rs:292+).
- DE registration: backend prefers Tauri wrapper (verified) → own GNOME path needed.
- Main poll task (300ms) is the single place to act on IPC/menu/hotkey signals.
- udev rule has no binary references; dev machine already has it (installer ran).

</code>

<specifics>
## Specific Ideas

- Verifikasi: `cargo test` (socket IPC round-trip? single-instance guard unit — careful:
  tests share runtime dir; use temp socket path via env override) + live: tray-only
  start, `--toggle` flips visibility (observe via... headless visibility check —
  window list via `wlr`? GNOME: use `gdbus`? Simplest observable: app log lines on
  toggle (temp probe) + process stays single (second `--toggle` exits fast).
- Theme flip: `gsettings set ... color-scheme prefer-dark/light` → probe log observes.
- Side-by-side: run Tauri bin + gpui bin, assert both alive + distinct config dirs
  + distinct app-ids (static) → flip SYS-06/PKG-03/WIND-05 to Complete.
- HUMAN-CHECKLIST.md: dark+light side-by-side steps for popup/pickers/settings/wizard.

</specifics>

<deferred>
## Deferred Ideas

- Non-GNOME DE auto-registration for the GPUI binary (manual path documented).
- Release LTO build + publish artifacts.
- Grid virtualization / Fuse-ranking follow-ups (carried from Phase 3).
- Full settings keyboard Tab-order.

</deferred>
