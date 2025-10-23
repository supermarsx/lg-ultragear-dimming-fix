<#
Clean build and test artifacts

Usage:
  pwsh -File scripts\clean.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Tag([string]$Tag,[string]$Color,[string]$Message,[switch]$NoNewline){
  Write-Host $Tag -ForegroundColor $Color -NoNewline
  if($NoNewline){ Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
}

function Remove-Safe([string]$Path){
  if(Test-Path -LiteralPath $Path){
    try { Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop; Tag '[DEL ]' Magenta ("removed: {0}" -f $Path) }
    catch { Tag '[ERROR]' Red ("failed to remove '{0}': {1}" -f $Path, $_.Exception.Message) }
  }
}

Tag '[STRT]' Cyan 'cleaning artifacts'

# Common folders
$folders = @('dist','TestResults','coverage','.coverage')
foreach($f in $folders){ Remove-Safe $f }

# Common files
$files = @('*.trx','*.testlog','*.coverage','*.log','*.nupkg')
foreach($pat in $files){ Get-ChildItem -Recurse -File -Filter $pat -ErrorAction SilentlyContinue | ForEach-Object { Remove-Safe $_.FullName } }

Tag '[DONE]' Cyan 'clean finished'
exit 0

