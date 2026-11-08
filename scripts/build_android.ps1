# AirMedia Android Production Build Script (PowerShell)
# Compiles Rust cdylib via cargo-ndk, packages Android assets & resources, and builds APK
$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $ProjectRoot

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  AirMedia Android Production Release Build" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Extract Version from Cargo.toml
$CargoToml = Get-Content (Join-Path $ProjectRoot "Cargo.toml") -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    $Version = "0.1.0"
}
Write-Host "==> Building AirMedia v$Version for Android..." -ForegroundColor Cyan

# 2. Synchronize High-Resolution Assets
Write-Host "[1/5] Synchronizing multi-resolution icons from dist/airmedia.png..." -ForegroundColor Yellow
cargo run --example generate_assets
if ($LASTEXITCODE -ne 0) {
    Write-Host "Asset generation failed" -ForegroundColor Red
    exit 1
}

# 3. Ensure cargo-ndk and Rust Android Targets
Write-Host "[2/5] Checking Android Rust toolchain targets..." -ForegroundColor Yellow
$InstalledTargets = rustup target list --installed
$RequiredTargets = @("aarch64-linux-android", "armv7-linux-androideabi", "x86_64-linux-android")

foreach ($t in $RequiredTargets) {
    if ($InstalledTargets -notcontains $t) {
        Write-Host "    Installing missing target: $t..."
        rustup target add $t
    }
}

if (-not (Get-Command "cargo-ndk" -ErrorAction SilentlyContinue)) {
    Write-Host "    Installing cargo-ndk build tool..."
    cargo install cargo-ndk --locked
}

# 4. Compile Native cdylib for Supported ABIs
Write-Host "[3/5] Compiling native Rust shared libraries (.so) via cargo-ndk..." -ForegroundColor Yellow
$JniLibsDir = Join-Path $ProjectRoot "android\app\src\main\jniLibs"
if (-not (Test-Path $JniLibsDir)) {
    New-Item -ItemType Directory -Path $JniLibsDir -Force | Out-Null
}

# AAudio (linked by cpal) only exists from API 26; cargo-ndk's default platform is older.
cargo ndk `
    --platform 26 `
    -t arm64-v8a `
    -t armeabi-v7a `
    -t x86_64 `
    -o $JniLibsDir `
    build --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Native compilation failed" -ForegroundColor Red
    exit 1
}

# 5. Assemble APK via Gradle
Write-Host "[4/5] Building Android APK with Gradle..." -ForegroundColor Yellow
Push-Location (Join-Path $ProjectRoot "android")

if (Test-Path ".\gradlew.bat") {
    $GradleCmd = ".\gradlew.bat"
} elseif (Get-Command "gradle" -ErrorAction SilentlyContinue) {
    $GradleCmd = "gradle"
} else {
    Write-Host "ERROR: Gradle not found! Please install Gradle or configure gradlew." -ForegroundColor Red
    exit 1
}

& $GradleCmd assembleRelease --no-daemon
Pop-Location

# 6. Stage APKs to dist/ directory
Write-Host "[5/5] Staging distribution APKs to dist/..." -ForegroundColor Yellow
$DistDir = Join-Path $ProjectRoot "dist"
if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
}

$ApkSrc = Join-Path $ProjectRoot "android\app\build\outputs\apk\release"
if (-not (Test-Path $ApkSrc)) {
    $ApkSrc = Join-Path $ProjectRoot "android\app\build\outputs\apk\debug"
}

$ApkFiles = Get-ChildItem -Path $ApkSrc -Filter "*.apk" -ErrorAction SilentlyContinue
if ($ApkFiles) {
    $DestName = "airmedia-v$Version-android-universal.apk"
    $DestPath = Join-Path $DistDir $DestName
    Copy-Item $ApkFiles[0].FullName $DestPath -Force
    Write-Host "    Staged: $DestPath" -ForegroundColor Green

    # Calculate SHA-256
    $Hash = (Get-FileHash -Path $DestPath -Algorithm SHA256).Hash.ToLower()
    Set-Content -Path (Join-Path $DistDir "$DestName.sha256") -Value "$Hash  $DestName"

    Write-Host "==========================================================" -ForegroundColor Green
    Write-Host "  Android APK build succeeded!" -ForegroundColor Green
    Write-Host "  Package: dist/$DestName" -ForegroundColor Green
    Write-Host "==========================================================" -ForegroundColor Green
} else {
    Write-Host "ERROR: No APK output found in $ApkSrc" -ForegroundColor Red
    exit 1
}

Pop-Location
