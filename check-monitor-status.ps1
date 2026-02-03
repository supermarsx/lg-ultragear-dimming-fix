<#
.SYNOPSIS
  Check the status of LG UltraGear color profile auto-reapply monitor.

.DESCRIPTION
  Displays the current status of the scheduled task, recent execution history,
  and verifies the color profile is properly installed.

.USAGE
  PS> .\check-monitor-status.ps1
#>

[CmdletBinding()]
param(
    [Parameter(HelpMessage="Show detailed task configuration")]
    [switch]$Detailed
)

$taskName = "LG-UltraGear-ColorProfile-AutoReapply"
$profileName = "lg-ultragear-full-cal.icm"
$colorPath = "$env:WINDIR\System32\spool\drivers\color"

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host " LG UltraGear Auto-Reapply Monitor - Status Check" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

# Check if scheduled task exists
Write-Host "[1/3] Checking scheduled task..." -ForegroundColor Yellow
try {
    $task = Get-ScheduledTask -TaskName $taskName -ErrorAction Stop
    Write-Host "      ✓ Task found: $taskName" -ForegroundColor Green
    Write-Host "      Status: " -NoNewline
    
    switch ($task.State) {
        "Ready"    { Write-Host "Ready (active)" -ForegroundColor Green }
        "Running"  { Write-Host "Currently running" -ForegroundColor Cyan }
        "Disabled" { Write-Host "DISABLED - Enable it!" -ForegroundColor Red }
        default    { Write-Host $task.State -ForegroundColor Yellow }
    }
    
    $taskInfo = Get-ScheduledTaskInfo -TaskName $taskName -ErrorAction SilentlyContinue
    if ($taskInfo) {
        Write-Host "      Last run: " -NoNewline
        if ($taskInfo.LastRunTime -eq (Get-Date 0)) {
            Write-Host "Never" -ForegroundColor Yellow
        } else {
            Write-Host $taskInfo.LastRunTime.ToString("yyyy-MM-dd HH:mm:ss") -ForegroundColor Gray
        }
        
        Write-Host "      Last result: " -NoNewline
        if ($taskInfo.LastTaskResult -eq 0) {
            Write-Host "Success (0x0)" -ForegroundColor Green
        } else {
            Write-Host ("0x{0:X}" -f $taskInfo.LastTaskResult) -ForegroundColor Red
        }
        
        Write-Host "      Next run: " -NoNewline
        if ($taskInfo.NextRunTime -eq (Get-Date 0)) {
            Write-Host "On event trigger" -ForegroundColor Gray
        } else {
            Write-Host $taskInfo.NextRunTime.ToString("yyyy-MM-dd HH:mm:ss") -ForegroundColor Gray
        }
    }
    
    if ($Detailed) {
        Write-Host ""
        Write-Host "      Triggers:" -ForegroundColor Cyan
        foreach ($trigger in $task.Triggers) {
            Write-Host "        - $($trigger.CimClass.CimClassName)" -ForegroundColor Gray
        }
        Write-Host "      Actions:" -ForegroundColor Cyan
        foreach ($action in $task.Actions) {
            Write-Host "        - Execute: $($action.Execute)" -ForegroundColor Gray
            Write-Host "          Arguments: $($action.Arguments)" -ForegroundColor DarkGray
        }
    }
    
} catch {
    Write-Host "      ✗ Task NOT found" -ForegroundColor Red
    Write-Host "      To install: run install-with-auto-reapply.bat" -ForegroundColor Yellow
}

Write-Host ""

# Check if color profile exists
Write-Host "[2/3] Checking color profile..." -ForegroundColor Yellow
$profilePath = Join-Path $colorPath $profileName
if (Test-Path -LiteralPath $profilePath) {
    Write-Host "      ✓ Profile installed: $profileName" -ForegroundColor Green
    $profile = Get-Item -LiteralPath $profilePath
    Write-Host "      Location: $($profile.DirectoryName)" -ForegroundColor Gray
    Write-Host "      Size: $($profile.Length) bytes" -ForegroundColor Gray
    Write-Host "      Modified: $($profile.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))" -ForegroundColor Gray
} else {
    Write-Host "      ✗ Profile NOT found: $profilePath" -ForegroundColor Red
    Write-Host "      Run install script to install profile first" -ForegroundColor Yellow
}

Write-Host ""

# Check for LG UltraGear monitors
Write-Host "[3/3] Checking for LG UltraGear monitors..." -ForegroundColor Yellow
try {
    $monitors = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop
    $lgMonitors = $monitors | Where-Object {
        $friendlyName = ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
        $friendlyName -match 'LG.*ULTRAGEAR'
    }
    
    if ($lgMonitors) {
        Write-Host "      ✓ Found $($lgMonitors.Count) LG UltraGear monitor(s)" -ForegroundColor Green
        foreach ($mon in $lgMonitors) {
            $name = ($mon.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
            Write-Host "        - $name" -ForegroundColor Gray
        }
    } else {
        Write-Host "      ⚠ No LG UltraGear monitors detected" -ForegroundColor Yellow
        Write-Host "        (Task will activate when monitor connects)" -ForegroundColor Gray
    }
} catch {
    Write-Host "      ⚠ Could not query monitors: $($_.Exception.Message)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

# Summary
$taskExists = $null -ne (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue)
$profileExists = Test-Path -LiteralPath $profilePath

if ($taskExists -and $profileExists) {
    Write-Host "✓ Status: Auto-reapply monitor is ACTIVE and working" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Your color profile will automatically reapply when:" -ForegroundColor Gray
    Write-Host "  • Monitor disconnects and reconnects" -ForegroundColor Gray
    Write-Host "  • System wakes from sleep" -ForegroundColor Gray
    Write-Host "  • User logs in or unlocks workstation" -ForegroundColor Gray
    Write-Host ""
} elseif ($taskExists -and -not $profileExists) {
    Write-Host "⚠ Status: Task exists but profile is missing!" -ForegroundColor Yellow
    Write-Host "  Run: install-lg-ultragear-no-dimming.ps1" -ForegroundColor Yellow
} elseif (-not $taskExists -and $profileExists) {
    Write-Host "⚠ Status: Profile installed but auto-reapply is NOT active" -ForegroundColor Yellow
    Write-Host "  Run: install-with-auto-reapply.bat" -ForegroundColor Yellow
} else {
    Write-Host "✗ Status: Neither profile nor monitor is installed" -ForegroundColor Red
    Write-Host "  Run: install-with-auto-reapply.bat" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Commands:" -ForegroundColor Cyan
Write-Host "  Test now:      Start-ScheduledTask -TaskName '$taskName'" -ForegroundColor Gray
Write-Host "  View history:  taskschd.msc" -ForegroundColor Gray
Write-Host "  Uninstall:     .\install-monitor-watcher.ps1 -Uninstall" -ForegroundColor Gray
Write-Host ""
