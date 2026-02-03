@echo off
:: LG UltraGear Complete Installation Launcher
:: Installs color profile + auto-reapply monitor in one go

setlocal EnableDelayedExpansion

:: Check for admin and elevate if needed
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Requesting administrator privileges...
    powershell -NoProfile -Command "Start-Process -Verb RunAs -FilePath '%~f0'"
    exit /b
)

:: Run the complete installer
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-complete.ps1"

exit /b %errorlevel%
