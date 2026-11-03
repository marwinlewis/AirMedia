<#
.SYNOPSIS
    Production Release Build Script for AirMedia on Windows.

.DESCRIPTION
    Builds the optimized production binary with LTO, stripped symbols, embedded PE VERSIONINFO,
    and native Windows Application Manifest (asInvoker, DPI awareness, Windows 10/11 compatibility).
    Generates both an industry-standard Inno Setup installer (.exe) and a clean portable package (.zip)
    along with SHA-256 checksums. Supports automated Authenticode Code Signing when credentials are provided.

.PARAMETER BundleFfmpeg
    Explicitly bundles ffmpeg.exe and ffprobe.exe alongside airmedia.exe if requested.
    Default is $false to keep the build lightweight (~12 MB vs 400+ MB).

.PARAMETER SkipTests
    Skips the pre-build test validation suite.

.PARAMETER CreateZip
    Compresses the distribution directory into a ready-to-ship ZIP file.
#>

param(
    [string]$Version = "",
    [switch]$BundleFfmpeg = $false,
    [switch]$SkipTests = $false,
    [switch]$CreateZip = $true
)

$ErrorActionPreference = "Stop"

$ProjectRoot = (Resolve-Path "$PSScriptRoot\..").Path
Set-Location $ProjectRoot

# Extract single source of truth metadata from Cargo.toml
$CargoToml = Get-Content (Join-Path $ProjectRoot "Cargo.toml") -Raw
$AppName = "AirMedia"
$AppPublisher = "AirMedia Open Source Project"
$AppURL = "https://github.com/marwinlewis/airmedia"

if (-not $Version) {
    if ($env:RELEASE_VERSION) {
        $Version = $env:RELEASE_VERSION.TrimStart('v')
    } else {
        if ($CargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
            $Version = $Matches[1]
        } else {
            $Version = "0.1.0"
        }
    }
}
$Version = $Version.TrimStart('v')

if ($CargoToml -match '(?m)^authors\s*=\s*\["([^"<]+)') {
    $AppPublisher = $Matches[1].Trim()
}
if ($CargoToml -match '(?m)^repository\s*=\s*"([^"]+)"') {
    $AppURL = $Matches[1].Trim()
}

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "     AirMedia - Production Release Build (Windows x64)     " -ForegroundColor Cyan
Write-Host "     Version:   v$Version                                  " -ForegroundColor Cyan
Write-Host "     Publisher: $AppPublisher                              " -ForegroundColor Cyan
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

# 3. Build Optimized Release Binary with Embedded PE Resources & Manifest
Write-Host "[3/6] Compiling release binary with embedded PE metadata & manifest..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo release build failed."
    exit $LASTEXITCODE
}
Write-Host "  Cargo build completed successfully!" -ForegroundColor Green

$ExeSource = Join-Path $ProjectRoot "target\release\airmedia.exe"

# Helper for Authenticode Code Signing (Crucial to avoid SmartScreen / AV warnings)
function Invoke-CodeSigning {
    param([string]$FilePath)

    $CertFile = $env:WINDOWS_CERTIFICATE_PATH
    $CertPass = $env:WINDOWS_CERTIFICATE_PASSWORD

    if (-not $CertFile) {
        Write-Host "  Note: Code signing skipped (no certificate configured in WINDOWS_CERTIFICATE_PATH)." -ForegroundColor DarkGray
        return
    }
    if (-not (Test-Path $CertFile)) {
        Write-Error "WINDOWS_CERTIFICATE_PATH points to '$CertFile', which does not exist."
    }

    # signtool.exe ships with the Windows SDK and is usually not on PATH.
    $SignTool = Get-Command "signtool.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1
    if (-not $SignTool) {
        $SignTool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $SignTool) {
        Write-Error "signtool.exe not found. Install the Windows SDK (winget install Microsoft.WindowsSDK.10.0.26100)."
    }

    Write-Host "  Signing $FilePath with Authenticode certificate..." -ForegroundColor Cyan
    & $SignTool sign /fd SHA256 /f $CertFile /p $CertPass /tr "http://timestamp.digicert.com" /td SHA256 $FilePath
    if ($LASTEXITCODE -ne 0) {
        Write-Error "signtool failed with exit code $LASTEXITCODE for $FilePath"
    }
    Write-Host "  [OK] Successfully signed: $FilePath" -ForegroundColor Green
}

