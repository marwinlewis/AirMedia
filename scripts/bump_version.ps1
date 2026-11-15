<#
.SYNOPSIS
    Bumps the AirMedia version across all supported platforms and build configurations.

.DESCRIPTION
    Synchronizes the version across all platforms:
      1. Cargo.toml                 - Core Rust package version (single source of truth)
      2. Cargo.lock                 - Updated via `cargo check`
      3. android/app/build.gradle   - versionName and auto-incremented versionCode
      4. assets/windows/airmedia.manifest - Win32 assembly 4-part version (X.Y.Z.0)
      5. scripts/installer.iss      - Inno Setup fallback version definition
      6. assets/macos/Info.plist    - CFBundleVersion & CFBundleShortVersionString

.PARAMETER Target
    Version bump type ('patch', 'minor', 'major') or explicit version ('0.2.0', 'v0.2.0').

.PARAMETER AndroidVersionCode
    Optional integer override for Android versionCode. Defaults to current versionCode + 1.

.PARAMETER DryRun
    Preview the version changes without writing files to disk.

.PARAMETER Commit
    Create a git commit with the bumped version files.

.PARAMETER Tag
    Create a git tag (e.g. v0.2.0).

.PARAMETER Push
    Push git commit and tags to remote origin.

.EXAMPLE
    .\scripts\bump_version.ps1 patch
    .\scripts\bump_version.ps1 minor -Commit -Tag
    .\scripts\bump_version.ps1 0.2.0
    .\scripts\bump_version.ps1 patch -DryRun
#>

[CmdletBinding()]
param(
    [Parameter(Position = 0, Mandatory = $false)]
    [string]$Target = "",

    [Parameter(Mandatory = $false)]
    [int]$AndroidVersionCode = 0,

    [switch]$DryRun,
    [switch]$Commit,
    [switch]$Tag,
    [switch]$Push
)

$ErrorActionPreference = "Stop"

# Ensure UTF-8 without BOM output
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

# Resolve repository root
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $ProjectRoot

# Paths
$CargoTomlPath      = Join-Path $ProjectRoot "Cargo.toml"
$CargoLockPath      = Join-Path $ProjectRoot "Cargo.lock"
$AndroidGradlePath  = Join-Path $ProjectRoot "android\app\build.gradle"
$WinManifestPath    = Join-Path $ProjectRoot "assets\windows\airmedia.manifest"
$InnoScriptPath     = Join-Path $ProjectRoot "scripts\installer.iss"
$MacPlistPath       = Join-Path $ProjectRoot "assets\macos\Info.plist"

if (-not (Test-Path $CargoTomlPath)) {
    Write-Error "Cargo.toml not found at '$CargoTomlPath'. Please run from the AirMedia repository."
    exit 1
}

# 1. Read Current Version from Cargo.toml
$CargoTomlRaw = [System.IO.File]::ReadAllText($CargoTomlPath)
if ($CargoTomlRaw -match '(?m)^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"') {
    $CurrentVersion = $Matches[1]
} else {
    Write-Error "Could not parse [package] version in $CargoTomlPath"
    exit 1
}

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  AirMedia Multi-Platform Version Bump Tool" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "Current version: $CurrentVersion" -ForegroundColor Yellow

# Interactive prompt if Target is not provided
if ([string]::IsNullOrWhiteSpace($Target)) {
    Write-Host ""
    Write-Host "Options: 'patch', 'minor', 'major', or specific version (e.g. '0.2.0')" -ForegroundColor Gray
    $Target = Read-Host "Enter target version or bump type"
    if ([string]::IsNullOrWhiteSpace($Target)) {
        Write-Host "No version specified. Aborted." -ForegroundColor Red
        exit 0
    }
}

$Target = $Target.Trim()
$TargetLower = $Target.ToLowerInvariant()

# Parse current version components (semver)
$VersionRegex = '^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$'
if ($CurrentVersion -match $VersionRegex) {
    $CurMajor = [int]$Matches[1]
    $CurMinor = [int]$Matches[2]
    $CurPatch = [int]$Matches[3]
} else {
    Write-Error "Current version '$CurrentVersion' does not match standard semver (X.Y.Z)."
    exit 1
}

