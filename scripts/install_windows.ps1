<#
.SYNOPSIS
    AirMedia Windows Production Installer with On-the-Fly Prerequisite Codec Setup.

.DESCRIPTION
    Installs the AirMedia GPU-accelerated media player.
    1. Checks for preinstalled FFmpeg and FFprobe codecs (in PATH, local app dir, or portable).
    2. If preinstalled codecs are found, uses them immediately with ZERO downloads.
    3. If missing, downloads the official minimal essentials codec package on the fly
       into %LOCALAPPDATA%\AirMedia\bin and registers it.
    4. Installs AirMedia to %LOCALAPPDATA%\Programs\AirMedia.
    5. Creates Start Menu and Desktop shortcuts.

.PARAMETER Silent
    Runs the installation without interactive prompts.

.PARAMETER NoLaunch
    Does not launch AirMedia after installation finishes.
#>

param(
    [switch]$Silent = $false,
    [switch]$NoLaunch = $false
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "       AirMedia - Modern Media Player Installation        " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

$SourceDir = $PSScriptRoot
$SourceExe = Join-Path $SourceDir "airmedia.exe"

if (-not (Test-Path $SourceExe)) {
    # Check parent directory if run from scripts folder
    $ParentExe = Join-Path (Split-Path $SourceDir -Parent) "airmedia.exe"
    if (Test-Path $ParentExe) {
        $SourceDir = Split-Path $SourceDir -Parent
        $SourceExe = $ParentExe
    } else {
        # Check target/release
        $ReleaseExe = Join-Path (Split-Path $SourceDir -Parent) "target\release\airmedia.exe"
        if (Test-Path $ReleaseExe) {
            $SourceExe = $ReleaseExe
        }
    }
}

if (-not (Test-Path $SourceExe)) {
    Write-Error "Could not find airmedia.exe in '$SourceDir'. Please build before installing."
    exit 1
}

# 1. Prerequisite Check: FFmpeg & FFprobe
Write-Host ""
Write-Host "[1/4] Checking prerequisite codecs (FFmpeg & FFprobe)..." -ForegroundColor Yellow

$FfmpegPath = $null
$FfprobePath = $null
$CodecSource = "None"

# Check A: Next to source exe (portable mode)
$PortableFfmpeg = Join-Path $SourceDir "ffmpeg.exe"
$PortableFfprobe = Join-Path $SourceDir "ffprobe.exe"
if ((Test-Path $PortableFfmpeg) -and (Test-Path $PortableFfprobe)) {
    $FfmpegPath = $PortableFfmpeg
    $FfprobePath = $PortableFfprobe
    $CodecSource = "Portable (Distribution Folder)"
}

# Check B: LocalAppData AirMedia bin
if (-not $FfmpegPath) {
    $AppBin = Join-Path $env:LOCALAPPDATA "AirMedia\bin"
    $LocalFfmpeg = Join-Path $AppBin "ffmpeg.exe"
    $LocalFfprobe = Join-Path $AppBin "ffprobe.exe"
    if ((Test-Path $LocalFfmpeg) -and (Test-Path $LocalFfprobe)) {
        $FfmpegPath = $LocalFfmpeg
        $FfprobePath = $LocalFfprobe
        $CodecSource = "Local AppData ($AppBin)"
    }
}

# Check C: System PATH (Preinstalled codecs)
if (-not $FfmpegPath) {
    $SysFfmpeg = Get-Command "ffmpeg.exe" -ErrorAction SilentlyContinue
    $SysFfprobe = Get-Command "ffprobe.exe" -ErrorAction SilentlyContinue
    if ($SysFfmpeg -and $SysFfprobe) {
        $FfmpegPath = $SysFfmpeg.Source
        $FfprobePath = $SysFfprobe.Source
        $CodecSource = "Preinstalled System Codec (PATH)"
    }
}

if ($FfmpegPath -and $FfprobePath) {
    Write-Host "  [OK] Preinstalled codecs detected!" -ForegroundColor Green
    Write-Host "       Source:  $CodecSource" -ForegroundColor Gray
    Write-Host "       FFmpeg:  $FfmpegPath" -ForegroundColor Gray
    Write-Host "       FFprobe: $FfprobePath" -ForegroundColor Gray
    Write-Host "  [OK] Zero downloads needed! AirMedia will use existing preinstalled codecs." -ForegroundColor Green
} else {
    Write-Host "  [!] Codecs not found on system (FFmpeg / FFprobe required for playback)." -ForegroundColor DarkYellow
    Write-Host "  [*] Downloading official minimal FFmpeg codecs on the fly..." -ForegroundColor Cyan

    $TargetBinDir = Join-Path $env:LOCALAPPDATA "AirMedia\bin"
    if (-not (Test-Path $TargetBinDir)) {
        New-Item -ItemType Directory -Path $TargetBinDir -Force | Out-Null
    }

    $ZipPath = Join-Path $env:TEMP "airmedia-ffmpeg-download.zip"
    $ExtractDir = Join-Path $env:TEMP "airmedia-ffmpeg-extract"
    $DownloadUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"

    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
        Write-Host "      Downloading from official release mirror..." -ForegroundColor Gray
        
        $Curl = Get-Command "curl.exe" -ErrorAction SilentlyContinue
        if ($Curl) {
            & $Curl.Source -L --progress-bar -o $ZipPath $DownloadUrl
        } else {
            Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
        }

        Write-Host "      Extracting codec binaries..." -ForegroundColor Gray
        if (Test-Path $ExtractDir) { Remove-Item -Recurse -Force $ExtractDir -ErrorAction SilentlyContinue }
        Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

        $FfmpegFound = Get-ChildItem -Path $ExtractDir -Filter "ffmpeg.exe" -Recurse | Select-Object -First 1
        $FfprobeFound = Get-ChildItem -Path $ExtractDir -Filter "ffprobe.exe" -Recurse | Select-Object -First 1

        if (-not $FfmpegFound -or -not $FfprobeFound) {
            throw "FFmpeg binaries were not found inside the downloaded archive."
        }

        Copy-Item $FfmpegFound.FullName (Join-Path $TargetBinDir "ffmpeg.exe") -Force
        Copy-Item $FfprobeFound.FullName (Join-Path $TargetBinDir "ffprobe.exe") -Force

        # Cleanup temporary files
        Remove-Item -Recurse -Force $ExtractDir -ErrorAction SilentlyContinue
        Remove-Item -Force $ZipPath -ErrorAction SilentlyContinue

        Write-Host "  [OK] Codecs installed successfully into: $TargetBinDir" -ForegroundColor Green

        # Ensure user PATH has this folder
        $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($UserPath -notlike "*$TargetBinDir*") {
            [Environment]::SetEnvironmentVariable("Path", "$UserPath;$TargetBinDir", "User")
            Write-Host "  [OK] Added codec directory to user PATH." -ForegroundColor Gray
        }
    } catch {
        Write-Host "  [!] Automated codec download encountered an issue: $_" -ForegroundColor DarkYellow
        Write-Host "      You can install FFmpeg manually with: winget install Gyan.FFmpeg" -ForegroundColor DarkYellow
        Write-Host "      AirMedia will also offer on-the-fly download upon app launch." -ForegroundColor DarkYellow
    }
}

