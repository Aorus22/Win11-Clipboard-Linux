#!/bin/sh
# Build + bundling dependencies for the Anylinux AppImage (Arch Linux container,
# see .github/workflows/build-appimage.yml). Arch is the only supported build
# base for quick-sharun: the AppImage carries its own glibc, so the build host
# decides what ends up inside it.
set -eu

echo "Installing package dependencies..."
echo "---------------------------------------------------------------"
pacman -Syu --noconfirm --needed \
    base-devel git pkgconf rust clang openssl \
    gtk3 libayatana-appindicator librsvg \
    fontconfig freetype2 \
    libx11 libxcb libxkbcommon libxkbcommon-x11 wayland \
    vulkan-icd-loader vulkan-headers \
    xdotool xclip wl-clipboard \
    xorg-xwayland

echo "Installing debloated packages (Mesa, Vulkan drivers)..."
echo "---------------------------------------------------------------"
# The packages come from GitHub releases, which now and then answers 500: retry.
for attempt in 1 2 3 4; do
    get-debloated-pkgs --add-common --prefer-nano && break
    [ "$attempt" -lt 4 ] || { echo "debloated packages unavailable" >&2; exit 1; }
    echo "retrying in 20 s (attempt $attempt failed)..."
    sleep 20
done
