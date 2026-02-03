@echo off
:: LG UltraGear Auto-Dimming Fix - Launcher
:: Just double-click to install everything!

net session >nul 2>&1
if %errorlevel% neq 0 (
    powershell -NoProfile -Command "Start-Process -Verb RunAs -FilePath 'cmd.exe' -ArgumentList '/c cd /d \"%~dp0\" && \"%~f0\"'"
    exit /b
)

cd /d "%~dp0"
echo Starting installer...
:: Skip Windows Terminal re-hosting when launched from bat (it handles its own window)
powershell -NoProfile -ExecutionPolicy Bypass -Command "& '%~dp0install-lg-ultragear-no-dimming.ps1' -Interactive -SkipWindowsTerminal; if ($LASTEXITCODE -ne 0) { Read-Host 'Error occurred. Press Enter' }"
pause
