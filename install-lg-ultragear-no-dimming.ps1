<#
.SYNOPSIS
  LG UltraGear No-Auto-Dim installer.

.DESCRIPTION
  Installs and associates a bundled ICC/ICM color profile with LG UltraGear
  displays to mitigate firmware auto-dimming by constraining the effective
  luminance range. Requires Administrator for system-wide installation.

.STEPS
  - Install/refresh the color profile in the Windows color store
  - Discover connected displays by friendly name (e.g. "LG ULTRAGEAR")
  - Associate the profile per display (system-wide; optionally per-user)
  - Optionally set the profile as default
  - Refresh Windows color settings

.USAGE
  PS> .\install-lg-ultragear-no-dimming.ps1 -Verbose
  PS> .\install-lg-ultragear-no-dimming.ps1 -MonitorNameMatch "LG ULTRAGEAR" -ProfilePath .\lg-ultragear-full-cal.icm -PerUser -Verbose

.LOGGING TAGS
  [STRT] start, [STEP] step, [INFO] info, [NOTE] note, [SKIP] skipped,
  [CRT ] created, [DEL ] deleted, [OK ] success, [ERR ] error, [DONE] finished.

.NOTES
  - Supports Windows Terminal re-host for better UX (title + tab color)
  - Enforces SHA256 integrity of embedded profile unless -SkipHashCheck
#>

[CmdletBinding(SupportsShouldProcess = $true)]
param(
    $ProfilePath = '.\\lg-ultragear-full-cal.icm',
    $MonitorNameMatch = 'LG ULTRAGEAR',
    $EmbedFromPath,
    [switch]$PerUser,
    [switch]$NoSetDefault,
    [switch]$SkipHdrAssociation,
    [switch]$EnableGenericDefault,
    [switch]$EnableHdrAssociation,
    [switch]$NoPrompt,
    [switch]$InstallOnly,
    [switch]$Probe,
    [switch]$DryRun,
    [switch]$SkipElevation,
    [switch]$SkipWindowsTerminal,
    [switch]$KeepTemp,
    [switch]$SkipHashCheck,
    [switch]$InstallMonitor,
    [switch]$UninstallMonitor,
    [switch]$SkipMonitor,
    [string]$MonitorTaskName = "LG-UltraGear-ColorProfile-AutoReapply",
    [switch]$Interactive,
    [switch]$NonInteractive,
    [switch]$Uninstall,
    [switch]$UninstallFull,
    [switch]$Reinstall,
    [switch]$Refresh,
    [Alias('h', '?')]
    [switch]$Help
)

