#!/usr/bin/env bash
# Builds the Android APK inside Docker from a clean copy of the repo, so no local
# Java, Android SDK or NDK is needed. The APK and its checksum land in dist/.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
export MSYS_NO_PATHCONV=1 # Keep Git Bash on Windows from rewriting the container paths.

IMAGE=airmedia-android-build
docker build -t "${IMAGE}" -f scripts/android-build.Dockerfile scripts

mkdir -p dist

# Only tracked and new non-ignored files are copied in, like a CI checkout. Ignored
# build output on the host can't hide a missing file.
git ls-files -co --exclude-standard -z \
    | tar --null -T - -cf - \
    | docker run -i --rm \
        -v airmedia-android-cache:/cache \
        -v "$(pwd)/dist:/out" \
        "${IMAGE}" \
        bash -c 'set -e
            mkdir /work && tar -xf - -C /work && cd /work
            ./scripts/build_android.sh
            cp dist/*.apk dist/*.sha256 /out/'
