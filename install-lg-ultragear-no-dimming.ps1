<#
LG UltraGear No-Auto-Dim – installer

Automates installing and associating the bundled ICC/ICM color profile with
LG UltraGear displays to mitigate firmware auto-dimming by constraining the
effective luminance range. Run from an elevated PowerShell prompt.

Steps
- Install the color profile into the system color store
- Discover monitors by friendly name (e.g. "LG ULTRAGEAR")
- Associate the profile with each matched display (system-wide, and optionally per-user)
- Optionally set it as the default profile for the display
- Nudge Windows to refresh color settings

Usage examples
  PS> .\install-lg-ultragear-no-dimming.ps1 -Verbose
  PS> .\install-lg-ultragear-no-dimming.ps1 -MonitorNameMatch "LG ULTRAGEAR" -ProfilePath .\lg-ultragear-full-cal.icm -PerUser -Verbose

#>

[CmdletBinding(SupportsShouldProcess=$true)]
param(
  [string]$ProfilePath = ".\lg-ultragear-full-cal.icm",
  [string]$MonitorNameMatch = "LG ULTRAGEAR",
  [switch]$PerUser,               # Also associate in current-user scope
  [switch]$NoSetDefault,          # Do not set as default
  [switch]$SkipHdrAssociation,    # Skip ColorProfileAddDisplayAssociation
  [switch]$NoPrompt,              # Do not wait for Enter before exiting
  [switch]$InstallOnly,           # Only install profile; no association
  [switch]$Probe,                 # Probe only; no changes
  [switch]$DryRun                 # Simulate (-WhatIf)
)

begin {
  function Test-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p  = [Security.Principal.WindowsPrincipal]::new($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  }

  # Auto-elevate if not already running as Administrator
  if (-not (Test-IsAdmin)) {
    Write-Host "Elevating to Administrator..." -ForegroundColor Yellow
    $scriptPath = if ($PSCommandPath) { $PSCommandPath } else { $MyInvocation.MyCommand.Path }
    $argsList = @('-NoProfile','-ExecutionPolicy','Bypass','-File', ('"' + $scriptPath + '"'))
    foreach ($kv in $PSBoundParameters.GetEnumerator()) {
      $name = '-' + $kv.Key
      $val  = $kv.Value
      if ($val -is [System.Management.Automation.SwitchParameter]) {
        if ([bool]$val) { $argsList += $name }
      } elseif ($val -is [bool]) {
        if ($val) { $argsList += $name }
      } else {
        $s = [string]$val
        if ($s -match '"') { $s = $s -replace '"','\"' }
        if ($s -match '\s') { $s = '"' + $s + '"' }
        $argsList += $name
        $argsList += $s
      }
    }
    $joined = [string]::Join(' ', $argsList)
    Start-Process -FilePath powershell.exe -ArgumentList $joined -Verb RunAs | Out-Null
    exit
  }

  $ErrorActionPreference = 'Stop'

  # WCS scope constants
  $WCS_SCOPE_CURRENT_USER = 0
  $WCS_SCOPE_SYSTEM_WIDE  = 2

  # WCS default profile constants
  $CPT_ICC = 1  # COLORPROFILETYPE.CPT_ICC
  $CPS_DEV = 0  # COLORPROFILESUBTYPE.CPS_DEVICE

  # Logging helpers with color + emojis
  function Log-Info([string]$msg)    { Write-Host "ℹ️  $msg" -ForegroundColor Cyan }
  function Log-Action([string]$msg)  { Write-Host "⚙️  $msg" -ForegroundColor Yellow }
  function Log-Ok([string]$msg)      { Write-Host "✅ $msg" -ForegroundColor Green }
  function Log-Warn([string]$msg)    { Write-Host "⚠️  $msg" -ForegroundColor DarkYellow }
  function Log-Note([string]$msg)    { Write-Host "📝 $msg" -ForegroundColor Gray }

  Log-Info "starting LG UltraGear no-dimming installer"

  if ($DryRun) {
    $script:WhatIfPreference = $true
    Log-Note "dry-run enabled (-WhatIf): no changes will be made"
  }

  # P/Invoke shims for mscms.dll and user32
  $src = @"
using System;
using System.Runtime.InteropServices;

public static class WcsNative {
  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true, EntryPoint="InstallColorProfileW")]
  public static extern bool InstallColorProfile(string machine, string profilePath);

  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern bool WcsAssociateColorProfileWithDevice(uint scope, string profile, string deviceName);

  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern bool WcsSetDefaultColorProfile(uint scope, string deviceName, int cpt, int cps, uint profileID, string profileName);

  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true, EntryPoint="ColorProfileAddDisplayAssociation")]
  public static extern bool ColorProfileAddDisplayAssociation(string profile, string deviceName, uint scope, uint profileType);
}