begin {
    # Hint to static analysis: parameters are intentionally used across nested scopes
    $null = $ProfilePath, $MonitorNameMatch, $PerUser, $NoSetDefault, $SkipHdrAssociation, $NoPrompt, $InstallOnly, $Probe, $SkipElevation, $SkipWindowsTerminal, $KeepTemp, $SkipHashCheck, $InstallMonitor, $UninstallMonitor, $SkipMonitor, $MonitorTaskName, $Interactive, $NonInteractive, $Uninstall, $UninstallFull, $Reinstall, $Refresh
    # Mark parameters as referenced for static analyzers

    # Check if running with no arguments (interactive mode)
    $script:IsInteractive = $Interactive -or (($PSBoundParameters.Count -eq 0 -and -not $Help) -and -not $NonInteractive)

    # Record the launch context so relative paths stay consistent after re-invocation.
    $script:InvocationPath = if ($PSCommandPath) { $PSCommandPath } else { $MyInvocation.MyCommand.Path }
    $script:InvocationDirectory = if ($script:InvocationPath) { Split-Path -Path $script:InvocationPath -Parent } else { $null }
    # Capture the caller's working directory if available; this helps rebuild relative paths later.
    try {
        $script:OriginalWorkingDirectory = (Get-Location).ProviderPath
    } catch {
        try { $script:OriginalWorkingDirectory = (Get-Location).Path } catch { $script:OriginalWorkingDirectory = $null }
    }

    # =========================================================================
    # VERSION (YY.N format - read from VERSION file or embedded)
    # =========================================================================
    # __VERSION_EMBED__ is replaced by CI during build; fallback reads VERSION file
    $script:VERSION_EMBEDDED = '__VERSION_EMBED__'
    if ($script:VERSION_EMBEDDED -eq '__VERSION_EMBED__') {
        # Not embedded - try to read from VERSION file
        $versionFile = Join-Path $script:InvocationDirectory 'VERSION'
        if (Test-Path -LiteralPath $versionFile) {
            $script:VERSION_EMBEDDED = (Get-Content -LiteralPath $versionFile -Raw).Trim()
        } else {
            $script:VERSION_EMBEDDED = '26.1'  # Fallback default
        }
    }

    # =========================================================================
    # TUI CONFIGURATION
    # =========================================================================
    $script:TUI_WIDTH = 76
    $script:TUI_HEIGHT = 32
    $script:TUI_TITLE = "LG UltraGear Auto-Dimming Fix"
    $script:TUI_VERSION = $script:VERSION_EMBEDDED
    $script:TUI_PAGE = "main"  # main, install, uninstall, advanced

    # Advanced option toggles (persist across menu navigation)
    $script:Toggle_HdrAssociation = $false
    $script:Toggle_PerUser = $false
    $script:Toggle_DryRun = $false
    $script:Toggle_SkipElevation = $false
    $script:Toggle_GenericDefault = $false

    # =========================================================================
    # TUI FUNCTIONS
    # =========================================================================
    function Set-ConsoleSize {
        try {
            $raw = $Host.UI.RawUI
            $bufferSize = $raw.BufferSize
            $windowSize = $raw.WindowSize

            # Set buffer first (must be >= window)
            if ($bufferSize.Width -lt $script:TUI_WIDTH) {
                $bufferSize.Width = $script:TUI_WIDTH + 5
                $raw.BufferSize = $bufferSize
            }

            # Set window size
            $windowSize.Width = $script:TUI_WIDTH
            $windowSize.Height = $script:TUI_HEIGHT
            $raw.WindowSize = $windowSize
        } catch {
            # Expected on terminals that don't support resizing (ISE, non-console hosts)
            $null = $_
        }
    }

    function Write-TUIBox {
        param(
            [string]$Title = "",
            [string]$Char = "═",
            [string]$Left = "╔",
            [string]$Right = "╗",
            [ConsoleColor]$Color = "Cyan"
        )
        $innerWidth = $script:TUI_WIDTH - 2
        if ($Title) {
            $padding = $innerWidth - $Title.Length - 2
            $leftPad = [math]::Floor($padding / 2)
            $rightPad = [math]::Ceiling($padding / 2)
            $line = $Left + ($Char * $leftPad) + " $Title " + ($Char * $rightPad) + $Right
        } else {
            $line = $Left + ($Char * $innerWidth) + $Right
        }
        Write-Host $line -ForegroundColor $Color
    }

    function Write-TUILine {
        param(
            [string]$Text = "",
            [ConsoleColor]$Color = "White",
            [ConsoleColor]$BorderColor = "Cyan",
            [switch]$Center
        )
        $innerWidth = $script:TUI_WIDTH - 4
        if ($Center) {
            $padding = $innerWidth - $Text.Length
            $leftPad = [math]::Floor($padding / 2)
            $rightPad = [math]::Ceiling($padding / 2)
            $content = (" " * $leftPad) + $Text + (" " * $rightPad)
        } else {
            $content = $Text.PadRight($innerWidth)
        }
        Write-Host "║ " -ForegroundColor $BorderColor -NoNewline
        Write-Host $content -ForegroundColor $Color -NoNewline
        Write-Host " ║" -ForegroundColor $BorderColor
    }

    function Write-TUIEmpty {
        param([ConsoleColor]$BorderColor = "Cyan")
        Write-TUILine -Text "" -BorderColor $BorderColor
    }

    function Write-TUIMenuItem {
        param(
            [string]$Key,
            [string]$Text,
            [ConsoleColor]$KeyColor = "Yellow",
            [ConsoleColor]$TextColor = "White",
            [ConsoleColor]$BorderColor = "Cyan"
        )
        $innerWidth = $script:TUI_WIDTH - 4  # 72 chars between borders
        $keyPart = "[$Key]"
        # We output: "  " (2) + keyPart + " " (1) + Text, must total innerWidth
        $prefixLen = 2 + $keyPart.Length + 1  # indent + key + space before text
        $textPadded = $Text.PadRight($innerWidth - $prefixLen)

        Write-Host "║ " -ForegroundColor $BorderColor -NoNewline
        Write-Host "  " -NoNewline
        Write-Host $keyPart -ForegroundColor $KeyColor -NoNewline
        Write-Host " " -NoNewline
        Write-Host $textPadded -ForegroundColor $TextColor -NoNewline
        Write-Host " ║" -ForegroundColor $BorderColor
    }

    function Write-TUIStatus {
        param(
            [string]$Label,
            [string]$Value,
            [ConsoleColor]$LabelColor = "Gray",
            [ConsoleColor]$ValueColor = "Green",
            [ConsoleColor]$BorderColor = "Cyan"
        )
        $innerWidth = $script:TUI_WIDTH - 4  # 72 chars between borders
        # We output: "  " (2) + Label + " " (1) + Value, must total innerWidth
        $prefixLen = 2 + $Label.Length + 1
        $valuePadded = $Value.PadRight($innerWidth - $prefixLen)

        Write-Host "║ " -ForegroundColor $BorderColor -NoNewline
        Write-Host "  $Label " -ForegroundColor $LabelColor -NoNewline
        Write-Host $valuePadded -ForegroundColor $ValueColor -NoNewline
        Write-Host " ║" -ForegroundColor $BorderColor
    }

    function Get-MonitorStatus {
        $taskExists = $null -ne (Get-ScheduledTask -TaskName $MonitorTaskName -ErrorAction SilentlyContinue)
        $profilePath = Join-Path $env:WINDIR "System32\spool\drivers\color\lg-ultragear-full-cal.icm"
        $profileExists = Test-Path -LiteralPath $profilePath

        $lgCount = 0
        try {
            Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop | ForEach-Object {
                $name = ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
                if ($name -match 'LG.*ULTRAGEAR') { $lgCount++ }
            }
        } catch {
            # WMI query may fail on systems without monitor support
            $null = $_
        }

        return @{
            TaskExists    = $taskExists
            ProfileExists = $profileExists
            LGCount       = $lgCount
        }
    }

    function Write-TUISeparator {
        param(
            [string]$Title = "",
            [ConsoleColor]$Color = "DarkCyan"
        )
        if ($Title) {
            Write-TUIBox -Title $Title -Char "─" -Left "╟" -Right "╢" -Color $Color
        } else {
            Write-TUIBox -Char "─" -Left "╟" -Right "╢" -Color $Color
        }
    }

    function Show-TUIHeader {
        param([hashtable]$Status)

        Write-TUIBox -Title $script:TUI_TITLE -Left "╔" -Right "╗"
        Write-TUILine -Text "Version $script:TUI_VERSION  │  github.com/supermarsx/lg-ultragear-dimming-fix" -Center -Color DarkGray
        Write-TUISeparator

        # Status section
        Write-TUIEmpty
        Write-TUILine -Text "┌─ CURRENT STATUS ─────────────────────────────────────────────────────┐" -Color DarkCyan

        $profileStatus = if ($Status.ProfileExists) { "● Installed" } else { "○ Not Installed" }
        $profileColor = if ($Status.ProfileExists) { "Green" } else { "Red" }
        Write-TUIStatus -Label "  Color Profile:" -Value $profileStatus -ValueColor $profileColor

        $monitorStatus = if ($Status.TaskExists) { "● Active" } else { "○ Inactive" }
        $monitorColor = if ($Status.TaskExists) { "Green" } else { "Yellow" }
        Write-TUIStatus -Label "  Auto-Reapply: " -Value $monitorStatus -ValueColor $monitorColor

        $lgStatus = if ($Status.LGCount -gt 0) { "● $($Status.LGCount) monitor(s) detected" } else { "○ None detected" }
        $lgColor = if ($Status.LGCount -gt 0) { "Green" } else { "Gray" }
        Write-TUIStatus -Label "  LG UltraGear: " -Value $lgStatus -ValueColor $lgColor

        Write-TUILine -Text "└──────────────────────────────────────────────────────────────────────┘" -Color DarkCyan
        Write-TUIEmpty
    }

    function Show-TUIMainMenu {
        Clear-Host
        Set-ConsoleSize
        $status = Get-MonitorStatus

        Show-TUIHeader -Status $status
        Write-TUISeparator -Title " MAIN MENU "

        Write-TUIEmpty
        Write-TUILine -Text "  INSTALL OPTIONS" -Color Cyan
        Write-TUIMenuItem -Key "1" -Text "Default Install (SDR Profile + Auto-Reapply)"
        Write-TUIMenuItem -Key "2" -Text "SDR Only (Profile without Auto-Reapply)"
        Write-TUIMenuItem -Key "3" -Text "Auto-Reapply Only (Monitor Task Only)"
        Write-TUIEmpty
        Write-TUILine -Text "  MAINTENANCE" -Color Cyan
        Write-TUIMenuItem -Key "4" -Text "Refresh Install (Re-apply current settings)"
        Write-TUIMenuItem -Key "5" -Text "Reinstall (Clean reinstall everything)"
        Write-TUIMenuItem -Key "6" -Text "Probe (Detect connected monitors)"
        Write-TUIEmpty
        Write-TUILine -Text "  UNINSTALL" -Color Cyan
        Write-TUIMenuItem -Key "7" -Text "Remove Auto-Reapply (Keep profile)"
        Write-TUIMenuItem -Key "8" -Text "Full Uninstall (Remove everything)"
        Write-TUIEmpty
        Write-TUILine -Text "  ADVANCED" -Color Cyan

        # Show active toggle summary
        $activeToggles = @()
        if ($script:Toggle_HdrAssociation) { $activeToggles += "HDR" }
        if ($script:Toggle_PerUser) { $activeToggles += "PerUser" }
        if ($script:Toggle_GenericDefault) { $activeToggles += "GenericDef" }
        if ($script:Toggle_DryRun) { $activeToggles += "DryRun" }
        if ($script:Toggle_SkipElevation) { $activeToggles += "NoAdmin" }

        if ($activeToggles.Count -gt 0) {
            $toggleText = "Advanced Options (" + ($activeToggles -join ", ") + ")"
            Write-TUIMenuItem -Key "A" -Text $toggleText -TextColor "Green"
        } else {
            Write-TUIMenuItem -Key "A" -Text "Advanced Options (None active)"
        }
        Write-TUIEmpty
        Write-TUIMenuItem -Key "Q" -Text "Quit" -KeyColor "Red" -TextColor "DarkGray"
        Write-TUIEmpty

        Write-TUIBox -Char "═" -Left "╚" -Right "╝"
    }

    function Write-TUIToggle {
        param(
            [string]$Key,
            [string]$Text,
            [bool]$Enabled,
            [ConsoleColor]$BorderColor = "Cyan"
        )
        $innerWidth = $script:TUI_WIDTH - 4  # 72 chars between borders
        $keyPart = "[$Key]"
        $toggle = if ($Enabled) { "[ON ]" } else { "[OFF]" }
        $toggleColor = if ($Enabled) { "Green" } else { "DarkGray" }

        # We output: "  " (2) + keyPart + " " (1) + toggle + " " (1) + Text, must total innerWidth
        $prefixLen = 2 + $keyPart.Length + 1 + $toggle.Length + 1
        $textPadded = $Text.PadRight($innerWidth - $prefixLen)

        Write-Host "║ " -ForegroundColor $BorderColor -NoNewline
        Write-Host "  " -NoNewline
        Write-Host $keyPart -ForegroundColor Yellow -NoNewline
        Write-Host " " -NoNewline
        Write-Host $toggle -ForegroundColor $toggleColor -NoNewline
        Write-Host " " -NoNewline
        Write-Host $textPadded -ForegroundColor White -NoNewline
        Write-Host " ║" -ForegroundColor $BorderColor
    }

    function Show-TUIAdvancedMenu {
        Clear-Host
        Set-ConsoleSize
        $status = Get-MonitorStatus

        Show-TUIHeader -Status $status
        Write-TUISeparator -Title " ADVANCED OPTIONS (Toggles) "

        Write-TUIEmpty
        Write-TUILine -Text "  INSTALL MODIFIERS" -Color Cyan
        Write-TUIToggle -Key "1" -Text "HDR Association (Include HDR color binding)" -Enabled $script:Toggle_HdrAssociation
        Write-TUIToggle -Key "2" -Text "Per-User Install (User scope, not system)" -Enabled $script:Toggle_PerUser
        Write-TUIToggle -Key "3" -Text "Generic Default (Legacy default profile API)" -Enabled $script:Toggle_GenericDefault
        Write-TUIEmpty
        Write-TUILine -Text "  TESTING" -Color Cyan
        Write-TUIToggle -Key "4" -Text "Dry Run (Simulate without changes)" -Enabled $script:Toggle_DryRun
        Write-TUIToggle -Key "5" -Text "Skip Elevation (Run without admin)" -Enabled $script:Toggle_SkipElevation
        Write-TUIEmpty
        Write-TUILine -Text "  These toggles affect main menu install options" -Color DarkGray
        Write-TUIEmpty
        Write-TUILine -Text "  NAVIGATION" -Color Cyan
        Write-TUIMenuItem -Key "B" -Text "Back to Main Menu" -KeyColor "Yellow"
        Write-TUIMenuItem -Key "Q" -Text "Quit" -KeyColor "Red" -TextColor "DarkGray"
        Write-TUIEmpty

        Write-TUIBox -Char "═" -Left "╚" -Right "╝"
    }

    function Show-TUIProcessing {
        param([string]$Message)
        Clear-Host
        Write-TUIBox -Title " PROCESSING " -Left "╔" -Right "╗"
        Write-TUIEmpty
        Write-TUILine -Text $Message -Color Yellow
        Write-TUIEmpty
        Write-TUIBox -Char "═" -Left "╚" -Right "╝"
        Write-Host ""
    }

    function Wait-TUIKeyPress {
        Write-Host ""
        Write-Host "  Press any key to continue..." -ForegroundColor DarkGray
        try {
            $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
        } catch {
            # Fallback for non-interactive terminals
            Read-Host "  Press Enter to continue"
        }
    }

    function Invoke-ToggleConfiguration {
        # Apply advanced option toggles to script variables before install
        if ($script:Toggle_HdrAssociation) {
            $script:EnableHdrAssociation = $true
            $script:SkipHdrAssociation = $false
        }
        if ($script:Toggle_PerUser) {
            $script:PerUser = $true
        }
        if ($script:Toggle_GenericDefault) {
            $script:EnableGenericDefault = $true
        }
        if ($script:Toggle_DryRun) {
            $script:DryRun = $true
        }
        if ($script:Toggle_SkipElevation) {
            $script:SkipElevation = $true
        }
    }

    function Invoke-TUIAction {
        param(
            [string]$Choice,
            [string]$Menu = "main"
        )

        if ($Menu -eq "main") {
            switch ($Choice.ToUpper()) {
                "1" {
                    Show-TUIProcessing -Message "Installing SDR profile + auto-reapply monitor..."
                    $script:SkipMonitor = $false
                    $script:SkipHdrAssociation = -not $script:Toggle_HdrAssociation
                    Invoke-ToggleConfiguration
                    $script:IsInteractive = $false
                    return "install"
                }
                "2" {
                    Show-TUIProcessing -Message "Installing SDR profile only..."
                    $script:SkipMonitor = $true
                    $script:SkipHdrAssociation = -not $script:Toggle_HdrAssociation
                    Invoke-ToggleConfiguration
                    $script:IsInteractive = $false
                    return "install"
                }
                "3" {
                    Show-TUIProcessing -Message "Installing auto-reapply monitor only..."
                    $script:InstallMonitor = $true
                    Invoke-ToggleConfiguration
                    $script:IsInteractive = $false
                    return "installmonitor"
                }
                "4" {
                    Show-TUIProcessing -Message "Refreshing installation..."
                    $script:SkipMonitor = $false
                    $script:SkipHdrAssociation = -not $script:Toggle_HdrAssociation
                    Invoke-ToggleConfiguration
                    $script:IsInteractive = $false
                    return "install"
                }
                "5" {
                    Show-TUIProcessing -Message "Reinstalling everything..."
                    # Uninstall first, then install
                    Uninstall-AutoReapplyMonitor -TaskName $MonitorTaskName
                    $script:SkipMonitor = $false
                    $script:SkipHdrAssociation = -not $script:Toggle_HdrAssociation
                    Invoke-ToggleConfiguration
                    $script:IsInteractive = $false
                    return "install"
                }
                "6" {
                    Show-TUIProcessing -Message "Detecting connected monitors..."
                    $script:Probe = $true
                    $script:IsInteractive = $false
                    return "probe"
                }
                "7" {
                    Show-TUIProcessing -Message "Removing auto-reapply monitor..."
                    Uninstall-AutoReapplyMonitor -TaskName $MonitorTaskName
                    Wait-TUIKeyPress
                    return "menu"
                }
                "8" {
                    Show-TUIProcessing -Message "Performing full uninstall..."
                    Uninstall-AutoReapplyMonitor -TaskName $MonitorTaskName
                    # Remove profile from system
                    $profilePath = Join-Path $env:WINDIR "System32\spool\drivers\color\lg-ultragear-full-cal.icm"
                    if (Test-Path -LiteralPath $profilePath) {
                        try {
                            Remove-Item -LiteralPath $profilePath -Force -ErrorAction Stop
                            Write-Host "  [DEL ] Removed color profile" -ForegroundColor Green
                        } catch {
                            Write-Host "  [WARN] Could not remove profile: $($_.Exception.Message)" -ForegroundColor Yellow
                        }
                    } else {
                        Write-Host "  [NOTE] Profile not found (already removed)" -ForegroundColor Gray
                    }
                    Wait-TUIKeyPress
                    return "menu"
                }
                "A" { return "advanced" }
                "Q" { return "quit" }
                default { return "menu" }
            }
        } elseif ($Menu -eq "advanced") {
            switch ($Choice.ToUpper()) {
                "1" {
                    $script:Toggle_HdrAssociation = -not $script:Toggle_HdrAssociation
                    return "advanced"
                }
                "2" {
                    $script:Toggle_PerUser = -not $script:Toggle_PerUser
                    return "advanced"
                }
                "3" {
                    $script:Toggle_GenericDefault = -not $script:Toggle_GenericDefault
                    return "advanced"
                }
                "4" {
                    $script:Toggle_DryRun = -not $script:Toggle_DryRun
                    return "advanced"
                }
                "5" {
                    $script:Toggle_SkipElevation = -not $script:Toggle_SkipElevation
                    return "advanced"
                }
                "B" { return "main" }
                "Q" { return "quit" }
                default { return "advanced" }
            }
        }
        return "menu"
    }

    function Read-TUIKey {
        # Read a single key press without requiring Enter
        Write-Host ""
        Write-Host "  Select option: " -ForegroundColor White -NoNewline
        try {
            $key = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
            $char = $key.Character
            if ($char -match '[a-zA-Z0-9]') {
                Write-Host $char -ForegroundColor Cyan
                return $char.ToString().ToUpper()
            } else {
                Write-Host ""
                return ""
            }
        } catch {
            # Fallback for non-interactive terminals
            return (Read-Host).ToUpper()
        }
    }

    function Start-TUI {
        $currentMenu = "main"
        $continue = $true

        while ($continue) {
            switch ($currentMenu) {
                "main" { Show-TUIMainMenu }
                "advanced" { Show-TUIAdvancedMenu }
            }

            $choice = Read-TUIKey
            $action = Invoke-TUIAction -Choice $choice -Menu $currentMenu

            switch ($action) {
                "install" {
                    Invoke-Main
                    Wait-TUIKeyPress
                    # Reset flags for next iteration
                    $script:SkipMonitor = $false
                    $script:SkipHdrAssociation = $true
                    $script:InstallOnly = $false
                    $script:PerUser = $false
                    $script:DryRun = $false
                    $script:SkipElevation = $false
                    $script:EnableHdrAssociation = $false
                    $currentMenu = "main"
                }
                "installmonitor" {
                    try {
                        Install-AutoReapplyMonitor -TaskName $MonitorTaskName -InstallerPath $script:InvocationPath -MonitorMatch $MonitorNameMatch
                    } catch {
                        Write-Host "  [ERR ] Failed: $($_.Exception.Message)" -ForegroundColor Red
                    }
                    Wait-TUIKeyPress
                    $script:InstallMonitor = $false
                    $currentMenu = "main"
                }
                "probe" {
                    Invoke-Main
                    Wait-TUIKeyPress
                    $script:Probe = $false
                    $currentMenu = "main"
                }
                "advanced" { $currentMenu = "advanced" }
                "main" { $currentMenu = "main" }
                "menu" { }
                "quit" { $continue = $false }
            }
        }

        Clear-Host
        Write-Host ""
        Write-Host "  ╔════════════════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
        Write-Host "  ║                                                                        ║" -ForegroundColor Cyan
        Write-Host "  ║   " -ForegroundColor Cyan -NoNewline
        Write-Host "Thank you for using LG UltraGear Auto-Dimming Fix!".PadRight(69) -ForegroundColor White -NoNewline
        Write-Host "║" -ForegroundColor Cyan
        Write-Host "  ║                                                                        ║" -ForegroundColor Cyan
        Write-Host "  ║   " -ForegroundColor Cyan -NoNewline
        Write-Host "github.com/supermarsx/lg-ultragear-dimming-fix".PadRight(69) -ForegroundColor DarkGray -NoNewline
        Write-Host "║" -ForegroundColor Cyan
        Write-Host "  ║                                                                        ║" -ForegroundColor Cyan
        Write-Host "  ╚════════════════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
        Write-Host ""
    }

    # Set console appearance: black background, white text, and window title
    try {
        $raw = $Host.UI.RawUI
        # Save originals if needed in the future
        $script:OriginalFg = $raw.ForegroundColor
        $script:OriginalBg = $raw.BackgroundColor
        $script:OriginalTitle = $raw.WindowTitle
        $raw.BackgroundColor = 'Black'
        $raw.ForegroundColor = 'White'
        try { $raw.WindowTitle = $script:TUI_TITLE } catch { [Console]::Title = $script:TUI_TITLE }
        if (-not $script:IsInteractive -and -not $Help) {
            try { Clear-Host } catch {
                # Clear-Host may fail on non-console hosts
                $null = $_
            }
        }
    } catch {
        Write-Host "[NOTE] console color/title not set: $($_.Exception.Message)" -ForegroundColor White
    }

    function Show-Usage {
        <#
    .SYNOPSIS
      Print command-line usage and options.
    .NOTES
      Kept minimal to avoid side-effects before elevation.
    #>
        Write-Host ""
        Write-Host "  LG UltraGear Auto-Dimming Fix" -ForegroundColor Cyan
        Write-Host "  =============================" -ForegroundColor DarkCyan
        Write-Host ""
        Write-Host "Usage: " -NoNewline -ForegroundColor White
        Write-Host ".\\install-lg-ultragear-no-dimming.ps1 [options]" -ForegroundColor Gray
        Write-Host ""
        Write-Host "INSTALL OPTIONS:" -ForegroundColor Cyan
        Write-Host "  -ProfilePath <path>           Path to ICC/ICM file (default: embedded)"
        Write-Host "  -MonitorNameMatch <string>    Monitor name pattern (default: 'LG ULTRAGEAR')"
        Write-Host "  -PerUser                      Also associate profile in current-user scope"
        Write-Host "  -NoSetDefault                 Associate only; do not set as default"
        Write-Host "  -SkipHdrAssociation           Skip HDR/advanced-color association"
        Write-Host "  -EnableHdrAssociation         Enable HDR color profile association"
        Write-Host "  -InstallOnly                  Install profile file only; no associations"
        Write-Host ""
        Write-Host "MAINTENANCE:" -ForegroundColor Cyan
        Write-Host "  -Probe                        Detect and list connected monitors"
        Write-Host "  -Refresh                      Re-apply current settings"
        Write-Host "  -Reinstall                    Clean reinstall everything"
        Write-Host "  -DryRun                       Simulate operations (no changes)"
        Write-Host ""
        Write-Host "UNINSTALL:" -ForegroundColor Cyan
        Write-Host "  -Uninstall                    Remove auto-reapply monitor only"
        Write-Host "  -UninstallFull                Remove everything (profile + monitor)"
        Write-Host "  -UninstallMonitor             Alias for -Uninstall"
        Write-Host ""
        Write-Host "AUTO-REAPPLY MONITOR:" -ForegroundColor Cyan
        Write-Host "  -SkipMonitor                  Do not install auto-reapply monitor"
        Write-Host "  -InstallMonitor               Only install auto-reapply monitor"
        Write-Host "  -MonitorTaskName <name>       Custom scheduled task name"
        Write-Host ""
        Write-Host "BEHAVIOR:" -ForegroundColor Cyan
        Write-Host "  -Interactive                  Force interactive TUI mode"
        Write-Host "  -NonInteractive               Force non-interactive CLI mode"
        Write-Host "  -NoPrompt                     Do not wait for Enter before exit"
        Write-Host "  -SkipElevation                Do not auto-elevate (CI/testing)"
        Write-Host "  -SkipWindowsTerminal          Do not re-host under Windows Terminal"
        Write-Host "  -KeepTemp                     Keep temp profile files for inspection"
        Write-Host "  -SkipHashCheck                Skip SHA256 integrity verification"
        Write-Host "  -h, -?                        Show this help and exit"
        Write-Host ""
        Write-Host "EXAMPLES:" -ForegroundColor Cyan
        Write-Host "  # Interactive TUI (default when no args)" -ForegroundColor DarkGray
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1"
        Write-Host ""
        Write-Host "  # CLI: Full install with auto-reapply" -ForegroundColor DarkGray
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1 -NonInteractive -NoPrompt"
        Write-Host ""
        Write-Host "  # CLI: Profile only (no auto-reapply)" -ForegroundColor DarkGray
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1 -SkipMonitor -NoPrompt"
        Write-Host ""
        Write-Host "  # CLI: Detect monitors" -ForegroundColor DarkGray
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1 -Probe -NoPrompt"
        Write-Host ""
        Write-Host "  # CLI: Full uninstall" -ForegroundColor DarkGray
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1 -UninstallFull -NoPrompt"
        Write-Host ""
    }

    # Exit prompt helper to avoid premature window close on errors or completion
    $script:PromptShown = $false
    function Show-ExitPrompt {
        <#
    .SYNOPSIS
      Optional pause so launched consoles don't close immediately.
    .PARAMETER NoPrompt
      When set, skip the pause.
    #>
        if ($NoPrompt) { return }
        if ($script:PromptShown) { return }
        try {
            Write-Host ""
            Write-Host "Press Enter to exit..." -ForegroundColor White
            [void][System.Console]::ReadLine()
        } catch {
            Write-NoteMessage "Exit prompt skipped (no interactive console)."
        }
        $script:PromptShown = $true
    }

    # =========================================================================
    # AUTO-REAPPLY MONITOR FUNCTIONS
    # =========================================================================
    function Install-AutoReapplyMonitor {
        <#
        .SYNOPSIS
          Creates a scheduled task to auto-reapply color profile on display events.
        .NOTES
          Uses a fast pre-check that exits immediately if no LG UltraGear is detected.
          Only runs the full installer when an LG UltraGear monitor is actually connected.
        #>
        param(
            [string]$TaskName,
            [string]$InstallerPath,
            [string]$MonitorMatch
        )

        Write-ActionMessage "Installing auto-reapply monitor..."

        # Create the action script - optimized for speed with early exit
        $actionScript = @"
# LG UltraGear Color Profile Auto-Reapply - Fast Monitor
# Exits immediately if no matching monitor is connected

# Quick check for LG UltraGear - exits in <50ms if not found
try {
    `$found = `$false
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop | ForEach-Object {
        `$name = (`$_.UserFriendlyName | Where-Object { `$_ -ne 0 } | ForEach-Object { [char]`$_ }) -join ''
        if (`$name -match '$MonitorMatch') { `$found = `$true }
    }
    if (-not `$found) { exit 0 }
} catch { exit 0 }

# LG UltraGear detected - wait for display to stabilize then reapply
Start-Sleep -Milliseconds 1500
& '$InstallerPath' -NoSetDefault -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor -MonitorNameMatch '$MonitorMatch' 2>`$null | Out-Null

# Show toast notification (3 seconds)
try {
    [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
    [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

    `$template = '<toast duration="short"><visual><binding template="ToastGeneric"><text>LG UltraGear</text><text>Color profile reapplied</text></binding></visual><audio silent="true"/></toast>'

    `$xml = [Windows.Data.Xml.Dom.XmlDocument]::new()
    `$xml.LoadXml(`$template)
    `$toast = [Windows.UI.Notifications.ToastNotification]::new(`$xml)
    `$toast.ExpirationTime = [DateTimeOffset]::Now.AddSeconds(3)
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('LG UltraGear Monitor').Show(`$toast)
} catch {
    # Notification failed silently - not critical
}
"@

        $actionScriptPath = "$env:ProgramData\LG-UltraGear-Monitor\reapply-profile.ps1"
        $actionScriptDir = Split-Path -Path $actionScriptPath -Parent

        if (-not (Test-Path -LiteralPath $actionScriptDir)) {
            New-Item -ItemType Directory -Path $actionScriptDir -Force | Out-Null
        }

        Set-Content -Path $actionScriptPath -Value $actionScript -Force
        Write-SuccessMessage "Created action script: $actionScriptPath"

        # Create scheduled task with optimized triggers
        $action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -NoLogo -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$actionScriptPath`""

        # Trigger 1: Display/Monitor device events only (Device Interface Class GUID for monitors)
        $trigger1 = New-ScheduledTaskTrigger -AtLogOn
        $trigger1.CimInstanceProperties.Item('Enabled').Value = $true
        $trigger1.CimInstanceProperties.Item('Subscription').Value = @"
<QueryList>
  <Query Id="0" Path="System">
    <Select Path="System">
      *[System[Provider[@Name='Microsoft-Windows-Kernel-PnP'] and (EventID=20001 or EventID=20003)]]
      and
      *[EventData[Data[@Name='DeviceInstanceId'] and (contains(Data, 'DISPLAY') or contains(Data, 'MONITOR'))]]
    </Select>
  </Query>
</QueryList>
"@

        # Trigger 2: User logon (one-time check at login)
        $trigger2 = New-ScheduledTaskTrigger -AtLogOn

        # Trigger 3: Console connect (covers RDP, fast user switching, wake)
        $trigger3 = New-ScheduledTaskTrigger -AtLogOn
        $trigger3.CimInstanceProperties.Item('Enabled').Value = $true
        $trigger3.CimInstanceProperties.Item('StateChange').Value = 7  # ConsoleConnect

        # Trigger 4: Session unlock
        $trigger4 = New-ScheduledTaskTrigger -AtLogOn
        $trigger4.CimInstanceProperties.Item('Enabled').Value = $true
        $trigger4.CimInstanceProperties.Item('StateChange').Value = 8  # SessionUnlock

        $principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
        $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Seconds 30) -MultipleInstances IgnoreNew

        try { Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue } catch {
            # Task may not exist; ignore
            $null = $_
        }

        Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger1, $trigger2, $trigger3, $trigger4 -Principal $principal -Settings $settings -Description "Fast auto-reapply for LG UltraGear color profile. Only runs when LG UltraGear monitor is detected." | Out-Null

        Write-SuccessMessage "Auto-reapply monitor installed: $TaskName"
        Write-InfoMessage "Triggers: display connect, logon, console connect, unlock"
        Write-InfoMessage "Optimization: exits in <50ms if no LG UltraGear detected"
    }

    function Uninstall-AutoReapplyMonitor {
        param([string]$TaskName)

        Write-ActionMessage "Removing auto-reapply monitor..."
        try {
            Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction Stop
            Write-SuccessMessage "Task '$TaskName' removed"

            $actionScriptPath = "$env:ProgramData\LG-UltraGear-Monitor\reapply-profile.ps1"
            if (Test-Path $actionScriptPath) {
                Remove-Item -Path (Split-Path $actionScriptPath -Parent) -Recurse -Force -ErrorAction SilentlyContinue
                Write-SuccessMessage "Removed action script directory"
            }
        } catch {
            if ($_.Exception.Message -match "No MSFT_ScheduledTask objects found") {
                Write-NoteMessage "Task '$TaskName' not found (already removed)"
            } else {
                Write-WarnMessage "Failed to remove task: $($_.Exception.Message)"
            }
        }
    }

    # Determine if Get-FileHash is available (missing on some older/newer editions)
    try {
        $script:SupportsGetFileHash = [bool](Get-Command -Name Get-FileHash -ErrorAction Stop)
    } catch {
        $script:SupportsGetFileHash = $false
    }

    function Get-Sha256HashCompat {
        param(
            [Parameter(Mandatory = $true)]
            [string]$LiteralPath
        )

        if ($script:SupportsGetFileHash) {
            try {
                return (Microsoft.PowerShell.Utility\Get-FileHash -Algorithm SHA256 -LiteralPath $LiteralPath).Hash
            } catch {
                throw
            }
        }

        $fileStream = $null
        $sha256 = $null
        try {
            $fileStream = [System.IO.File]::Open($LiteralPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
            $sha256 = [System.Security.Cryptography.SHA256]::Create()
            $hashBytes = $sha256.ComputeHash($fileStream)
            return ([System.BitConverter]::ToString($hashBytes) -replace '-', '').ToUpperInvariant()
        } catch {
            throw
        } finally {
            if ($null -ne $fileStream) { $fileStream.Dispose() }
            if ($null -ne $sha256) { $sha256.Dispose() }
        }
    }

    # Logging helpers with colorized tags only; message text is default color
    function Write-InfoMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolInfo -ForegroundColor Yellow -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }
    function Write-ActionMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolAction -ForegroundColor Magenta -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }
    function Write-SuccessMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolSuccess -ForegroundColor Green -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }
    function Write-WarnMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolWarning -ForegroundColor Yellow -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }
    function Write-NoteMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolNote -ForegroundColor Gray -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }
    function Write-SkipMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolSkip -ForegroundColor DarkYellow -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }

    # Delete marker
    function Write-DeleteMessage($Message, [switch]$NoNewline) {
        # light pink => Magenta
        Write-Host $script:SymbolDelete -ForegroundColor Magenta -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }

    # Done marker
    function Write-DoneMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolDone -ForegroundColor Cyan -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }

    # Create marker (CRT) in orange
    function Write-CreateMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolCreate -ForegroundColor DarkYellow -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }

    # Init message: use STRT tag colored light blue (Cyan)
    function Write-InitMessage($Message, [switch]$NoNewline) {
        Write-Host $script:SymbolStart -ForegroundColor Cyan -NoNewline
        if ($NoNewline) { Write-Host ("  {0}" -f $Message) -NoNewline } else { Write-Host ("  {0}" -f $Message) }
    }

    # Help is printed after helper functions are defined (below),
    # and before auto-elevation to avoid UAC prompts for -Help.

    function Write-ErrorFull {
        <#
    .SYNOPSIS
      Streamlined one-line error reporter using [ERR ] tag.
    .PARAMETER ErrorRecord
      Error to render.
    .PARAMETER Context
      Additional context label to prefix.
    #>
        param(
            [Parameter(Mandatory)] [System.Management.Automation.ErrorRecord] $ErrorRecord,
            $Context
        )
        try {
            $msg = if ($ErrorRecord.Exception) { $ErrorRecord.Exception.Message } else { $ErrorRecord.ToString() }
            if ($Context) { $msg = "{0}: {1}" -f $Context, $msg }
            Write-Host $script:SymbolError -ForegroundColor Red -NoNewline
            Write-Host ("  {0}" -f $msg)
        } catch { Write-Host $script:SymbolError -ForegroundColor Red -NoNewline; Write-Host ("  {0}" -f $_.Exception.Message) }
    }

    function Test-IsAdmin {
        # Determine whether the current PowerShell session is elevated.
        $id = [Security.Principal.WindowsIdentity]::GetCurrent()
        $p = [Security.Principal.WindowsPrincipal]::new($id)
        return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    }

    function Ensure-Elevation {
        <#
    .SYNOPSIS
      Re-launches the script elevated (UAC) unless -SkipElevation.
    .PARAMETER Skip
      Skip auto-elevation.
    #>
        param([switch]$Skip)
        if ($Skip) { Write-NoteMessage "SkipElevation requested; continuing without auto-elevation."; return }
        if (Test-IsAdmin) { return }

        Write-ActionMessage "elevation required"
        Write-InfoMessage "relaunching with Administrator privileges..."

        $scriptPath = if ($script:InvocationPath) { $script:InvocationPath } else { $MyInvocation.MyCommand.Path }
        if (-not $scriptPath) { $scriptPath = $PSCommandPath }
        if (-not $scriptPath) { throw 'Unable to resolve script path for elevation relaunch.' }
        $argsList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $scriptPath)

        foreach ($kv in $PSBoundParameters.GetEnumerator()) {
            $name = '-' + $kv.Key
            $val = $kv.Value
            if ($val -is [System.Management.Automation.SwitchParameter]) {
                if ([bool]$val) { $argsList += $name }
            } elseif ($val -is [bool]) {
                if ($val) { $argsList += $name }
            } else {
                $argsList += $name
                if ($null -ne $val -and $val -ne '') { $argsList += $val }
            }
        }

        $workingDir = if ($script:OriginalWorkingDirectory -and (Test-Path -LiteralPath $script:OriginalWorkingDirectory)) {
            $script:OriginalWorkingDirectory
        } elseif ($script:InvocationDirectory -and (Test-Path -LiteralPath $script:InvocationDirectory)) {
            $script:InvocationDirectory
        } else {
            $env:SystemRoot
        }

        # Sanitize: remove null/empty and ensure string[]
        $argsList = @($argsList | Where-Object { $_ -ne $null -and $_ -ne '' } | ForEach-Object { [string]$_ })
        Start-Process -FilePath powershell.exe -ArgumentList $argsList -Verb RunAs -WorkingDirectory $workingDir | Out-Null
        exit
    }

    function Ensure-WindowsTerminal {
        <#
    .SYNOPSIS
      Re-hosts the script under Windows Terminal with title + tab color.
    .DESCRIPTION
      Launches wt.exe new-tab and runs the script via -Command, then exit 0
      so the tab closes cleanly. Adds -SkipWindowsTerminal to avoid loops.
    .PARAMETER Skip
      Skip Windows Terminal re-hosting.
    #>
        param([switch]$Skip)
        try {
            if ($Skip) { Write-NoteMessage "SkipWindowsTerminal requested; continuing in current host."; return }
            if ($env:WT_SESSION) { return }
            $wt = Get-Command wt.exe -ErrorAction SilentlyContinue
            if (-not $wt) { return }

            Write-ActionMessage "re-hosting under Windows Terminal"
            $scriptPath = if ($script:InvocationPath) { $script:InvocationPath } else { $MyInvocation.MyCommand.Path }
            $psArgs = @()
            foreach ($kv in $PSBoundParameters.GetEnumerator()) {
                $name = '-' + $kv.Key
                if ($kv.Key -eq 'SkipWindowsTerminal') { continue }
                $val = $kv.Value
                if ($val -is [System.Management.Automation.SwitchParameter]) {
                    if ([bool]$val) { $psArgs += $name }
                } elseif ($val -is [bool]) {
                    if ($val) { $psArgs += $name }
                } else {
                    $psArgs += $name
                    $psArgs += $val
                }
            }
            # Prevent loop by adding -SkipWindowsTerminal on re-invocation
            $psArgs += '-SkipWindowsTerminal'

            $workingDir = if ($script:OriginalWorkingDirectory -and (Test-Path -LiteralPath $script:OriginalWorkingDirectory)) {
                $script:OriginalWorkingDirectory
            } elseif ($script:InvocationDirectory -and (Test-Path -LiteralPath $script:InvocationDirectory)) {
                $script:InvocationDirectory
            } else {
                $env:SystemRoot
            }

            # Build a wt command as an argument array so quoting is handled correctly.
            # Use a descriptive title and a themed tab color (DodgerBlue).
            # Execute the script and then exit 0 to avoid WT's graceful hold screen.
            # Build -Command string via concatenation to avoid composite-format issues
            $psArgString = (($psArgs | ForEach-Object {
                        $s = [string]$_
                        if ($s -match '\s') { "'" + (($s -replace "'", "''")) + "'" } else { $s }
                    }) -join ' ')
            $quotedScriptPath = "'" + (([string]$scriptPath -replace "'", "''")) + "'"
            $suffix = if ([string]::IsNullOrWhiteSpace($psArgString)) { '' } else { ' ' + $psArgString }
            $cmdCore = "& { & " + $quotedScriptPath + $suffix + " }"
            # Quote the -Command payload so wt/pwsh treat it as a single argument
            $cmdArg = '"' + ($cmdCore -replace '"', '\"') + '"'
            $wtArgs = @(
                'new-tab', '--title', '"LG UltraGear No-Dimming Installer"', '--tabColor', '#1E90FF',
                '--', 'powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', $cmdArg
            )

            Start-Process -FilePath $wt.Path -ArgumentList $wtArgs -WorkingDirectory $workingDir | Out-Null
            exit 0
        } catch {
            Write-ErrorFull -ErrorRecord $_ -Context 'Ensure-WindowsTerminal'
        }
    }

    function Ensure-EmbeddedProfile {
        <#
    .SYNOPSIS
      Materialize the embedded ICC/ICM profile to a temp folder.
    .DESCRIPTION
      Tries ps2exe resource, then embedded Base64, then on-disk fallback.
      Logs size + SHA256 and enforces expected hash by default.
    .PARAMETER ProfileName
      File name to materialize (e.g., lg-ultragear-full-cal.icm).
    #>
        param($ProfileName)

        if (-not $ProfileName) { return $null }
        Write-ActionMessage ("materializing embedded profile '{0}'" -f $ProfileName)

        # Always materialize to a dedicated temp subfolder and clean it up on wrap-up.
        $tempRoot = [IO.Path]::GetTempPath()
        $unique = "lg-ug-profile-" + ([Guid]::NewGuid().ToString('N'))
        $destinationDirectory = Join-Path $tempRoot $unique
        $destination = Join-Path $destinationDirectory $ProfileName

        try {
            if (-not (Test-Path -LiteralPath $destinationDirectory)) {
                [IO.Directory]::CreateDirectory($destinationDirectory) | Out-Null
                Write-CreateMessage ("created folder: {0}" -f $destinationDirectory)
            }
        } catch {
            Write-NoteMessage ("failed to create temp directory '{0}': {1}" -f $destinationDirectory, $_.Exception.Message)
            # Retry with plain temp root without unique subdir
            $destinationDirectory = $tempRoot
            $destination = Join-Path $destinationDirectory $ProfileName
        }

        $script:MaterializedTempProfileDir = $destinationDirectory
        $script:MaterializedTempProfilePath = $destination

        # Expected SHA256 of the embedded profile materialization
        $expectedHash = '2A7158FAE9D7B4883BB5150879B825682697FE24AC805B74649299ADE014A839'
        if ($SkipHashCheck) { Write-NoteMessage "SkipHashCheck set; SHA256 integrity enforcement disabled." }

        # Try ps2exe resource extraction first if available
        $resourceCmd = Get-Command -Name Get-PS2EXEResource -ErrorAction SilentlyContinue
        if ($resourceCmd) {
            try {
                $stream = Get-PS2EXEResource -MemoryStream $ProfileName -ErrorAction Stop
                try {
                    $fileStream = [IO.File]::Open($destination, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)
                    try { $stream.WriteTo($fileStream) } finally { $fileStream.Dispose() }
                } finally { $stream.Dispose() }
                Write-CreateMessage ("extracted embedded profile resource to '{0}'" -f $destination)
                try {
                    $size = (Get-Item -LiteralPath $destination).Length
                    $hash = Get-Sha256HashCompat -LiteralPath $destination
                    Write-InfoMessage ("embedded profile size: {0} bytes" -f $size)
                    Write-InfoMessage ("embedded profile SHA256: {0}" -f $hash)
                    if (-not $SkipHashCheck -and ($hash.ToUpperInvariant() -ne $expectedHash)) { throw ("embedded profile hash mismatch after resource extract; expected {0}, got {1}" -f $expectedHash, $hash) }
                } catch { Write-NoteMessage ("could not compute hash/size: {0}" -f $_.Exception.Message) }
                return (Resolve-Path -LiteralPath $destination -ErrorAction Stop).Path
            } catch {
                Write-NoteMessage ("failed to extract embedded profile resource '{0}': {1}" -f $ProfileName, $_.Exception.Message)
            }
        }

        # Fallback: use built-in Base64 when the profile name matches the bundled asset
        if ($ProfileName -and $ProfileName -ieq $script:EmbeddedProfileName -and $script:EmbeddedProfileBase64) {
            try {
                # Decode Base64 from the embedded single-line payload with robust fallback
                $raw = $script:EmbeddedProfileBase64
                $rawStripped = ($raw -replace '\s', '')
                try {
                    $bytes = [Convert]::FromBase64String($rawStripped)
                } catch {
                    # Fallback: remove any non-Base64 characters and fix padding
                    $clean = ($rawStripped -replace "[^A-Za-z0-9\+/=]", "")
                    if (($clean.Length % 4) -ne 0) { $pad = 4 - ($clean.Length % 4); $clean = $clean + ('=' * $pad) }
                    Write-NoteMessage ("base64 sanitized: rawLen={0}, cleanLen={1}" -f $rawStripped.Length, $clean.Length)
                    try {
                        $bytes = [Convert]::FromBase64String($clean)
                    } catch {
                        throw ("embedded Base64 decode failed. rawLen={0} cleanLen={1}: {2}" -f $rawStripped.Length, $clean.Length, $_.Exception.Message)
                    }
                }
                [IO.File]::WriteAllBytes($destination, $bytes) | Out-Null
                Write-CreateMessage ("wrote embedded Base64 profile to '{0}'" -f $destination)
                try {
                    $size = $bytes.Length
                    $hash = Get-Sha256HashCompat -LiteralPath $destination
                    Write-InfoMessage ("embedded profile size: {0} bytes" -f $size)
                    Write-InfoMessage ("embedded profile SHA256: {0}" -f $hash)
                    if (-not $SkipHashCheck -and ($hash.ToUpperInvariant() -ne $expectedHash)) { throw ("embedded profile hash mismatch after Base64 write; expected {0}, got {1}" -f $expectedHash, $hash) }
                } catch { Write-NoteMessage ("could not compute hash/size: {0}" -f $_.Exception.Message) }
                return (Resolve-Path -LiteralPath $destination -ErrorAction Stop).Path
            } catch {
                Write-NoteMessage ("failed to write embedded Base64 profile '{0}': {1}" -f $ProfileName, $_.Exception.Message)
                # Attempt fallback to on-disk profile asset from known locations
                $fallbackCandidates = @()
                if ($script:InvocationDirectory) { $fallbackCandidates += (Join-Path $script:InvocationDirectory $ProfileName) }
                if ($script:OriginalWorkingDirectory) { $fallbackCandidates += (Join-Path $script:OriginalWorkingDirectory $ProfileName) }
                foreach ($cand in $fallbackCandidates) {
                    try {
                        if (Test-Path -LiteralPath $cand) {
                            Copy-Item -LiteralPath $cand -Destination $destination -Force -ErrorAction Stop
                            Write-CreateMessage ("copied profile from fallback asset '{0}'" -f $cand)
                            try {
                                $size = (Get-Item -LiteralPath $destination).Length
                                $hash = Get-Sha256HashCompat -LiteralPath $destination
                                Write-InfoMessage ("embedded profile size: {0} bytes" -f $size)
                                Write-InfoMessage ("embedded profile SHA256: {0}" -f $hash)
                                if (-not $SkipHashCheck -and ($hash.ToUpperInvariant() -ne $expectedHash)) { throw ("embedded profile hash mismatch after fallback asset copy; expected {0}, got {1}" -f $expectedHash, $hash) }
                            } catch { Write-NoteMessage ("could not compute hash/size: {0}" -f $_.Exception.Message) }
                            return (Resolve-Path -LiteralPath $destination -ErrorAction Stop).Path
                        }
                    } catch { Write-NoteMessage ("fallback copy failed from '{0}': {1}" -f $cand, $_.Exception.Message) }
                }
            }
        }

        # Final fallback: copy embedded profile from on-disk asset if present
        $finalFallback = @()
        if ($script:InvocationDirectory) { $finalFallback += (Join-Path $script:InvocationDirectory $ProfileName) }
        if ($script:OriginalWorkingDirectory) { $finalFallback += (Join-Path $script:OriginalWorkingDirectory $ProfileName) }
        foreach ($cand in $finalFallback) {
            try {
                if (Test-Path -LiteralPath $cand) {
                    Copy-Item -LiteralPath $cand -Destination $destination -Force -ErrorAction Stop
                    Write-CreateMessage ("copied profile from fallback asset '{0}'" -f $cand)
                    try {
                        $size = (Get-Item -LiteralPath $destination).Length
                        $hash = Get-Sha256HashCompat -LiteralPath $destination
                        Write-InfoMessage ("embedded profile size: {0} bytes" -f $size)
                        Write-InfoMessage ("embedded profile SHA256: {0}" -f $hash)
                        if (-not $SkipHashCheck -and ($hash.ToUpperInvariant() -ne $expectedHash)) { throw ("embedded profile hash mismatch after final fallback copy; expected {0}, got {1}" -f $expectedHash, $hash) }
                    } catch { Write-NoteMessage ("could not compute hash/size: {0}" -f $_.Exception.Message) }
                    return (Resolve-Path -LiteralPath $destination -ErrorAction Stop).Path
                }
            } catch { Write-NoteMessage ("fallback copy failed from '{0}': {1}" -f $cand, $_.Exception.Message) }
        }

        return $null
    }

    function Resolve-ProfilePath {
        <#
    .SYNOPSIS
      Resolve a user path; fallback to embedded profile if not found.
    .PARAMETER InputPath
      Path or file name to resolve.
    #>
        param($InputPath)

        if (-not $InputPath) { return $null }
        Write-ActionMessage ("resolving profile path for input '{0}'" -f $InputPath)

        $candidates = @()
        # Try the caller-provided value first and backfill with known directories when relative.
        if ([IO.Path]::IsPathRooted($InputPath)) {
            $candidates += $InputPath
        } else {
            $candidates += $InputPath
            if ($script:OriginalWorkingDirectory) {
                $candidates += (Join-Path $script:OriginalWorkingDirectory $InputPath)
            }
            if ($script:InvocationDirectory) {
                $candidates += (Join-Path $script:InvocationDirectory $InputPath)
            }
        }

        Write-NoteMessage ("profile path candidates: {0}" -f ($candidates -join '; '))

        foreach ($candidate in $candidates) {
            try {
                $resolved = Resolve-Path -LiteralPath $candidate -ErrorAction Stop
                Write-SuccessMessage ("resolved profile path: {0}" -f $resolved.Path)
                return $resolved.Path
            } catch {
                Write-NoteMessage ("profile lookup skipped for candidate '{0}': {1}" -f $candidate, $_.Exception.Message)
            }
        }

        $profileName = [IO.Path]::GetFileName($InputPath)
        Write-InfoMessage ("attempting to materialize embedded profile for '{0}'" -f $profileName)
        return Ensure-EmbeddedProfile -ProfileName $profileName
    }

    function Get-UnicodeStringFromCodePoint {
        <#
    .SYNOPSIS
      Convert code points to a UTF-16 string (helper).
    #>
        param([int[]]$CodePoints)

        if (-not $CodePoints) { return ::Empty }

        $builder = [System.Text.StringBuilder]::new()
        foreach ($codePoint in $CodePoints) {
            if ($codePoint -le 0xFFFF) {
                [void]$builder.Append([char]$codePoint)
            } else {
                $adjusted = $codePoint - 0x10000
                $highSurrogate = [int][math]::Floor($adjusted / 0x400) + 0xD800
                $lowSurrogate = ($adjusted % 0x400) + 0xDC00
                [void]$builder.Append([char]$highSurrogate)
                [void]$builder.Append([char]$lowSurrogate)
            }
        }

        return $builder.ToString()
    }

    # Embedded profile payload (Base64) for self-contained EXE
    $script:EmbeddedProfileName = 'lg-ultragear-full-cal.icm'
    $script:EmbeddedProfileBase64 = 'AAAQGGFyZ2wCIAAAbW50clJHQiBYWVogB+kAAwALAA4AMgA7YWNzcE1TRlQAAAAAAABtHgAAXIYAAAAAAAAAAAAAAAAAAPbWAAEAAAAA0y1EQ0FMAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQZGVzYwAAAUQAAABnY3BydAAAAawAAAAaZG1uZAAAAcgAAABpZG1kZAAAAUQAAABnbW1vZAAAAjQAAAAod3RwdAAAAlwAAAAUYXJ0cwAAAnAAAAAsY2hybQAAApwAAAAkclhZWgAAAsAAAAAUclRSQwAAAtQAAAAOZ1hZWgAAAuQAAAAUZ1RSQwAAAtQAAAAOYlhZWgAAAvgAAAAUYlRSQwAAAtQAAAAObWV0YQAAAwwAAAb4dmNndAAACgQAAAYSZGVzYwAAAAAAAAANTEcgVUxUUkFHRUFSAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB0ZXh0AAAAAENyZWF0ZWQgZnJvbSBFRElEAAAAZGVzYwAAAAAAAAAPTEcgRWxlY3Ryb25pY3MAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAbW1vZAAAAAAAAB5tAABchgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFhZWiAAAAAAAADz2AABAAAAARYIc2YzMgAAAAAAAOUlAABEM///1q7//z/zAAG2qAAACWUAAAn1///udwABB5RjaHJtAAAAAAADAAAAAKbAAABVgAAASYAAAKJAAAAnQAAAC4BYWVogAAAAAAAAdkYAADwLAAABvGN1cnYAAAAAAAAAAQIzAABYWVogAAAAAAAAWj8AALjPAAATW1hZWiAAAAAAAAAmUAAACyUAAL4WZGljdAAAAAAAAAAaAAAAEAAAAbAAAAAMAAABvAAAAEgAAAIEAAAAEgAAAhgAAAAGAAACIAAAABgAAAI4AAAACAAAAkAAAAAaAAACXAAAAAoAAAJoAAAAEgAAAnwAAAAQAAACjAAAABQAAAKgAAAAGAAAArgAAAAUAAACzAAAABYAAALkAAAAGAAAAvwAAAAWAAADFAAAABgAAAMsAAAAGAAAA0QAAAAWAAADXAAAABgAAAN0AAAAFgAAA4wAAAAWAAADpAAAABgAAAO8AAAAGAAAA9QAAAAYAAAD7AAAABgAAAQEAAAAIgAABCgAAAAcAAAERAAAABQAAARYAAAABgAABGAAAAAUAAAEdAAAABgAAASMAAAAFgAABKQAAAAYAAAEvAAAABAAAATMAAAAQAAABQwAAAA2AAAFRAAAAAIAAAVIAAAAFgAABWAAAAAIAAAFaAAAABgAAAWAAAAAGgAABZwAAAAsAAAFyAAAAAwAAAXUAAAAKAAABfwAAAAMAAAGCAAAADIAAAY8AAAADAAABkgAAAAOAAAGWAAAABoAAAZ0AAAAIgAABpgAAABeAHAAcgBlAGYAaQB4AEUARABJAEQAXwAsAEQAQQBUAEEAXwAsAE8AUABFAE4ASQBDAEMAXwAsAEcAQQBNAFUAVABfACwATQBBAFAAUABJAE4ARwBfAEUARABJAEQAXwBtAG4AZgB0AAAARwBTAE0AAABFAEQASQBEAF8AbQBuAGYAdABfAGkAZAA3ADcAOAA5AEUARABJAEQAXwBtAG8AZABlAGwAXwBpAGQAAAAyADMANgA4ADYAAABFAEQASQBEAF8AZABhAHQAZQAAADIAMAAyADQALQBUADIAOQBFAEQASQBEAF8AcgBlAGQAXwB4ADAALgA2ADUAMQAzADYANwAxADgANwA1AEUARABJAEQAXwByAGUAZABfAHkAMAAuADMAMwAzADkAOAA0ADMANwA1AAAARQBEAEkARABfAGcAcgBlAGUAbgBfAHgAMAAuADIAOAA3ADEAMAA5ADMANwA1AAAARQBEAEkARABfAGcAcgBlAGUAbgBfAHkAMAAuADYAMwAzADcAOAA5ADAANgAyADUARQBEAEkARABfAGIAbAB1AGUAXwB4AAAAMAAuADEANQAzADMAMgAwADMAMQAyADUARQBEAEkARABfAGIAbAB1AGUAXwB5AAAAMAAuADAANAA0ADkAMgAxADgANwA1AAAARQBEAEkARABfAHcAaABpAHQAZQBfAHgAMAAuADMAMQAzADQANwA2ADUANgAyADUARQBEAEkARABfAHcAaABpAHQAZQBfAHkAMAAuADMAMgA5ADEAMAAxADUANgAyADUARQBEAEkARABfAG0AYQBuAHUAZgBhAGMAdAB1AHIAZQByAAAATABHACAARQBsAGUAYwB0AHIAbwBuAGkAYwBzAEUARABJAEQAXwBnAGEAbQBtAGEAMgAuADIAAABFAEQASQBEAF8AbQBvAGQAZQBsAEwARwAgAFUATABUAFIAQQBHAEUAQQBSAEUARABJAEQAXwBzAGUAcgBpAGEAbAAAADQAMAA3AEIATwBXAEMAMAA2ADcAOQAzAEUARABJAEQAXwBtAGQANQA4ADAAYgA5ADUANgA2ADAANABjADAAOAA0AGQAOQBiADkAOQA3ADgAMwA3ADUAMwA4ADAANAAyAGQAZQBiAGUATwBQAEUATgBJAEMAQwBfAGEAdQB0AG8AbQBhAHQAaQBjAF8AZwBlAG4AZQByAGEAdABlAGQAAAAxAAAARABBAFQAQQBfAHMAbwB1AHIAYwBlAAAAZQBkAGkAZABHAEEATQBVAFQAXwB2AG8AbAB1AG0AZQAxAC4AMQA5ADAANAAxADQAMwA4ADkANgA2AAAARwBBAE0AVQBUAF8AYwBvAHYAZQByAGEAZwBlACgAZABjAGkALQBwADMAKQAwAC4AOAAyADQAMgBHAEEATQBVAFQAXwBjAG8AdgBlAHIAYQBnAGUAKABzAHIAZwBiACkAMAAuADkAOQAzADUARwBBAE0AVQBUAF8AYwBvAHYAZQByAGEAZwBlACgAYQBkAG8AYgBlAC0AcgBnAGIAKQAAADAALgA3ADYANQA5AEwAaQBjAGUAbgBzAGUAAABQAHUAYgBsAGkAYwAgAEQAbwBtAGEAaQBuAAAATQBBAFAAUABJAE4ARwBfAGQAZQB2AGkAYwBlAF8AaQBkAAAAeAByAGEAbgBkAHIALQBMAEcAIABFAGwAZQBjAHQAcgBvAG4AaQBjAHMALQBMAEcAIABVAEwAVABSAEEARwBFAEEAUgAtADQAMAA3AEIATwBXAEMAMAA2ADcAOQAzAAB2Y2d0AAAAAAAAAAAAAwEAAAIMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDA0NDg4PDxAQERESEhMTFBQVFRYWFxcYGBkZGhobGxwcHR0eHh8fICAhISIiIyMkJCUlJiYnJygoKSkqKisrLCwtLS4uLy8wMDExMjIzMzQ0NTU2Njc3ODg5OTo6Ozs8PD09Pj4/P0BAQUFCQkNDRERFRUZGR0dISElJSkpLS0xMTU1OTk9PUFBRUVJSU1NUVFVVVlZXV1hYWVlaWltbXFxdXV5eX19gYGFhYmJjY2RkZWVmZmdnaGhpaWpqa2tsbG1tbm5vb3BwcXFycnNzdHR1dXZ2d3d4eHl5enp7e3x8fX1+fn9/gICBgYKCg4OEhIWFhoaHh4iIiYmKiouLjIyNjY6Oj4+QkJGRkpKTk5SUlZWWlpeXmJiZmZqam5ucnJ2dnp6fn6CgoaGioqOjpKSlpaamp6eoqKmpqqqrq6ysra2urq+vsLCxsbKys7O0tLW1tra3t7i4ubm6uru7vLy9vb6+v7/AwMHBwsLDw8TExcXGxsfHyMjJycrKy8vMzM3Nzs7Pz9DQ0dHS0tPT1NTV1dbW19fY2NnZ2trb29zc3d3e3t/f4ODh4eLi4+Pk5OXl5ubn5+jo6enq6uvr7Ozt7e7u7+/w8PHx8vLz8/T09fX29vf3+Pj5+fr6+/v8/P39/v7//wwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDQ0ODg8PEBARERISExMUFBUVFhYXFxgYGRkaGhsbHBwdHR4eHx8gICEhIiIjIyQkJSUmJicnKCgpKSoqKyssLC0tLi4vLzAwMTEyMjMzNDQ1NTY2Nzc4ODk5Ojo7Ozw8PT0+Pj8/QEBBQUJCQ0NEREVFRkZHR0hISUlKSktLTExNTU5OT09QUFFRUlJTU1RUVVVWVldXWFhZWVpaW1tcXF1dXl5fX2BgYWFiYmNjZGRlZWZmZ2doaGlpampra2xsbW1ubm9vcHBxcXJyc3N0dHV1dnZ3d3h4eXl6ent7fHx9fX5+f3+AgIGBgoKDg4SEhYWGhoeHiIiJiYqKi4uMjI2Njo6Pj5CQkZGSkpOTlJSVlZaWl5eYmJmZmpqbm5ycnZ2enp+foKChoaKio6OkpKWlpqanp6ioqamqqqurrKytra6ur6+wsLGxsrKzs7S0tbW2tre3uLi5ubq6u7u8vL29vr6/v8DAwcHCwsPDxMTFxcbGx8fIyMnJysrLy8zMzc3Ozs/P0NDR0dLS09PU1NXV1tbX19jY2dna2tvb3Nzd3d7e39/g4OHh4uLj4+Tk5eXm5ufn6Ojp6erq6+vs7O3t7u7v7/Dw8fHy8vPz9PT19fb29/f4+Pn5+vr7+/z8/f3+/v//DAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwNDQ4ODw8QEBEREhITExQUFRUWFhcXGBgZGRoaGxscHB0dHh4fHyAgISEiIiMjJCQlJSYmJycoKCkpKiorKywsLS0uLi8vMDAxMTIyMzM0NDU1NjY3Nzg4OTk6Ojs7PDw9PT4+Pz9AQEFBQkJDQ0RERUVGRkdHSEhJSUpKS0tMTE1NTk5PT1BQUVFSUlNTVFRVVVZWV1dYWFlZWlpbW1xcXV1eXl9fYGBhYWJiY2NkZGVlZmZnZ2hoaWlqamtrbGxtbW5ub29wcHFxcnJzc3R0dXV2dnd3eHh5eXp6e3t8fH19fn5/f4CAgYGCgoODhISFhYaGh4eIiImJioqLi4yMjY2Ojo+PkJCRkZKSk5OUlJWVlpaXl5iYmZmampubnJydnZ6en5+goKGhoqKjo6SkpaWmpqenqKipqaqqq6usrK2trq6vr7CwsbGysrOztLS1tba2t7e4uLm5urq7u7y8vb2+vr+/wMDBwcLCw8PExMXFxsbHx8jIycnKysvLzMzNzc7Oz8/Q0NHR0tLT09TU1dXW1tfX2NjZ2dra29vc3N3d3t7f3+Dg4eHi4uPj5OTl5ebm5+fo6Onp6urr6+zs7e3u7u/v8PDx8fLy8/P09PX19vb39/j4+fn6+vv7/Pz9/f7+//8AAA=='

    # Console-safe ASCII labels for log prefixes
    $script:SymbolInfo = '[INFO]'
    $script:SymbolAction = '[STEP]'
    $script:SymbolSuccess = '[ OK ]'
    $script:SymbolWarning = '[WARN]'
    $script:SymbolNote = '[NOTE]'
    $script:SymbolSkip = '[SKIP]'
    $script:SymbolDelete = '[DEL ]'
    $script:SymbolError = '[ERR ]'
    $script:SymbolStart = '[STRT]'
    $script:SymbolDone = '[DONE]'
    $script:SymbolCreate = '[CRT ]'

    if ($Help) { Show-Usage; exit }

    # Re-host under Windows Terminal if available (before elevation)
    Ensure-WindowsTerminal -Skip:$SkipWindowsTerminal.IsPresent

    # Auto-elevate if not already running as Administrator
    Ensure-Elevation -Skip:$SkipElevation.IsPresent

    $ErrorActionPreference = 'Stop'
    $VerbosePreference = 'SilentlyContinue'  # Suppress native VERBOSE lines; we log via ASCII indicators

    # WCS scope constants
    $WCS_SCOPE_CURRENT_USER = 0
    $WCS_SCOPE_SYSTEM_WIDE = 2

    # WCS default profile constants
    $CPT_ICC = 1  # COLORPROFILETYPE.CPT_ICC
    $CPS_DEV = 0  # COLORPROFILESUBTYPE.CPS_DEVICE
    $COLOR_PROFILE_TYPE_SDR = 0  # COLORPROFILETYPE_STANDARD_DYNAMIC_RANGE
    $COLOR_PROFILE_SUBTYPE_SDR = 0  # COLORPROFILESUBTYPE_STANDARD_DYNAMIC_RANGE

    Write-InitMessage "starting LG UltraGear no-dimming installer"

    if ($DryRun) {
        $script:WhatIfPreference = $true
        Write-NoteMessage "dry-run enabled (-WhatIf): no changes will be made"
    }

    # P/Invoke shims: load each interop function in its own Add-Type for clearer diagnostics
    function Add-PInvokeType {
        <#
    .SYNOPSIS
      Load a single P/Invoke type and log outcome.
    .PARAMETER Name
      Friendly name used in logs.
    .PARAMETER Code
      C# definition to compile.
    #>
        param($Name, $Code)
        try {
            Write-ActionMessage ("loading P/Invoke: {0}" -f $Name)
            Add-Type -TypeDefinition $Code -ErrorAction Stop
            Write-SuccessMessage ("P/Invoke loaded: {0}" -f $Name)
        } catch {
            Write-ErrorFull -ErrorRecord $_ -Context ("Add-PInvokeType:{0}" -f $Name)
            throw
        }
    }

    $srcInstall = @"
