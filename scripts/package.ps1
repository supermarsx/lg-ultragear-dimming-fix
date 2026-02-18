#Requires -Version 5.1
<#
.SYNOPSIS
    Build a release binary and create the distribution package.
.DESCRIPTION
    Builds in release mode, copies the binary and supporting files to
    dist/, and creates a zip archive ready for GitHub Releases.
    Mirrors the CI packaging step exactly.
.PARAMETER SkipBuild
    Skip the cargo build step (use an existing release binary).
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$binaryName = 'lg-ultragear-dimming-fix'
$root       = Split-Path $PSScriptRoot -Parent

Push-Location $root
try {
    # --- Build release binary ------------------------------------------------
    if (-not $SkipBuild) {
        Write-Host '[package] Building release binary...' -ForegroundColor Cyan
        cargo build --release
        if ($LASTEXITCODE -ne 0) {
            Write-Host '[package] Build FAILED' -ForegroundColor Red
            exit $LASTEXITCODE
        }
    }

    $releaseBin = "target\release\$binaryName.exe"
    if (-not (Test-Path $releaseBin)) {
        Write-Host "[package] ERROR: $releaseBin not found. Run without -SkipBuild." -ForegroundColor Red
        exit 1
    }

    # --- Read version --------------------------------------------------------
    $version = (Get-Content -Path 'VERSION' -Raw).Trim()
    Write-Host "[package] Version: $version" -ForegroundColor Cyan

    # --- Prepare dist folder -------------------------------------------------
    $dist = 'dist'
    if (Test-Path $dist) { Remove-Item $dist -Recurse -Force }
    New-Item -ItemType Directory -Path $dist -Force | Out-Null

    Copy-Item $releaseBin               "$dist\$binaryName.exe" -Force
    Copy-Item 'VERSION'                 "$dist\VERSION"         -Force
    Copy-Item 'readme.md'              "$dist\readme.md"       -Force
    Copy-Item 'license.md'            "$dist\license.md"     -Force
    Copy-Item 'lg-ultragear-full-cal.icm' "$dist\lg-ultragear-full-cal.icm" -Force

    # --- Create zip ----------------------------------------------------------
    $zipName = "$binaryName.zip"
    $zipPath = "$dist\$zipName"
    if (Test-Path $zipPath) { Remove-Item $zipPath -Force }

    $items = @(
        "$dist\$binaryName.exe",
        "$dist\lg-ultragear-full-cal.icm",
        "$dist\readme.md",
        "$dist\license.md",
        "$dist\VERSION"
    )
    Compress-Archive -Path $items -DestinationPath $zipPath -CompressionLevel Optimal

    # --- Summary -------------------------------------------------------------
    $binSize = (Get-Item "$dist\$binaryName.exe").Length
    $zipSize = (Get-Item $zipPath).Length

    Write-Host ''
    Write-Host '[package] Done!' -ForegroundColor Green
    Write-Host "  Version : $version"
    Write-Host ("  Binary  : {0:N0} bytes ({1:N2} MB)" -f $binSize, ($binSize / 1MB))
    Write-Host ("  Package : {0:N0} bytes ({1:N2} MB)" -f $zipSize, ($zipSize / 1MB))
    Write-Host "  Output  : $dist\"
    Write-Host ''
    Get-ChildItem $dist | Format-Table Name, @{N='Size';E={"{0:N0}" -f $_.Length};A='Right'} -AutoSize
} finally {
    Pop-Location
}
