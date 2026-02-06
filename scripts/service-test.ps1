<#
.SYNOPSIS
    Run Rust tests for the Windows service.

.DESCRIPTION
    Runs `cargo test` inside the service/ directory.
    Includes unit tests and integration tests.

.EXAMPLE
    pwsh -File scripts\service-test.ps1
    pwsh -File scripts\service-test.ps1 -Verbose
#>

[CmdletBinding()]
param(
    [switch]$Nocapture
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

$ScriptRoot = Split-Path -Parent $PSCommandPath
$RepoRoot = Resolve-Path (Join-Path $ScriptRoot '..')
$ServiceDir = Join-Path $RepoRoot 'service'

if (-not (Test-Path -LiteralPath (Join-Path $ServiceDir 'Cargo.toml'))) {
    throw "service/Cargo.toml not found at: $ServiceDir"
}

Ensure-Cargo

Tag -Tag '[STRT]' -Color Cyan -Message 'Running Rust service tests'

Push-Location $ServiceDir
try {
    $cargoArgs = @('test')
    if ($Nocapture) {
        $cargoArgs += '--'
        $cargoArgs += '--nocapture'
    }

    Tag -Tag '[STEP]' -Color Yellow -Message ("cargo {0}" -f ($cargoArgs -join ' '))
    & cargo @cargoArgs 2>&1 | Write-Host

    if ($LASTEXITCODE -ne 0) {
        Tag -Tag '[FAIL]' -Color Red -Message "cargo test failed (exit code $LASTEXITCODE)"
        exit $LASTEXITCODE
    }

    Tag -Tag '[ OK ]' -Color Green -Message 'All Rust tests passed'
} finally {
    Pop-Location
}

exit 0
