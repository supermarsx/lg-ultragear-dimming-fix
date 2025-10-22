@echo off
setlocal enabledelayedexpansion

rem Full-auto installer for LG UltraGear no-dimming profile
rem - Auto-elevates (UAC)
rem - Runs PowerShell with a temporary ExecutionPolicy bypass
rem - Invokes the installer script with sane defaults

set "SCRIPT=%~dp0install-lg-ultragear-no-dimming.ps1"

rem Check for admin rights
net session >nul 2>&1
if not %errorlevel%==0 (
  echo Requesting administrator privileges...
  powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
  exit /b
)

if not exist "%SCRIPT%" (
  echo PowerShell installer not found: %SCRIPT%
  exit /b 1
)

echo Running installer...
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" -Verbose -NoPrompt
set "ERR=%ERRORLEVEL%"
if not "%ERR%"=="0" (
  echo Installer returned error code %ERR%.
)
echo.
set /p _="Press Enter to exit..."
exit /b %ERR%
