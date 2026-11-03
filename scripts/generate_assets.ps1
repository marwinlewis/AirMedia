# AirMedia Asset Generator Script (PowerShell)
# Regenerates icons and asset packages across Windows, macOS, Linux, and Android from dist/airmedia.png
$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $ProjectRoot

Write-Host "==> Generating AirMedia multi-platform assets from dist/airmedia.png..." -ForegroundColor Cyan
cargo run --example generate_assets

if ($LASTEXITCODE -eq 0) {
    Write-Host "==> Assets generated successfully!" -ForegroundColor Green
} else {
    Write-Host "==> Asset generation failed!" -ForegroundColor Red
}

Pop-Location
