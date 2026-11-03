# AirMedia 🚀
> **Ultra-Lightweight, Modern, GPU-Accelerated Media Player written in Rust**

AirMedia is an ultra-fast, modern media player designed to deliver the rich feature set of VLC while staying exceptionally lightweight (<13 MB binary, starts up in milliseconds, negligible RAM usage). It leverages modern hardware acceleration technologies to run video and audio decoding and presentation directly on the GPU.

---

## 🌟 Modern Architecture & Technologies

### 1. Direct GPU Rendering Pipeline
- **Graphics Engine**: Built on [`egui`](https://github.com/emilk/egui) & [`wgpu`](https://github.com/gfx-rs/wgpu) supporting **DirectX 12, DirectX 11, and Vulkan**.
- **Hardware Texture Streaming**: Decoded video frames are directly streamed to GPU textures. The GPU handles bilinear/bicubic sampling, interpolation, and scaling in real-time with virtually zero CPU overhead.
- **Aspect Ratio Modes**: Hardware-accelerated presentation with support for:
  - **Fit (Letterbox/Pillarbox)** — Preserves source aspect ratio.
  - **Fill (Crop to Window)** — Zero black bars.
  - **Stretch** — Stretches to window boundaries.
  - **Force 16:9, 4:3, 21:9 (Ultrawide), 1:1, or 100% Original**.

### 2. Multi-Backend Hardware Video Acceleration (GPU)
AirMedia supports seamless on-the-fly hardware decoding through:
- **DirectX 11 (D3D11VA)**: Default low-overhead hardware decoding on Windows.
- **DirectX 12 (D3D12VA)**: Next-gen DirectX 12 hardware acceleration.
- **NVIDIA NVDEC (CUDA)**: Dedicated silicon decoder engines on NVIDIA GPUs.
- **Vulkan Video**: Cross-vendor open standard hardware video decoding.
- **DirectX (DXVA2)**: Legacy DirectX Video Acceleration.
- **Auto (GPU)**: Automatically probes hardware capabilities and selects the optimal GPU decoder with graceful software CPU fallback.

### 3. Low-Latency WASAPI Audio Engine
- **WASAPI Direct Stream**: Powered by [`cpal`](https://github.com/rust-audio/cpal), communicating directly with Windows Audio Session API (WASAPI) with minimal latency and high sample accuracy.
- **Master PTS Clock Synchronization**: AV (Audio-Video) sync uses the audio stream's presentation timestamp as the master clock. Video frames are dynamically throttled or caught up if load spikes occur.
- **WSOLA Pitch-Preserving Time Stretching**: Speed changes from `0.25x` to `2.5x` preserve natural audio pitch.
- **Volume Boost**: Up to **150% volume boost** (just like VLC).
- **Real-Time Audio Spectrum Visualizer**: Dynamic 16-band animated spectrum analyzer in the control bar and full-screen visualization for audio tracks.

---

## 🖥️ Interface

AirMedia uses a minimal, immersive interface: the video fills the window and all chrome fades away while you watch.

1. **Empty state**: a clean start screen with *Open file* / *Open stream* actions and drag-and-drop anywhere.
2. **Top overlay**: app menu button, current title, and quiet technical details (resolution, frame rate, decoder).
   The app menu holds everything from the classic menu bar (Open, Recent, Playback, Audio, Video, Subtitles,
   Playlist, Media information, Codec Setup, Preferences, About, Quit) with shortcuts shown inline.
3. **Bottom overlay**:
   - Thin seek bar that grows on hover, with a floating timestamp preview; seeks on release.
   - Play/Pause, Previous, Next, volume (0–150%, with a 100% marker) and elapsed / total (or remaining) time.
   - **Subtitles** popup: track selection, load external file, delay adjustment.
   - **Settings** popup: playback speed, aspect ratio, audio track, audio delay, hardware decoder, media information.
   - Playlist and fullscreen toggles.
4. **Playlist drawer**: slides in from the right with search, shuffle, repeat (all / one), clear, and a now-playing indicator.
5. **Dialogs**: Effects (10-band equalizer with presets and preamp, video color adjustments), Media information,
   Codec Engine Setup, Preferences, Open network stream, and About — all sharing one consistent dark style.
6. **Auto-hide**: controls and the cursor hide after a configurable idle time while playing.

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Space` | Play / Pause |
| `S` | Stop playback |
| `F` or `F11` | Toggle Fullscreen |
| `Esc` | Exit Fullscreen |
| `Left` / `Right` | Seek backward / forward 5s |
| `Shift + Left` / `Right` | Seek backward / forward 15s |
| `Ctrl + Left` / `Right` | Seek backward / forward 60s |
| `Up` / `Down` | Volume Up / Down (5%) |
| `M` | Mute / Unmute |
| `.` | Step 1 frame forward |
| `,` | Step 1 frame backward |
| `[` / `]` | Decrease / Increase playback speed (0.25x steps) |
| `Backspace` or `=` | Reset playback speed to 1.0x |
| `N` / `P` | Next / Previous track in playlist |
| `A` | Cycle Aspect Ratio (Fit ➔ Fill ➔ 16:9 ➔ 4:3 ➔ 21:9 ➔ Original) |
| `H` | Cycle Hardware Acceleration (Auto ➔ D3D11VA ➔ D3D12VA ➔ CUDA ➔ Vulkan ➔ CPU) |
| `O` | Open media file dialog |
| `L` | Toggle Playlist drawer |
| `I` | Toggle Codec & Media Information dialog |
| `Mouse Wheel` | Scroll anywhere on player window to adjust volume |
| `Double Click` | Toggle Fullscreen |

---

## 🎨 Asset Generation (`dist/airmedia.png`)

AirMedia includes an automated asset generation engine that derives all multi-platform icons, macOS ICNS bundles, Windows ICO frames, Linux hicolor themes, and Android mipmap packages directly from the high-resolution source icon at `dist/airmedia.png`:

```bash
# Via Cargo example
cargo run --example generate_assets

# Or via dedicated scripts
.\scripts\generate_assets.ps1   # PowerShell
./scripts/generate_assets.sh    # Linux / macOS Bash
```

**Generated Assets:**
- **Primary Desktop Icon**: `assets/icon.png` (512x512 crisp PNG)
- **Windows Icon**: `assets/windows/icon.ico` (multi-resolution ICO with 16, 24, 32, 48, 64, 128, 256 px frames)
- **macOS Bundle Icon**: `assets/macos/AppIcon.icns` (multi-resolution ICNS with 16, 32, 64, 128, 256, 512, 1024 px frames)
- **Linux Icons**: `assets/linux/airmedia.png` & standard `assets/linux/icons/hicolor/{16 to 512}/apps/airmedia.png`
- **Android Icons**: `assets/android/` & `android/app/src/main/res/` mipmaps (`mdpi`, `hdpi`, `xhdpi`, `xxhdpi`, `xxxhdpi` with standard, round, and adaptive foregrounds, plus 512x512 Play Store icon)

---

## 🚀 Building & Distribution

### Codec Prerequisites & Smart On-The-Fly Resolution
AirMedia keeps its distribution package exceptionally lightweight (**~12.5 MB total folder size** / **~4.8 MB zip archive**):
1. **Preinstalled Detection**: When launched or installed, AirMedia checks if FFmpeg and FFprobe are already installed on the system (PATH, Conda, Chocolatey, Winget, etc.). If found, it uses them directly with **zero downloads needed**.
2. **On-The-Fly Installation**: If codecs are not found, either the installer script (`install.bat` / `install.ps1`) or the in-app Codec Setup modal will automatically download the official minimal release essentials package on the fly into `%LOCALAPPDATA%\AirMedia\bin`.
3. **Manual Installation**: You can also install codecs globally anytime with `winget install Gyan.FFmpeg` or `choco install ffmpeg`.

---

## 📦 Production Release Builds

AirMedia includes automated production build scripts that run the test suite, compile with Link-Time Optimization (LTO) and symbol stripping, stage the installer scripts, and generate a compact release archive.

### Windows (x64) Production Build

Run from PowerShell:
```powershell
# Standard lightweight production build (~12 MB, prerequisite mode)
.\build.ps1

# Or run via Command Prompt / double-click:
build.bat
```

**Build Output:**
- **Native Windows Installer**: `dist/airmedia-v<version>-windows-x64-setup.exe` (Industry-standard Inno Setup installer with uninstaller, shortcuts, and AV-safe structure)
- **Portable Release Package**: `dist/airmedia-v<version>-windows-x64.zip` (~5.2 MB compressed, with embedded manifest & PE resources)
- **Distribution Directory**: `dist/airmedia-windows-x64/` (~12 MB total)

### Linux & macOS Production Build (Cross-Platform)

When building on Linux:
```bash
# Install development headers
sudo apt-get install -y libasound2-dev libudev-dev pkg-config libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libfontconfig1-dev libfreetype6-dev ffmpeg

# Build and package
chmod +x scripts/build_unix.sh
./scripts/build_unix.sh
```

**Build Output:**
- `dist/airmedia-v<version>-linux-x86_64.tar.gz` (includes `airmedia` binary, `airmedia.desktop`, high-res icons, and `install.sh`)
- SHA-256 checksum file

When building on macOS:
```bash
chmod +x scripts/build_macos.sh
./scripts/build_macos.sh
```

**Build Output:**
- `dist/airmedia-v<version>-macos-universal.dmg` (Apple Disk Image with drag-and-drop `/Applications` install)
- `dist/airmedia-v<version>-macos-universal.tar.gz` (Universal `AirMedia.app` bundle)
- `dist/airmedia-v<version>-macos-arm64.tar.gz` (Apple Silicon `AirMedia.app`)
- `dist/airmedia-v<version>-macos-x64.tar.gz` (Intel `AirMedia.app`)
- SHA-256 checksum files for each package

### Android Production Build (Mobile APK)

AirMedia runs directly on Android via `android.app.NativeActivity` with zero Java boilerplate overhead, leveraging Vulkan GPU presentation and NDK hardware decoding.

To compile for Android:
```bash
# Prerequisites: Android SDK & NDK, Java 17+, cargo-ndk
cargo install cargo-ndk --locked
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# Build and package APK
chmod +x scripts/build_android.sh
./scripts/build_android.sh
```

Or from PowerShell:
```powershell
.\scripts\build_android.ps1
```

**Build Output:**
- `dist/airmedia-v<version>-android-universal.apk` (Universal multi-architecture APK with embedded `arm64-v8a`, `armeabi-v7a`, `x86_64` native binaries)
- SHA-256 checksum file

---

## 🛡️ Antivirus False Positive Mitigation & Code Signing

AirMedia implements thorough executable hardening to prevent flags from Windows Defender, SmartScreen, and macOS Gatekeeper:
- Embedded Win32 `asInvoker` application manifest, `supportedOS` compatibility tags, and PerMonitorV2 High-DPI awareness.
- Full Win32 PE `VERSIONINFO` metadata and multi-resolution `.ico` embedded at compile time via `winres`.
- Standard Inno Setup installer avoiding suspicious PowerShell scripting or `%TEMP%` drops.
- Hardened Runtime (`--options runtime`) and ad-hoc code signing on macOS `.app` bundles.
- Optional Authenticode and Apple Developer ID code signing via GitHub Actions secrets.

For detailed false-positive reporting guidelines and submission steps to Microsoft WDSI and VirusTotal, see **[`docs/SECURITY_AND_PACKAGING.md`](docs/SECURITY_AND_PACKAGING.md)**.

---

## 🤖 Automated CI/CD & GitHub Releases

AirMedia includes a fully automated GitHub Actions workflow (`.github/workflows/release.yml`) that compiles, tests, packages, and signs all releases across Windows, Linux, macOS, and Android.

### Triggering a Production Release

1. **Push a Version Tag**:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
   GitHub Actions will automatically run the parallel build matrix, generate packages for all operating systems and architectures, create the GitHub Release, and attach all archives, APKs, and consolidated `SHA256SUMS.txt`.

2. **Manual Dispatch (One-Click Release)**:
   - Navigate to **Actions** ➔ **Release** on GitHub.
   - Click **Run workflow**.
   - Optionally provide a custom tag name, or choose to create a draft/pre-release, or test-build artifacts without publishing.

### Release Matrix Artifacts

| Platform | Architecture | Package | Description |
|---|---|---|---|
| **Windows** | x86_64 | `airmedia-v<version>-windows-x64.zip` | Stripped binary with automated setup scripts |
| **Linux** | x86_64 | `airmedia-v<version>-linux-x86_64.tar.gz` | Optimized binary for modern Linux distributions |
| **macOS** | Apple Silicon (arm64) | `airmedia-v<version>-macos-arm64.tar.gz` | Native M1/M2/M3/M4 Apple Silicon build |
| **macOS** | Intel (x86_64) | `airmedia-v<version>-macos-x64.tar.gz` | Native 64-bit Intel Mac build |
| **macOS** | Universal | `airmedia-v<version>-macos-universal.tar.gz` | Multi-architecture binary running on any Mac |
| **Android** | Multi-ABI (Universal) | `airmedia-v<version>-android-universal.apk` | NativeActivity Vulkan GPU player APK |
| **All** | All | `SHA256SUMS.txt` | Consolidated SHA-256 verification checksums |

---

### Run Automated Tests
```bash
cargo test
```
