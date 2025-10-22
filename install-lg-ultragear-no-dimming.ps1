<#
LG UltraGear No-Auto-Dim - installer

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

[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [string]$ProfilePath = ".\lg-ultragear-full-cal.icm",
    [string]$MonitorNameMatch = "LG ULTRAGEAR",
    [switch]$PerUser,               # Also associate in current-user scope
    [switch]$NoSetDefault,          # Do not set as default
    [switch]$SkipHdrAssociation,    # Skip ColorProfileAddDisplayAssociation
    [switch]$NoPrompt,              # Do not wait for Enter before exiting
    [switch]$InstallOnly,           # Only install profile; no association
    [switch]$Probe,                 # Probe only; no changes
    [switch]$DryRun,                # Simulate (-WhatIf)
    [switch]$SkipElevation          # Skip auto-elevation (for CI/testing)
)

begin {
    # Record the launch context so relative paths stay consistent after re-invocation.
    $script:InvocationPath = if ($PSCommandPath) { $PSCommandPath } else { $MyInvocation.MyCommand.Path }
    $script:InvocationDirectory = if ($script:InvocationPath) { Split-Path -LiteralPath $script:InvocationPath -Parent } else { $null }
    # Capture the caller's working directory if available; this helps rebuild relative paths later.
    try {
        $script:OriginalWorkingDirectory = (Get-Location).ProviderPath
    } catch {
        try { $script:OriginalWorkingDirectory = (Get-Location).Path } catch { $script:OriginalWorkingDirectory = $null }
    }

    function Test-IsAdmin {
        # Determine whether the current PowerShell session is elevated.
        $id = [Security.Principal.WindowsIdentity]::GetCurrent()
        $p = [Security.Principal.WindowsPrincipal]::new($id)
        return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    }

    function Ensure-EmbeddedProfile {
        param([string]$ProfileName)

        if (-not $ProfileName) { return $null }

        # Default to the script file location so packaged and loose files behave the same.
        $candidateBase = if ($script:InvocationDirectory -and (Test-Path -LiteralPath $script:InvocationDirectory)) {
            $script:InvocationDirectory
        } elseif ($script:OriginalWorkingDirectory -and (Test-Path -LiteralPath $script:OriginalWorkingDirectory)) {
            $script:OriginalWorkingDirectory
        } else {
            [IO.Path]::GetTempPath()
        }

        $destination = Join-Path $candidateBase $ProfileName
        $destinationDirectory = Split-Path -Parent $destination

        try {
            if (-not (Test-Path -LiteralPath $destinationDirectory)) {
                # Create the directory lazily so we can copy the embedded profile into place.
                [IO.Directory]::CreateDirectory($destinationDirectory) | Out-Null
            }
        } catch {
            # Fall back to the temp folder if we cannot reuse the original directory (e.g., missing path).
            $destination = Join-Path ([IO.Path]::GetTempPath()) $ProfileName
            $destinationDirectory = Split-Path -Parent $destination
            if (-not (Test-Path -LiteralPath $destinationDirectory)) {
                [IO.Directory]::CreateDirectory($destinationDirectory) | Out-Null
            }
        }

        if (Test-Path -LiteralPath $destination) {
            return (Resolve-Path -LiteralPath $destination -ErrorAction Stop).Path
        }

        # ps2exe exposes embedded payloads through Get-PS2EXEResource; skip if not available.
        $resourceCmd = Get-Command -Name Get-PS2EXEResource -ErrorAction SilentlyContinue
        if (-not $resourceCmd) { return $null }

        try {
            # Materialize the embedded profile so downstream file comparisons work transparently.
            $stream = Get-PS2EXEResource -MemoryStream $ProfileName -ErrorAction Stop
            try {
                $fileStream = [IO.File]::Open($destination, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)
                try {
                    $stream.WriteTo($fileStream)
                } finally {
                    $fileStream.Dispose()
                }
            } finally {
                $stream.Dispose()
            }
            Write-Verbose ("Extracted embedded profile resource to '{0}'" -f $destination)
            return (Resolve-Path -LiteralPath $destination -ErrorAction Stop).Path
        } catch {
            Write-Verbose ("Failed to extract embedded profile resource '{0}': {1}" -f $ProfileName, $_)
            return $null
        }
    }

    function Resolve-ProfilePath {
        param([string]$InputPath)

        if (-not $InputPath) { return $null }

        $candidates = @()
        # Try the caller-provided value first and backfill with known directories when relative.
        if ([IO.Path]::IsPathRooted($InputPath)) {
            $candidates += $InputPath
        } else {
            $candidates += $InputPath
            if ($script:OriginalWorkingDirectory) {
                $candidates += (Join-Path $script:OriginalWorkingDirectory $InputPath)
            }
            if ($script:InvocationDirectory) {
                $candidates += (Join-Path $script:InvocationDirectory $InputPath)
            }
        }

        foreach ($candidate in $candidates) {
            try {
                $resolved = Resolve-Path -LiteralPath $candidate -ErrorAction Stop
                return $resolved.Path
            } catch {
                Write-Verbose ("Profile lookup skipped for candidate '{0}': {1}" -f $candidate, $_.Exception.Message)
            }
        }

        $profileName = [IO.Path]::GetFileName($InputPath)
        return Ensure-EmbeddedProfile -ProfileName $profileName
    }

    function Get-UnicodeStringFromCodePoint {
        param([int[]]$CodePoints)

        if (-not $CodePoints) { return [string]::Empty }

        $builder = [System.Text.StringBuilder]::new()
        foreach ($codePoint in $CodePoints) {
            if ($codePoint -le 0xFFFF) {
                [void]$builder.Append([char]$codePoint)
            } else {
                $adjusted = $codePoint - 0x10000
                $highSurrogate = [int][math]::Floor($adjusted / 0x400) + 0xD800
                $lowSurrogate = ($adjusted % 0x400) + 0xDC00
                [void]$builder.Append([char]$highSurrogate)
                [void]$builder.Append([char]$lowSurrogate)
            }
        }

        return $builder.ToString()
    }

    $script:SymbolInfo = Get-UnicodeStringFromCodePoint -CodePoints @(0x2139, 0xFE0F)
    $script:SymbolAction = Get-UnicodeStringFromCodePoint -CodePoints @(0x2699, 0xFE0F)
    $script:SymbolSuccess = Get-UnicodeStringFromCodePoint -CodePoints @(0x2705)
    $script:SymbolWarning = Get-UnicodeStringFromCodePoint -CodePoints @(0x26A0, 0xFE0F)
    $script:SymbolNote = Get-UnicodeStringFromCodePoint -CodePoints @(0x1F4DD)

    # Auto-elevate if not already running as Administrator
    if (-not $SkipElevation.IsPresent) {
        if (-not (Test-IsAdmin)) {
            Write-Host "Elevating to Administrator..." -ForegroundColor Yellow
            $scriptPath = if ($script:InvocationPath) { $script:InvocationPath } else { $MyInvocation.MyCommand.Path }
            $argsList = New-Object System.Collections.Generic.List[string]
            $argsList.AddRange(@('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $scriptPath))
            foreach ($kv in $PSBoundParameters.GetEnumerator()) {
                $name = '-' + $kv.Key
                $val = $kv.Value
                if ($val -is [System.Management.Automation.SwitchParameter]) {
                    if ([bool]$val) { $argsList.Add($name) }
                } elseif ($val -is [bool]) {
                    if ($val) { $argsList.Add($name) }
                } else {
                    $argsList.Add($name)
                    $argsList.Add([string]$val)
                }
            }

            $workingDir = if ($script:OriginalWorkingDirectory -and (Test-Path -LiteralPath $script:OriginalWorkingDirectory)) {
                $script:OriginalWorkingDirectory
            } elseif ($script:InvocationDirectory -and (Test-Path -LiteralPath $script:InvocationDirectory)) {
                $script:InvocationDirectory
            } else {
                $env:SystemRoot
            }

            Start-Process -FilePath powershell.exe -ArgumentList $argsList.ToArray() -Verb RunAs -WorkingDirectory $workingDir | Out-Null
            exit
        }
    } else {
        Write-Verbose "SkipElevation requested; continuing without auto-elevation."
    }

    $ErrorActionPreference = 'Stop'

    # WCS scope constants
    $WCS_SCOPE_CURRENT_USER = 0
    $WCS_SCOPE_SYSTEM_WIDE = 2

    # WCS default profile constants
    $CPT_ICC = 1  # COLORPROFILETYPE.CPT_ICC
    $CPS_DEV = 0  # COLORPROFILESUBTYPE.CPS_DEVICE

    # Logging helpers with color + icon glyphs (approved verb 'Write')
    function Write-InfoMessage([string]$Message) { Write-Host ("{0}  {1}" -f $script:SymbolInfo, $Message) -ForegroundColor Cyan }
    function Write-ActionMessage([string]$Message) { Write-Host ("{0}  {1}" -f $script:SymbolAction, $Message) -ForegroundColor Yellow }
    function Write-SuccessMessage([string]$Message) { Write-Host ("{0} {1}" -f $script:SymbolSuccess, $Message) -ForegroundColor Green }
    function Write-WarnMessage([string]$Message) { Write-Host ("{0}  {1}" -f $script:SymbolWarning, $Message) -ForegroundColor DarkYellow }
    function Write-NoteMessage([string]$Message) { Write-Host ("{0} {1}" -f $script:SymbolNote, $Message) -ForegroundColor Gray }

    Write-InfoMessage "starting LG UltraGear no-dimming installer"

    if ($DryRun) {
        $script:WhatIfPreference = $true
        Write-NoteMessage "dry-run enabled (-WhatIf): no changes will be made"
    }

    # P/Invoke shims for mscms.dll and user32 keep all native calls in one place.
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
    Write-Verbose "Profile path (requested): $ProfilePath"
    $profileFull = Resolve-ProfilePath -InputPath $ProfilePath
    if (-not $profileFull) {
        throw "Profile not found at '$ProfilePath'. Place the ICC/ICM file there, pass -ProfilePath, or include it in the packaged executable."
    }
    Write-Verbose "Profile path (resolved): $profileFull"
    $profileName = [IO.Path]::GetFileName($profileFull)
    $installedStore = Join-Path $env:WINDIR 'System32\spool\drivers\color'
    $installedPath = Join-Path $installedStore $profileName

    Write-ActionMessage "install/refresh color profile: $profileName"
    if ($PSCmdlet.ShouldProcess($profileFull, "Install/refresh color profile in system store")) {
        if (Test-Path -LiteralPath $installedPath) {
            $srcHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $profileFull).Hash
            $dstHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedPath).Hash
            if ($srcHash -ne $dstHash) {
                Copy-Item -LiteralPath $profileFull -Destination $installedPath -Force
                Write-SuccessMessage "profile updated at: $installedPath"
            } else {
                Write-NoteMessage "profile already current at: $installedPath"
            }
        } else {
            if (-not [WcsNative]::InstallColorProfile($null, $profileFull)) {
                $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                throw "InstallColorProfile failed (Win32=$code)."
            }
            Write-SuccessMessage "profile installed to: $installedPath"
        }
    }

    if ($InstallOnly) {
        Write-InfoMessage "install-only mode: skipping association and defaults"
        if (-not $NoPrompt) {
            try {
                Write-Host ""
                Write-Host "Press Enter to exit..." -ForegroundColor DarkGray
                [void][System.Console]::ReadLine()
            } catch {
                Write-Verbose "InstallOnly prompt skipped (no interactive console)."
            }
        }
        return
    }

    # Collect all detected monitors so reporting and matching remain transparent to the user.
    Write-ActionMessage "enumerating monitors via WmiMonitorID ..."
    $monitors = Get-CimInstance -Namespace root/wmi -Class WmiMonitorID | ForEach-Object {
        $name = -join ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ })
        [PSCustomObject]@{
            InstanceName = $_.InstanceName
            FriendlyName = $name
        }
    }

    # Fail fast if Windows cannot enumerate any displays.
    if (-not $monitors) { throw "No monitors returned by WMI (WmiMonitorID)." }

    $targets = $monitors | Where-Object { $_.FriendlyName -like "*${MonitorNameMatch}*" }
    Write-Host ""
    Write-InfoMessage "detected monitors:"
    $monitors | ForEach-Object { Write-Host (" - {0} [{1}]" -f $_.FriendlyName, $_.InstanceName) -ForegroundColor Gray }
    Write-Host ""
    Write-InfoMessage ("matched (contains '{0}'):" -f $MonitorNameMatch)
    if ($targets) { $targets | ForEach-Object { Write-Host (" - {0} [{1}]" -f $_.FriendlyName, $_.InstanceName) -ForegroundColor Green } }
    else { Write-Host " - none" -ForegroundColor DarkYellow }
    Write-Host ""
    if (-not $targets) { throw "Nothing to do. Adjust -MonitorNameMatch." }

    if ($Probe) {
        # Probe mode stops after logging so no system changes occur.
        Write-InfoMessage "probe mode: no changes will be made"
        if (-not $NoPrompt) {
            try {
                Write-Host ""
                Write-Host "Press Enter to exit..." -ForegroundColor DarkGray
                [void][System.Console]::ReadLine()
            } catch {
                Write-Verbose "Probe prompt skipped (no interactive console)."
            }
        }
        return
    }

    # Apply the requested profile operations to every matched monitor.
    foreach ($m in $targets) {
        $deviceName = $m.InstanceName
        Write-ActionMessage "associating profile with: $($m.FriendlyName)"
        Write-Verbose "Device key: $deviceName"

        if ($PSCmdlet.ShouldProcess($deviceName, "Associate profile (system-wide)")) {
            if (-not [WcsNative]::WcsAssociateColorProfileWithDevice([uint32]$WCS_SCOPE_SYSTEM_WIDE, $installedPath, $deviceName)) {
                $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                Write-WarnMessage ("system-wide association failed (Win32={0})" -f $code)
            } else { Write-SuccessMessage "system-wide association ok" }
        }

        if ($PerUser.IsPresent) {
            if ($PSCmdlet.ShouldProcess($deviceName, "Associate profile (current user)")) {
                if (-not [WcsNative]::WcsAssociateColorProfileWithDevice([uint32]$WCS_SCOPE_CURRENT_USER, $installedPath, $deviceName)) {
                    $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                    Write-WarnMessage ("per-user association failed (Win32={0})" -f $code)
                } else { Write-SuccessMessage "per-user association ok" }
            }
        }

        if (-not $NoSetDefault) {
            if ($PSCmdlet.ShouldProcess($deviceName, "Set as default profile")) {
                if (-not [WcsNative]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_SYSTEM_WIDE, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
                    $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                    Write-WarnMessage ("set default (system) failed (Win32={0})" -f $code)
                } else { Write-SuccessMessage "set default (system) ok" }
                if ($PerUser.IsPresent) {
                    if (-not [WcsNative]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_CURRENT_USER, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
                        $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                        Write-WarnMessage ("set default (user) failed (Win32={0})" -f $code)
                    } else { Write-SuccessMessage "set default (user) ok" }
                }
            }
        }

        if (-not $SkipHdrAssociation) {
            try {
                # profileType 0 => ICC. No error if SDR.
                if ($PSCmdlet.ShouldProcess($deviceName, "HDR/advanced-color association")) {
                    [void][WcsNative]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_SYSTEM_WIDE, 0)
                    if ($PerUser.IsPresent) { [void][WcsNative]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_CURRENT_USER, 0) }
                    Write-SuccessMessage "HDR/advanced-color association ok"
                }
            } catch {
                Write-Verbose "HDR association API not available; skipping."
            }
        }
    }

    Write-ActionMessage "refreshing color settings"
    $HWND_BROADCAST = [IntPtr]0xffff
    $WM_SETTINGCHANGE = 0x1A
    $SMTO_ABORTIFHUNG = 0x0002
    [UIntPtr]$res = [UIntPtr]::Zero
    [void][Win32]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Color', $SMTO_ABORTIFHUNG, 2000, [ref]$res)

    Write-SuccessMessage "done. associated profile '$profileName' with all displays matching '$MonitorNameMatch'."

    if (-not $NoPrompt) {
        # Keep parity with interactive usage by pausing unless -NoPrompt was supplied or stdin is unavailable.
        try {
            Write-Host ""
            Write-Host "Press Enter to exit..." -ForegroundColor DarkGray
            [void][System.Console]::ReadLine()
        } catch {
            Write-Verbose "Final prompt skipped (no interactive console)."
        }
    }
}
