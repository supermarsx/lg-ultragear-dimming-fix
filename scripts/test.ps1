Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Ensure-Pester {
    $pesterModule = Get-Module -ListAvailable -Name Pester | Where-Object { $_.Version -ge [version]'5.0.0' } | Select-Object -First 1
    if (-not $pesterModule) {
        try { Set-PSRepository -Name PSGallery -InstallationPolicy Trusted -ErrorAction SilentlyContinue } catch { Write-Verbose 'Ignoring PSGallery repository setup error.' }
        Install-Module Pester -Scope CurrentUser -Force -ErrorAction Stop
    }
    Import-Module Pester -MinimumVersion 5.0.0 -ErrorAction Stop | Out-Null
}

Ensure-Pester
Invoke-Pester -CI
Write-Host "[ OK ] Tests passed" -ForegroundColor Green

