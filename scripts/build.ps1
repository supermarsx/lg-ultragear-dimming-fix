#Requires -Version 5.1
<#
.SYNOPSIS
    Build the project.
.DESCRIPTION
    Runs `cargo build`. Pass -Release for an optimised release build
    (LTO, stripped, single codegen-unit — matches CI).
.PARAMETER Release
    Build with the release profile.
#>
[CmdletBinding()]
param(
    [switch]$Release
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $profile = if ($Release) { 'release' } else { 'dev' }
    Write-Host "[build] Building ($profile)..." -ForegroundColor Cyan

    $args_ = @('build')
    if ($Release) { $args_ += '--release' }
    & cargo @args_

    if ($LASTEXITCODE -ne 0) {
        Write-Host '[build] FAILED' -ForegroundColor Red
        exit $LASTEXITCODE
    }

    $binaryName = 'lg-ultragear-dimming-fix.exe'
    $binaryDir  = if ($Release) { 'target\release' } else { 'target\debug' }
    $binaryPath = Join-Path $binaryDir $binaryName

    if (Test-Path $binaryPath) {
        $size = (Get-Item $binaryPath).Length
        Write-Host ("[build] OK  {0}  ({1:N0} bytes / {2:N2} MB)" -f $binaryPath, $size, ($size / 1MB)) -ForegroundColor Green
    } else {
        Write-Host '[build] OK' -ForegroundColor Green
    }
} finally {
    Pop-Location
}
