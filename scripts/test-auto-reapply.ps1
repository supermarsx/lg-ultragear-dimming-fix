<#
.SYNOPSIS
  Standalone auto-reapply script for LG UltraGear color profile.
.DESCRIPTION
  Checks for LG UltraGear monitor, reapplies the color profile, and shows a toast notification.
  This is a self-contained script that can be run independently or by the scheduled task.
.PARAMETER NoNotification
  Skip the toast notification after reapplying.
.PARAMETER MonitorMatch
  Pattern to match monitor names (default: 'LG UltraGear').
#>
[CmdletBinding()]
param(
    [switch]$NoNotification,
    [string]$MonitorMatch = 'LG UltraGear'
)

$ErrorActionPreference = 'Stop'

# ============================================================================
# MONITOR DETECTION
# ============================================================================

$found = $false
try {
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop | ForEach-Object {
        $name = ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
        if ($name -match $MonitorMatch) {
            $found = $true
        }
    }
} catch {
    exit 0  # Can't enumerate monitors, exit silently
}

if (-not $found) {
    exit 0  # No matching monitor, exit silently
}

# ============================================================================
# COLOR PROFILE APPLICATION
# ============================================================================

$profileName = 'lg-ultragear-full-cal.icm'
$profilePath = Join-Path $env:WINDIR "System32\spool\drivers\color\$profileName"
$defaultProfilePath = Join-Path $env:WINDIR "System32\spool\drivers\color\sRGB Color Space Profile.icm"

if (-not (Test-Path -LiteralPath $profilePath)) {
    exit 1  # Profile not installed
}

# Load WCS API for color profile association and disassociation
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class WcsProfile {
    [DllImport("mscms.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern bool WcsAssociateColorProfileWithDevice(
        uint scope, [MarshalAs(UnmanagedType.LPWStr)] string profileName,
        [MarshalAs(UnmanagedType.LPWStr)] string deviceName);

    [DllImport("mscms.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern bool WcsDisassociateColorProfileFromDevice(
        uint scope, [MarshalAs(UnmanagedType.LPWStr)] string profileName,
        [MarshalAs(UnmanagedType.LPWStr)] string deviceName);
}
'@ -ErrorAction SilentlyContinue

# Get matching monitor device IDs - first switch to default, then reapply fix
# This forces Windows to actually refresh the color profile
$WCS_SCOPE_SYSTEM_WIDE = 2
Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue | ForEach-Object {
    $name = ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
    if ($name -match $MonitorMatch) {
        $deviceKey = $_.InstanceName -replace '_0$', ''
        try {
            # Step 1: Disassociate the fix profile (reverts to default)
            [void][WcsProfile]::WcsDisassociateColorProfileFromDevice($WCS_SCOPE_SYSTEM_WIDE, $profilePath, $deviceKey)

            # Step 2: Brief pause to let Windows process the change
            Start-Sleep -Milliseconds 100

            # Step 3: Re-associate the fix profile
            [void][WcsProfile]::WcsAssociateColorProfileWithDevice($WCS_SCOPE_SYSTEM_WIDE, $profilePath, $deviceKey)
        } catch {
            # Association failed, continue silently
        }
    }
}

# Force display refresh using multiple Windows APIs for maximum reliability
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class Win32Display {
    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    public static extern IntPtr SendMessageTimeout(
        IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
        uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    public static extern int ChangeDisplaySettingsEx(
        string lpszDeviceName, IntPtr lpDevMode, IntPtr hwnd,
        uint dwflags, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool InvalidateRect(IntPtr hWnd, IntPtr lpRect, bool bErase);
}
'@ -ErrorAction SilentlyContinue

# Method 1: ChangeDisplaySettingsEx with null - triggers full display refresh
[void][Win32Display]::ChangeDisplaySettingsEx($null, [IntPtr]::Zero, [IntPtr]::Zero, 0, [IntPtr]::Zero)

# Method 2: Broadcast WM_SETTINGCHANGE with "Color" parameter
$HWND_BROADCAST = [IntPtr]0xffff
$WM_SETTINGCHANGE = 0x1A
$SMTO_ABORTIFHUNG = 0x0002
[UIntPtr]$res = [UIntPtr]::Zero
[void][Win32Display]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Color', $SMTO_ABORTIFHUNG, 2000, [ref]$res)

# Method 3: Invalidate all windows to force repaint with new color profile
[void][Win32Display]::InvalidateRect([IntPtr]::Zero, [IntPtr]::Zero, $true)

# Method 4: Trigger Windows Calibration Loader - this is the key to reloading ICC profiles
try {
    Start-ScheduledTask -TaskPath "\Microsoft\Windows\WindowsColorSystem\" -TaskName "Calibration Loader" -ErrorAction SilentlyContinue
} catch {
    # Task may not exist on all systems
}

# ============================================================================
# TOAST NOTIFICATION
# ============================================================================

if (-not $NoNotification) {
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
    } catch {
        # Notification failed silently - not critical
    }
}

exit 0
