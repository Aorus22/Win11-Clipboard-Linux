# Requirements: Windows 11 Clipboard History — v0.8.0 GPUI Frontend Port

**Defined:** 2026-09-08
**Core Value:** Super+V yang cepat dan cantik di Linux — popup ala Windows 11 yang instan dan familiar.

## v0.8.0 Requirements

Paritas penuh versi Tauri dalam GPUI Rust, tampilan 1:1 identik. React/Tauri tetap hidup berdampingan.

### Clipboard history (CORE)

- [ ] **CORE-01**: User can view clipboard history list (text / rich-text / image) in the GPUI window with layout identical to the React version
- [ ] **CORE-02**: User can search and filter history with the same behavior as the current search
- [ ] **CORE-03**: User can pin and unpin items; pinned items stay on top and survive cleanup
- [ ] **CORE-04**: User can paste the selected item via Enter with the same paste behavior as now
- [ ] **CORE-05**: User can delete a single item and clear history
- [ ] **CORE-06**: User can dismiss the popup via Esc; the window hides without quitting the app

### Pickers (PICK)

- [ ] **PICK-01**: User can switch tabs (clipboard / emoji / kaomoji / symbols) via a tab bar identical to the current one
- [ ] **PICK-02**: User can browse and search emoji and paste the selected emoji
- [ ] **PICK-03**: User can browse kaomoji categories and use custom kaomojis from settings
- [ ] **PICK-04**: User can browse symbols and paste the selected symbol
- [ ] **PICK-05**: User can operate the whole popup via keyboard (arrows / Enter / Esc) like the current app

### Settings & setup (SET)

- [ ] **SET-01**: User can open the settings window and switch theme mode (system / light / dark)
- [ ] **SET-02**: User can adjust dark/light background opacity with live preview
- [ ] **SET-03**: User can adjust UI scale, max history size, and auto-delete interval
- [ ] **SET-04**: User can toggle smart actions, UI polish, and dynamic tray icon, and manage custom kaomojis
- [ ] **SET-05**: New user gets the setup wizard flow (permissions, shortcut registration, autostart) with identical steps

### Window behavior (WIND)

- [ ] **WIND-01**: User sees the popup open frameless at 360×480 following the mouse cursor across monitors
- [ ] **WIND-02**: User sees acrylic blur, rounded corners, and dark/light styling matching the React opacity and glass tokens
- [ ] **WIND-03**: Popup stays always-on-top and hides (never closes) on focus loss or Esc
- [ ] **WIND-04**: User on NVIDIA / AppImage gets a fully opaque fallback without rounded corners, same as now
- [ ] **WIND-05**: User can install and run the GPUI build without disturbing the installed React/Tauri app

### System integration (SYS) — backend reuse

- [ ] **SYS-01**: User can toggle the GPUI window from anywhere via Super+V / Ctrl+Alt+V on X11 and Wayland
- [ ] **SYS-02**: User gets a tray icon with dynamic state and menu (open / settings / quit)
- [ ] **SYS-03**: User can enable autostart that behaves identically to the current app
- [ ] **SYS-04**: App theme follows the system theme via the existing XDG portal listener
- [ ] **SYS-05**: Clipboard monitoring captures text / rich-text / image on X11 and Wayland via the reused backend
- [ ] **SYS-06**: User can run the GPUI build alongside the Tauri build without shortcut, clipboard, or config conflicts (coexistence policy decided in plan-phase)

### Packaging (PKG)

- [ ] **PKG-01**: Maintainer can produce deb / rpm / appimage bundles reusing the existing wrapper, udev rules, and install scripts
- [ ] **PKG-02**: Installer keeps the one-time uinput permission flow unchanged
- [ ] **PKG-03**: GPUI binary and config paths are versioned so they never silently clash with the Tauri build

## Future Requirements

Deferred past v0.8.0. Tracked but not in the current roadmap.

### GIF / providers

- **GIF-01**: User can search and paste GIFs via a sustainable provider (Tenor API is dead; out of scope until a provider exists)

## Out of Scope

| Feature | Reason |
|---------|--------|
| GIF tab in the GPUI port | No sustainable provider since Google killed the Tenor API; stays disabled as in v0.7.1 |
| Removing or replacing the React/Tauri app | Must keep shipping untouched during this milestone (user decision) |
| Rewriting backend clipboard / paste / shortcut logic | Reused as-is to avoid regressions in uinput, X11 injection, and permissions |
| Mobile app | Linux desktop only, same as before |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CORE-01 | Phase 2 | In Progress |
| CORE-02 | Phase 2 | In Progress |
| CORE-03 | Phase 2 | In Progress |
| CORE-04 | Phase 2 | In Progress |
| CORE-05 | Phase 2 | In Progress |
| CORE-06 | Phase 2 | In Progress |
| PICK-01 | Phase 3 | In Progress |
| PICK-02 | Phase 3 | In Progress |
| PICK-03 | Phase 3 | In Progress |
| PICK-04 | Phase 3 | In Progress |
| PICK-05 | Phase 3 | In Progress |
| SET-01 | Phase 4 | Pending |
| SET-02 | Phase 4 | Pending |
| SET-03 | Phase 4 | Pending |
| SET-04 | Phase 4 | Pending |
| SET-05 | Phase 4 | Pending |
| WIND-01 | Phase 2 | In Progress |
| WIND-02 | Phase 2 | In Progress |
| WIND-03 | Phase 2 | In Progress |
| WIND-04 | Phase 2 | In Progress |
| WIND-05 | Phase 1 | Pending |
| SYS-01 | Phase 5 | Pending |
| SYS-02 | Phase 5 | Pending |
| SYS-03 | Phase 5 | Pending |
| SYS-04 | Phase 5 | Pending |
| SYS-05 | Phase 5 | Pending |
| SYS-06 | Phase 1 | Pending |
| PKG-01 | Phase 5 | Pending |
| PKG-02 | Phase 5 | Pending |
| PKG-03 | Phase 1 | Pending |

**Coverage:**
- v0.8.0 requirements: 30 total
- Mapped to phases: 30
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-08*
*Last updated: 2026-09-08 after initial definition*
