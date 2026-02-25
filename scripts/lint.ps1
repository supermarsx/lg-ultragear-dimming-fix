#Requires -Version 5.1
<#
.SYNOPSIS
    Lint all crates with cargo clippy.
.DESCRIPTION
    Runs `cargo clippy --all-targets --all-features -- -D warnings` to
    catch common mistakes and enforce idiomatic Rust.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    Write-Host '[lint] Running clippy on all targets...' -ForegroundColor Cyan
    cargo clippy --all-targets --all-features -- -D warnings

    if ($LASTEXITCODE -ne 0) {
        Write-Host '[lint] FAILED' -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host '[lint] OK' -ForegroundColor Green
}
finally {
    Pop-Location
}
