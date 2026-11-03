#!/usr/bin/env bash
# ==============================================================================
# Production Release Build Script for AirMedia (Linux & macOS)
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

# Support custom or auto-detected version
VERSION="${1:-${RELEASE_VERSION:-}}"
if [ -z "${VERSION}" ]; then
    VERSION=$(grep -m1 '^version = ' Cargo.toml | cut -d '"' -f2 || echo "0.1.0")
fi
VERSION="${VERSION#v}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
if [ "${OS}" = "darwin" ]; then
    OS="macos"
fi
ARCH="$(uname -m)"

echo "=== AirMedia Production Release Build (${OS}-${ARCH} v${VERSION}) ==="

# 1. Verify Rust Toolchain
echo "[1/5] Verifying toolchain..."
rustc --version
cargo --version

# 2. Run Test Suite
echo "[2/5] Running automated tests..."
if [ "${SKIP_TESTS:-false}" != "true" ]; then
    cargo test --quiet
else
    echo "Skipping tests as requested."
fi

# 3. Build Optimized Binary
echo "[3/5] Building release binary..."
TARGET_ARG=""
BINARY_PATH="target/release/airmedia"
if [ -n "${TARGET:-}" ]; then
    TARGET_ARG="--target ${TARGET}"
    BINARY_PATH="target/${TARGET}/release/airmedia"
fi
cargo build --release ${TARGET_ARG}

# 4. Prepare Distribution Directory
PACKAGE_NAME="airmedia-${OS}-${ARCH}"
if [ -n "${PACKAGE_SUFFIX:-}" ]; then
    PACKAGE_NAME="airmedia-${PACKAGE_SUFFIX}"
fi
DIST_DIR="${PROJECT_ROOT}/dist/${PACKAGE_NAME}"
rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}"

echo "[4/5] Staging distribution files in ${DIST_DIR}..."

# Setup standard Linux hierarchy and direct binary
mkdir -p "${DIST_DIR}/bin"
mkdir -p "${DIST_DIR}/share/applications"
mkdir -p "${DIST_DIR}/share/icons/hicolor/256x256/apps"

cp "${BINARY_PATH}" "${DIST_DIR}/bin/airmedia"
cp "${BINARY_PATH}" "${DIST_DIR}/airmedia" # Top-level compatibility
chmod +x "${DIST_DIR}/bin/airmedia" "${DIST_DIR}/airmedia"

# Desktop integration
if [ -f "${PROJECT_ROOT}/assets/linux/airmedia.desktop" ]; then
    cp "${PROJECT_ROOT}/assets/linux/airmedia.desktop" "${DIST_DIR}/share/applications/airmedia.desktop"
    cp "${PROJECT_ROOT}/assets/linux/airmedia.desktop" "${DIST_DIR}/airmedia.desktop"
fi

if [ -f "${PROJECT_ROOT}/assets/linux/airmedia.png" ]; then
    cp "${PROJECT_ROOT}/assets/linux/airmedia.png" "${DIST_DIR}/share/icons/hicolor/256x256/apps/airmedia.png"
    cp "${PROJECT_ROOT}/assets/linux/airmedia.png" "${DIST_DIR}/airmedia.png"
fi

# Linux installer helper
if [ -f "${PROJECT_ROOT}/scripts/install_linux.sh" ]; then
    cp "${PROJECT_ROOT}/scripts/install_linux.sh" "${DIST_DIR}/install.sh"
    chmod +x "${DIST_DIR}/install.sh"
fi

cp "README.md" "${DIST_DIR}/README.md"
if [ -f "LICENSE" ]; then
    cp "LICENSE" "${DIST_DIR}/LICENSE"
fi

# 5. Create Tarball Package & Checksum
echo "[5/5] Creating release tarball and checksum..."
mkdir -p "${PROJECT_ROOT}/dist"
TAR_FILE="airmedia-v${VERSION}-${OS}-${ARCH}.tar.gz"
if [ -n "${PACKAGE_SUFFIX:-}" ]; then
    TAR_FILE="airmedia-v${VERSION}-${PACKAGE_SUFFIX}.tar.gz"
fi
TAR_PATH="${PROJECT_ROOT}/dist/${TAR_FILE}"
tar -czf "${TAR_PATH}" -C "${PROJECT_ROOT}/dist" "${PACKAGE_NAME}"

# Generate SHA256 checksum
cd "${PROJECT_ROOT}/dist"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${TAR_FILE}" > "${TAR_FILE}.sha256"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${TAR_FILE}" > "${TAR_FILE}.sha256"
fi

echo "=== Build Succeeded! ==="
echo "Tarball:  ${TAR_PATH}"
echo "Checksum: $(cat "${TAR_FILE}.sha256" 2>/dev/null || echo 'N/A')"
ls -lh "${TAR_PATH}"
