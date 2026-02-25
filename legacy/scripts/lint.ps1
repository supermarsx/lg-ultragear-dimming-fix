Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Ensure-Analyzer {
    if (-not (Get-Module -ListAvailable -Name PSScriptAnalyzer)) {
        try { Set-PSRepository -Name PSGallery -InstallationPolicy Trusted -ErrorAction SilentlyContinue } catch { Write-Verbose 'Ignoring PSGallery repository setup error.' }
        Install-Module PSScriptAnalyzer -Scope CurrentUser -Force -ErrorAction Stop
    }
    Import-Module PSScriptAnalyzer -ErrorAction Stop | Out-Null
}

Ensure-Analyzer
Invoke-ScriptAnalyzer -Path (Get-Location).Path -Recurse -EnableExit -Settings ./scripts/PSScriptAnalyzerSettings.psd1
Write-Host "[ OK ] Lint passed" -ForegroundColor Green

