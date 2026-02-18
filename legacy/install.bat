@echo off
:: LG UltraGear Auto-Dimming Fix - Launcher

:: Check for admin rights
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Requesting administrator privileges...
    powershell -Command "Start-Process -Verb RunAs -FilePath powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','%~dp0install-lg-ultragear-no-dimming.ps1','-Interactive','-SkipWindowsTerminal'"
    exit /b
)

:: Already admin - set window size and run
mode con: cols=100 lines=50
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-lg-ultragear-no-dimming.ps1" -Interactive -SkipWindowsTerminal
pause
