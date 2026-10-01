# Docker build environment

Reproducible toolchain for the Win11 Clipboard AppImages. Every AppImage build
in CI goes through this image, so a CI failure is a code failure — never a
"worked on my machine" toolchain difference.

| File | Purpose | Extras on top of the Rust base |
| --- | --- | --- |
| `Dockerfile.build` | Builds both AppImages (Tauri + GPUI) | Node 20, pinned linuxdeploy + appimagetool, WebKitGTK + window/GL/font headers |

The image is **environment-only**: it never `COPY`s the source. The repo is
mounted at `/src` at run time, which keeps the image cacheable across commits
and lets the repo own the build steps (`docker/build-appimage.sh`, plus the
repo's own `scripts/build-gpui-appimage.sh` and `tauri build`).

## Build the AppImages

```bash
docker build -f docker/Dockerfile.build -t winclip-build .
docker run --rm -v "$PWD:/src" -w /src winclip-build bash docker/build-appimage.sh
# -> dist/win11-clipboard-history_*_amd64.AppImage              Tauri frontend
#    dist/win11-clipboard-history_*_x86_64.AppImage             GPUI frontend
#    dist/SHA256SUMS.txt
```

Two AppImages exist because the repo ships two frontends:

| File | Frontend | Stack |
| --- | --- | --- |
| `win11-clipboard-history_*_amd64.AppImage` | Tauri (React) | WebKitGTK system webview |
| `win11-clipboard-history_*_x86_64.AppImage` | GPUI (native Rust) | linuxdeploy-bundled libs, GTK3 stays on the host |

Useful mounts to keep caches warm between runs (all optional):

```bash
docker run --rm -v "$PWD:/src" -w /src \
  -v "$HOME/.cargo/registry:/usr/local/cargo/registry" \
  -v "$HOME/.cargo/git:/usr/local/cargo/git" \
  -v "$HOME/.npm:/root/.npm" \
  winclip-build bash docker/build-appimage.sh
```

Running as the host user instead of root keeps `target/` and `dist/` writable
for your own `cargo` afterwards:

```bash
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/winclip-build \
  -v "$PWD:/src" -w /src winclip-build bash docker/build-appimage.sh
```

## Why Debian bookworm

The AppImage must run on Fedora and Ubuntu alike. Building against bookworm's
glibc 2.36 (older than both) means the produced binary only ever references
symbols every target already exports. The GPU drivers and the X11/Wayland
client libraries are always resolved from the host at run time by SONAME.

## How the AppImage tools get in

`scripts/build-gpui-appimage.sh` fetches linuxdeploy and appimagetool itself,
through `LINUXDEPLOY_URL` / `APPIMAGETOOL_URL`, and runs each one with
`--appimage-extract-and-run` (so no `/dev/fuse` is needed). `Dockerfile.build`
therefore downloads both AppImages into the image and exports those two env vars
as `file:///opt/...` URLs: the repo script is unchanged, no network is needed
during the build, and the tool versions are pinned by the image instead of by
"whatever `continuous` was that day". The image build itself runs each tool's
`--version` as a smoke test. (The Tauri AppImage needs no external helpers —
`tauri build` bundles it natively.)

## What CI does with them

One workflow owns packaging end to end:

| workflow | responsibility | triggers |
| --- | --- | --- |
| `build-appimage.yml` | builder image + both AppImages | every push to `main`, `v*` tags, PRs (artifact only) |

Nothing is published automatically: invoke the workflow manually from the
Actions tab ("Run workflow") with a tag — `latest` for the rolling release, or
`vX.Y.Z` for a versioned one — and the built AppImages land in that GitHub
Release, keeping permanent download URLs that survive merges:

```bash
gh release download latest --repo Aorus22/Win11-Clipboard-Linux --pattern '*.AppImage'
```
