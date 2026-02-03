<#
.SYNOPSIS
  Complete LG UltraGear dimming fix with auto-reapply monitor.

.DESCRIPTION
  All-in-one installer that:
  1. Installs the color profile to fix auto-dimming
  2. Sets up automatic reapplication on monitor reconnection
  
  Combines profile installation and persistent monitoring in a single script.

.USAGE
  PS> .\install-complete.ps1
  PS> .\install-complete.ps1 -SkipMonitor
  PS> .\install-complete.ps1 -MonitorNameMatch "LG ULTRAGEAR"

.PARAMETERS
  -SkipMonitor        Install profile only, don't create auto-reapply monitor
  -UninstallMonitor   Remove the auto-reapply monitor only
  -MonitorNameMatch   Monitor name pattern to match (default: 'LG ULTRAGEAR')
  -NoPrompt           Don't wait for Enter before exit

.NOTES
  Requires Administrator privileges.
#>

[CmdletBinding()]
param(
    [Parameter(HelpMessage="Skip creating the auto-reapply monitor")]
    [switch]$SkipMonitor,
    
    [Parameter(HelpMessage="Remove the auto-reapply monitor")]
    [switch]$UninstallMonitor,
    
    [Parameter(HelpMessage="Monitor name match pattern")]
    [string]$MonitorNameMatch = 'LG ULTRAGEAR',
    
    [Parameter(HelpMessage="Don't wait for Enter before exit")]
    [switch]$NoPrompt
)

$ErrorActionPreference = 'Continue'
$script:InstallerPath = Join-Path $PSScriptRoot "install-lg-ultragear-no-dimming.ps1"
$script:MonitorWatcherPath = Join-Path $PSScriptRoot "install-monitor-watcher.ps1"
$TaskName = "LG-UltraGear-ColorProfile-AutoReapply"

function Write-Step {
    param([string]$Message)
    Write-Host "`n═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
    Write-Host "  $Message" -ForegroundColor Cyan
    Write-Host "═══════════════════════════════════════════════════════════════`n" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Message)
    Write-Host "[OK  ] $Message" -ForegroundColor Green
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERR ] $Message" -ForegroundColor Red
}

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Yellow
}

# Check for admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "This script requires Administrator privileges."
    Write-Info "Right-click and select 'Run as Administrator'"
    if (-not $NoPrompt) {
        Write-Host "`nPress Enter to exit..." -ForegroundColor Gray
        [void][Console]::ReadLine()
    }
    exit 1
}

# Handle uninstall
if ($UninstallMonitor) {
    Write-Step "Uninstalling Auto-Reapply Monitor"
    
    if (Test-Path $script:MonitorWatcherPath) {
        & $script:MonitorWatcherPath -Uninstall
        exit $LASTEXITCODE
    } else {
        try {
            Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction Stop
            Write-Success "Monitor removed successfully"
        } catch {
            Write-Error "Failed to remove monitor: $($_.Exception.Message)"
            exit 1
        }
    }
    exit 0
}

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  LG UltraGear Auto-Dimming Fix - Complete Installation       ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Step 1: Install Color Profile
Write-Step "STEP 1/2: Installing Color Profile"

if (-not (Test-Path $script:InstallerPath)) {
    Write-Error "Main installer not found: $script:InstallerPath"
    Write-Info "Please ensure install-lg-ultragear-no-dimming.ps1 is in the same directory"
    if (-not $NoPrompt) {
        Write-Host "`nPress Enter to exit..." -ForegroundColor Gray
        [void][Console]::ReadLine()
    }
    exit 1
}

try {
    $installParams = @{
        NoPrompt = $true
        SkipElevation = $true
        SkipWindowsTerminal = $true
        MonitorNameMatch = $MonitorNameMatch
    }
    
    & $script:InstallerPath @installParams
    
    if ($LASTEXITCODE -eq 0 -or $null -eq $LASTEXITCODE) {
        Write-Success "Color profile installed successfully"
    } else {
        Write-Error "Profile installation failed with exit code: $LASTEXITCODE"
        if (-not $NoPrompt) {
            Write-Host "`nPress Enter to exit..." -ForegroundColor Gray
            [void][Console]::ReadLine()
        }
        exit $LASTEXITCODE
    }
} catch {
    Write-Error "Profile installation failed: $($_.Exception.Message)"
    if (-not $NoPrompt) {
        Write-Host "`nPress Enter to exit..." -ForegroundColor Gray
        [void][Console]::ReadLine()
    }
    exit 1
}

# Step 2: Install Monitor (optional)
if (-not $SkipMonitor) {
    Write-Host ""
    Write-Step "STEP 2/2: Installing Auto-Reapply Monitor"
    
    if (-not (Test-Path $script:MonitorWatcherPath)) {
        Write-Error "Monitor watcher script not found: $script:MonitorWatcherPath"
        Write-Info "Skipping auto-reapply monitor installation"
        Write-Info "Color profile is installed but won't auto-reapply on reconnection"
    } else {
        try {
            & $script:MonitorWatcherPath -InstallerPath $script:InstallerPath -MonitorNameMatch $MonitorNameMatch
            
            if ($LASTEXITCODE -eq 0 -or $null -eq $LASTEXITCODE) {
                Write-Success "Auto-reapply monitor installed successfully"
            } else {
                Write-Error "Monitor installation failed with exit code: $LASTEXITCODE"
                Write-Info "Color profile is installed but won't auto-reapply on reconnection"
            }
        } catch {
            Write-Error "Monitor installation failed: $($_.Exception.Message)"
            Write-Info "Color profile is installed but won't auto-reapply on reconnection"
        }
    }
} else {
    Write-Host ""
    Write-Info "Skipping auto-reapply monitor (use -SkipMonitor:$false to enable)"
}

# Final summary
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║                  Installation Complete!                      ║" -ForegroundColor Green
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

if (-not $SkipMonitor) {
    Write-Host "✓ Color profile installed and active" -ForegroundColor Green
    Write-Host "✓ Auto-reapply monitor installed" -ForegroundColor Green
    Write-Host ""
    Write-Host "The fix will automatically reapply when:" -ForegroundColor Cyan
    Write-Host "  • Monitor disconnects and reconnects" -ForegroundColor Gray
    Write-Host "  • System wakes from sleep" -ForegroundColor Gray
    Write-Host "  • User logs in or unlocks workstation" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Useful commands:" -ForegroundColor Cyan
    Write-Host "  Check status:  .\check-monitor-status.ps1" -ForegroundColor Gray
    Write-Host "  Uninstall:     .\install-complete.ps1 -UninstallMonitor" -ForegroundColor Gray
} else {
    Write-Host "✓ Color profile installed and active" -ForegroundColor Green
    Write-Host "⚠ Auto-reapply monitor was NOT installed" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "To enable persistent auto-reapply, run:" -ForegroundColor Cyan
    Write-Host "  .\install-complete.ps1" -ForegroundColor Gray
}

Write-Host ""

if (-not $NoPrompt) {
    Write-Host "Press Enter to exit..." -ForegroundColor Gray
    [void][Console]::ReadLine()
}
