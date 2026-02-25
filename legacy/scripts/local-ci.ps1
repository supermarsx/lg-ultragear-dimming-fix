<#
Local CI wrapper: format, lint, test, build

Usage:
  pwsh -File scripts\local-ci.ps1
  pwsh -File scripts\local-ci.ps1 -NoFormat -NoBuild

Notes:
  - Uses PSScriptAnalyzer (format + lint) if available
  - Uses Pester (tests) if available
  - Uses ps2exe (build) if available; always copies main script to dist/
#>

[CmdletBinding()]
param(
    [switch]$NoFormat,
    [switch]$NoLint,
    [switch]$NoTest,
    [switch]$NoBuild,
    [switch]$Strict
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Tag([string]$Tag, [string]$Color, [string]$Message, [switch]$NoNewline) {
    Write-Host $Tag -ForegroundColor $Color -NoNewline
    if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
}

function Ensure-Dir([string]$path) { if (-not (Test-Path -LiteralPath $path)) { [IO.Directory]::CreateDirectory($path) | Out-Null } }

Tag -Tag '[STRT]' -Color Cyan -Message 'local CI: format, lint, test, build'

# Discover project root (dir of this script)
$ScriptRoot = Split-Path -Parent $PSCommandPath
$RepoRoot = Resolve-Path (Join-Path $ScriptRoot '..')
Set-Location $RepoRoot

$mainScript = 'install-lg-ultragear-no-dimming.ps1'
if (-not (Test-Path -LiteralPath $mainScript)) {
    Tag -Tag '[WARN]' -Color Yellow -Message ("main script not found at {0}" -f (Join-Path (Get-Location) $mainScript))
}

# Format
if (-not $NoFormat) {
    $psa = Get-Module -ListAvailable PSScriptAnalyzer | Select-Object -First 1
    if ($psa) {
        Import-Module PSScriptAnalyzer -ErrorAction Stop | Out-Null
        Tag -Tag '[STEP]' -Color Magenta -Message 'format (Invoke-Formatter)'
        $files = Get-ChildItem -Recurse -Include *.ps1 -File | Select-Object -Expand FullName
        foreach ($f in $files) {
            try {
                $formatted = Invoke-Formatter -ScriptDefinition (Get-Content -LiteralPath $f -Raw) -Settings CodeFormattingOTBS
                if ($formatted) { Set-Content -LiteralPath $f -Value $formatted -NoNewline }
            } catch { Tag -Tag '[WARN]' -Color Yellow -Message ("format failed for {0}: {1}" -f $f, $_.Exception.Message) }
        }
        Tag -Tag '[ OK ]' -Color Green -Message 'format step completed'
    } else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'PSScriptAnalyzer not available; skipping format'
    }
}

# Lint
if (-not $NoLint) {
    $psa = Get-Module -ListAvailable PSScriptAnalyzer | Select-Object -First 1
    if ($psa) {
        Import-Module PSScriptAnalyzer -ErrorAction Stop | Out-Null
        Tag -Tag '[STEP]' -Color Magenta -Message 'lint (Invoke-ScriptAnalyzer)'
        $results = Invoke-ScriptAnalyzer -Path . -Recurse -Severity @('Error', 'Warning')
        if ($results) { $results | Format-Table RuleName, Severity, ScriptName, Line -AutoSize | Out-Host }
        $errors = @($results | Where-Object Severity -EQ 'Error')
        $warnings = @($results | Where-Object Severity -EQ 'Warning')
        if ($Strict -and $warnings.Count) { Tag -Tag '[ERROR]' -Color Red -Message ("lint: {0} warning(s) under --Strict" -f $warnings.Count); exit 2 }
        if ($errors.Count) { Tag -Tag '[ERROR]' -Color Red -Message ("lint: {0} error(s)" -f $errors.Count); exit 2 }
        Tag -Tag '[ OK ]' -Color Green -Message 'lint passed'
    } else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'PSScriptAnalyzer not available; skipping lint'
    }
}

# Test
if (-not $NoTest) {
    $pester = Get-Module -ListAvailable Pester | Where-Object { $_.Version -ge [version]'5.0.0' } | Select-Object -First 1
    if ($pester) {
        Import-Module Pester -MinimumVersion 5.0.0 -ErrorAction Stop | Out-Null
        $tests = Get-ChildItem -Recurse -Include *.Tests.ps1 -File
        if ($tests) {
            Tag -Tag '[STEP]' -Color Magenta -Message ("tests (Pester {0})" -f $pester.Version)
            $config = [Pester.Configuration]::Default
            $config.Run.Path = (Get-Location).Path
            $config.Run.PassThru = $true
            $config.Run.Exit = $false
            $result = Invoke-Pester -Configuration $config
            if (-not $result.Success) { Tag -Tag '[ERROR]' -Color Red -Message ("tests failed: {0} failed" -f $result.FailedCount); exit 3 }
            Tag -Tag '[ OK ]' -Color Green -Message 'tests passed'
        } else {
            Tag -Tag '[SKIP]' -Color DarkYellow -Message 'no tests discovered (*.Tests.ps1)'
        }
    } else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'Pester >= 5 not available; skipping tests'
    }
}

# Build
if (-not $NoBuild) {
    Ensure-Dir 'dist'
    Copy-Item -LiteralPath $mainScript -Destination (Join-Path 'dist' $mainScript) -Force -ErrorAction Stop
    $ps2exe = Get-Command Invoke-ps2exe -ErrorAction SilentlyContinue
    if ($ps2exe) {
        Tag -Tag '[STEP]' -Color Magenta -Message 'build (ps2exe)'
        $out = Join-Path 'dist' 'install-lg-ultragear-no-dimming.exe'
        try {
            Invoke-ps2exe -inputFile $mainScript -outputFile $out -noConsole -requireAdmin:$false -verbose:$false
            Tag -Tag '[ OK ]' -Color Green -Message ("built: {0}" -f $out)
        } catch {
            Tag -Tag '[WARN]' -Color Yellow -Message ("ps2exe build failed: {0}" -f $_.Exception.Message)
        }
    } else {
        Tag -Tag '[SKIP]' -Color DarkYellow -Message 'ps2exe not available; only copied script to dist/'
    }
}

Tag -Tag '[DONE]' -Color Cyan -Message 'local CI finished'
exit 0

