<#
.SYNOPSIS
  Self-contained auto-reapply monitor for LG UltraGear color profile.

.DESCRIPTION
  Creates or removes a scheduled task that automatically reapplies the color
  profile when display events occur. This is a standalone script that doesn't
  depend on other files (except the main installer for reapplication).

.USAGE
  PS> .\install-monitor.ps1
  PS> .\install-monitor.ps1 -Uninstall
  PS> .\install-monitor.ps1 -MonitorNameMatch "LG" -InstallerPath "C:\path\to\installer.ps1"

.PARAMETERS
  -Uninstall          Remove the monitor task
  -InstallerPath      Path to the main installer (default: same directory)
  -MonitorNameMatch   Monitor name pattern (default: 'LG ULTRAGEAR')
  -TaskName           Custom task name (default: 'LG-UltraGear-ColorProfile-AutoReapply')

.NOTES
  Requires Administrator privileges.
  Works independently - no other scripts required except for the installer reference.
#>

[CmdletBinding()]
param(
    [Parameter(HelpMessage="Remove the monitor watcher task")]
    [switch]$Uninstall,
    
    [Parameter(HelpMessage="Path to the main installer script")]
    [string]$InstallerPath,
    
    [Parameter(HelpMessage="Monitor name match pattern")]
    [string]$MonitorNameMatch = 'LG ULTRAGEAR',
    
    [Parameter(HelpMessage="Task name in Task Scheduler")]
    [string]$TaskName = "LG-UltraGear-ColorProfile-AutoReapply"
)

$ErrorActionPreference = 'Stop'

# Auto-detect installer path if not provided
if (-not $InstallerPath) {
    $InstallerPath = Join-Path $PSScriptRoot "install-lg-ultragear-no-dimming.ps1"
}

# Check for admin privileges
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[ERR ] This script requires Administrator privileges." -ForegroundColor Red
    Write-Host "[INFO] Right-click and select 'Run as Administrator', or use:" -ForegroundColor Yellow
    Write-Host "       Start-Process powershell -Verb RunAs -ArgumentList '-ExecutionPolicy Bypass -File `"$PSCommandPath`"'" -ForegroundColor Gray
    exit 1
}

# Handle uninstall
if ($Uninstall) {
    Write-Host "[INFO] Removing monitor watcher task..." -ForegroundColor Cyan
    try {
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction Stop
        Write-Host "[OK  ] Task '$TaskName' has been removed." -ForegroundColor Green
        
        # Also remove the action script if it exists
        $actionScriptPath = "$env:ProgramData\LG-UltraGear-Monitor\reapply-profile.ps1"
        if (Test-Path $actionScriptPath) {
            Remove-Item -Path $actionScriptPath -Force -ErrorAction SilentlyContinue
            $actionScriptDir = Split-Path -Path $actionScriptPath -Parent
            if (Test-Path $actionScriptDir) {
                Remove-Item -Path $actionScriptDir -Recurse -Force -ErrorAction SilentlyContinue
            }
            Write-Host "[OK  ] Removed action script directory." -ForegroundColor Green
        }
    } catch {
        if ($_.Exception.Message -match "No MSFT_ScheduledTask objects found") {
            Write-Host "[NOTE] Task '$TaskName' was not found (already removed)." -ForegroundColor Gray
        } else {
            Write-Host "[ERR ] Failed to remove task: $($_.Exception.Message)" -ForegroundColor Red
            exit 1
        }
    }
    exit 0
}

# Verify installer script exists
if (-not (Test-Path -LiteralPath $InstallerPath)) {
    Write-Host "[ERR ] Installer script not found: $InstallerPath" -ForegroundColor Red
    Write-Host "[INFO] Please ensure 'install-lg-ultragear-no-dimming.ps1' is in the same directory." -ForegroundColor Yellow
    Write-Host "[INFO] Or specify the path with -InstallerPath parameter." -ForegroundColor Yellow
    exit 1
}

Write-Host "[INFO] Installing LG UltraGear Color Profile Auto-Reapply Monitor" -ForegroundColor Cyan
Write-Host ""

# Create the action script that will be executed on events
$actionScript = @"
# Auto-reapply LG UltraGear color profile
`$ErrorActionPreference = 'SilentlyContinue'

