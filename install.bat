@echo off
:: LG UltraGear Auto-Dimming Fix - Launcher
:: Just double-click to install everything!

net session >nul 2>&1
if %errorlevel% neq 0 (
    powershell -NoProfile -Command "Start-Process -Verb RunAs -FilePath '%~f0'"
    exit /b
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-lg-ultragear-no-dimming.ps1"