using System;
using System.Runtime.InteropServices;
public static class WcsInstall {
  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true, EntryPoint="InstallColorProfileW")]
  public static extern bool InstallColorProfile(string machine, string profilePath);
}
"@
    Add-PInvokeType -Name 'WcsInstall.InstallColorProfile' -Code $srcInstall

    $srcAssociate = @"
using System;
using System.Runtime.InteropServices;
public static class WcsAssociate {
  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern bool WcsAssociateColorProfileWithDevice(uint scope, string profile, string deviceName);
}
"@
    Add-PInvokeType -Name 'WcsAssociate.WcsAssociateColorProfileWithDevice' -Code $srcAssociate

    $srcDefault = @"
using System;
using System.Runtime.InteropServices;
public static class WcsDefault {
  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern bool WcsSetDefaultColorProfile(uint scope, string deviceName, int cpt, int cps, uint profileID, string profileName);
}
"@
    Add-PInvokeType -Name 'WcsDefault.WcsSetDefaultColorProfile' -Code $srcDefault

    $srcHdrAssoc = @"
using System;
using System.Runtime.InteropServices;
public static class WcsHdrAssoc {
  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true, EntryPoint="ColorProfileAddDisplayAssociation")]
  public static extern bool ColorProfileAddDisplayAssociation(string profile, string deviceName, uint scope, uint profileType);
}
"@
    Add-PInvokeType -Name 'WcsHdrAssoc.ColorProfileAddDisplayAssociation' -Code $srcHdrAssoc

    $srcSdrAssoc = @"
