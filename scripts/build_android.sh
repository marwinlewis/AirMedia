#!/usr/bin/env bash
# AirMedia Android Production Build Script (Cross-Platform)
# Compiles Rust cdylib via cargo-ndk, packages Android assets & resources, and builds APK
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=========================================================="
echo "  AirMedia Android Production Release Build"
echo "=========================================================="

cd "${PROJECT_ROOT}"

# 1. Extract Version from Cargo.toml
VERSION=$(grep -m1 '^version = ' Cargo.toml | cut -d '"' -f2)
echo "==> Building AirMedia v${VERSION} for Android..."

# 2. Synchronize High-Resolution Assets
echo "[1/5] Synchronizing multi-resolution icons from dist/airmedia.png..."
cargo run --example generate_assets

# 3. Ensure cargo-ndk and Rust Android Targets
echo "[2/5] Checking Android Rust toolchain targets..."
TARGETS=(
    "aarch64-linux-android"
    "armv7-linux-androideabi"
    "x86_64-linux-android"
)

for target in "${TARGETS[@]}"; do
    if ! rustup target list --installed | grep -q "^${target}$"; then
        echo "    Installing missing target: ${target}..."
        rustup target add "${target}"
    fi
done

if ! command -v cargo-ndk &>/dev/null; then
    echo "    Installing cargo-ndk build tool..."
    cargo install cargo-ndk --locked
fi

# Detect Android NDK path if not already set
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    if [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/ndk" ]; then
        LATEST_NDK=$(ls -1d "${ANDROID_HOME}/ndk"/* 2>/dev/null | sort -V | tail -n 1)
        if [ -n "${LATEST_NDK}" ] && [ -d "${LATEST_NDK}" ]; then
            export ANDROID_NDK_HOME="${LATEST_NDK}"
            echo "    Auto-detected ANDROID_NDK_HOME: ${ANDROID_NDK_HOME}"
        fi
    fi
fi

# 4. Compile Native cdylib for Supported ABIs
echo "[3/5] Compiling native Rust shared libraries (.so) via cargo-ndk..."
JNILIBS_DIR="${PROJECT_ROOT}/android/app/src/main/jniLibs"
mkdir -p "${JNILIBS_DIR}"

cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    -o "${JNILIBS_DIR}" \
    build --release

echo "==> Native libraries compiled:"
ls -la "${JNILIBS_DIR}"/*/*.so || true

# 5. Assemble APK via Gradle
echo "[4/5] Building Android APK with Gradle..."
cd "${PROJECT_ROOT}/android"

if [ -f "./gradlew" ]; then
    chmod +x ./gradlew
    GRADLE_CMD="./gradlew"
elif command -v gradle &>/dev/null; then
    GRADLE_CMD="gradle"
else
    echo "ERROR: Gradle not found! Please install Gradle or configure gradlew."
    exit 1
fi

${GRADLE_CMD} assembleRelease --no-daemon

# 6. Stage APKs to dist/ directory
echo "[5/5] Staging distribution APKs to dist/..."
DIST_DIR="${PROJECT_ROOT}/dist"
mkdir -p "${DIST_DIR}"

APK_SRC="${PROJECT_ROOT}/android/app/build/outputs/apk/release"
if [ ! -d "${APK_SRC}" ]; then
    # Fallback to debug output if release build was unsigned
    APK_SRC="${PROJECT_ROOT}/android/app/build/outputs/apk/debug"
fi

FOUND_APK=0
for apk in "${APK_SRC}"/*.apk; do
    if [ -f "${apk}" ]; then
        DEST_NAME="airmedia-v${VERSION}-android-universal.apk"
        cp "${apk}" "${DIST_DIR}/${DEST_NAME}"
        echo "    Staged: ${DIST_DIR}/${DEST_NAME}"
        
        # Calculate SHA-256
        if command -v sha256sum &>/dev/null; then
            (cd "${DIST_DIR}" && sha256sum "${DEST_NAME}" > "${DEST_NAME}.sha256")
        elif command -v shasum &>/dev/null; then
            (cd "${DIST_DIR}" && shasum -a 256 "${DEST_NAME}" > "${DEST_NAME}.sha256")
        fi
        FOUND_APK=1
        break
    fi
done

if [ "${FOUND_APK}" -eq 1 ]; then
    echo "=========================================================="
    echo "  Android APK build succeeded!"
    echo "  Package: dist/airmedia-v${VERSION}-android-universal.apk"
    echo "=========================================================="
else
    echo "ERROR: No APK output found in ${APK_SRC}"
    exit 1
fi
