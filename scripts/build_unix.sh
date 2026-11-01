#!/usr/bin/env bash
# ==============================================================================
# Production Release Build Script for AirMedia (Linux & macOS)
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

echo "=== AirMedia Production Release Build (${OS}-${ARCH}) ==="

# 1. Verify Rust Toolchain
echo "[1/5] Verifying toolchain..."
rustc --version
cargo --version

# 2. Run Test Suite
echo "[2/5] Running automated tests..."
cargo test --quiet

# 3. Build Optimized Binary
echo "[3/5] Building release binary..."
cargo build --release

# 4. Prepare Distribution Directory
DIST_DIR="${PROJECT_ROOT}/dist/airmedia-${OS}-${ARCH}"
rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}"

echo "[4/5] Staging distribution files in ${DIST_DIR}..."
cp "target/release/airmedia" "${DIST_DIR}/airmedia"
chmod +x "${DIST_DIR}/airmedia"
cp "README.md" "${DIST_DIR}/README.md"
if [ -f "LICENSE" ]; then
    cp "LICENSE" "${DIST_DIR}/LICENSE"
fi

# 5. Create Tarball Package
echo "[5/5] Creating release tarball..."
TAR_PATH="${PROJECT_ROOT}/dist/airmedia-v0.1.0-${OS}-${ARCH}.tar.gz"
tar -czf "${TAR_PATH}" -C "${PROJECT_ROOT}/dist" "airmedia-${OS}-${ARCH}"

echo "=== Build Succeeded! ==="
echo "Tarball: ${TAR_PATH}"
ls -lh "${TAR_PATH}"
