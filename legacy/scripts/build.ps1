param(
    [string]$OutputDir = "dist"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Ensure-Packager {
    if (-not (Get-Module -ListAvailable -Name ps2exe)) {
        try { Set-PSRepository -Name PSGallery -InstallationPolicy Trusted -ErrorAction SilentlyContinue } catch { Write-Verbose 'Ignoring PSGallery repository setup error.' }
        Install-Module ps2exe -Scope CurrentUser -Force -ErrorAction Stop
    }
    Import-Module ps2exe -ErrorAction Stop | Out-Null
}

function New-CleanDir {
    [CmdletBinding(SupportsShouldProcess = $true)]
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        if ($PSCmdlet.ShouldProcess($Path, 'Create directory')) {
            New-Item -ItemType Directory -Path $Path -Force | Out-Null
        }
    }
}

Ensure-Packager
New-CleanDir -Path $OutputDir

$scriptIn = 'install-lg-ultragear-no-dimming.ps1'
$profileFile = 'lg-ultragear-full-cal.icm'
$exeOut = Join-Path $OutputDir 'install-lg-ultragear-no-dimming.exe'

if (-not (Test-Path -LiteralPath $scriptIn)) { throw "Missing input script: $scriptIn" }
if (-not (Test-Path -LiteralPath $profileFile)) { throw "Missing profile: $profileFile" }

# Read version from VERSION file
$versionFile = 'VERSION'
if (Test-Path -LiteralPath $versionFile) {
    $version = (Get-Content -LiteralPath $versionFile -Raw).Trim()
} else {
    $version = '26.1'
}
Write-Host "[INFO] Version: $version" -ForegroundColor Cyan

# Inject version into script before building
Write-Host "[STEP] Injecting version into script" -ForegroundColor Yellow
$content = Get-Content -LiteralPath $scriptIn -Raw -Encoding UTF8
$content = $content -replace "'__VERSION_EMBED__'", "'$version'"
$tempScript = Join-Path $OutputDir 'install-lg-ultragear-no-dimming-versioned.ps1'
[System.IO.File]::WriteAllText($tempScript, $content, [System.Text.UTF8Encoding]::new($true))

Write-Host "[STEP] Building executable: $exeOut" -ForegroundColor Yellow
Invoke-ps2exe -inputFile $tempScript -outputFile $exeOut -x64 -noConsole:$false `
    -title 'LG UltraGear No-Dimming Installer' `
    -description 'Automated installer for LG UltraGear no-dimming ICC profile.' `
    -product 'LG UltraGear Dimming Fix' `
    -version $version `
    -embedFiles $profileFile

# Cleanup temp versioned script
Remove-Item -LiteralPath $tempScript -Force -ErrorAction SilentlyContinue

$zip = Join-Path (Resolve-Path $OutputDir) 'lg-ultragear-dimming-fix.zip'
if (Test-Path $zip) { Remove-Item $zip -Force }

$items = @(
    'install-lg-ultragear-no-dimming.ps1',
    'install-full-auto.bat',
    'lg-ultragear-full-cal.icm',
    'readme.md',
    'license.md',
    $exeOut
) | Where-Object { Test-Path $_ }

Write-Host "[STEP] Creating package: $zip" -ForegroundColor Yellow
Compress-Archive -Path $items -DestinationPath $zip -CompressionLevel Optimal
Write-Host "[ OK ] Build artifacts:" -ForegroundColor Green
Write-Host " - $exeOut"
Write-Host " - $zip"