# 2. Compute New Version
switch ($TargetLower) {
    "patch" {
        $NewMajor = $CurMajor
        $NewMinor = $CurMinor
        $NewPatch = $CurPatch + 1
        $NewVersion = "$NewMajor.$NewMinor.$NewPatch"
    }
    "minor" {
        $NewMajor = $CurMajor
        $NewMinor = $CurMinor + 1
        $NewPatch = 0
        $NewVersion = "$NewMajor.$NewMinor.$NewPatch"
    }
    "major" {
        $NewMajor = $CurMajor + 1
        $NewMinor = 0
        $NewPatch = 0
        $NewVersion = "$NewMajor.$NewMinor.$NewPatch"
    }
    default {
        $CleanTarget = $Target.TrimStart('v').Trim()
        if ($CleanTarget -match '^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$') {
            $NewMajor = [int]$Matches[1]
            $NewMinor = [int]$Matches[2]
            $NewPatch = [int]$Matches[3]
            $NewVersion = $CleanTarget
        } else {
            Write-Error "Invalid target version '$Target'. Expected 'patch', 'minor', 'major', or 'X.Y.Z'."
            exit 1
        }
    }
}

$WinFourPartVersion = "$NewMajor.$NewMinor.$NewPatch.0"

Write-Host "Target version:  $NewVersion" -ForegroundColor Green
Write-Host "Windows version: $WinFourPartVersion" -ForegroundColor Green

# 3. Parse Android Gradle
$AndroidGradleRaw = ""
$CurrentAndroidCode = 0
$NewAndroidCode = 0
if (Test-Path $AndroidGradlePath) {
    $AndroidGradleRaw = [System.IO.File]::ReadAllText($AndroidGradlePath)
    if ($AndroidGradleRaw -match '(?m)^\s*versionCode\s+(\d+)') {
        $CurrentAndroidCode = [int]$Matches[1]
    }
    if ($AndroidVersionCode -gt 0) {
        $NewAndroidCode = $AndroidVersionCode
    } else {
        $NewAndroidCode = $CurrentAndroidCode + 1
    }
    Write-Host "Android versionCode: $CurrentAndroidCode -> $NewAndroidCode" -ForegroundColor Green
}

if ($DryRun) {
    Write-Host "`n[DRY RUN MODE - No files will be modified]`n" -ForegroundColor Magenta
}

$ModifiedFiles = [System.Collections.Generic.List[string]]::new()

# Helper to write files
function Update-FileContent {
    param(
        [string]$Path,
        [string]$Content,
        [string]$Description
    )
    if ($DryRun) {
        Write-Host "  [DRY RUN] Would update: $Description ($Path)" -ForegroundColor Magenta
    } else {
        [System.IO.File]::WriteAllText($Path, $Content, $Utf8NoBom)
        Write-Host "  [UPDATED] $Description" -ForegroundColor Green
        $script:ModifiedFiles.Add($Path)
    }
}

Write-Host "`nUpdating platform manifest files..." -ForegroundColor Yellow

# A. Cargo.toml
$NewCargoToml = [regex]::Replace(
    $CargoTomlRaw,
    '((?m)^\[package\][\s\S]*?^version\s*=\s*)"[^"]+"',
    "`${1}`"$NewVersion`""
)
Update-FileContent -Path $CargoTomlPath -Content $NewCargoToml -Description "Cargo.toml (version = `"$NewVersion`")"

# B. Android build.gradle
if (Test-Path $AndroidGradlePath) {
    $NewGradle = $AndroidGradleRaw
    $NewGradle = [regex]::Replace($NewGradle, '(?m)^(\s*versionCode\s+)\d+', "`${1}$NewAndroidCode")
    $NewGradle = [regex]::Replace($NewGradle, '(?m)^(\s*versionName\s+)"[^"]+"', "`${1}`"$NewVersion`"")
    Update-FileContent -Path $AndroidGradlePath -Content $NewGradle -Description "android/app/build.gradle (versionName: `"$NewVersion`", versionCode: $NewAndroidCode)"
}

