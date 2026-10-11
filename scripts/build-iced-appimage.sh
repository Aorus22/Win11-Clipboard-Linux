#!/usr/bin/env bash
# Build a portable AppImage for the iced / libcosmic frontend (win11-clipboard-history-iced).
#
#   ./scripts/build-iced-appimage.sh              # build dist/*.AppImage
#   ./scripts/build-iced-appimage.sh --install    # build, then register for this user
#   ./scripts/build-iced-appimage.sh --clean      # drop the staged AppDir
#
# Layout of the produced AppDir:
#   usr/bin/win11-clipboard-history-iced
#   usr/share/applications/win11-clipboard-history-iced.desktop
#   usr/share/icons/hicolor/{128x128,256x256,scalable}/apps/win11-clipboard-history-iced.*
#
# Bundling policy: linuxdeploy collects the binary's shared-library
# dependencies. libvulkan and the GPU drivers stay on the host on purpose —
# bundling them breaks driver matching on most distros. The tray is pure D-Bus
# (StatusNotifierItem), so no GTK / appindicator libraries are needed.
#
# Both helper tools are downloaded (and cached) as AppImages and run with
# `--appimage-extract-and-run`, so no FUSE/libfuse2 is required.
# Set APPIMAGE_RUNTIME_FILE to a local type2 runtime for offline packaging.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICED_DIR="$ROOT/iced-app"
BIN_NAME="win11-clipboard-history-iced"
APP_DIR="$ICED_DIR/target/appimage/$BIN_NAME.AppDir"
DIST_DIR="$ICED_DIR/dist"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/win11-clipboard-appimage"

LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage}"
APPIMAGETOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"

INSTALL=0
for arg in "$@"; do
    case "$arg" in
        --install) INSTALL=1 ;;
        --clean)
            echo "Removing $APP_DIR"
            rm -rf "$APP_DIR"
            exit 0
            ;;
        -h|--help)
            sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 2
            ;;
    esac
done

log() { printf '\033[0;36m[appimage]\033[0m %s\n' "$1"; }
fail() { printf '\033[0;31m[appimage] %s\033[0m\n' "$1" >&2; exit 1; }

command -v cargo >/dev/null || fail "cargo not found (install Rust: https://rustup.rs)"
command -v mksquashfs >/dev/null || fail "mksquashfs not found (install squashfs-tools)"

VERSION="$(grep -m1 '^version' "$ICED_DIR/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
ARCH="$(uname -m)"
[[ "$ARCH" == "x86_64" ]] || fail "only x86_64 is supported by the bundled tools (got $ARCH)"

# The artifact keeps the `-iced` suffix so it never overwrites the GPUI build's
# `win11-clipboard-history_<version>_<arch>.AppImage` (both can be installed).
OUT_NAME="win11-clipboard-history-iced_${VERSION}_${ARCH}.AppImage"

# --- Helper tools -----------------------------------------------------------

fetch_tool() {
    local url="$1" dest="$2"
    if [[ -x "$dest" ]]; then
        return
    fi
    log "Downloading $(basename "$dest")"
    mkdir -p "$(dirname "$dest")"
    curl -fL --retry 3 -o "$dest" "$url" || fail "download failed: $url"
    chmod +x "$dest"
}

run_appimage_tool() {
    "$1" --appimage-extract-and-run "${@:2}"
}

# --- Build ------------------------------------------------------------------

log "Building release binary (cargo build --release)"
cargo build --release --manifest-path "$ICED_DIR/Cargo.toml"
BIN="$ICED_DIR/target/release/$BIN_NAME"
[[ -x "$BIN" ]] || fail "release binary missing: $BIN"

log "Staging AppDir"
rm -rf "$APP_DIR"
install -Dm755 "$BIN" "$APP_DIR/usr/bin/$BIN_NAME"

# Everything (emoji dataset, icons, the colour-emoji font) is embedded in the binary.

# Desktop entry: same template as `make gpui-install`, with the prefix dropped
# so Exec resolves through AppRun inside the mounted image.
mkdir -p "$APP_DIR/usr/share/applications"
sed 's|__BINDIR__/||g' "$ICED_DIR/dist/$BIN_NAME.desktop" \
    > "$APP_DIR/usr/share/applications/$BIN_NAME.desktop"