public static class Win32 {
  [DllImport("user32.dll", SetLastError=true)]
  public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
}
"@
  Add-Type -TypeDefinition $src -ErrorAction Stop
}

process {
  Write-Verbose "Profile path: $ProfilePath"
  if (-not (Test-Path -LiteralPath $ProfilePath)) {
    throw "Profile not found at '$ProfilePath'. Place the ICC/ICM file there or pass -ProfilePath."
  }

  $profileFull = (Resolve-Path -LiteralPath $ProfilePath).Path
  $profileName = [IO.Path]::GetFileName($profileFull)
  $installedStore = Join-Path $env:WINDIR 'System32\spool\drivers\color'
  $installedPath  = Join-Path $installedStore $profileName

  Log-Action "install/refresh color profile: $profileName"
  if ($PSCmdlet.ShouldProcess($profileFull, "Install/refresh color profile in system store")) {
    if (Test-Path -LiteralPath $installedPath) {
      $srcHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $profileFull).Hash
      $dstHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedPath).Hash
      if ($srcHash -ne $dstHash) {
        Copy-Item -LiteralPath $profileFull -Destination $installedPath -Force
        Write-Host "✅ profile updated at: $installedPath" -ForegroundColor Green
      } else {
        Write-Host "📝 profile already current at: $installedPath" -ForegroundColor Gray
      }
    } else {
      if (-not [WcsNative]::InstallColorProfile($null, $profileFull)) {
        $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "InstallColorProfile failed (Win32=$code)."
      }
      Write-Host "✅ profile installed to: $installedPath" -ForegroundColor Green
    }
  }

  if ($InstallOnly) {
    Write-Host "ℹ️  install-only mode: skipping association and defaults" -ForegroundColor Cyan
    if (-not $NoPrompt) { try { Write-Host ""; Write-Host "Press Enter to exit..." -ForegroundColor DarkGray; [void][System.Console]::ReadLine() } catch {} }
    return
  }

  Write-Host "🔎 enumerating monitors via WmiMonitorID ..." -ForegroundColor Yellow
  $monitors = Get-CimInstance -Namespace root/wmi -Class WmiMonitorID | ForEach-Object {
    $name = -join ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ })
    [PSCustomObject]@{
      InstanceName = $_.InstanceName
      FriendlyName = $name
    }
  }

  if (-not $monitors) { throw "No monitors returned by WMI (WmiMonitorID)." }

  $targets = $monitors | Where-Object { $_.FriendlyName -like "*${MonitorNameMatch}*" }
  Write-Host ""
  Write-Host "🖥️  detected monitors:" -ForegroundColor White
  $monitors | ForEach-Object { Write-Host (" - {0} [{1}]" -f $_.FriendlyName, $_.InstanceName) -ForegroundColor Gray }
  Write-Host ""
  Write-Host ("🎯 matched (contains '{0}'):" -f $MonitorNameMatch) -ForegroundColor White
  if ($targets) { $targets | ForEach-Object { Write-Host (" - {0} [{1}]" -f $_.FriendlyName, $_.InstanceName) -ForegroundColor Green } }
  else { Write-Host " - none" -ForegroundColor DarkYellow }
  Write-Host ""
  if (-not $targets) { throw "Nothing to do. Adjust -MonitorNameMatch." }

  if ($Probe) {
    Write-Host "ℹ️  probe mode: no changes will be made" -ForegroundColor Cyan
    if (-not $NoPrompt) { try { Write-Host ""; Write-Host "Press Enter to exit..." -ForegroundColor DarkGray; [void][System.Console]::ReadLine() } catch {} }
    return
  }

  foreach ($m in $targets) {
    $deviceName = $m.InstanceName
    Write-Host "🔗 associating profile with: $($m.FriendlyName)" -ForegroundColor Cyan
    Write-Verbose "Device key: $deviceName"

    if ($PSCmdlet.ShouldProcess($deviceName, "Associate profile (system-wide)")) {
      if (-not [WcsNative]::WcsAssociateColorProfileWithDevice([uint32]$WCS_SCOPE_SYSTEM_WIDE, $installedPath, $deviceName)) {
        $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        Write-Host ("❌ system-wide association failed (Win32={0})" -f $code) -ForegroundColor Red
      } else { Write-Host "✅ system-wide association ok" -ForegroundColor Green }
    }

    if ($PerUser.IsPresent) {
      if ($PSCmdlet.ShouldProcess($deviceName, "Associate profile (current user)")) {
        if (-not [WcsNative]::WcsAssociateColorProfileWithDevice([uint32]$WCS_SCOPE_CURRENT_USER, $installedPath, $deviceName)) {
          $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
          Write-Host ("❌ per-user association failed (Win32={0})" -f $code) -ForegroundColor DarkRed
        } else { Write-Host "✅ per-user association ok" -ForegroundColor Green }
      }
    }

    if (-not $NoSetDefault) {
      if ($PSCmdlet.ShouldProcess($deviceName, "Set as default profile")) {
        if (-not [WcsNative]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_SYSTEM_WIDE, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
          $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
          Write-Host ("❌ set default (system) failed (Win32={0})" -f $code) -ForegroundColor DarkRed
        } else { Write-Host "✅ set default (system) ok" -ForegroundColor Green }
        if ($PerUser.IsPresent) {
          if (-not [WcsNative]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_CURRENT_USER, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
            $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
            Write-Host ("❌ set default (user) failed (Win32={0})" -f $code) -ForegroundColor DarkRed
          } else { Write-Host "✅ set default (user) ok" -ForegroundColor Green }
        }
      }
    }

    if (-not $SkipHdrAssociation) {
      try {
        # profileType 0 => ICC. No error if SDR.
        if ($PSCmdlet.ShouldProcess($deviceName, "HDR/advanced-color association")) {
          [void][WcsNative]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_SYSTEM_WIDE, 0)
          if ($PerUser.IsPresent) { [void][WcsNative]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_CURRENT_USER, 0) }
          Write-Host "✅ HDR/advanced-color association ok" -ForegroundColor Green
        }
      } catch {
        Write-Verbose "HDR association API not available; skipping."
      }
    }
  }

  Write-Host "🔄 refreshing color settings" -ForegroundColor Yellow
  $HWND_BROADCAST = [IntPtr]0xffff
  $WM_SETTINGCHANGE = 0x1A
  $SMTO_ABORTIFHUNG = 0x0002
  [UIntPtr]$res = [UIntPtr]::Zero
  [void][Win32]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Color', $SMTO_ABORTIFHUNG, 2000, [ref]$res)

  Write-Host "🎉 done. associated profile '$profileName' with all displays matching '$MonitorNameMatch'." -ForegroundColor Green

  if (-not $NoPrompt) {
    try {
      Write-Host ""; Write-Host "Press Enter to exit..." -ForegroundColor DarkGray
      [void][System.Console]::ReadLine()
    } catch {}
  }
}
