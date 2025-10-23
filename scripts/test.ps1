Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Ensure-Pester {
    if (-not (Get-Module -ListAvailable -Name Pester -MinimumVersion 5.0.0)) {
        try { Set-PSRepository -Name PSGallery -InstallationPolicy Trusted -ErrorAction SilentlyContinue } catch { Write-Verbose 'Ignoring PSGallery repository setup error.' }
        Install-Module Pester -Scope CurrentUser -Force -ErrorAction Stop
    }
    Import-Module Pester -MinimumVersion 5.0.0 -ErrorAction Stop | Out-Null
}

Ensure-Pester
Invoke-Pester -CI
Write-Host "[ OK ] Tests passed" -ForegroundColor Green