chmod 644 "$APP_DIR/usr/share/applications/$BIN_NAME.desktop"

ICON_BASE="$APP_DIR/usr/share/icons/hicolor"
install -Dm644 "$ROOT/src-tauri/icons/128x128.png" "$ICON_BASE/128x128/apps/$BIN_NAME.png"
install -Dm644 "$ROOT/src-tauri/icons/icon.png" "$ICON_BASE/256x256/apps/$BIN_NAME.png"
install -Dm644 "$ROOT/src-tauri/icons/icon.svg" "$ICON_BASE/scalable/apps/$BIN_NAME.svg"
cp "$ICON_BASE/256x256/apps/$BIN_NAME.png" "$APP_DIR/.DirIcon"

fetch_tool "$LINUXDEPLOY_URL" "$CACHE_DIR/linuxdeploy-$ARCH.AppImage"
fetch_tool "$APPIMAGETOOL_URL" "$CACHE_DIR/appimagetool-$ARCH.AppImage"

log "Deploying shared-library dependencies (linuxdeploy)"
LD_ARGS=(
    --appdir "$APP_DIR"
    --executable "$APP_DIR/usr/bin/$BIN_NAME"
    --desktop-file "$APP_DIR/usr/share/applications/$BIN_NAME.desktop"
    --icon-file "$ICON_BASE/256x256/apps/$BIN_NAME.png"
)
run_appimage_tool "$CACHE_DIR/linuxdeploy-$ARCH.AppImage" "${LD_ARGS[@]}"

mkdir -p "$DIST_DIR"
OUT="$DIST_DIR/$OUT_NAME"

log "Packing $OUT_NAME (appimagetool)"
PACK_ARGS=()
if [[ -n "${APPIMAGE_RUNTIME_FILE:-}" ]]; then
    [[ -f "$APPIMAGE_RUNTIME_FILE" ]] || fail "runtime missing: $APPIMAGE_RUNTIME_FILE"
    PACK_ARGS+=(--runtime-file "$APPIMAGE_RUNTIME_FILE")
fi
ARCH="$ARCH" VERSION="$VERSION" \
    run_appimage_tool "$CACHE_DIR/appimagetool-$ARCH.AppImage" "${PACK_ARGS[@]}" "$APP_DIR" "$OUT"
chmod +x "$OUT"

log "Done: $OUT"
du -h "$OUT" | awk '{print "     size: " $1}'

# --- Optional user-level install -------------------------------------------

if [[ "$INSTALL" == "1" ]]; then
    BIN_DIR="$HOME/Applications"
    APP_DIR_USER="$HOME/.local/share/applications"
    ICON_HOME="$HOME/.local/share/icons/hicolor"

    log "Installing AppImage for this user"
    mkdir -p "$BIN_DIR" "$APP_DIR_USER"
    install -Dm755 "$OUT" "$BIN_DIR/$OUT_NAME"
    INSTALLED_APPIMAGE="$BIN_DIR/$OUT_NAME"

    # Absolute Exec so the menu entry and its action work from anywhere.
    sed "s|__BINDIR__/$BIN_NAME|$INSTALLED_APPIMAGE|g" \
        "$ICED_DIR/dist/$BIN_NAME.desktop" \
        > "$APP_DIR_USER/$BIN_NAME.desktop"
    install -Dm644 "$ROOT/src-tauri/icons/128x128.png" \
        "$ICON_HOME/128x128/apps/$BIN_NAME.png"
    install -Dm644 "$ROOT/src-tauri/icons/icon.png" \
        "$ICON_HOME/256x256/apps/$BIN_NAME.png"
    install -Dm644 "$ROOT/src-tauri/icons/icon.svg" \
        "$ICON_HOME/scalable/apps/$BIN_NAME.svg"

    update-desktop-database "$APP_DIR_USER" 2>/dev/null || true
    gtk-update-icon-cache -f -t "$ICON_HOME" 2>/dev/null || true

    # No autostart entry here: the setup wizard / Settings offers it, so installing
    # this build never starts a second clipboard manager at login by itself.

    log "Installed: $INSTALLED_APPIMAGE"
fi
