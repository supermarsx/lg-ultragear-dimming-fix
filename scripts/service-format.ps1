<#
.SYNOPSIS
    Check or fix Rust formatting for the service.

.DESCRIPTION
    Runs `cargo fmt --check` inside the service/ directory.
    With -Fix, applies formatting in-place.

.EXAMPLE
    pwsh -File scripts\service-format.ps1
    pwsh -File scripts\service-format.ps1 -Fix
#>

[CmdletBinding()]
param(
    [switch]$Fix
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Tag([string]$Tag, [string]$Color, [string]$Message) {
    Write-Host $Tag -ForegroundColor $Color -NoNewline
    Write-Host ("  {0}" -f $Message)
}

function Ensure-Cargo {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "cargo not found. Install Rust from https://rustup.rs"
    }
}

function Ensure-Rustfmt {
    $components = rustup component list 2>&1
    if ($components -notmatch 'rustfmt.*installed') {
        Tag -Tag '[INFO]' -Color Yellow -Message 'Installing rustfmt...'
        rustup component add rustfmt 2>&1 | Write-Host
    }
}

$ScriptRoot = Split-Path -Parent $PSCommandPath
$RepoRoot = Resolve-Path (Join-Path $ScriptRoot '..')
$ServiceDir = Join-Path $RepoRoot 'service'

if (-not (Test-Path -LiteralPath (Join-Path $ServiceDir 'Cargo.toml'))) {
    throw "service/Cargo.toml not found at: $ServiceDir"
}

Ensure-Cargo
Ensure-Rustfmt

Push-Location $ServiceDir
try {
    if ($Fix) {
        Tag -Tag '[STEP]' -Color Yellow -Message 'cargo fmt (applying fixes)'
        cargo fmt 2>&1 | Write-Host

        if ($LASTEXITCODE -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message "cargo fmt failed (exit code $LASTEXITCODE)"
            exit $LASTEXITCODE
        }

        Tag -Tag '[ OK ]' -Color Green -Message 'Formatting applied'
    } else {
        Tag -Tag '[STEP]' -Color Yellow -Message 'cargo fmt --check'
        cargo fmt --check 2>&1 | Write-Host

        if ($LASTEXITCODE -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message 'Formatting check failed. Run: pwsh -File scripts\service-format.ps1 -Fix'
            exit $LASTEXITCODE
        }

        Tag -Tag '[ OK ]' -Color Green -Message 'Formatting check passed'
    }
} finally {
    Pop-Location
}

exit 0
