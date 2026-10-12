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
get-debloated-pkgs --add-common --prefer-nano
