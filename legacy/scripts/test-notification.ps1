<#
.SYNOPSIS
  Test the toast notification used by the auto-reapply monitor.
.DESCRIPTION
  Shows the same toast notification that appears when the color profile is reapplied.
  Useful for verifying notifications work on the current system.
#>
[CmdletBinding()]
param()

Write-Host "Testing LG UltraGear toast notification..." -ForegroundColor Cyan

try {
    [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
    [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

    $template = @'
<toast duration="long">
  <visual>
    <binding template="ToastGeneric">
      <text>LG UltraGear</text>
      <text>Color profile reapplied</text>
    </binding>
  </visual>
</toast>
'@

    $xml = [Windows.Data.Xml.Dom.XmlDocument]::new()
    $xml.LoadXml($template)
    $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)

    # Use PowerShell's registered AppUserModelId for reliable notifications
    $appId = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)

    Write-Host "[ OK ] Toast notification sent successfully!" -ForegroundColor Green
    Write-Host "       You should see a notification in the bottom-right corner." -ForegroundColor Gray
} catch {
    Write-Host "[FAIL] Toast notification failed: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}
