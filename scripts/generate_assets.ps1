# AirMedia Asset Generator Script (PowerShell)
# Regenerates icons and asset packages across Windows, macOS, Linux, and Android from assets/airmedia.svg
$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $ProjectRoot

Write-Host "==> Generating AirMedia multi-platform assets from assets/airmedia.svg..." -ForegroundColor Cyan
New-Item -ItemType Directory -Path (Join-Path $ProjectRoot "dist") -Force | Out-Null
cargo run --example generate_assets

if ($LASTEXITCODE -eq 0) {
    Write-Host "==> Assets generated successfully!" -ForegroundColor Green
} else {
    Write-Host "==> Asset generation failed!" -ForegroundColor Red
}

Pop-Location
