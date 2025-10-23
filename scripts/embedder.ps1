<#
Embed an ICC/ICM color profile into the main installer script by updating
the embedded Base64 payload, the file name, and the expected SHA256 hash.
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProfilePath,
    [string]$MainScriptPath = '.\install-lg-ultragear-no-dimming.ps1',
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Tag([string]$Tag, [string]$Color, [string]$Message, [switch]$NoNewline) {
    Write-Host $Tag -ForegroundColor $Color -NoNewline
    if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
}

try {
    $main = (Resolve-Path -LiteralPath $MainScriptPath -ErrorAction Stop).Path
    $icc = (Resolve-Path -LiteralPath $ProfilePath   -ErrorAction Stop).Path
} catch { Tag '[ERROR]' Red $_.Exception.Message; exit 1 }

$bytes = [IO.File]::ReadAllBytes($icc)
$b64 = [Convert]::ToBase64String($bytes)
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $icc).Hash
$name = Split-Path -Leaf $icc

Tag '[INFO]' Yellow ("source='{0}', bytes={1}, sha256={2}" -f $name, $bytes.Length, $hash)

$content = Get-Content -LiteralPath $main -Raw

# Replace profile name
$patternName = '\$script:EmbeddedProfileName\s*=\s*''[^'']*'''
$replacementName = "`$script:EmbeddedProfileName = '$name'"
$content = [Regex]::Replace($content, $patternName, $replacementName)

# Replace Base64 (single-line)
$patternB64 = '\$script:EmbeddedProfileBase64\s*=\s*''[^'']*'''
$replacementB64 = "`$script:EmbeddedProfileBase64 = '$b64'"
$content = [Regex]::Replace($content, $patternB64, $replacementB64)

# Replace expected hash inside Ensure-EmbeddedProfile
$patternHash = '\$expectedHash\s*=\s*''[0-9A-Fa-f]+'''
$replacementHash = "`$expectedHash = '$hash'"
$content = [Regex]::Replace($content, $patternHash, $replacementHash)

# Verify round-trip before writing
$m = [regex]::Match($content, '\$script:EmbeddedProfileBase64\s*=\s*''([^'']+)''', 'Singleline')
if (-not $m.Success) { throw "Could not locate EmbeddedProfileBase64 in updated script." }
$b64new = $m.Groups[1].Value
$bytes2 = [Convert]::FromBase64String(($b64new -replace '\s', ''))
$hash2 = ([Security.Cryptography.SHA256]::Create()).ComputeHash($bytes2)
$hash2hex = -join ($hash2 | ForEach-Object { $_.ToString('X2') })
if ($hash2hex -ine $hash) { throw "Round-trip hash mismatch: expected $hash got $hash2hex" }

if ($DryRun) {
    Tag '[SKIP]' DarkYellow 'dry-run set; not writing file'
} else {
    Set-Content -LiteralPath $main -Value $content -NoNewline
    Tag '[ OK ]' Green 'updated main script with embedded profile + hash'
}

Tag '[DONE]' Cyan 'embed verification ok'
exit 0
