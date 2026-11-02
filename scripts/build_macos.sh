#!/usr/bin/env bash
# ==============================================================================
# Production Release Build Script for AirMedia on macOS
# Compiles Apple Silicon (arm64), Intel (x86_64), and Universal macOS binaries.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

VERSION="${1:-${RELEASE_VERSION:-}}"
if [ -z "${VERSION}" ]; then
    VERSION=$(grep -m1 '^version = ' Cargo.toml | cut -d '"' -f2 || echo "0.1.0")
fi
VERSION="${VERSION#v}"

echo "=========================================================="
echo "    AirMedia - Production Release Build (macOS v${VERSION})   "
echo "=========================================================="

# 1. Verify Rust Toolchain and add targets
echo "[1/6] Verifying toolchain and macOS targets..."
rustup target add aarch64-apple-darwin x86_64-apple-darwin 2>/dev/null || true
rustc --version
cargo --version

# 2. Run Test Suite
echo "[2/6] Running automated tests..."
if [ "${SKIP_TESTS:-false}" != "true" ]; then
    cargo test --quiet
else
    echo "Skipping tests as requested."
fi

# 3. Build Optimized Binaries for both ARM64 and x86_64
echo "[3/6] Compiling release binary for Apple Silicon (aarch64)..."
cargo build --release --target aarch64-apple-darwin

echo "[3/6] Compiling release binary for Intel (x86_64)..."
cargo build --release --target x86_64-apple-darwin

# 4. Create Universal Binary using lipo
echo "[4/6] Creating macOS Universal Binary..."
UNIVERSAL_DIR="${PROJECT_ROOT}/target/universal/release"
mkdir -p "${UNIVERSAL_DIR}"
lipo -create \
    -output "${UNIVERSAL_DIR}/airmedia" \
    "${PROJECT_ROOT}/target/aarch64-apple-darwin/release/airmedia" \
    "${PROJECT_ROOT}/target/x86_64-apple-darwin/release/airmedia"

# 5. Package Distributions
mkdir -p "${PROJECT_ROOT}/dist"

package_target() {
    local BIN_SRC="$1"
    local SUFFIX="$2"
    local DIST_NAME="airmedia-${SUFFIX}"
    local STAGE_DIR="${PROJECT_ROOT}/dist/${DIST_NAME}"
    local TAR_FILE="airmedia-v${VERSION}-${SUFFIX}.tar.gz"

    echo "  Staging ${TAR_FILE}..."
    rm -rf "${STAGE_DIR}"
    mkdir -p "${STAGE_DIR}"
    cp "${BIN_SRC}" "${STAGE_DIR}/airmedia"
    chmod +x "${STAGE_DIR}/airmedia"
    cp "${PROJECT_ROOT}/README.md" "${STAGE_DIR}/README.md"
    if [ -f "${PROJECT_ROOT}/LICENSE" ]; then
        cp "${PROJECT_ROOT}/LICENSE" "${STAGE_DIR}/LICENSE"
    fi

    tar -czf "${PROJECT_ROOT}/dist/${TAR_FILE}" -C "${PROJECT_ROOT}/dist" "${DIST_NAME}"
    cd "${PROJECT_ROOT}/dist"
    shasum -a 256 "${TAR_FILE}" > "${TAR_FILE}.sha256"
    cd "${PROJECT_ROOT}"
}

echo "[5/6] Packaging distributions..."
package_target "${PROJECT_ROOT}/target/aarch64-apple-darwin/release/airmedia" "macos-arm64"
package_target "${PROJECT_ROOT}/target/x86_64-apple-darwin/release/airmedia" "macos-x64"
package_target "${UNIVERSAL_DIR}/airmedia" "macos-universal"

echo "[6/6] Build and packaging complete!"
echo "Generated macOS distribution packages in dist/:"
ls -lh dist/airmedia-v${VERSION}-macos*
