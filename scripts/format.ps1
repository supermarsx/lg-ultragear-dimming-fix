#Requires -Version 5.1
<#
.SYNOPSIS
    Check code formatting with cargo fmt.
.DESCRIPTION
    Runs `cargo fmt --all -- --check` to verify all Rust source files
    conform to the project's formatting rules. Pass -Fix to auto-format.
.PARAMETER Fix
    Apply formatting fixes instead of just checking.
#>
[CmdletBinding()]
param(
    [switch]$Fix
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    if ($Fix) {
        Write-Host '[fmt] Formatting all crates...' -ForegroundColor Cyan
        cargo fmt --all
    } else {
        Write-Host '[fmt] Checking formatting...' -ForegroundColor Cyan
        cargo fmt --all -- --check
    }

    if ($LASTEXITCODE -ne 0) {
        Write-Host '[fmt] FAILED' -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host '[fmt] OK' -ForegroundColor Green
} finally {
    Pop-Location
}
