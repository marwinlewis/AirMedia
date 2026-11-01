<#
.SYNOPSIS
    Production Release Build Script for AirMedia on Windows.

.DESCRIPTION
    Builds the optimized production binary with LTO and stripped symbols,
    runs automated tests, stages distribution files, and creates a ready-to-distribute
    ZIP package (~12 MB). FFmpeg and FFprobe are treated as prerequisites;
    they are downloaded on the fly by the installer or in-app dialog if not already
    preinstalled on the user's system.

.PARAMETER BundleFfmpeg
    Explicitly bundles ffmpeg.exe and ffprobe.exe alongside airmedia.exe if requested.
    Default is $false to keep the build lightweight (~12.5 MB vs 437 MB).

.PARAMETER SkipTests
    Skips the pre-build test validation suite.

.PARAMETER CreateZip
    Compresses the distribution directory into a ready-to-ship ZIP file.
#>

param(
    [switch]$BundleFfmpeg = $false,
    [switch]$SkipTests = $false,
    [switch]$CreateZip = $true
)

$ErrorActionPreference = "Stop"

$ProjectRoot = (Resolve-Path "$PSScriptRoot\..").Path
Set-Location $ProjectRoot

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "     AirMedia - Production Release Build (Windows x64)     " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Check Toolchain
Write-Host "[1/6] Verifying Rust toolchain..." -ForegroundColor Yellow
$RustVersion = rustc --version
$CargoVersion = cargo --version
Write-Host "  $RustVersion" -ForegroundColor Gray
Write-Host "  $CargoVersion" -ForegroundColor Gray

# 2. Run Test Suite
if (-not $SkipTests) {
    Write-Host "[2/6] Running automated test suite..." -ForegroundColor Yellow
    cargo test --quiet
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Tests failed! Aborting production build."
        exit $LASTEXITCODE
    }
    Write-Host "  All tests passed successfully!" -ForegroundColor Green
} else {
    Write-Host "[2/6] Skipping test suite as requested." -ForegroundColor DarkGray
}

# 3. Build Optimized Release Binary
Write-Host "[3/6] Compiling optimized release binary..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo release build failed."
    exit $LASTEXITCODE
}
Write-Host "  Cargo build completed!" -ForegroundColor Green

# 4. Prepare Distribution Directory
$DistDir = Join-Path $ProjectRoot "dist\airmedia-windows-x64"
if (Test-Path $DistDir) {
    Remove-Item -Recurse -Force $DistDir
}
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

Write-Host "[4/6] Staging distribution files in $DistDir..." -ForegroundColor Yellow

$ExeSource = Join-Path $ProjectRoot "target\release\airmedia.exe"
Copy-Item $ExeSource (Join-Path $DistDir "airmedia.exe") -Force

Copy-Item (Join-Path $ProjectRoot "README.md") (Join-Path $DistDir "README.md") -Force
if (Test-Path (Join-Path $ProjectRoot "LICENSE")) {
    Copy-Item (Join-Path $ProjectRoot "LICENSE") (Join-Path $DistDir "LICENSE") -Force
}

# Stage installer scripts
Copy-Item (Join-Path $ProjectRoot "scripts\install_windows.ps1") (Join-Path $DistDir "install.ps1") -Force
Copy-Item (Join-Path $ProjectRoot "scripts\install_windows.bat") (Join-Path $DistDir "install.bat") -Force

# 5. Handle FFmpeg/FFprobe Prerequisites
if ($BundleFfmpeg) {
    Write-Host "[5/6] Locating and bundling FFmpeg / FFprobe binaries (Heavy Bundle Mode)..." -ForegroundColor Yellow
    $FfmpegCmd = Get-Command "ffmpeg.exe" -ErrorAction SilentlyContinue
    $FfprobeCmd = Get-Command "ffprobe.exe" -ErrorAction SilentlyContinue

    if ($FfmpegCmd -and $FfprobeCmd) {
        Write-Host "  Bundling: $($FfmpegCmd.Source)" -ForegroundColor Gray
        Copy-Item $FfmpegCmd.Source (Join-Path $DistDir "ffmpeg.exe") -Force

        Write-Host "  Bundling: $($FfprobeCmd.Source)" -ForegroundColor Gray
        Copy-Item $FfprobeCmd.Source (Join-Path $DistDir "ffprobe.exe") -Force
        Write-Host "  FFmpeg dependencies bundled successfully!" -ForegroundColor Green
    } else {
        Write-Host "  Warning: ffmpeg.exe or ffprobe.exe was not found in PATH." -ForegroundColor DarkYellow
    }
} else {
    Write-Host "[5/6] Lean mode: Codecs treated as prerequisites (Zero heavy binaries bundled)." -ForegroundColor Green
    Write-Host "      Preinstalled FFmpeg/FFprobe will be auto-detected on target systems." -ForegroundColor Gray
    Write-Host "      If missing, installer/app downloads them on the fly." -ForegroundColor Gray
}

# 6. Create Distribution Archive
if ($CreateZip) {
    Write-Host "[6/6] Creating release archive (.zip)..." -ForegroundColor Yellow
    $ZipPath = Join-Path $ProjectRoot "dist\airmedia-v0.1.0-windows-x64.zip"
    if (Test-Path $ZipPath) {
        Remove-Item -Force $ZipPath
    }

    Compress-Archive -Path "$DistDir\*" -DestinationPath $ZipPath -CompressionLevel Optimal
    $ZipSize = (Get-Item $ZipPath).Length / 1MB
    $ZipHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash

    Write-Host "  ZIP Archive:  $ZipPath" -ForegroundColor Green
    Write-Host "  Archive Size: $($ZipSize.ToString('F2')) MB" -ForegroundColor Gray
    Write-Host "  SHA-256:      $ZipHash" -ForegroundColor Gray
}

$ExeSize = (Get-Item (Join-Path $DistDir "airmedia.exe")).Length / 1MB
$TotalDistSize = (Get-ChildItem -Path $DistDir -Recurse | Measure-Object -Property Length -Sum).Sum / 1MB

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  AirMedia Windows x64 Build Succeeded!" -ForegroundColor Green
Write-Host "  Executable Size:       $($ExeSize.ToString('F2')) MB" -ForegroundColor Gray
Write-Host "  Distribution Dir Size: $($TotalDistSize.ToString('F2')) MB" -ForegroundColor Green
Write-Host "  Output Directory:      $DistDir" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
