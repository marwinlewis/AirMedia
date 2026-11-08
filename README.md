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

AirMedia is built around two screens: a library to browse and a player that gets out of the way.

1. **Home**: a transparent navigation bar over a full-bleed banner featuring what is playing, what you can resume or what is
   next, followed by horizontally scrolling rows: *Pick Up Where You Left Off*, *Your Playlist*, *Recently Played*, *Videos*,
   *Music* and one row per folder. On first run it shows the ways to get media in, and nothing fake.
2. **Cards**: 16:9 thumbnails generated in the background with ffmpeg and cached on disk (`<config>/thumbnails`), with a
   picture next to the file (`movie.jpg`, or `cover.jpg` / `folder.jpg` for music) taking priority. Hover for play and info;
   right-click to add to or remove from the playlist or history. Progress bars show how far you got.
3. **Videos / Music / Search**: responsive grids that stay smooth with thousands of items. `Ctrl + F` searches titles and folders.
4. **Details**: *More Info* opens an overlay with artwork, metadata, *Resume* / *Start Over* and playlist actions.
5. **Player**: edge-to-edge video with a back button and title on top, and the seek bar (floating timestamp, A-B markers) and
   transport at the bottom: play/pause, ±10 s, volume (0–150%, with a 100% mark), elapsed / total or remaining time, previous /
   next, shuffle, repeat, playlist, speed, **audio & subtitles** (tracks, delay, load a file), **settings** (speed, aspect ratio,
   hardware decoder, equalizer, screenshot, jump to time, A-B loop, media information, preferences) and fullscreen.
   Near the end of an item, a button jumps straight to the next one.
6. **Music**: dimmed artwork fills the window behind the cover, title and a live spectrum visualizer.
7. **Mini player**: press back while something plays and it keeps going in a bar under the library, with seeking, transport and
   volume; click it to return.
8. **Playlist drawer**: slides in from the right on either screen with search, multi-select, drag-to-reorder, shuffle, repeat and
   a now-playing indicator. Playlists (`.m3u`, `.m3u8`, `.pls`) can be imported and saved from the app menu.
9. **App menu**: the menu button holds everything else (Recent, Playback, Audio, Video, Subtitles, Codec Setup, Preferences,
   About, Quit) with shortcuts shown inline.
10. **Session memory**: the playlist, recent files, equalizer settings and the playback position of each file are restored on
    the next launch (files resume where you stopped if you were more than 30 s in and not near the end).
11. **Subtitles**: sidecar files (`movie.srt`, `movie.en.srt`, `.vtt`, `.ass`) load automatically; embedded text tracks are
    extracted in the background. UTF-8, UTF-16 and legacy Windows-1252 files are supported.
12. **Auto-hide**: controls and the cursor hide after a configurable idle time while a video plays.

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Space` | Play / Pause |
| `S` | Stop and return to the library |
| `N / P` | Next / Previous track in playlist |
| `Media keys` | Play/Pause, Stop, Next and Previous track |
| `.` | Step 1 frame forward |
| `,` | Step 1 frame backward |
| `[ / ]` | Decrease / Increase playback speed (0.25x steps) |
| `Backspace` or `=` | Reset playback speed to 1.0x |
| `Left / Right` | Seek backward / forward 5s |
| `Shift + Left / Right` | Seek backward / forward 15s |
| `Ctrl + Left / Right` | Seek backward / forward 60s |
| `Ctrl + G` | Jump to time |
| `B` | A-B loop: set A, then B, then clear |
| `Up / Down` | Volume up / down (5%) |
| `M` | Mute / Unmute |
| `Mouse wheel` | Adjust volume over the video |
| `Ctrl + E` | Equalizer & video filters |
| `A` | Cycle aspect ratio (Fit, Fill, 16:9, 4:3, 21:9, Original) |
| `H` | Cycle hardware acceleration (Auto, D3D11VA, D3D12VA, CUDA, Vulkan, CPU) |
| `Shift + S` | Take screenshot |
| `L` | Toggle playlist drawer |
| `Delete` | Remove selected playlist items |
| `Ctrl / Shift + Click` | Select several playlist items |
| `Drag` | Reorder playlist items |
| `O` | Open media file |
| `Ctrl + N` | Open network stream |
| `Ctrl + F` | Search the library |
| `F` or `F11` | Toggle fullscreen |
| `Double click` | Toggle fullscreen |
| `Esc` | Close dialog / exit fullscreen / back to the library |
| `Ctrl + T` | Toggle always on top |
| `I` | Media information |
| `Ctrl + P` | Preferences |
| `?` or `F1` | Show keyboard shortcuts |
| `Ctrl + Q` | Quit |

Press `?` or `F1` in the player for the same list. The table is generated from `src/shortcuts.rs`; `cargo test` fails if it drifts. Hardware media keys work while the AirMedia window has focus.

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
4. **Portable bundles**: to ship codecs with a portable build, put `ffmpeg`/`ffprobe` in a `codecs` folder next to the
   executable. Binaries placed directly beside `airmedia.exe` are ignored on purpose, so a stray `ffmpeg.exe` dropped into
   e.g. your Downloads folder can never be executed by AirMedia.

The in-app downloader verifies the archive against the SHA-256 checksum published by gyan.dev before installing it.
Media is opened with an ffmpeg protocol whitelist: local files can only read local files, and network streams cannot read
local files.

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

> **Known limitation:** playback currently relies on external `ffmpeg`/`ffprobe` executables, which are not available on
> Android. The APK builds and launches, but cannot play media yet.

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
