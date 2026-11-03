<#
.SYNOPSIS
    AirMedia Windows Desktop Setup Helper.

.DESCRIPTION
    Installs AirMedia to %LOCALAPPDATA%\Programs\AirMedia, creates desktop/start-menu shortcuts,
    and checks codec prerequisites.
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
    $ParentExe = Join-Path (Split-Path $SourceDir -Parent) "airmedia.exe"
    if (Test-Path $ParentExe) {
        $SourceDir = Split-Path $SourceDir -Parent
        $SourceExe = $ParentExe
    } else {
        $ReleaseExe = Join-Path (Split-Path $SourceDir -Parent) "target\release\airmedia.exe"
        if (Test-Path $ReleaseExe) {
            $SourceExe = $ReleaseExe
        }
    }
}

if (-not (Test-Path $SourceExe)) {
    Write-Error "Could not find airmedia.exe in '$SourceDir'. Please ensure the build completed before installing."
    exit 1
}

# 1. Prerequisite Check: FFmpeg & FFprobe
Write-Host ""
Write-Host "[1/3] Checking codec prerequisites (FFmpeg & FFprobe)..." -ForegroundColor Yellow

$SysFfmpeg = Get-Command "ffmpeg.exe" -ErrorAction SilentlyContinue
$SysFfprobe = Get-Command "ffprobe.exe" -ErrorAction SilentlyContinue

if ($SysFfmpeg -and $SysFfprobe) {
    Write-Host "  [OK] System codecs detected: $($SysFfmpeg.Source)" -ForegroundColor Green
} else {
    Write-Host "  [!] Codecs not detected in PATH." -ForegroundColor DarkYellow
    Write-Host "      You can install FFmpeg anytime with: winget install Gyan.FFmpeg" -ForegroundColor Gray
    Write-Host "      Or use AirMedia's built-in Codec Setup Manager on first launch." -ForegroundColor Gray
}

# 2. Install AirMedia Binary to User Programs directory
Write-Host ""
Write-Host "[2/3] Installing AirMedia application..." -ForegroundColor Yellow
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\AirMedia"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Copy-Item $SourceExe (Join-Path $InstallDir "airmedia.exe") -Force
$InstalledExe = Join-Path $InstallDir "airmedia.exe"

foreach ($file in @("README.md", "LICENSE", "icon.png")) {
    $src = Join-Path $SourceDir $file
    if (Test-Path $src) {
        Copy-Item $src (Join-Path $InstallDir $file) -Force
    }
}

Write-Host "  [OK] Installed to: $InstalledExe" -ForegroundColor Green

# 3. Create Shortcuts
Write-Host ""
Write-Host "[3/3] Creating application shortcuts..." -ForegroundColor Yellow
$WshShell = New-Object -ComObject WScript.Shell

$DesktopPath = [Environment]::GetFolderPath("Desktop")
$DesktopLnk = Join-Path $DesktopPath "AirMedia.lnk"
$Shortcut = $WshShell.CreateShortcut($DesktopLnk)
$Shortcut.TargetPath = $InstalledExe
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "AirMedia - Ultra-Lightweight Modern GPU Media Player"
$Shortcut.Save()
Write-Host "  [OK] Desktop shortcut created." -ForegroundColor Gray

$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\AirMedia"
if (-not (Test-Path $StartMenuDir)) {
    New-Item -ItemType Directory -Path $StartMenuDir -Force | Out-Null
}
$StartMenuLnk = Join-Path $StartMenuDir "AirMedia.lnk"
$SmShortcut = $WshShell.CreateShortcut($StartMenuLnk)
$SmShortcut.TargetPath = $InstalledExe
$SmShortcut.WorkingDirectory = $InstallDir
$SmShortcut.Description = "AirMedia - Ultra-Lightweight Modern GPU Media Player"
$SmShortcut.Save()
Write-Host "  [OK] Start Menu shortcut created." -ForegroundColor Gray

Write-Host ""
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "         AirMedia Installation Finished!                  " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Cyan

if (-not $NoLaunch -and -not $Silent) {
    Start-Process $InstalledExe
}
