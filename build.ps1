param(
    [string]$Version = "",
    [switch]$BundleFfmpeg = $false,
    [switch]$SkipTests = $false,
    [switch]$CreateZip = $true
)

& "$PSScriptRoot\scripts\build_windows.ps1" -Version $Version -BundleFfmpeg:$BundleFfmpeg -SkipTests:$SkipTests -CreateZip:$CreateZip
