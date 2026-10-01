#!/usr/bin/env bash
# In-container steps for the Win11 Clipboard AppImages.
#
# Split of responsibilities:
#   docker/Dockerfile.build                  -> toolchain + system headers
#   this script                              -> the steps, run against the MOUNTED repo (/src)
#   scripts/build-gpui-appimage.sh           -> the GPUI packaging (reused as-is)
#   `tauri build` (+ tauri.conf.json)        -> the Tauri packaging (reused as-is)
#
# Usage (from the repo root):
#   docker run --rm -v "$PWD:/src" -w /src winclip-build bash docker/build-appimage.sh
#
# Output (one file per frontend, plus checksums):
#   dist/win11-clipboard-history_*_amd64.AppImage          Tauri frontend
#   dist/win11-clipboard-history-gpui_*_x86_64.AppImage    GPUI frontend
#   dist/SHA256SUMS.txt
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-/src}"
DIST_DIR="${ROOT_DIR}/dist"

cd "${ROOT_DIR}"

# The Tauri build runs its own beforeBuildCommand (`npm run build`: tsc + vite),
# so installing the frontend deps first is the only setup needed here.
echo "[1/4] Installing frontend dependencies (npm ci)..."
npm ci

echo "[2/4] Building Tauri AppImage (tauri build --bundles appimage)..."
npm run tauri -- build --bundles appimage

echo "[3/4] Building GPUI AppImage..."
bash "${ROOT_DIR}/scripts/build-gpui-appimage.sh"
# The helper tools come from the image via the LINUXDEPLOY_URL /
# APPIMAGETOOL_URL env vars baked into Dockerfile.build, so no network is
# needed for this step.

echo "[4/4] Collecting artifacts + checksums..."
mkdir -p "${DIST_DIR}"
cp "${ROOT_DIR}"/src-tauri/target/release/bundle/appimage/*.AppImage "${DIST_DIR}/"
cp "${ROOT_DIR}"/gpui-app/dist/*.AppImage "${DIST_DIR}/"
cd "${DIST_DIR}"
sha256sum *.AppImage | tee SHA256SUMS.txt

images=()
for image in *.AppImage; do
    images+=("$image")
done
if [[ "${#images[@]}" -ne 2 ]]; then
    echo "expected 2 AppImages (Tauri + GPUI), found ${#images[@]}: ${images[*]}" >&2
    exit 1
fi

echo
echo "Done. Artifacts:"
ls -la "${DIST_DIR}"/*.AppImage
