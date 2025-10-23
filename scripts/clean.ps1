<#
Clean build and test artifacts

Usage:
  pwsh -File scripts\clean.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Tag([string]$Tag, [string]$Color, [string]$Message, [switch]$NoNewline) {
    Write-Host $Tag -ForegroundColor $Color -NoNewline
    if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
}

[CmdletBinding(SupportsShouldProcess = $true)]
function Remove-Safe([string]$Path) {
    if (Test-Path -LiteralPath $Path) {
        try {
            if ($PSCmdlet.ShouldProcess($Path, 'Remove')) {
                Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
                Tag -Tag '[DEL ]' -Color Magenta -Message ("removed: {0}" -f $Path)
            }
        } catch { Tag -Tag '[ERR ]' -Color Red -Message ("failed to remove '{0}': {1}" -f $Path, $_.Exception.Message) }
    }
}

Tag -Tag '[STRT]' -Color Cyan -Message 'cleaning artifacts'

# Common folders
$folders = @('dist', 'TestResults', 'coverage', '.coverage')
foreach ($f in $folders) { Remove-Safe $f }

# Common files
$files = @('*.trx', '*.testlog', '*.coverage', '*.log', '*.nupkg')
foreach ($pat in $files) { Get-ChildItem -Recurse -File -Filter $pat -ErrorAction SilentlyContinue | ForEach-Object { Remove-Safe $_.FullName } }

Tag -Tag '[DONE]' -Color Cyan -Message 'clean finished'
exit 0
