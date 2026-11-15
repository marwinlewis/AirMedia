#!/usr/bin/env bash
# ==============================================================================
# AirMedia Multi-Platform Version Bump Tool (Bash)
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

TARGET=""
ANDROID_VERSION_CODE=0
DRY_RUN=false
DO_COMMIT=false
DO_TAG=false
DO_PUSH=false

print_usage() {
    cat <<EOF
Usage: ./scripts/bump_version.sh [TARGET] [OPTIONS]

TARGET:
    patch                Increment patch version (e.g. 0.1.0 -> 0.1.1)
    minor                Increment minor version (e.g. 0.1.0 -> 0.2.0)
    major                Increment major version (e.g. 0.1.0 -> 1.0.0)
    <version>            Explicit semver string (e.g. 0.2.0 or v0.2.0)

OPTIONS:
    --android-code <num> Override Android versionCode (defaults to current + 1)
    --dry-run            Show proposed changes without writing to disk
    --commit             Create git commit with bumped manifests
    --tag                Create annotated git tag (v<version>)
    --push               Push commit and tag to origin
    -h, --help           Show this help message
EOF
}

# Parse flags
while [[ $# -gt 0 ]]; do
    case "$1" in
        patch|minor|major)
            TARGET="$1"
            shift
            ;;
        --android-code)
            ANDROID_VERSION_CODE="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --commit)
            DO_COMMIT=true
            shift
            ;;
        --tag)
            DO_TAG=true
            shift
            ;;
        --push)
            DO_PUSH=true
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            if [[ -z "${TARGET}" && ! "$1" =~ ^-- ]]; then
                TARGET="$1"
                shift
            else
                echo "Unknown option: $1" >&2
                print_usage
                exit 1
            fi
            ;;
    esac
done

CARGO_TOML="Cargo.toml"
ANDROID_GRADLE="android/app/build.gradle"
WIN_MANIFEST="assets/windows/airmedia.manifest"
INNO_SCRIPT="scripts/installer.iss"
MAC_PLIST="assets/macos/Info.plist"

if [ ! -f "${CARGO_TOML}" ]; then
    echo "Error: Cargo.toml not found in ${PROJECT_ROOT}" >&2
    exit 1
fi

# Extract current version from Cargo.toml
CURRENT_VERSION=$(grep -m1 '^version = ' "${CARGO_TOML}" | cut -d '"' -f2)

echo "=========================================================="
echo "  AirMedia Multi-Platform Version Bump Tool"
echo "=========================================================="
echo "Current version: ${CURRENT_VERSION}"

if [ -z "${TARGET}" ]; then
    echo ""
    read -r -p "Enter target version or bump type (patch / minor / major / X.Y.Z): " TARGET
    if [ -z "${TARGET}" ]; then
        echo "No version specified. Aborted."
        exit 0
    fi
fi

# Parse current semver
IFS='.' read -r CUR_MAJOR CUR_MINOR CUR_PATCH <<< "${CURRENT_VERSION%%-*}"

