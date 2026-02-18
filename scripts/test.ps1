#Requires -Version 5.1
<#
.SYNOPSIS
    Run all tests across every crate.
.DESCRIPTION
    Runs `cargo test --all-targets`. Pass -Verbose for full test output
    or -Filter to run a subset of tests by name.
.PARAMETER Filter
    Optional test name filter passed to cargo test.
.PARAMETER Release
    Run tests with the release profile.
#>
[CmdletBinding()]
param(
    [string]$Filter,
    [switch]$Release
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $args_ = @('test', '--all-targets')
    if ($Release) { $args_ += '--release' }
    if ($Filter)  { $args_ += @('--', $Filter) }

    Write-Host "[test] Running: cargo $($args_ -join ' ')" -ForegroundColor Cyan
    & cargo @args_

    if ($LASTEXITCODE -ne 0) {
        Write-Host '[test] FAILED' -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host '[test] OK' -ForegroundColor Green
} finally {
    Pop-Location
}
