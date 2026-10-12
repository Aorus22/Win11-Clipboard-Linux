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
rm -f "$OUTPATH"/*.zsync
( cd "$OUTPATH" && sha256sum ./*.AppImage | sed 's| \./| |' > SHA256SUMS.txt && cat SHA256SUMS.txt )
