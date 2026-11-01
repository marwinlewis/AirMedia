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
# Standard lightweight production build (~12.5 MB, prerequisite mode)
.\build.ps1

# Or run via Command Prompt / double-click:
build.bat
```

**Build Output:**
- **Distribution Directory**: `dist/airmedia-windows-x64/` (~12.5 MB total)
  - `airmedia.exe` (~12.5 MB optimized stripped binary)
  - `install.bat` & `install.ps1` (automated installer with on-the-fly prerequisite setup)
  - `README.md` & `LICENSE`
- **Release Package**: `dist/airmedia-v0.1.0-windows-x64.zip` (~4.8 MB compressed)

### Installing on User Machines
Users can simply unzip the release package and:
- **Option A (Quick Launch)**: Double-click `airmedia.exe` directly. If codecs are missing, AirMedia displays a friendly in-app dialog offering 1-click on-the-fly codec download.
- **Option B (Desktop Install)**: Double-click `install.bat`. It checks for preinstalled FFmpeg/FFprobe, downloads on the fly only if missing, creates Start Menu and Desktop shortcuts, and registers file associations.

### Linux & macOS Production Build (Cross-Platform)

When building on macOS or Linux:
```bash
chmod +x scripts/build_unix.sh
./scripts/build_unix.sh
```

**Build Output:**
- `dist/airmedia-v0.1.0-<os>-<arch>.tar.gz`

### Run Automated Tests
```bash
cargo test
```
