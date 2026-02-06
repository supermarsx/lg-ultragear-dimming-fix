<#
.SYNOPSIS
    Parallel CI pipeline for the Rust Windows service.

.DESCRIPTION
    Runs format check, clippy lint, and tests in PARALLEL, then does a
    sequential release build + copy to dist/.  Much faster than the
    sequential service-ci.ps1 because the three check steps share no
    build-cache writes and can overlap.

.EXAMPLE
    pwsh -File scripts\service-ci-parallel.ps1
    pwsh -File scripts\service-ci-parallel.ps1 -NoBuild
    pwsh -File scripts\service-ci-parallel.ps1 -AllowWarnings
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

# ── Helpers ───────────────────────────────────────────────────────

function Tag([string]$Tag, [string]$Color, [string]$Message) {
    Write-Host $Tag -ForegroundColor $Color -NoNewline
    Write-Host ("  {0}" -f $Message)
}

function Ensure-Cargo {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "cargo not found. Install Rust from https://rustup.rs"
    }
}

# ── Paths ─────────────────────────────────────────────────────────

$ScriptRoot  = Split-Path -Parent $PSCommandPath
$RepoRoot    = Resolve-Path (Join-Path $ScriptRoot '..')
$ServiceDir  = Join-Path $RepoRoot 'service'

if (-not (Test-Path -LiteralPath (Join-Path $ServiceDir 'Cargo.toml'))) {
    throw "service/Cargo.toml not found at: $ServiceDir"
}

Ensure-Cargo

$startTime = Get-Date
Tag -Tag '[STRT]' -Color Cyan -Message 'Rust service CI (parallel): fmt + clippy + test → build'

# ── Phase 1 — Parallel: format, lint, test ────────────────────────

$jobs = @()

if (-not $NoFormat) {
    $jobs += Start-Job -Name 'fmt' -ScriptBlock {
        param($dir)
        Set-Location $dir
        $out = cargo fmt --check 2>&1
        $code = $LASTEXITCODE
        [PSCustomObject]@{ Name = 'Format'; ExitCode = $code; Output = ($out -join "`n") }
    } -ArgumentList $ServiceDir
}

if (-not $NoLint) {
    $clippySuffix = if ($AllowWarnings) { '' } else { '-- -D warnings' }
    $jobs += Start-Job -Name 'clippy' -ScriptBlock {
        param($dir, $strict)
        Set-Location $dir
        if ($strict) {
            $out = cargo clippy --all-targets -- -D warnings 2>&1
        } else {
            $out = cargo clippy --all-targets 2>&1
        }
        $code = $LASTEXITCODE
        [PSCustomObject]@{ Name = 'Clippy'; ExitCode = $code; Output = ($out -join "`n") }
    } -ArgumentList $ServiceDir, (-not $AllowWarnings)
}

if (-not $NoTest) {
    $jobs += Start-Job -Name 'test' -ScriptBlock {
        param($dir)
        Set-Location $dir
        $out = cargo test 2>&1
        $code = $LASTEXITCODE
        [PSCustomObject]@{ Name = 'Test'; ExitCode = $code; Output = ($out -join "`n") }
    } -ArgumentList $ServiceDir
}

# Wait for all parallel jobs
$failures = @()

if ($jobs.Count -gt 0) {
    Tag -Tag '[INFO]' -Color DarkYellow -Message ("Waiting for {0} parallel job(s): {1}" -f $jobs.Count, (($jobs | ForEach-Object { $_.Name }) -join ', '))

    $results = $jobs | Wait-Job | Receive-Job

    foreach ($r in $results) {
        if ($r.ExitCode -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message "$($r.Name) failed (exit code $($r.ExitCode))"
            Write-Host $r.Output
            $failures += $r.Name
        } else {
            Tag -Tag '[ OK ]' -Color Green -Message "$($r.Name) passed"
        }
    }

    $jobs | Remove-Job -Force
}

# Handle skipped steps
if ($NoFormat) { Tag -Tag '[SKIP]' -Color DarkYellow -Message 'format (--NoFormat)' }
if ($NoLint)   { Tag -Tag '[SKIP]' -Color DarkYellow -Message 'lint (--NoLint)' }
if ($NoTest)   { Tag -Tag '[SKIP]' -Color DarkYellow -Message 'test (--NoTest)' }

if ($failures.Count -gt 0) {
    Tag -Tag '[FAIL]' -Color Red -Message ("Parallel phase failed: {0}" -f ($failures -join ', '))
    exit 1
}

Tag -Tag '[ OK ]' -Color Green -Message 'Parallel phase passed'

# ── Phase 2 — Sequential: release build + dist ────────────────────

if (-not $NoBuild) {
    Tag -Tag '[STEP]' -Color Magenta -Message 'cargo build --release'

    Push-Location $ServiceDir
    try {
        cargo build --release 2>&1 | Write-Host
        if ($LASTEXITCODE -ne 0) {
            Tag -Tag '[FAIL]' -Color Red -Message 'Release build failed'
            exit 2
        }

        $bin = Join-Path $ServiceDir 'target\release\lg-ultragear-color-svc.exe'
        if (Test-Path $bin) {
            $size = [math]::Round((Get-Item $bin).Length / 1KB, 1)
            Tag -Tag '[ OK ]' -Color Green -Message "Release build: $bin ($size KB)"

            # ── Copy to dist/ ─────────────────────────────────────
            $DistDir = Join-Path $RepoRoot 'dist'
            if (-not (Test-Path $DistDir)) {
                New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
            }
            $DistBin = Join-Path $DistDir 'lg-ultragear-color-svc.exe'
            Copy-Item -LiteralPath $bin -Destination $DistBin -Force
            $distSize = [math]::Round((Get-Item $DistBin).Length / 1KB, 1)
            Tag -Tag '[ OK ]' -Color Green -Message "Copied to: $DistBin ($distSize KB)"
        } else {
            Tag -Tag '[ OK ]' -Color Green -Message 'Release build succeeded'
        }
    } finally {
        Pop-Location
    }
} else {
    Tag -Tag '[SKIP]' -Color DarkYellow -Message 'build (--NoBuild)'
}

# ── Done ──────────────────────────────────────────────────────────

$elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
Tag -Tag '[DONE]' -Color Cyan -Message "Rust service CI (parallel) finished (${elapsed}s)"
exit 0
