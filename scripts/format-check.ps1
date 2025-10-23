Param(
    [switch]$Fix
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Ensure-Analyzer {
    if (-not (Get-Module -ListAvailable -Name PSScriptAnalyzer)) {
        try { Set-PSRepository -Name PSGallery -InstallationPolicy Trusted -ErrorAction SilentlyContinue } catch {}
        Install-Module PSScriptAnalyzer -Scope CurrentUser -Force -ErrorAction Stop
    }
    Import-Module PSScriptAnalyzer -ErrorAction Stop | Out-Null
}

function Format-RepoFiles {
    Param([switch]$Apply)
    $changed = @()
    $files = Get-ChildItem -Recurse -Include *.ps1 -File |
        Where-Object { $_.FullName -notmatch "\\\.git\\|\\dist\\" }
    foreach ($f in $files) {
        $original = Get-Content -LiteralPath $f.FullName -Raw
        $formatted = Invoke-Formatter -ScriptDefinition $original -Settings CodeFormattingOTBS
        if ($formatted -ne $original) {
            $changed += $f.FullName
            if ($Apply) {
                [IO.File]::WriteAllText($f.FullName, $formatted, [Text.UTF8Encoding]::new($false))
            }
        }
    }
    return ,$changed
}

Ensure-Analyzer
$changed = Format-RepoFiles -Apply:$Fix.IsPresent
if ($changed.Count -gt 0) {
    if ($Fix) {
        Write-Host "[ OK ] Applied formatting to:" -ForegroundColor Green
    } else {
        Write-Host "[WARN] The following files need formatting:" -ForegroundColor DarkYellow
    }
    $changed | ForEach-Object { Write-Host " - $_" }
    if (-not $Fix) { throw "Formatting check failed. Run scripts/format-check.ps1 -Fix." }
}
else {
    Write-Host "[ OK ] Formatting clean" -ForegroundColor Green
}

