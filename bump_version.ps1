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

& "$PSScriptRoot\scripts\bump_version.ps1" @PSBoundParameters
