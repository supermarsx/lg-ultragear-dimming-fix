<#
.SYNOPSIS
  Test the auto-reapply monitor logic.
.DESCRIPTION
  Simulates what the scheduled task does: checks for LG UltraGear monitor and reapplies the color profile.
  Useful for verifying the auto-reapply works correctly without waiting for a trigger event.
.PARAMETER ShowNotification
  Show a toast notification after reapplying (default: true).
#>
[CmdletBinding()]
param(
    [switch]$NoNotification
)

$MonitorMatch = 'LG UltraGear'

Write-Host "Testing auto-reapply monitor..." -ForegroundColor Cyan
Write-Host ""

# Quick check for LG UltraGear
Write-Host "[STEP] Checking for LG UltraGear monitor..." -ForegroundColor White
$found = $false
try {
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop | ForEach-Object {
        $name = ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
        Write-Host "       Found: $name" -ForegroundColor Gray
        if ($name -match $MonitorMatch) {
            $found = $true
            Write-Host "       ^ Matches '$MonitorMatch'" -ForegroundColor Green
        }
    }
} catch {
    Write-Host "[FAIL] Could not enumerate monitors: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

# Check if LG UltraGear was found
if (-not $found) {
    Write-Host ""
    Write-Host "[SKIP] No LG UltraGear monitor detected - auto-reapply would exit early" -ForegroundColor Yellow
    exit 0
}

Write-Host ""
Write-Host "[STEP] LG UltraGear detected - reapplying color profile..." -ForegroundColor White

# Get the installer path
$installerPath = Join-Path $PSScriptRoot "..\install-lg-ultragear-no-dimming.ps1"
if (-not (Test-Path $installerPath)) {
    Write-Host "[FAIL] Installer not found at: $installerPath" -ForegroundColor Red
    exit 1
}

# Run the installer in reapply mode (same flags as the scheduled task action)
try {
    & $installerPath -NoSetDefault -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor -MonitorNameMatch $MonitorMatch 2>$null | Out-Null
    Write-Host "[ OK ] Color profile reapplied" -ForegroundColor Green
} catch {
    Write-Host "[FAIL] Reapply failed: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

# Show toast notification
if (-not $NoNotification) {
    Write-Host ""
    Write-Host "[STEP] Showing notification..." -ForegroundColor White
    try {
        [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
        [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

        $template = '<toast duration="long"><visual><binding template="ToastGeneric"><text>LG UltraGear</text><text>Color profile reapplied</text></binding></visual></toast>'

        $xml = [Windows.Data.Xml.Dom.XmlDocument]::new()
        $xml.LoadXml($template)
        $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)

        # Use PowerShell's registered AppUserModelId for reliable notifications
        $appId = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'
        [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
        Write-Host "[ OK ] Notification sent" -ForegroundColor Green
    } catch {
        Write-Host "[WARN] Notification failed: $($_.Exception.Message)" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "[DONE] Auto-reapply test complete" -ForegroundColor Cyan
