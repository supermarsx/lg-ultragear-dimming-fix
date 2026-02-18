<#
.SYNOPSIS
    Full CI pipeline for the Rust workspace.

.DESCRIPTION
    Runs format check → clippy lint → tests → release build in sequence.
    Any step failure stops the pipeline.

.EXAMPLE
    pwsh -File scripts\service-ci.ps1
    pwsh -File scripts\service-ci.ps1 -NoFormat -NoBuild
    pwsh -File scripts\service-ci.ps1 -AllowWarnings
#>

[CmdletBinding()]
param(
    [switch]$NoFormat,
    [switch]$NoLint,
    [switch]$NoTest,
    [switch]$NoBuild,
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

$ScriptRoot = Split-Path -Parent $PSCommandPath
$RepoRoot = Resolve-Path (Join-Path $ScriptRoot '..')

if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot 'Cargo.toml'))) {
    throw "Cargo.toml not found at: $RepoRoot"
}

Ensure-Cargo

Tag -Tag '[STRT]' -Color Cyan -Message 'Rust workspace CI: format → lint → test → build'

$stepCount = 0
$startTime = Get-Date

Push-Location $RepoRoot
try {
    # ── Step 1: Format Check ──────────────────────────────────────
    if (-not $NoFormat) {
        $stepCount++
        Tag -Tag '[STEP]' -Color Magenta -Message "[$stepCount] cargo fmt --all --check"
        cargo fmt --all --check 2>&1 | Write-Host
        if ($LASTEXITCODE -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message 'Format check failed. Run: pwsh -File scripts\service-format.ps1 -Fix'
            exit 1
        }
        Tag -Tag '[ OK ]' -Color Green -Message 'Format check passed'
    }
    else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'format (--NoFormat)'
    }

    # ── Step 2: Clippy Lint ───────────────────────────────────────
    if (-not $NoLint) {
        $stepCount++
        $clippyArgs = @('clippy', '--workspace', '--all-targets')
        if (-not $AllowWarnings) {
            $clippyArgs += '--'
            $clippyArgs += '-D'
            $clippyArgs += 'warnings'
        }
        Tag -Tag '[STEP]' -Color Magenta -Message ("[$stepCount] cargo {0}" -f ($clippyArgs -join ' '))
        & cargo @clippyArgs 2>&1 | Write-Host
        if ($LASTEXITCODE -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message 'Clippy lint failed'
            exit 2
        }
        Tag -Tag '[ OK ]' -Color Green -Message 'Clippy lint passed'
    }
    else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'lint (--NoLint)'
    }

    # ── Step 3: Tests ─────────────────────────────────────────────
    if (-not $NoTest) {
        $stepCount++
        Tag -Tag '[STEP]' -Color Magenta -Message "[$stepCount] cargo test --workspace"
        cargo test --workspace 2>&1 | Write-Host
        if ($LASTEXITCODE -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message 'Tests failed'
            exit 3
        }
        Tag -Tag '[ OK ]' -Color Green -Message 'All tests passed'
    }
    else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'test (--NoTest)'
    }

    # ── Step 4: Release Build ─────────────────────────────────────
    if (-not $NoBuild) {
        $stepCount++
        Tag -Tag '[STEP]' -Color Magenta -Message "[$stepCount] cargo build --release"
        cargo build --release 2>&1 | Write-Host
        if ($LASTEXITCODE -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message 'Release build failed'
            exit 4
        }

        $bin = Join-Path $RepoRoot 'target\release\lg-ultragear.exe'
        if (Test-Path $bin) {
            $size = [math]::Round((Get-Item $bin).Length / 1KB, 1)
            Tag -Tag '[ OK ]' -Color Green -Message "Release build: $bin ($size KB)"

            # ── Copy to dist/ ─────────────────────────────────────
            $DistDir = Join-Path $RepoRoot 'dist'
            if (-not (Test-Path $DistDir)) {
                New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
            }
            $DistBin = Join-Path $DistDir 'lg-ultragear.exe'
            Copy-Item -LiteralPath $bin -Destination $DistBin -Force
            Tag -Tag '[ OK ]' -Color Green -Message "Copied to: $DistBin"
        }
        else {
            Tag -Tag '[ OK ]' -Color Green -Message 'Release build succeeded'
        }
    }
    else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'build (--NoBuild)'
    }
}
finally {
    Pop-Location
}

$elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
Tag -Tag '[DONE]' -Color Cyan -Message "Rust workspace CI finished ($stepCount steps, ${elapsed}s)"
exit 0