using System;
using System.Runtime.InteropServices;
public static class WcsSdrAssoc {
  [DllImport("mscms.dll", CharSet=CharSet.Unicode, SetLastError=true, EntryPoint="ColorProfileSetDisplayDefaultAssociation")]
  public static extern bool ColorProfileSetDisplayDefaultAssociation(string profile, string deviceName, uint scope, uint profileType, uint profileSubType, uint profileId);
}
"@
    Add-PInvokeType -Name 'WcsSdrAssoc.ColorProfileSetDisplayDefaultAssociation' -Code $srcSdrAssoc

    $srcSendMessage = @"
using System;
using System.Runtime.InteropServices;
public static class Win32SendMessage {
  [DllImport("user32.dll", SetLastError=true)]
  public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
}
"@
    Add-PInvokeType -Name 'Win32SendMessage.SendMessageTimeout' -Code $srcSendMessage
    function Invoke-Main {
        [CmdletBinding(SupportsShouldProcess = $true)]
        param()
        <#
    .SYNOPSIS
      Main workflow: prepare profile, install, associate, refresh.
    .NOTES
      Honors -InstallOnly, -PerUser, -NoSetDefault, -Probe, -InstallMonitor, -UninstallMonitor, -Uninstall, -UninstallFull, -Reinstall, -Refresh.
    #>
        # Handle uninstall operations first
        if ($UninstallFull) {
            Write-ActionMessage "Performing full uninstall..."
            Uninstall-AutoReapplyMonitor -TaskName $MonitorTaskName
            $profilePath = Join-Path $env:WINDIR "System32\spool\drivers\color\lg-ultragear-full-cal.icm"
            if (Test-Path -LiteralPath $profilePath) {
                try {
                    Remove-Item -LiteralPath $profilePath -Force -ErrorAction Stop
                    Write-DeleteMessage "Removed color profile"
                } catch {
                    Write-WarnMessage "Could not remove profile: $($_.Exception.Message)"
                }
            } else {
                Write-NoteMessage "Profile not found (already removed)"
            }
            Write-DoneMessage "Full uninstall complete"
            Show-ExitPrompt
            return
        }

        if ($Uninstall -or $UninstallMonitor) {
            Uninstall-AutoReapplyMonitor -TaskName $MonitorTaskName
            Show-ExitPrompt
            return
        }

        # Handle reinstall (uninstall then install)
        if ($Reinstall) {
            Write-ActionMessage "Reinstalling (removing existing first)..."
            Uninstall-AutoReapplyMonitor -TaskName $MonitorTaskName
            $script:SkipMonitor = $false
        }

        if ($InstallMonitor) {
            Install-AutoReapplyMonitor -TaskName $MonitorTaskName -InstallerPath $script:InvocationPath -MonitorMatch $MonitorNameMatch
            Show-ExitPrompt
            return
        }

        try {
            Write-ActionMessage "preparing embedded profile"
            $profileName = $script:EmbeddedProfileName
            $profileFull = Ensure-EmbeddedProfile -ProfileName $profileName
            if (-not $profileFull) {
                throw ("embedded profile could not be materialized: expected '{0}'" -f $profileName)
            }
            Write-NoteMessage "using embedded profile at: $profileFull"
            $installedStore = Join-Path $env:WINDIR 'System32\\spool\\drivers\\color'
            $installedPath = Join-Path $installedStore $profileName

            Write-ActionMessage "install/refresh color profile: $profileName"
            if ($PSCmdlet.ShouldProcess($profileFull, "Install/refresh color profile in system store")) {
                # Ensure destination directory exists
                $dir = Split-Path -Parent $installedPath
                if (-not (Test-Path -LiteralPath $dir)) { [IO.Directory]::CreateDirectory($dir) | Out-Null; Write-CreateMessage ("created folder: {0}" -f $dir) }

                $srcHash = Get-Sha256HashCompat -LiteralPath $profileFull
                $srcSize = (Get-Item -LiteralPath $profileFull).Length
                Write-InfoMessage ("source profile size: {0} bytes" -f $srcSize)
                Write-InfoMessage ("source profile SHA256: {0}" -f $srcHash)

                if (Test-Path -LiteralPath $installedPath) {
                    $dstHash = Get-Sha256HashCompat -LiteralPath $installedPath
                    if ($srcHash -ne $dstHash) {
                        Copy-Item -LiteralPath $profileFull -Destination $installedPath -Force
                        Write-SuccessMessage "profile updated at: $installedPath"
                    } else {
                        Write-SkipMessage "profile already current (skipped) at: $installedPath"
                    }
                } else {
                    $copied = $false
                    try {
                        Copy-Item -LiteralPath $profileFull -Destination $installedPath -Force -ErrorAction Stop
                        $copied = $true
                        Write-SuccessMessage "profile copied to color store"
                    } catch {
                        Write-NoteMessage ("direct copy failed; attempting InstallColorProfile: {0}" -f $_.Exception.Message)
                    }
                    if (-not $copied) {
                        if (-not [WcsInstall]::InstallColorProfile($null, $profileFull)) {
                            $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                            throw "InstallColorProfile failed (Win32=$code)."
                        }
                        Write-SuccessMessage "profile installed via InstallColorProfile"
                    }
                    Write-NoteMessage ("installed path: {0}" -f $installedPath)
                }
            }

            if ($InstallOnly) {
                Write-InfoMessage "install-only mode: skipping association and defaults"
                Show-ExitPrompt
                return
            }

            # Collect all detected monitors so reporting and matching remain transparent to the user.
            Write-ActionMessage "enumerating monitors via WmiMonitorID ..." -NoNewline
            $monitors = Get-CimInstance -Namespace root/wmi -Class WmiMonitorID | ForEach-Object {
                $name = -join ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ })
                [PSCustomObject]@{
                    InstanceName = $_.InstanceName
                    FriendlyName = $name
                }
            }

            # Fail fast if Windows cannot enumerate any displays.
            if (-not $monitors) { throw "no monitors returned by WMI (WmiMonitorID)" }

            $targets = $monitors | Where-Object { $_.FriendlyName -like "*${MonitorNameMatch}*" }
            $targetsCount = @($targets).Count
            Write-Host ""
            Write-InfoMessage ("detected monitor count: {0}" -f (@($monitors).Count))
            Write-InfoMessage ("detected monitors ({0}):" -f (@($monitors).Count))
            $monitors | ForEach-Object { Write-Host (" - {0} [{1}]" -f $_.FriendlyName, $_.InstanceName) }
            Write-InfoMessage ("matched ({0}) (contains '{1}'):" -f $targetsCount, $MonitorNameMatch)
            if ($targets) { $targets | ForEach-Object { Write-Host (" - {0} [{1}]" -f $_.FriendlyName, $_.InstanceName) } } else { Write-Host " - none" }
            Write-Host ""
            Write-ActionMessage "compatibility check"
            if ($targets) {
                Write-SuccessMessage ("found {0} compatible monitor(s)" -f $targets.Count)
            } else {
                Write-SkipMessage "no compatible monitors matched; adjust -MonitorNameMatch"
            }
            Write-Host ""

            if ($Probe) {
                # Probe mode stops after logging so no system changes occur.
                Write-InfoMessage "probe mode: no changes will be made"
                Show-ExitPrompt
                return
            }

            if (-not $targets) { throw ("no compatible monitors matched. adjust -MonitorNameMatch (current='{0}')" -f $MonitorNameMatch) }

            # Apply the requested profile operations to every matched monitor.
            foreach ($m in $targets) {
                $deviceName = $m.InstanceName
                Write-ActionMessage "associating profile with: $($m.FriendlyName)"
                Write-NoteMessage "Device key: $deviceName"

                if ($PSCmdlet.ShouldProcess($deviceName, "Associate profile (system-wide)")) {
                    if (-not [WcsAssociate]::WcsAssociateColorProfileWithDevice([uint32]$WCS_SCOPE_SYSTEM_WIDE, $installedPath, $deviceName)) {
                        $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                        Write-WarnMessage ("system-wide association failed (Win32={0})" -f $code)
                    } else { Write-SuccessMessage "system-wide association ok" }
                }

                if ($PerUser.IsPresent) {
                    if ($PSCmdlet.ShouldProcess($deviceName, "Associate profile (current user)")) {
                        if (-not [WcsAssociate]::WcsAssociateColorProfileWithDevice([uint32]$WCS_SCOPE_CURRENT_USER, $installedPath, $deviceName)) {
                            $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                            Write-WarnMessage ("per-user association failed (Win32={0})" -f $code)
                        } else { Write-SuccessMessage "per-user association ok" }
                    }
                }

                # Control whether we call the legacy WcsSetDefaultColorProfile API.
                # Default installer behavior: do NOT call it. It will only be invoked when
                # -EnableGenericDefault is passed and -NoSetDefault is not present.
                if ($EnableGenericDefault -and -not $NoSetDefault) {
                    if ($PSCmdlet.ShouldProcess($deviceName, "Set as generic default profile (requested via -EnableGenericDefault)")) {
                        if (-not [WcsDefault]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_SYSTEM_WIDE, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
                            $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                            Write-WarnMessage ("set generic default (system) failed (Win32={0})" -f $code)
                        } else { Write-SuccessMessage "set generic default (system) ok" }
                        if ($PerUser.IsPresent) {
                            if (-not [WcsDefault]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_CURRENT_USER, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
                                $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                                Write-WarnMessage ("set generic default (user) failed (Win32={0})" -f $code)
                            } else { Write-SuccessMessage "set generic default (user) ok" }
                        }
                    }
                } else {
                    if ($NoSetDefault) { Write-InfoMessage "NoSetDefault requested; skipping generic default profile operations" } else { Write-NoteMessage "Generic default profile not enabled; skipping. Use -EnableGenericDefault to allow." }
                }

                try {
                    if ($PSCmdlet.ShouldProcess($deviceName, "SDR/default association")) {
                        [void][WcsSdrAssoc]::ColorProfileSetDisplayDefaultAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_SYSTEM_WIDE, [uint32]$COLOR_PROFILE_TYPE_SDR, [uint32]$COLOR_PROFILE_SUBTYPE_SDR, 0)
                        if ($PerUser.IsPresent) {
                            [void][WcsSdrAssoc]::ColorProfileSetDisplayDefaultAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_CURRENT_USER, [uint32]$COLOR_PROFILE_TYPE_SDR, [uint32]$COLOR_PROFILE_SUBTYPE_SDR, 0)
                        }
                        Write-SuccessMessage "SDR/default association ok"
                    }
                } catch {
                    Write-NoteMessage "SDR association API not available; skipping."
                }

                # HDR/advanced-color association is opt-in. Default install will NOT touch HDR/advanced-color.
                # Use -EnableHdrAssociation to explicitly add the profile to the advanced-color/HDR association.
                if ($EnableHdrAssociation -and -not $SkipHdrAssociation) {
                    try {
                        if ($PSCmdlet.ShouldProcess($deviceName, "HDR/advanced-color association (requested via -EnableHdrAssociation)")) {
                            [void][WcsHdrAssoc]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_SYSTEM_WIDE, 0)
                            if ($PerUser.IsPresent) { [void][WcsHdrAssoc]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_CURRENT_USER, 0) }
                            Write-SuccessMessage "HDR/advanced-color association ok"
                        }
                    } catch {
                        Write-NoteMessage "HDR association API not available; skipping."
                    }
                } else {
                    if ($SkipHdrAssociation) { Write-InfoMessage "SkipHdrAssociation requested; skipping HDR/advanced-color association" } else { Write-NoteMessage "HDR/advanced-color association not enabled; skipping. Use -EnableHdrAssociation to allow." }
                }
            }

            Write-ActionMessage "refreshing color settings"
            $HWND_BROADCAST = [IntPtr]0xffff
            $WM_SETTINGCHANGE = 0x1A
            $SMTO_ABORTIFHUNG = 0x0002
            [UIntPtr]$res = [UIntPtr]::Zero
            [void][Win32SendMessage]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Color', $SMTO_ABORTIFHUNG, 2000, [ref]$res)

            Write-SuccessMessage "done. associated profile '$profileName' with all displays matching '$MonitorNameMatch'."

            # =========================================================================
            # AUTO-REAPPLY MONITOR INSTALLATION
            # =========================================================================
            if (-not $SkipMonitor -and -not $Probe -and -not $InstallOnly -and -not $DryRun) {
                Write-Host ""
                Write-StepMessage "installing auto-reapply monitor"
                try {
                    Install-AutoReapplyMonitor -TaskName $MonitorTaskName -InstallerPath $script:InvocationPath -MonitorMatch $MonitorNameMatch
                } catch {
                    Write-WarnMessage "Auto-reapply monitor installation failed: $($_.Exception.Message)"
                    Write-NoteMessage "Profile is installed but won't auto-reapply on reconnection"
                }
            } elseif ($SkipMonitor) {
                Write-NoteMessage "Skipping auto-reapply monitor (-SkipMonitor specified)"
            }
        } catch {
            Write-ErrorFull -ErrorRecord $_ -Context "Invoke-Main"
            exit 1
        } finally {
            Write-ActionMessage "wrapping up"
            # Delete materialized temp profile (and its unique folder) if applicable
            try {
                if ($KeepTemp.IsPresent) {
                    Write-NoteMessage ("KeepTemp set; retaining temp files. Path: {0}" -f $script:MaterializedTempProfilePath)
                } elseif ($script:MaterializedTempProfilePath -and (Test-Path -LiteralPath $script:MaterializedTempProfilePath)) {
                    $tempRoot = [IO.Path]::GetTempPath()
                    $full = (Resolve-Path -LiteralPath $script:MaterializedTempProfilePath).Path
                    $tempFull = (Resolve-Path -LiteralPath $tempRoot).Path
                    if ($full.StartsWith($tempFull, [StringComparison]::OrdinalIgnoreCase)) {
                        Remove-Item -LiteralPath $script:MaterializedTempProfilePath -Force -ErrorAction Stop
                        Write-DeleteMessage ("deleted temp profile: {0}" -f $script:MaterializedTempProfilePath)
                    }
                }
                if (-not $KeepTemp.IsPresent -and $script:MaterializedTempProfileDir -and (Test-Path -LiteralPath $script:MaterializedTempProfileDir)) {
                    $tempRoot = [IO.Path]::GetTempPath()
                    $dirFull = (Resolve-Path -LiteralPath $script:MaterializedTempProfileDir).Path
                    $tempFull = (Resolve-Path -LiteralPath $tempRoot).Path
                    if ($dirFull.StartsWith($tempFull, [StringComparison]::OrdinalIgnoreCase)) {
                        Remove-Item -LiteralPath $script:MaterializedTempProfileDir -Recurse -Force -ErrorAction Stop
                        Write-DeleteMessage ("deleted temp folder: {0}" -f $script:MaterializedTempProfileDir)
                    }
                }
            } catch {
                Write-NoteMessage ("temp cleanup skipped: {0}" -f $_.Exception.Message)
            }
            # Emit done as the last status line
            Write-DoneMessage "terminated"
            Show-ExitPrompt
        }
    }
}

process {
    if ($script:IsInteractive) {
        Start-TUI
    } else {
        Invoke-Main
    }
}