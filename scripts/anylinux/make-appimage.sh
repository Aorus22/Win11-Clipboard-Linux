#!/bin/sh
# Build the GPUI frontend and turn it into an Anylinux AppImage with
# quick-sharun: glibc, the loader and every library (GTK, Mesa, Vulkan ICDs,
# xkbcommon, ...) travel inside, so the image runs on any x86_64 Linux with a
# kernel >= 4.x — no per-distro builds. NVIDIA's proprietary driver cannot be
# bundled; it is picked up from the host.
#
# Runs in the Arch container of .github/workflows/build-appimage.yml, after
# scripts/anylinux/get-dependencies.sh. Output: dist/win11-clipboard-history_<ver>_<arch>.AppImage
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"

BIN=win11-clipboard-history-gpui
ARCH=$(uname -m)
VERSION=$(grep -m1 '^version' gpui-app/Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
export ARCH VERSION
export OUTPATH="$ROOT/dist"

echo "Building $BIN $VERSION"
cargo build --release --manifest-path gpui-app/Cargo.toml

# quick-sharun wants the application installed under /usr.
install -Dm755 "gpui-app/target/release/$BIN" "/usr/bin/$BIN"

# Desktop entry: the Exec line resolves through the AppImage's own launcher.
mkdir -p "$ROOT/build"
sed 's|__BINDIR__/||g' "gpui-app/dist/$BIN.desktop" > "$ROOT/build/$BIN.desktop"
export DESKTOP="$ROOT/build/$BIN.desktop"
export ICON="$ROOT/src-tauri/icons/icon.png"
export MAIN_BIN="$BIN"

# GPUI renders with Vulkan; GTK is linked for the tray backend and the theme
# reader. The three helper tools are run by the app (clipboard + paste), so they
# travel inside too.
export DEPLOY_VULKAN=1
export DEPLOY_OPENGL=1
export DEPLOY_GTK=1

quick-sharun "/usr/bin/$BIN" /usr/bin/xdotool /usr/bin/xclip /usr/bin/wl-copy /usr/bin/wl-paste

# Runtime data the app reads from <APPDIR>/share/... (see `assets_dir`).
DATA="$ROOT/AppDir/share/win11-clipboard-history-gpui/assets"
mkdir -p "$DATA"
cp "$ROOT/app-shared/assets/emojis.json" "$DATA/"
cp -a "$ROOT/gpui-app/assets/icons" "$DATA/"
cp "$ROOT/gpui-app/assets/NotoColorEmoji-CBDT.ttf" "$DATA/" 2>/dev/null || true

quick-sharun --make-appimage

# Stable public name: win11-clipboard-history_<version>_<arch>.AppImage
set -- "$OUTPATH"/*.AppImage
[ -f "$1" ] || { echo "no AppImage produced" >&2; exit 1; }
FINAL="$OUTPATH/win11-clipboard-history_${VERSION}_${ARCH}.AppImage"
[ "$1" = "$FINAL" ] || mv "$1" "$FINAL"

# --- Runtime policy: mount for the app's lifetime, no longer ------------------
# uruntime's default unmounts a shared FUSE mount after 5s *without use*
# (URUNTIME_MOUNT=3). This app maps the image once and then sits idle for hours,
# so the mount went away under the running daemon: the app SIGBUSed on the next
# page it had not touched yet, and every later `<AppImage> --toggle` was refused
# ("cannot safely inspect existing target directory; refusing reuse") — Super+V
# dead with nothing on screen to explain it. `1` (unmount once the application
# exits) ties the mount to the process that needs it. Not `0`: that keeps it
# mounted forever and leaves mounts and supervisors behind for one-shot runs such
# as `--version`. Values are single fixed-width digits, so editing them in a
# finished image is documented (uruntime docs/CONFIGURATION.md). Verified on a
# real image: mount gone at 4-5s before, alive past 50s after.
sed -i 's|URUNTIME_MOUNT=[0-9]|URUNTIME_MOUNT=1|' "$FINAL"
grep -aq 'URUNTIME_MOUNT=1' "$FINAL" || { echo "could not pin URUNTIME_MOUNT" >&2; exit 1; }

# DwarFS block cache: the runtime default (1.5G) is far more than this ~47M image
# needs and is allocated with malloc, so under memory pressure the allocation
# fails and dwarfs aborts, taking the mount with it. Embed a modest value in the
# image so it holds for every launch, including ones we do not write ourselves
# (a manual run, an AppImage integration).
printf 'DWARFS_CACHESIZE=64M\n' > "$ROOT/build/runtime.env"
"$FINAL" --appimage-addenvs "$ROOT/build/runtime.env"
grep -aq 'DWARFS_CACHESIZE=64M' "$FINAL" || { echo "could not embed DWARFS_CACHESIZE" >&2; exit 1; }

rm -f "$OUTPATH"/*.zsync
( cd "$OUTPATH" && sha256sum ./*.AppImage | sed 's| \./| |' > SHA256SUMS.txt && cat SHA256SUMS.txt )
