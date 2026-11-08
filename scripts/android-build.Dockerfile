# Clean-room Android build environment, mirroring the build-android CI job.
# Used by scripts/build_android_docker.sh. The base image ships the Android SDK,
# NDK and JDK 17; this adds what build_android.sh needs on the host side.
FROM ghcr.io/cirruslabs/android-sdk:34-ndk

# Host libs: build_android.sh runs `cargo run --example generate_assets` natively first.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential curl pkg-config \
        libasound2-dev libudev-dev libssl-dev \
        libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
        libxkbcommon-dev libfontconfig1-dev libfreetype6-dev \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable \
        --target aarch64-linux-android --target armv7-linux-androideabi --target x86_64-linux-android \
    && cargo install cargo-ndk --locked

# Build output lives in volumes so repeat runs are incremental.
ENV CARGO_TARGET_DIR=/cache/target GRADLE_USER_HOME=/cache/gradle
