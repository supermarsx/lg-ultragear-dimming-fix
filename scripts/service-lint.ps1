<#
.SYNOPSIS
    Run Clippy linter on the Rust workspace.

.DESCRIPTION
    Runs `cargo clippy --workspace` from the workspace root.
    Treats all warnings as errors by default.

.EXAMPLE
    pwsh -File scripts\service-lint.ps1
    pwsh -File scripts\service-lint.ps1 -AllowWarnings
#>

[CmdletBinding()]
param(
    [switch]$AllowWarnings
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

function Ensure-Clippy {
    $components = rustup component list 2>&1
    if ($components -notmatch 'clippy.*installed') {
        Tag -Tag '[INFO]' -Color Yellow -Message 'Installing clippy...'
        rustup component add clippy 2>&1 | Write-Host
    }
}

$ScriptRoot = Split-Path -Parent $PSCommandPath
$RepoRoot = Resolve-Path (Join-Path $ScriptRoot '..')

if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot 'Cargo.toml'))) {
    throw "Cargo.toml not found at: $RepoRoot"
}

Ensure-Cargo
Ensure-Clippy

Tag -Tag '[STRT]' -Color Cyan -Message 'Linting Rust workspace (clippy)'

Push-Location $RepoRoot
try {
    $cargoArgs = @('clippy', '--workspace', '--all-targets')
    if (-not $AllowWarnings) {
        $cargoArgs += '--'
        $cargoArgs += '-D'
        $cargoArgs += 'warnings'
    }

    Tag -Tag '[STEP]' -Color Yellow -Message ("cargo {0}" -f ($cargoArgs -join ' '))
    & cargo @cargoArgs 2>&1 | Write-Host

    if ($LASTEXITCODE -ne 0) {
        Tag -Tag '[FAIL]' -Color Red -Message "cargo clippy failed (exit code $LASTEXITCODE)"
        exit $LASTEXITCODE
    }

    Tag -Tag '[ OK ]' -Color Green -Message 'Clippy lint passed'
} finally {
    Pop-Location
}

exit 0