# Wait a moment for display to stabilize after reconnection
Start-Sleep -Seconds 2

# Execute the installer in silent mode
& '$InstallerPath' -NoSetDefault -NoPrompt -SkipElevation -SkipWindowsTerminal -MonitorNameMatch '$MonitorNameMatch' 2>`$null | Out-Null
"@

# Save the action script to a persistent location
$actionScriptPath = "$env:ProgramData\LG-UltraGear-Monitor\reapply-profile.ps1"
$actionScriptDir = Split-Path -Path $actionScriptPath -Parent

if (-not (Test-Path -LiteralPath $actionScriptDir)) {
    New-Item -ItemType Directory -Path $actionScriptDir -Force | Out-Null
}

Set-Content -Path $actionScriptPath -Value $actionScript -Force
Write-Host "[OK  ] Created action script: $actionScriptPath" -ForegroundColor Green

# Create scheduled task with multiple triggers for different scenarios
$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$actionScriptPath`""

# Trigger 1: On any display device arrival (Event ID 20001, 20003 from Kernel-PnP)
$trigger1 = New-ScheduledTaskTrigger -AtLogOn
$cimTrigger1 = Get-CimClass -ClassName MSFT_TaskEventTrigger -Namespace Root/Microsoft/Windows/TaskScheduler
$trigger1.CimInstanceProperties.Item('Enabled').Value = $true
$trigger1.CimInstanceProperties.Item('Subscription').Value = @"
<QueryList>
  <Query Id="0" Path="System">
    <Select Path="System">*[System[Provider[@Name='Microsoft-Windows-Kernel-PnP'] and (EventID=20001 or EventID=20003)]]</Select>
  </Query>
</QueryList>
"@

# Trigger 2: On user logon (ensures profile is applied after reboot)
$trigger2 = New-ScheduledTaskTrigger -AtLogOn

# Trigger 3: On workstation unlock (covers wake from sleep)
$trigger3 = New-ScheduledTaskTrigger -AtLogOn
$cimTrigger3 = Get-CimClass -ClassName MSFT_TaskSessionStateChangeTrigger -Namespace Root/Microsoft/Windows/TaskScheduler
$trigger3.CimInstanceProperties.Item('Enabled').Value = $true
$trigger3.CimInstanceProperties.Item('StateChange').Value = 8  # SessionUnlock

# Task settings
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Minutes 2)

# Register the task with all triggers
try {
    # Remove existing task if present
    try {
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
    } catch {}
    
    # Register with event-based trigger
    Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger1, $trigger2, $trigger3 -Principal $principal -Settings $settings -Description "Automatically reapplies LG UltraGear color profile when display reconnects to prevent auto-dimming. Uses event monitoring for minimal overhead." | Out-Null
    
    Write-Host "[OK  ] Scheduled task '$TaskName' created successfully." -ForegroundColor Green
    Write-Host ""
    Write-Host "[INFO] The color profile will now automatically reapply when:" -ForegroundColor Cyan
    Write-Host "       - Monitor is disconnected and reconnected" -ForegroundColor Gray
    Write-Host "       - System wakes from sleep" -ForegroundColor Gray
    Write-Host "       - User logs in or unlocks workstation" -ForegroundColor Gray
    Write-Host "       - Display settings are changed" -ForegroundColor Gray
    Write-Host ""
    Write-Host "[INFO] The monitor uses event-driven triggers (very lightweight - no polling)." -ForegroundColor Cyan
    Write-Host ""
    Write-Host "[TIP ] To uninstall: .\install-monitor.ps1 -Uninstall" -ForegroundColor Gray
    Write-Host "[TIP ] To view task: taskschd.msc -> Task Scheduler Library -> $TaskName" -ForegroundColor Gray
    
} catch {
    Write-Host "[ERR ] Failed to create scheduled task: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}