case "${TARGET}" in
    patch)
        NEW_MAJOR="${CUR_MAJOR}"
        NEW_MINOR="${CUR_MINOR}"
        NEW_PATCH=$((CUR_PATCH + 1))
        NEW_VERSION="${NEW_MAJOR}.${NEW_MINOR}.${NEW_PATCH}"
        ;;
    minor)
        NEW_MAJOR="${CUR_MAJOR}"
        NEW_MINOR=$((CUR_MINOR + 1))
        NEW_PATCH=0
        NEW_VERSION="${NEW_MAJOR}.${NEW_MINOR}.${NEW_PATCH}"
        ;;
    major)
        NEW_MAJOR=$((CUR_MAJOR + 1))
        NEW_MINOR=0
        NEW_PATCH=0
        NEW_VERSION="${NEW_MAJOR}.${NEW_MINOR}.${NEW_PATCH}"
        ;;
    *)
        CLEAN_TARGET="${TARGET#v}"
        if [[ "${CLEAN_TARGET}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$ ]]; then
            NEW_VERSION="${CLEAN_TARGET}"
            IFS='.' read -r NEW_MAJOR NEW_MINOR NEW_PATCH <<< "${NEW_VERSION%%-*}"
        else
            echo "Error: Invalid version format '${TARGET}'. Expected 'patch', 'minor', 'major', or 'X.Y.Z'." >&2
            exit 1
        fi
        ;;
esac

WIN_VERSION="${NEW_MAJOR}.${NEW_MINOR}.${NEW_PATCH}.0"

echo "Target version:  ${NEW_VERSION}"
echo "Windows version: ${WIN_VERSION}"

# Calculate Android versionCode
CURRENT_ANDROID_CODE=0
if [ -f "${ANDROID_GRADLE}" ]; then
    CURRENT_ANDROID_CODE=$(grep -m1 'versionCode' "${ANDROID_GRADLE}" | tr -dc '0-9' || echo "1")
fi

if [ "${ANDROID_VERSION_CODE}" -gt 0 ]; then
    NEW_ANDROID_CODE="${ANDROID_VERSION_CODE}"
else
    NEW_ANDROID_CODE=$((CURRENT_ANDROID_CODE + 1))
fi
echo "Android versionCode: ${CURRENT_ANDROID_CODE} -> ${NEW_ANDROID_CODE}"

if [ "${DRY_RUN}" = true ]; then
    echo -e "\n[DRY RUN MODE - No files will be modified]\n"
fi

update_file() {
    local FILE="$1"
    local DESC="$2"
    local SED_EXPR="$3"

    if [ ! -f "${FILE}" ]; then
        return
    fi

    if [ "${DRY_RUN}" = true ]; then
        echo "  [DRY RUN] Would update: ${DESC} (${FILE})"
    else
        # Portable in-place sed
        local TMP_FILE
        TMP_FILE="$(mktemp)"
        sed -E "${SED_EXPR}" "${FILE}" > "${TMP_FILE}"
        mv "${TMP_FILE}" "${FILE}"
        echo "  [UPDATED] ${DESC}"
    fi
}

echo -e "\nUpdating platform manifest files..."

# A. Cargo.toml
if [ "${DRY_RUN}" = true ]; then
    echo "  [DRY RUN] Would update: Cargo.toml (version = \"${NEW_VERSION}\")"
else
    awk -v new_ver="${NEW_VERSION}" '
        BEGIN { in_pkg = 0; done = 0 }
        /^\[package\]/ { in_pkg = 1; print; next }
        /^\[/ && !/^\[package\]/ { in_pkg = 0; print; next }
        in_pkg && !done && /^version\s*=/ {
            print "version = \"" new_ver "\""
            done = 1
            next
        }
        { print }
    ' "${CARGO_TOML}" > "${CARGO_TOML}.tmp" && mv "${CARGO_TOML}.tmp" "${CARGO_TOML}"
    echo "  [UPDATED] Cargo.toml (version = \"${NEW_VERSION}\")"
fi

# B. Android build.gradle
if [ -f "${ANDROID_GRADLE}" ]; then
    update_file "${ANDROID_GRADLE}" \
        "android/app/build.gradle (versionName: \"${NEW_VERSION}\", versionCode: ${NEW_ANDROID_CODE})" \
        "s/(versionCode\s+)[0-9]+/\1${NEW_ANDROID_CODE}/; s/(versionName\s+)\"[^\"]+\"/\1\"${NEW_VERSION}\"/"
fi

# C. Windows Manifest
if [ -f "${WIN_MANIFEST}" ]; then
    update_file "${WIN_MANIFEST}" \
        "assets/windows/airmedia.manifest (version = \"${WIN_VERSION}\")" \
        "s/(<assemblyIdentity[^>]*version=\")[^\"]+(\")/\1${WIN_VERSION}\2/"
fi

# D. Inno Setup installer.iss
if [ -f "${INNO_SCRIPT}" ]; then
    update_file "${INNO_SCRIPT}" \
        "scripts/installer.iss (MyAppVersion = \"${NEW_VERSION}\")" \
        "s/(#define\s+MyAppVersion\s+)\"[^\"]+\"/\1\"${NEW_VERSION}\"/"
fi

# E. macOS Info.plist
if [ -f "${MAC_PLIST}" ]; then
    if [ "${DRY_RUN}" = true ]; then
        echo "  [DRY RUN] Would update: assets/macos/Info.plist (CFBundleVersion = \"${NEW_VERSION}\")"
    else
        awk -v new_ver="${NEW_VERSION}" '
            /<key>CFBundleVersion<\/key>/ { print; getline; sub(/<string>[^<]*<\/string>/, "<string>" new_ver "</string>"); print; next }
            /<key>CFBundleShortVersionString<\/key>/ { print; getline; sub(/<string>[^<]*<\/string>/, "<string>" new_ver "</string>"); print; next }
            { print }
        ' "${MAC_PLIST}" > "${MAC_PLIST}.tmp" && mv "${MAC_PLIST}.tmp" "${MAC_PLIST}"
        echo "  [UPDATED] assets/macos/Info.plist (CFBundleVersion = \"${NEW_VERSION}\")"
    fi
fi

# F. Sync Cargo.lock
if [ "${DRY_RUN}" = false ]; then
    if command -v cargo &>/dev/null; then
        echo -e "\nUpdating Cargo.lock via cargo check..."
        cargo check --quiet || echo "  [WARNING] cargo check returned non-zero code."
        echo "  [UPDATED] Cargo.lock synchronized"
    fi
fi

echo "=========================================================="
echo "  Version bump to v${NEW_VERSION} completed successfully!"
echo "=========================================================="

if [ "${DRY_RUN}" = false ] && { [ "${DO_COMMIT}" = true ] || [ "${DO_TAG}" = true ] || [ "${DO_PUSH}" = true ]; }; then
    if command -v git &>/dev/null; then
        echo -e "\nRunning Git operations..."
        git add Cargo.toml Cargo.lock "${ANDROID_GRADLE}" "${WIN_MANIFEST}" "${INNO_SCRIPT}" "${MAC_PLIST}" 2>/dev/null || true
        git commit -m "chore(release): bump version to ${NEW_VERSION}" || echo "  [GIT] Nothing to commit or commit skipped."

        if [ "${DO_TAG}" = true ] || [ "${DO_PUSH}" = true ]; then
            git tag -a "v${NEW_VERSION}" -m "AirMedia Release v${NEW_VERSION}"
            echo "  [GIT] Created tag: v${NEW_VERSION}"
        fi

        if [ "${DO_PUSH}" = true ]; then
            echo "  [GIT] Pushing to origin..."
            git push origin HEAD --follow-tags
        fi
    fi
else
    echo -e "\nNext steps to publish release:"
    echo "  git add Cargo.toml Cargo.lock android/app/build.gradle assets/windows/airmedia.manifest scripts/installer.iss assets/macos/Info.plist"
    echo "  git commit -m \"chore(release): bump version to ${NEW_VERSION}\""
    echo "  git tag v${NEW_VERSION}"
    echo "  git push origin HEAD --tags"
fi
