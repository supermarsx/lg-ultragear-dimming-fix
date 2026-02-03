@echo off
:: ============================================================================
:: LG UltraGear Auto-Dimming Fix - Complete Installer
:: ============================================================================
:: This batch file does everything:
:: 1. Installs the color profile to fix auto-dimming
:: 2. Sets up automatic reapplication on monitor reconnection
::
:: Just double-click to run!
:: ============================================================================

setlocal EnableDelayedExpansion

echo.
echo ========================================================================
echo  LG UltraGear Auto-Dimming Fix - Complete Installation
echo ========================================================================
echo.

:: Check for admin and elevate if needed
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [INFO] Requesting administrator privileges...
    powershell -NoProfile -Command "Start-Process -Verb RunAs -FilePath '%~f0'"
    exit /b
)

:: Verify required files exist
set "SCRIPT_DIR=%~dp0"
set "INSTALLER=%SCRIPT_DIR%install-lg-ultragear-no-dimming.ps1"

if not exist "%INSTALLER%" (
    echo [ERROR] Required file not found: install-lg-ultragear-no-dimming.ps1
    echo [INFO]  Please ensure all files are in the same directory
    pause
    exit /b 1
)

echo [STEP 1/2] Installing color profile...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%INSTALLER%" -NoPrompt -SkipElevation -SkipWindowsTerminal
if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Profile installation failed!
    pause
    exit /b 1
)

echo.
echo.
echo [STEP 2/2] Installing auto-reapply monitor...
echo.

:: Create the monitor directly in PowerShell (embedded script)
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
"$ErrorActionPreference='Stop'; ^
$TaskName='LG-UltraGear-ColorProfile-AutoReapply'; ^
$InstallerPath='%INSTALLER%'; ^
$MonitorNameMatch='LG ULTRAGEAR'; ^
Write-Host '[INFO] Creating auto-reapply monitor...' -ForegroundColor Cyan; ^
$actionScript=@' ^
`$ErrorActionPreference='SilentlyContinue'; ^
Start-Sleep -Seconds 2; ^
^& '%INSTALLER%' -NoSetDefault -NoPrompt -SkipElevation -SkipWindowsTerminal -MonitorNameMatch '%MonitorNameMatch%' 2>`$null | Out-Null ^
'@; ^
$actionScriptPath='$env:ProgramData\LG-UltraGear-Monitor\reapply-profile.ps1'; ^
$actionScriptDir=Split-Path -Path $actionScriptPath -Parent; ^
if (-not (Test-Path -LiteralPath $actionScriptDir)) { ^
    New-Item -ItemType Directory -Path $actionScriptDir -Force | Out-Null ^
}; ^
Set-Content -Path $actionScriptPath -Value $actionScript -Force; ^
Write-Host '[OK  ] Created action script' -ForegroundColor Green; ^
$action=New-ScheduledTaskAction -Execute 'powershell.exe' -Argument \"-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File \`\"$actionScriptPath\`\"\"; ^
$trigger1=New-ScheduledTaskTrigger -AtLogOn; ^
$cimTrigger1=Get-CimClass -ClassName MSFT_TaskEventTrigger -Namespace Root/Microsoft/Windows/TaskScheduler; ^
$trigger1.CimInstanceProperties.Item('Enabled').Value=$true; ^
$trigger1.CimInstanceProperties.Item('Subscription').Value='<QueryList><Query Id=\"0\" Path=\"System\"><Select Path=\"System\">*[System[Provider[@Name=''Microsoft-Windows-Kernel-PnP''] and (EventID=20001 or EventID=20003)]]</Select></Query></QueryList>'; ^
$trigger2=New-ScheduledTaskTrigger -AtLogOn; ^
$trigger3=New-ScheduledTaskTrigger -AtLogOn; ^
$cimTrigger3=Get-CimClass -ClassName MSFT_TaskSessionStateChangeTrigger -Namespace Root/Microsoft/Windows/TaskScheduler; ^
$trigger3.CimInstanceProperties.Item('Enabled').Value=$true; ^
$trigger3.CimInstanceProperties.Item('StateChange').Value=8; ^
$principal=New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest; ^
$settings=New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Minutes 2); ^
try { Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue } catch {}; ^
Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger1,$trigger2,$trigger3 -Principal $principal -Settings $settings -Description 'Automatically reapplies LG UltraGear color profile when display reconnects to prevent auto-dimming. Uses event monitoring for minimal overhead.' | Out-Null; ^
Write-Host '[OK  ] Scheduled task created successfully' -ForegroundColor Green"

if %errorlevel% neq 0 (
    echo.
    echo [WARNING] Monitor installation failed, but profile is installed.
    echo You can try running install-monitor.ps1 separately.
)

echo.
echo ========================================================================
echo  Installation Complete!
echo ========================================================================
echo.
echo The color profile will now automatically reapply when your monitor
echo reconnects, wakes from sleep, or after system restarts.
echo.
echo Useful commands:
echo   Check status:  check-monitor-status.ps1
echo   View task:     taskschd.msc
echo.
pause
