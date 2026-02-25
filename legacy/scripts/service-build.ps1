<#
.SYNOPSIS
    Build the Rust workspace binary (release mode).

.DESCRIPTION
    Runs `cargo build --release` from the workspace root.
    Outputs the binary to target/release/lg-ultragear.exe

.EXAMPLE
    pwsh -File scripts\service-build.ps1
    pwsh -File scripts\service-build.ps1 -DebugBuild
#>

[CmdletBinding()]
param(
    [switch]$DebugBuild
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

if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot 'Cargo.toml'))) {
    throw "Cargo.toml not found at: $RepoRoot"
}

Ensure-Cargo

Tag -Tag '[STRT]' -Color Cyan -Message 'Building Rust workspace'

Push-Location $RepoRoot
try {
    if ($DebugBuild) {
        Tag -Tag '[STEP]' -Color Yellow -Message 'cargo build (debug)'
        cargo build 2>&1 | Write-Host
    }
    else {
        Tag -Tag '[STEP]' -Color Yellow -Message 'cargo build --release'
        cargo build --release 2>&1 | Write-Host
    }

    if ($LASTEXITCODE -ne 0) {
        Tag -Tag '[FAIL]' -Color Red -Message "cargo build failed (exit code $LASTEXITCODE)"
        exit $LASTEXITCODE
    }

    if ($DebugBuild) {
        $bin = Join-Path $RepoRoot 'target\debug\lg-ultragear.exe'
    }
    else {
        $bin = Join-Path $RepoRoot 'target\release\lg-ultragear.exe'
    }

    if (Test-Path $bin) {
        $size = [math]::Round((Get-Item $bin).Length / 1KB, 1)
        Tag -Tag '[ OK ]' -Color Green -Message "Built: $bin ($size KB)"

        # ── Copy to dist/ ─────────────────────────────────────────
        $DistDir = Join-Path $RepoRoot 'dist'
        if (-not (Test-Path $DistDir)) {
            New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
        }
        $DistBin = Join-Path $DistDir 'lg-ultragear.exe'
        Copy-Item -LiteralPath $bin -Destination $DistBin -Force
        $distSize = [math]::Round((Get-Item $DistBin).Length / 1KB, 1)
        Tag -Tag '[ OK ]' -Color Green -Message "Copied to: $DistBin ($distSize KB)"
    }
    else {
        Tag -Tag '[WARN]' -Color Yellow -Message "Binary not found at expected path: $bin"
    }
}
finally {
    Pop-Location
}

exit 0
