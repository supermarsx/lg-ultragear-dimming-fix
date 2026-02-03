@echo off
:: LG UltraGear No-Dimming Fix with Auto-Reapply Monitor
:: This batch file installs the color profile AND sets up automatic reapplication

setlocal EnableDelayedExpansion

echo.
echo ========================================================================
echo  LG UltraGear No-Dimming Fix + Auto-Reapply Monitor Installer
echo ========================================================================
echo.
echo This will:
echo  1. Install the color profile to fix auto-dimming
echo  2. Set up automatic reapplication when monitor reconnects
echo.

:: Check for admin and elevate if needed
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [INFO] Requesting administrator privileges...
    powershell -NoProfile -Command "Start-Process -Verb RunAs -FilePath '%~f0'"
    exit /b
)

echo [STEP 1/2] Installing color profile...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-lg-ultragear-no-dimming.ps1" -NoPrompt
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
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-monitor-watcher.ps1"
if %errorlevel% neq 0 (
    echo.
    echo [WARNING] Monitor installation failed, but profile is installed.
    echo You can try running install-monitor-watcher.ps1 separately.
)

echo.
echo ========================================================================
echo  Installation Complete!
echo ========================================================================
echo.
echo The color profile will now automatically reapply when your monitor
echo reconnects, wakes from sleep, or after system restarts.
echo.
pause
