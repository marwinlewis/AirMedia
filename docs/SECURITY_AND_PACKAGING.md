# AirMedia Production Release & Antivirus Security Guide

This document details the hardening steps, packaging architecture, and false-positive mitigation procedures implemented for AirMedia production releases across Windows, macOS, and Linux.

---

## 1. Why Open-Source Desktop Apps Get Flagged by Antivirus

Newly compiled open-source desktop software (particularly binaries compiled with Rust, Go, or C++) frequently encounters heuristic flags from security software such as Windows Defender SmartScreen, CrowdStrike, and macOS Gatekeeper.

The primary triggers are:
1. **Low Global Reputation**: SmartScreen relies on telemetry. A brand-new binary hash has zero download history, triggering *"Windows protected your PC: Unknown publisher"*.
2. **Missing Executable Metadata**: Bare binaries lacking PE `VERSIONINFO`, Company/Product strings, or an application manifest resemble anonymous malicious payloads.
3. **Execution Policy & Scripting**: Installers written as `.bat` scripts calling `powershell -ExecutionPolicy Bypass` or downloading files into `%TEMP%` trigger AMSI and generic Trojan heuristic rules.
4. **Unsigned macOS Mach-O Executables**: macOS Big Sur and later flag unsigned or non-bundle binaries as "damaged" or "cannot be opened because Apple cannot verify the developer".

---

## 2. Hardening Measures Implemented in AirMedia

### A. Windows Hardening
- **Embedded Win32 Application Manifest** (`assets/windows/airmedia.manifest`):
  - `requestedExecutionLevel level="asInvoker" uiAccess="false"`: Explicitly states the application does not require administrator privileges, silencing UAC elevation warnings.
  - `supportedOS` GUIDs: Explicitly targets Windows 10, Windows 11, Windows 8.1, and Windows 7, preventing legacy compatibility shims.
  - `PerMonitorV2` High-DPI awareness: Native rendering on multi-monitor setups.
- **Embedded PE `VERSIONINFO` Resource** (via `winres` and `build.rs`):
  - FileDescription: `AirMedia - GPU Accelerated Media Player`
  - ProductName: `AirMedia`
  - CompanyName: `AirMedia Open Source Project`
  - LegalCopyright: `Copyright (C) 2026 AirMedia Contributors`
  - Version: Matched to `Cargo.toml`
  - Application Icon: Multi-resolution Windows ICO (`16x16` up to `256x256`).
- **Industry-Standard Inno Setup Installer** (`scripts/installer.iss`):
  - Compiled with Inno Setup into a standard native PE installer (`airmedia-v<version>-windows-x64-setup.exe`).
  - Pre-whitelisted structure recognized by Windows Defender and major AV engines.
  - Installs to `{autopf}\AirMedia` without triggering intrusive elevation prompts.
  - Provides a clean, registered Windows uninstaller in *Add or Remove Programs*.
- **No Malicious Scripting Patterns**:
  - Eliminated `ExecutionPolicy Bypass` commands.
  - Eliminated background zip downloads to temporary directories.
- **Automated Authenticode Signing Hook**:
  - `build_windows.ps1` supports signing with `signtool.exe` using RFC 3161 timestamps whenever a certificate is configured.

### B. macOS Hardening
- **Native `.app` Application Bundle**:
  - Built as a standard macOS bundle (`AirMedia.app`) containing `Contents/MacOS`, `Contents/Resources`, `Info.plist`, and `PkgInfo`.
  - Registered document types for audio and video file formats.
  - Discrete GPU automatic switching enabled (`NSSupportsAutomaticGraphicsSwitching`).
- **Code Signing with Hardened Runtime**:
  - `codesign --deep --force --options runtime --sign - "AirMedia.app"` applied automatically.
  - Enables Hardened Runtime (`--options runtime`) required for Apple Notarization and Gatekeeper compatibility.
- **DMG Disk Image**:
  - Universal binary (`arm64` + `x86_64`) packaged into an Apple Disk Image (`.dmg`) with an `/Applications` symlink for drag-and-drop installation.

### C. Linux Integration
- **FreeDesktop Desktop Entry** (`assets/linux/airmedia.desktop`):
  - Registered MIME types for video and audio playback.
  - Scalable PNG icon placed in standard `share/icons/hicolor/256x256/apps/`.
- **Non-Intrusive Installer** (`scripts/install_linux.sh`):
  - Supports system-wide (`/usr/local`) or per-user (`~/.local`) installation.

---

## 3. How to Set Up Code Signing in CI/CD (Optional but Recommended)

To achieve zero SmartScreen warnings immediately upon release, configure code signing in GitHub Repository Secrets:

### Windows Authenticode Signing
1. Obtain an Authenticode Code Signing Certificate (from DigiCert, Sectigo, or SSL.com) as a `.pfx` file.
2. In your GitHub repository, navigate to **Settings > Secrets and variables > Actions**.
3. Add the following secrets:
   - `WINDOWS_CERTIFICATE_PATH`: Path to the certificate file (or decode a base64 secret in CI).
   - `WINDOWS_CERTIFICATE_PASSWORD`: Certificate password.

### Apple Developer ID Signing & Notarization
1. Enroll in the Apple Developer Program.
2. Generate a **Developer ID Application** certificate.
3. Add the following secrets in GitHub Actions:
   - `MACOS_SIGNING_IDENTITY`: e.g. `Developer ID Application: Your Name (TEAMID)`.
   - `APPLE_ID`: Your Apple ID email.
   - `APPLE_PASSWORD`: App-specific password generated at appleid.apple.com.
   - `APPLE_TEAM_ID`: Your 10-character Apple Developer Team ID.

---

## 4. Submitting False Positives to Antivirus Vendors

If a newly published release triggers a heuristic warning before community reputation builds up:

### Microsoft Defender Security Intelligence (WDSI)
1. Visit [Microsoft Security Intelligence Submission Portal](https://www.microsoft.com/en-us/wdsi/filesubmission).
2. Select **Software developer** submission.
3. Upload `airmedia.exe` and `airmedia-setup.exe`.
4. Fill in product name: `AirMedia`, vendor: `AirMedia Open Source`.
5. Under detection details, specify: *"Clean open-source Rust media player. False positive heuristic detection."*
6. Microsoft typically analyzes and clears legitimate open-source submissions within 2 to 6 hours, pushing updated definitions globally to Windows Defender.

### VirusTotal
1. Scan the release archive or installer on [VirusTotal](https://www.virustotal.com/).
2. If any lesser-known AV engines show false-positive detections, use the "Vote / Comment" feature and submit false-positive reports directly via their vendor links.

---

## 5. User Instructions for Unblocking Fresh Releases

### Windows (SmartScreen)
- Click **More info** on the blue SmartScreen prompt, then click **Run anyway**.
- Alternatively, right-click the downloaded `.zip` or `.exe`, select **Properties**, check **Unblock** at the bottom, and click **OK**.

### macOS (Gatekeeper)
- Right-click (or Control-click) `AirMedia.app` and choose **Open**, then click **Open** in the dialog.
- Or run in Terminal:
  ```bash
  xattr -d com.apple.quarantine /Applications/AirMedia.app
  ```
