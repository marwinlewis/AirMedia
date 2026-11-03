#!/usr/bin/env bash
# ==============================================================================
# Production Release Build Script for AirMedia on macOS
# - Compiles Apple Silicon (arm64), Intel (x86_64), and Universal macOS binaries.
# - Constructs native AirMedia.app bundle with Info.plist & branding resources.
# - Applies macOS Code Signing & Hardened Runtime to avoid Gatekeeper quarantine.
# - Packages DMG (Disk Image with /Applications drag-and-drop) & Tarballs.
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

# 5. Build .app Bundle Helper
mkdir -p "${PROJECT_ROOT}/dist"

build_app_bundle() {
    local BIN_SRC="$1"
    local STAGE_DIR="$2"
    local APP_DIR="${STAGE_DIR}/AirMedia.app"
    local CONTENTS_DIR="${APP_DIR}/Contents"
    local MACOS_DIR="${CONTENTS_DIR}/MacOS"
    local RESOURCES_DIR="${CONTENTS_DIR}/Resources"

    rm -rf "${APP_DIR}"
    mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

    # Copy binary
    cp "${BIN_SRC}" "${MACOS_DIR}/airmedia"
    chmod +x "${MACOS_DIR}/airmedia"

    # Copy Info.plist and update version
    if [ -f "${PROJECT_ROOT}/assets/macos/Info.plist" ]; then
        sed "s/0.1.0/${VERSION}/g" "${PROJECT_ROOT}/assets/macos/Info.plist" > "${CONTENTS_DIR}/Info.plist"
    fi

    # PkgInfo
    echo -n "APPL????" > "${CONTENTS_DIR}/PkgInfo"

    # Branding
    if [ -f "${PROJECT_ROOT}/assets/icon.png" ]; then
        cp "${PROJECT_ROOT}/assets/icon.png" "${RESOURCES_DIR}/airmedia.png"
    fi
    if [ -f "${PROJECT_ROOT}/assets/macos/AppIcon.icns" ]; then
        cp "${PROJECT_ROOT}/assets/macos/AppIcon.icns" "${RESOURCES_DIR}/AppIcon.icns"
    elif [ -f "${PROJECT_ROOT}/assets/icon.png" ]; then
        # Create minimal icns from png if sips / iconutil are available
        cp "${PROJECT_ROOT}/assets/icon.png" "${RESOURCES_DIR}/AppIcon.png"
    fi

    # Code Signing (Crucial for macOS Gatekeeper)
    if [ -n "${MACOS_SIGNING_IDENTITY:-}" ]; then
        echo "    Signing AirMedia.app with identity '${MACOS_SIGNING_IDENTITY}' (Hardened Runtime)..."
        codesign --deep --force --options runtime --timestamp --sign "${MACOS_SIGNING_IDENTITY}" "${APP_DIR}"
    else
        echo "    Applying ad-hoc code signature with hardened runtime (prevents corrupted app flags)..."
        codesign --deep --force --options runtime --sign - "${APP_DIR}" 2>/dev/null || true
    fi
}

create_dmg() {
    local APP_SOURCE="$1"
    local DMG_NAME="$2"
    local DMG_FILE="${PROJECT_ROOT}/dist/${DMG_NAME}.dmg"
    local DMG_TEMP="${PROJECT_ROOT}/dist/temp_${DMG_NAME}"

    echo "    Creating DMG: ${DMG_FILE}..."
    rm -rf "${DMG_TEMP}" "${DMG_FILE}"
    mkdir -p "${DMG_TEMP}"
    cp -R "${APP_SOURCE}" "${DMG_TEMP}/"
    ln -s /Applications "${DMG_TEMP}/Applications"
    cp "${PROJECT_ROOT}/README.md" "${DMG_TEMP}/"
    if [ -f "${PROJECT_ROOT}/LICENSE" ]; then
        cp "${PROJECT_ROOT}/LICENSE" "${DMG_TEMP}/"
    fi

    if command -v hdiutil >/dev/null 2>&1; then
        hdiutil create -volname "AirMedia" -srcfolder "${DMG_TEMP}" -ov -format UDZO "${DMG_FILE}" >/dev/null
        rm -rf "${DMG_TEMP}"
        cd "${PROJECT_ROOT}/dist"
        shasum -a 256 "${DMG_NAME}.dmg" > "${DMG_NAME}.dmg.sha256"
        cd "${PROJECT_ROOT}"
        echo "    [OK] DMG Created: ${DMG_FILE}"
    else
        echo "    Warning: hdiutil not available; skipping DMG creation."
        rm -rf "${DMG_TEMP}"
    fi
}

package_target() {
    local BIN_SRC="$1"
    local SUFFIX="$2"
    local DIST_NAME="airmedia-${SUFFIX}"
    local STAGE_DIR="${PROJECT_ROOT}/dist/${DIST_NAME}"
    local TAR_FILE="airmedia-v${VERSION}-${SUFFIX}.tar.gz"

    echo "  Packaging ${DIST_NAME}..."
    rm -rf "${STAGE_DIR}"
    mkdir -p "${STAGE_DIR}"

    # 1. Build .app bundle
    build_app_bundle "${BIN_SRC}" "${STAGE_DIR}"

    # 2. Direct binary copy for CLI / scripted use
    cp "${BIN_SRC}" "${STAGE_DIR}/airmedia"
    chmod +x "${STAGE_DIR}/airmedia"
    cp "${PROJECT_ROOT}/README.md" "${STAGE_DIR}/README.md"
    if [ -f "${PROJECT_ROOT}/LICENSE" ]; then
        cp "${PROJECT_ROOT}/LICENSE" "${STAGE_DIR}/LICENSE"
    fi

    # 3. Create DMG for universal or primary target
    if [ "${SUFFIX}" = "macos-universal" ]; then
        create_dmg "${STAGE_DIR}/AirMedia.app" "airmedia-v${VERSION}-macos-universal"
    fi

    # 4. Create Tarball
    tar -czf "${PROJECT_ROOT}/dist/${TAR_FILE}" -C "${PROJECT_ROOT}/dist" "${DIST_NAME}"
    cd "${PROJECT_ROOT}/dist"
    shasum -a 256 "${TAR_FILE}" > "${TAR_FILE}.sha256"
    cd "${PROJECT_ROOT}"
}

echo "[5/6] Packaging macOS distribution bundles..."
package_target "${UNIVERSAL_DIR}/airmedia" "macos-universal"
package_target "${PROJECT_ROOT}/target/aarch64-apple-darwin/release/airmedia" "macos-arm64"
package_target "${PROJECT_ROOT}/target/x86_64-apple-darwin/release/airmedia" "macos-x64"

echo "[6/6] Build and packaging complete!"
echo "Generated macOS distribution packages in dist/:"
ls -lh dist/airmedia-v${VERSION}-macos*
