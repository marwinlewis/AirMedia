#!/usr/bin/env bash
# AirMedia Asset Generator Script (Bash)
# Regenerates icons and asset packages across Windows, macOS, Linux, and Android from dist/airmedia.png
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"
echo "==> Generating AirMedia multi-platform assets from dist/airmedia.png..."
cargo run --example generate_assets
echo "==> Assets generated successfully!"
