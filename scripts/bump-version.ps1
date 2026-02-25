#Requires -Version 5.1
<#
.SYNOPSIS
    Bump the VERSION file by incrementing the last numeric segment.
.DESCRIPTION
    VERSION is expected to be dot-separated numeric parts (for example: 26.1).
    The script increments the last segment (26.1 -> 26.2), writes it back to
    disk, and outputs the new version to stdout.
.PARAMETER Path
    Path to the version file. Defaults to "VERSION" at repository root.
#>
[CmdletBinding()]
param(
    [string]$Path = 'VERSION'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Test-Path -Path $Path)) {
    throw "Version file not found: $Path"
}

$raw = (Get-Content -Path $Path -Raw).Trim()
if ([string]::IsNullOrWhiteSpace($raw)) {
    throw "Version file is empty: $Path"
}

$parts = $raw.Split('.')
if ($parts.Count -lt 1) {
    throw "Invalid version format: $raw"
}

$numbers = @()
foreach ($part in $parts) {
    $value = 0
    if (-not [int]::TryParse($part, [ref]$value) -or $value -lt 0) {
        throw "Invalid version segment '$part' in version '$raw'"
    }
    $numbers += $value
}

$numbers[$numbers.Count - 1]++
$next = ($numbers -join '.')

Set-Content -Path $Path -Value $next

Write-Host "[version] Bumped: $raw -> $next" -ForegroundColor Cyan
Write-Output $next