Invoke-CodeSigning -FilePath $ExeSource

# 4. Prepare Distribution Directory
$DistDir = Join-Path $ProjectRoot "dist\airmedia-windows-x64"
Get-Process -Name "airmedia" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
if (Test-Path $DistDir) {
    Remove-Item -Recurse -Force $DistDir
}
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $ProjectRoot "dist") -Force | Out-Null

Write-Host "[4/6] Staging distribution files in $DistDir..." -ForegroundColor Yellow
Copy-Item $ExeSource (Join-Path $DistDir "airmedia.exe") -Force
Copy-Item (Join-Path $ProjectRoot "README.md") (Join-Path $DistDir "README.md") -Force
if (Test-Path (Join-Path $ProjectRoot "LICENSE")) {
    Copy-Item (Join-Path $ProjectRoot "LICENSE") (Join-Path $DistDir "LICENSE") -Force
}
if (Test-Path (Join-Path $ProjectRoot "assets\icon.png")) {
    Copy-Item (Join-Path $ProjectRoot "assets\icon.png") (Join-Path $DistDir "icon.png") -Force
}

# Stage safe portable installer scripts
Copy-Item (Join-Path $ProjectRoot "scripts\install_windows.ps1") (Join-Path $DistDir "install.ps1") -Force
Copy-Item (Join-Path $ProjectRoot "scripts\install_windows.bat") (Join-Path $DistDir "install.bat") -Force

# 5. Build Native Inno Setup Installer if available
Write-Host "[5/6] Checking for Inno Setup compiler (ISCC)..." -ForegroundColor Yellow
$IsccCandidates = @(
    (Get-Command "iscc.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue),
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)
$IsccExe = $IsccCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1

if ($IsccExe) {
    Write-Host "  Found Inno Setup Compiler: $IsccExe" -ForegroundColor Green
    Write-Host "  Compiling native Windows installer..." -ForegroundColor Yellow

    $IssScript = Join-Path $ProjectRoot "scripts\installer.iss"
    & "$IsccExe" "/DMyAppName=$AppName" "/DMyAppVersion=$Version" "/DMyAppPublisher=$AppPublisher" "/DMyAppURL=$AppURL" "$IssScript"
    if ($LASTEXITCODE -eq 0) {
        $SetupExeName = "airmedia-v$Version-windows-x64-setup.exe"
        $SetupExePath = Join-Path $ProjectRoot "dist\$SetupExeName"
        if (Test-Path $SetupExePath) {
            Invoke-CodeSigning -FilePath $SetupExePath
            $SetupHash = (Get-FileHash -Path $SetupExePath -Algorithm SHA256).Hash
            "$SetupHash  $SetupExeName" | Out-File -FilePath "$SetupExePath.sha256" -Encoding ascii -NoNewline
            Write-Host "  [OK] Inno Setup Installer created: $SetupExePath" -ForegroundColor Green
            Write-Host "  [OK] Installer SHA-256: $SetupHash" -ForegroundColor Gray
        }
    } else {
        Write-Host "  Warning: Inno Setup compilation exited with code $LASTEXITCODE." -ForegroundColor DarkYellow
    }
} else {
    Write-Host "  Inno Setup (iscc.exe) not installed on local system." -ForegroundColor DarkGray
    Write-Host "  (To build native setup.exe locally, run: winget install JRSoftware.InnoSetup)" -ForegroundColor DarkGray
}

# 6. Create Distribution Archive (Portable ZIP)
if ($CreateZip) {
    Write-Host "[6/6] Creating release archive (.zip)..." -ForegroundColor Yellow
    $ZipName = "airmedia-v$Version-windows-x64.zip"
    $ZipPath = Join-Path $ProjectRoot "dist\$ZipName"
    if (Test-Path $ZipPath) {
        Remove-Item -Force $ZipPath
    }

    Compress-Archive -Path "$DistDir\*" -DestinationPath $ZipPath -CompressionLevel Optimal
    $ZipSize = (Get-Item $ZipPath).Length / 1MB
    $ZipHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash

    $Sha256Path = "$ZipPath.sha256"
    "$ZipHash  $ZipName" | Out-File -FilePath $Sha256Path -Encoding ascii -NoNewline

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
