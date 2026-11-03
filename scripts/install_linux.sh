#!/usr/bin/env bash
# ==============================================================================
# AirMedia - Linux Installation Script
# Installs binary, desktop entry, and icon to /usr/local (or ~/.local with --user)
# ==============================================================================
set -euo pipefail

PREFIX="/usr/local"
if [ "${1:-}" = "--user" ] || [ "$(id -u)" -ne 0 ]; then
    PREFIX="${HOME}/.local"
    echo "Installing AirMedia in user directory (${PREFIX})..."
else
    echo "Installing AirMedia system-wide (${PREFIX})..."
fi

BIN_DIR="${PREFIX}/bin"
APP_DIR="${PREFIX}/share/applications"
ICON_DIR="${PREFIX}/share/icons/hicolor/256x256/apps"

mkdir -p "${BIN_DIR}" "${APP_DIR}" "${ICON_DIR}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Install binary
if [ -f "${SCRIPT_DIR}/bin/airmedia" ]; then
    install -m 755 "${SCRIPT_DIR}/bin/airmedia" "${BIN_DIR}/airmedia"
elif [ -f "${SCRIPT_DIR}/airmedia" ]; then
    install -m 755 "${SCRIPT_DIR}/airmedia" "${BIN_DIR}/airmedia"
else
    echo "Error: airmedia binary not found in ${SCRIPT_DIR}" >&2
    exit 1
fi

# Install desktop entry
if [ -f "${SCRIPT_DIR}/share/applications/airmedia.desktop" ]; then
    install -m 644 "${SCRIPT_DIR}/share/applications/airmedia.desktop" "${APP_DIR}/airmedia.desktop"
elif [ -f "${SCRIPT_DIR}/airmedia.desktop" ]; then
    install -m 644 "${SCRIPT_DIR}/airmedia.desktop" "${APP_DIR}/airmedia.desktop"
fi

# Install icon
if [ -f "${SCRIPT_DIR}/share/icons/hicolor/256x256/apps/airmedia.png" ]; then
    install -m 644 "${SCRIPT_DIR}/share/icons/hicolor/256x256/apps/airmedia.png" "${ICON_DIR}/airmedia.png"
elif [ -f "${SCRIPT_DIR}/airmedia.png" ]; then
    install -m 644 "${SCRIPT_DIR}/airmedia.png" "${ICON_DIR}/airmedia.png"
fi

# Update desktop and icon caches if tools are available
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${APP_DIR}" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${PREFIX}/share/icons/hicolor" 2>/dev/null || true
fi

echo "AirMedia successfully installed to ${BIN_DIR}/airmedia"
echo "You can launch AirMedia from your application menu or run 'airmedia'."