# 2. Install AirMedia Binary
Write-Host ""
Write-Host "[2/4] Installing AirMedia application..." -ForegroundColor Yellow
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\AirMedia"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Copy-Item $SourceExe (Join-Path $InstallDir "airmedia.exe") -Force
$InstalledExe = Join-Path $InstallDir "airmedia.exe"

$ReadmeSource = Join-Path $SourceDir "README.md"
if (Test-Path $ReadmeSource) {
    Copy-Item $ReadmeSource (Join-Path $InstallDir "README.md") -Force
}

$LicenseSource = Join-Path $SourceDir "LICENSE"
if (Test-Path $LicenseSource) {
    Copy-Item $LicenseSource (Join-Path $InstallDir "LICENSE") -Force
}

Write-Host "  [OK] Installed to: $InstalledExe" -ForegroundColor Green

# 3. Create Desktop and Start Menu Shortcuts
Write-Host ""
Write-Host "[3/4] Creating application shortcuts..." -ForegroundColor Yellow
$WshShell = New-Object -ComObject WScript.Shell

# Desktop shortcut
$DesktopPath = [Environment]::GetFolderPath("Desktop")
$DesktopLnk = Join-Path $DesktopPath "AirMedia.lnk"
$Shortcut = $WshShell.CreateShortcut($DesktopLnk)
$Shortcut.TargetPath = $InstalledExe
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "AirMedia - Ultra-Lightweight Modern GPU Media Player"
$Shortcut.Save()
Write-Host "  [OK] Desktop shortcut created." -ForegroundColor Gray

# Start Menu shortcut
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\AirMedia"
if (-not (Test-Path $StartMenuDir)) {
    New-Item -ItemType Directory -Path $StartMenuDir -Force | Out-Null
}
$StartMenuLnk = Join-Path $StartMenuDir "AirMedia.lnk"
$ShortcutStart = $WshShell.CreateShortcut($StartMenuLnk)
$ShortcutStart.TargetPath = $InstalledExe
$ShortcutStart.WorkingDirectory = $InstallDir
$ShortcutStart.Description = "AirMedia - Ultra-Lightweight Modern GPU Media Player"
$ShortcutStart.Save()
Write-Host "  [OK] Start Menu shortcut created." -ForegroundColor Gray

# 4. Finish
Write-Host ""
Write-Host "[4/4] Finalizing setup..." -ForegroundColor Yellow
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "      AirMedia has been successfully installed!           " -ForegroundColor Green
Write-Host "      Location: $InstalledExe                             " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Green

if (-not $NoLaunch) {
    Write-Host ""
    Write-Host "Launching AirMedia..." -ForegroundColor Cyan
    Start-Process $InstalledExe
}
