# Plan: Phase 1 — Foundation & coexistence

**Phase:** 1 | **Requirements:** SYS-06, PKG-03, WIND-05
**Mode:** inline (autonomous, no subagents — billing)

## Goal

`gpui-app/` scaffold compiles and opens a transparent frameless PopUp window (360×480) on the dev
machine; stack + coexistence decisions locked and recorded.

## Tasks (atomic commits)

### T1 — Scaffold `gpui-app` crate
- `gpui-app/Cargo.toml`: package `win11-clipboard-history-gpui`, `gpui = "=0.2.2"`,
  edition 2021, rust-version 1.77 (match backend).
- `gpui-app/src/main.rs`: `Application::new().run`, open PopUp window 360×480,
  `titlebar: None`, `WindowBackgroundAppearance::Transparent`, `app_id`
  `dev.gustavosett.clipboard-history-gpui`, root view renders placeholder
  (app name + version + "GPUI port — Phase 1").
- Verify: `cargo check -p` passes inside `gpui-app/`.
- Commit: `feat(gpui): scaffold gpui-app with transparent popup window`

### T2 — Backend reuse audit doc
- `gpui-app/BACKEND_REUSE.md`: per-module verdict from CONTEXT audit —
  Tauri-free (direct path-dep reuse later) vs needs adapter
  (config_manager monitor types, theme_manager AppHandle→channel, command attrs).
- No code changes to `src-tauri/`.
- Verify: doc lists all 18 modules with verdict.
- Commit: `docs(gpui): backend reuse audit per module`

### T3 — Lock decisions in PROJECT.md
- Append Phase 1 outcomes to Key Decisions table (gpui 0.2.2, tray-icon/global-hotkey plan,
  app-id/binary/config paths, pinning policy).
- Verify: `grep` decisions present.
- Commit: `docs: lock phase 1 stack + coexistence decisions`

### T4 — Runtime smoke test
- `cargo build` then run binary with timeout under the GNOME Wayland session;
  killed-by-timeout (still running, no panic) = pass.
- Verify: exit path documented (timeout kill vs immediate non-zero).
- No commit (verification only; record result in phase summary).

## Verification (phase gate)

1. `cargo check` clean in `gpui-app/` (T1).
2. BACKEND_REUSE.md covers all 18 modules (T2).
3. PROJECT.md decisions recorded (T3).
4. Binary stays alive ≥5s without panic on GNOME Wayland (T4).
5. `git status` clean except intended files; `src/` + `src-tauri/` untouched
   (`git diff --stat` shows only `gpui-app/` + `.planning/`).

## Risks

- Missing Linux system headers at link time → install via pacman (wayland etc. present).
- GPUI Wayland backend needs running compositor → dev session has one; CI/headless out of scope.
- First `cargo build` downloads ~hundreds of crates → network confirmed reachable.
