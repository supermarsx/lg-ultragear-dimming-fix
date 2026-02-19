#Requires -Version 5.1
<#
.SYNOPSIS
    Type-check all crates with cargo check.
.DESCRIPTION
    Runs `cargo check --all-targets --all-features` to verify the project
    compiles without producing a binary (faster than a full build).
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    Write-Host '[check] Running cargo check on all targets...' -ForegroundColor Cyan
    cargo check --all-targets --all-features

    if ($LASTEXITCODE -ne 0) {
        Write-Host '[check] FAILED' -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host '[check] OK' -ForegroundColor Green
}
finally {
    Pop-Location
}
