#!/usr/bin/env bash
# In-container steps for the Win11 Clipboard AppImage (GPUI only).
#
# Split of responsibilities:
#   docker/Dockerfile.build                  -> toolchain + system headers
#   this script                              -> the steps, run against the MOUNTED repo (/src)
#   scripts/build-gpui-appimage.sh           -> the actual packaging (reused as-is)
#
# Usage (from the repo root):
#   docker run --rm -v "$PWD:/src" -w /src winclip-build bash docker/build-appimage.sh
#
# Output:
#   dist/win11-clipboard-history_*_x86_64.AppImage             GPUI frontend
#   dist/SHA256SUMS.txt
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-/src}"
DIST_DIR="${ROOT_DIR}/dist"

cd "${ROOT_DIR}"

# Route rustc through sccache so rebuilds reuse cached crates. The cache dir
# comes from a host mount (CI persists it via actions/cache); without the mount
# sccache just runs uncached — never a failure.
export RUSTC_WRAPPER="${RUSTC_WRAPPER:-sccache}"
if command -v sccache >/dev/null; then
    sccache --zero-stats >/dev/null 2>&1 || true
fi

echo "[1/2] Building GPUI AppImage..."
bash "${ROOT_DIR}/scripts/build-gpui-appimage.sh"
# The helper tools come from the image via the LINUXDEPLOY_URL /
# APPIMAGETOOL_URL env vars baked into Dockerfile.build, so no network is
# needed for this step.

echo "[2/2] Collecting artifacts + checksums..."
mkdir -p "${DIST_DIR}"
cp "${ROOT_DIR}"/gpui-app/dist/*.AppImage "${DIST_DIR}/"
cd "${DIST_DIR}"
sha256sum *.AppImage | tee SHA256SUMS.txt

images=()
for image in *.AppImage; do
    images+=("$image")
done
if [[ "${#images[@]}" -ne 1 ]]; then
    echo "expected 1 GPUI AppImage, found ${#images[@]}: ${images[*]}" >&2
    exit 1
fi

echo
echo "Done. Artifacts:"
ls -la "${DIST_DIR}"/*.AppImage

if command -v sccache >/dev/null; then
    echo
    sccache --show-stats || true
fi