# C. Windows Manifest
if (Test-Path $WinManifestPath) {
    $WinManifestRaw = [System.IO.File]::ReadAllText($WinManifestPath)
    $NewWinManifest = [regex]::Replace(
        $WinManifestRaw,
        '(<assemblyIdentity\s+[^>]*version=")[^"]+(")',
        "`${1}$WinFourPartVersion`${2}"
    )
    Update-FileContent -Path $WinManifestPath -Content $NewWinManifest -Description "assets/windows/airmedia.manifest (version = `"$WinFourPartVersion`")"
}

# D. Inno Setup installer.iss
if (Test-Path $InnoScriptPath) {
    $InnoRaw = [System.IO.File]::ReadAllText($InnoScriptPath)
    $NewInno = [regex]::Replace(
        $InnoRaw,
        '(?m)^(#define\s+MyAppVersion\s+)"[^"]+"',
        "`${1}`"$NewVersion`""
    )
    Update-FileContent -Path $InnoScriptPath -Content $NewInno -Description "scripts/installer.iss (MyAppVersion = `"$NewVersion`")"
}

# E. macOS Info.plist
if (Test-Path $MacPlistPath) {
    $MacPlistRaw = [System.IO.File]::ReadAllText($MacPlistPath)
    $NewPlist = [regex]::Replace(
        $MacPlistRaw,
        '(<key>CFBundleVersion</key>\s*<string>)[^<]*(</string>)',
        "`${1}$NewVersion`${2}"
    )
    $NewPlist = [regex]::Replace(
        $NewPlist,
        '(<key>CFBundleShortVersionString</key>\s*<string>)[^<]*(</string>)',
        "`${1}$NewVersion`${2}"
    )
    Update-FileContent -Path $MacPlistPath -Content $NewPlist -Description "assets/macos/Info.plist (CFBundleVersion = `"$NewVersion`")"
}

# F. Update Cargo.lock via cargo check
if (-not $DryRun) {
    if (Get-Command "cargo" -ErrorAction SilentlyContinue) {
        Write-Host "`nUpdating Cargo.lock via cargo check..." -ForegroundColor Yellow
        & cargo check --quiet
        if ($LASTEXITCODE -eq 0 -and (Test-Path $CargoLockPath)) {
            Write-Host "  [UPDATED] Cargo.lock synchronized" -ForegroundColor Green
            $script:ModifiedFiles.Add($CargoLockPath)
        } else {
            Write-Host "  [WARNING] cargo check returned non-zero code ($LASTEXITCODE)." -ForegroundColor DarkYellow
        }
    } else {
        Write-Host "  [SKIP] 'cargo' not found in PATH; Cargo.lock not automatically refreshed." -ForegroundColor DarkGray
    }
}

Write-Host "`n==========================================================" -ForegroundColor Cyan
Write-Host "  Version bump to v$NewVersion completed successfully!" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Cyan

# 4. Git Integration (Commit, Tag, Push)
if (-not $DryRun -and ($Commit -or $Tag -or $Push)) {
    if (Get-Command "git" -ErrorAction SilentlyContinue) {
        Write-Host "`nRunning Git operations..." -ForegroundColor Yellow

        # Stage modified files
        foreach ($f in $ModifiedFiles) {
            & git add "$f"
        }

        # Commit
        $CommitMsg = "chore(release): bump version to $NewVersion"
        & git commit -m "$CommitMsg"
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  [GIT] Created commit: '$CommitMsg'" -ForegroundColor Green
        } else {
            Write-Host "  [GIT] Git commit skipped or nothing to commit." -ForegroundColor DarkYellow
        }

        # Tag
        if ($Tag -or $Push) {
            $TagName = "v$NewVersion"
            & git tag -a "$TagName" -m "AirMedia Release $TagName"
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  [GIT] Created tag: '$TagName'" -ForegroundColor Green
            } else {
                Write-Host "  [GIT] Failed to create tag '$TagName' (tag may already exist)." -ForegroundColor DarkYellow
            }
        }

        # Push
        if ($Push) {
            Write-Host "  [GIT] Pushing commits and tags to origin..." -ForegroundColor Yellow
            & git push origin HEAD --follow-tags
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  [GIT] Successfully pushed to origin!" -ForegroundColor Green
            } else {
                Write-Host "  [GIT] Failed to push to origin." -ForegroundColor Red
            }
        }
    } else {
        Write-Host "[WARNING] 'git' CLI not found. Skipped git commit/tag." -ForegroundColor DarkYellow
    }
} else {
    Write-Host "`nNext steps to publish release:" -ForegroundColor Yellow
    Write-Host "  git add Cargo.toml Cargo.lock android/app/build.gradle assets/windows/airmedia.manifest scripts/installer.iss assets/macos/Info.plist" -ForegroundColor Gray
    Write-Host "  git commit -m `"chore(release): bump version to $NewVersion`"" -ForegroundColor Gray
    Write-Host "  git tag v$NewVersion" -ForegroundColor Gray
    Write-Host "  git push origin HEAD --tags" -ForegroundColor Gray
}
