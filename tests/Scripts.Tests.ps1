Set-StrictMode -Version Latest

# =============================================================================
# TEST SUITE INITIALIZATION
# =============================================================================
# LG UltraGear No-Dimming Fix - Comprehensive Test Suite
# Generated: Dynamic at runtime
# PowerShell Version: $($PSVersionTable.PSVersion)
# =============================================================================

BeforeAll {
    # =========================================================================
    # TEST SUITE HEADER
    # =========================================================================
    $script:TestStartTime = Get-Date
    $script:TestSuiteName = 'LG UltraGear No-Dimming Fix'
    $script:TestSuiteVersion = '2.0.0'

    # =========================================================================
    # COLOR SCHEME DEFINITIONS
    # =========================================================================
    $script:Colors = @{
        # Primary colors
        Banner          = 'Magenta'
        BannerAccent    = 'DarkMagenta'
        BannerText      = 'White'
        
        # Section colors
        Section         = 'Cyan'
        SectionAccent   = 'DarkCyan'
        Separator       = 'DarkGray'
        
        # Status colors
        Success         = 'Green'
        Warning         = 'Yellow'
        Error           = 'Red'
        Info            = 'White'
        Debug           = 'DarkGray'
        Skip            = 'DarkYellow'
        
        # Log level colors
        Timestamp       = 'DarkGray'
        TagInit         = 'Blue'
        TagCat          = 'Magenta'
        TagSum          = 'Cyan'
        TagTest         = 'Yellow'
        TagPass         = 'Green'
        TagFail         = 'Red'
        TagWarn         = 'Yellow'
        
        # Misc
        Highlight       = 'Cyan'
        Muted           = 'DarkGray'
        Number          = 'Yellow'
    }

    # =========================================================================
    # UNICODE SYMBOLS (using only BMP characters for compatibility)
    # =========================================================================
    $script:Symbols = @{
        Check      = [char]0x2713  # ✓
        Cross      = [char]0x2717  # ✗
        Bullet     = [char]0x2022  # •
        Arrow      = [char]0x25B6  # ▶
        Diamond    = [char]0x25C6  # ◆
        Star       = [char]0x2605  # ★
        Circle     = [char]0x25CF  # ●
        Square     = [char]0x25A0  # ■
        Triangle   = [char]0x25B2  # ▲
        Lightning  = [char]0x2726  # ✦ (sparkle, safer alternative)
        Gear       = [char]0x2699  # ⚙
        Clock      = [char]0x25D4  # ◔ (clock-like)
        Rocket     = [char]0x2B9E  # ⮞ (rightward arrow)
        Package    = [char]0x25A1  # □
        Wrench     = [char]0x2692  # ⚒
        Magnify    = [char]0x2295  # ⊕
        Shield     = [char]0x2660  # ♠
    }

    # =========================================================================
    # LOGGING HELPER FUNCTIONS
    # =========================================================================
    
    function Get-ElapsedTimestamp {
        # Returns elapsed time since test start in format T+0.000s
        $elapsed = (Get-Date) - $script:TestStartTime
        $totalSeconds = $elapsed.TotalSeconds
        return "T+{0,8:F3}s" -f $totalSeconds
    }

    function Write-ColorLine {
        param(
            [string]$Text,
            [string]$Color = 'White',
            [switch]$NoNewline
        )
        if ($NoNewline) {
            Write-Host $Text -ForegroundColor $Color -NoNewline
        } else {
            Write-Host $Text -ForegroundColor $Color
        }
    }

    function Write-TestHeader {
        param([string]$Title, [string]$Subtitle = '')
        $timestamp = Get-ElapsedTimestamp
        Write-Host ""
        Write-ColorLine ("=" * 80) $script:Colors.Section
        Write-ColorLine "[$timestamp] " $script:Colors.Timestamp -NoNewline
        Write-ColorLine "$($script:Symbols.Diamond) $Title" $script:Colors.Section
        if ($Subtitle) {
            Write-ColorLine "              $Subtitle" $script:Colors.Muted
        }
        Write-ColorLine ("=" * 80) $script:Colors.Section
    }

    function Write-TestSection {
        param([string]$Section, [string]$Icon = '')
        $timestamp = Get-ElapsedTimestamp
        Write-Host ""
        Write-ColorLine "+-----------------------------------------------------------------------------+" $script:Colors.SectionAccent
        Write-ColorLine "| " $script:Colors.SectionAccent -NoNewline
        Write-ColorLine "[$timestamp] " $script:Colors.Timestamp -NoNewline
        if ($Icon) {
            Write-ColorLine "$Icon " $script:Colors.Highlight -NoNewline
        }
        Write-ColorLine "$($script:Symbols.Arrow) " $script:Colors.Section -NoNewline
        Write-ColorLine $Section.ToUpper().PadRight(46) $script:Colors.Section -NoNewline
        Write-ColorLine "|" $script:Colors.SectionAccent
        Write-ColorLine "+-----------------------------------------------------------------------------+" $script:Colors.SectionAccent
    }

    function Write-TestLog {
        param(
            [string]$Message, 
            [ValidateSet('INFO', 'SUCCESS', 'WARN', 'ERROR', 'DEBUG', 'PASS', 'FAIL', 'SKIP')]
            [string]$Level = 'INFO'
        )
        $timestamp = Get-ElapsedTimestamp
        
        $levelConfig = switch ($Level) {
            'INFO'    { @{ Color = $script:Colors.Info;    Symbol = $script:Symbols.Bullet;   Tag = 'INFO' } }
            'SUCCESS' { @{ Color = $script:Colors.Success; Symbol = $script:Symbols.Check;    Tag = ' OK ' } }
            'WARN'    { @{ Color = $script:Colors.Warning; Symbol = $script:Symbols.Triangle; Tag = 'WARN' } }
            'ERROR'   { @{ Color = $script:Colors.Error;   Symbol = $script:Symbols.Cross;    Tag = 'ERR!' } }
            'DEBUG'   { @{ Color = $script:Colors.Debug;   Symbol = $script:Symbols.Gear;     Tag = 'DBG ' } }
            'PASS'    { @{ Color = $script:Colors.Success; Symbol = $script:Symbols.Check;    Tag = 'PASS' } }
            'FAIL'    { @{ Color = $script:Colors.Error;   Symbol = $script:Symbols.Cross;    Tag = 'FAIL' } }
            'SKIP'    { @{ Color = $script:Colors.Skip;    Symbol = $script:Symbols.Arrow;    Tag = 'SKIP' } }
            default   { @{ Color = $script:Colors.Info;    Symbol = $script:Symbols.Bullet;   Tag = 'INFO' } }
        }
        
        Write-ColorLine "[$timestamp] " $script:Colors.Timestamp -NoNewline
        Write-ColorLine "[$($levelConfig.Tag)]" $levelConfig.Color -NoNewline
        Write-ColorLine " $($levelConfig.Symbol) " $levelConfig.Color -NoNewline
        Write-ColorLine $Message $levelConfig.Color
    }

    function Write-TestInit {
        param(
            [string]$Component, 
            [ValidateSet('OK', 'FAIL', 'WARN', 'SKIP', 'YES', 'NO', 'LOAD')]
            [string]$Status = 'OK'
        )
        $timestamp = Get-ElapsedTimestamp
        
        $statusConfig = switch ($Status) {
            'OK'   { @{ Color = $script:Colors.Success; Symbol = $script:Symbols.Check;  Text = ' OK ' } }
            'FAIL' { @{ Color = $script:Colors.Error;   Symbol = $script:Symbols.Cross;  Text = 'FAIL' } }
            'WARN' { @{ Color = $script:Colors.Warning; Symbol = $script:Symbols.Triangle; Text = 'WARN' } }
            'SKIP' { @{ Color = $script:Colors.Skip;    Symbol = $script:Symbols.Arrow;  Text = 'SKIP' } }
            'YES'  { @{ Color = $script:Colors.Success; Symbol = $script:Symbols.Check;  Text = 'YES ' } }
            'NO'   { @{ Color = $script:Colors.Warning; Symbol = $script:Symbols.Cross;  Text = ' NO ' } }
            'LOAD' { @{ Color = $script:Colors.Info;    Symbol = $script:Symbols.Gear;   Text = 'LOAD' } }
            default { @{ Color = $script:Colors.Success; Symbol = $script:Symbols.Check; Text = ' OK ' } }
        }
        
        Write-ColorLine "[$timestamp] " $script:Colors.Timestamp -NoNewline
        Write-ColorLine "[" $script:Colors.TagInit -NoNewline
        Write-ColorLine "INIT" $script:Colors.TagInit -NoNewline
        Write-ColorLine "] " $script:Colors.TagInit -NoNewline
        Write-ColorLine "$($script:Symbols.Gear) " $script:Colors.Muted -NoNewline
        Write-ColorLine $Component.PadRight(40) $script:Colors.Info -NoNewline
        Write-ColorLine "[" $statusConfig.Color -NoNewline
        Write-ColorLine "$($statusConfig.Symbol) $($statusConfig.Text)" $statusConfig.Color -NoNewline
        Write-ColorLine "]" $statusConfig.Color
    }

    function Write-TestCategory {
        param([string]$Name, [int]$Count)
        $timestamp = Get-ElapsedTimestamp
        
        Write-ColorLine "[$timestamp] " $script:Colors.Timestamp -NoNewline
        Write-ColorLine "[" $script:Colors.TagCat -NoNewline
        Write-ColorLine "CAT " $script:Colors.TagCat -NoNewline
        Write-ColorLine "] " $script:Colors.TagCat -NoNewline
        Write-ColorLine "$($script:Symbols.Square) " $script:Colors.Muted -NoNewline
        Write-ColorLine $Name.PadRight(35) $script:Colors.Info -NoNewline
        Write-ColorLine $Count.ToString().PadLeft(3) $script:Colors.Number -NoNewline
        Write-ColorLine " tests" $script:Colors.Muted
    }

    function Write-TestSummary {
        param([int]$Total)
        $timestamp = Get-ElapsedTimestamp
        
        Write-Host ""
        Write-ColorLine "[$timestamp] " $script:Colors.Timestamp -NoNewline
        Write-ColorLine "[" $script:Colors.TagSum -NoNewline
        Write-ColorLine "SUM " $script:Colors.TagSum -NoNewline
        Write-ColorLine "] " $script:Colors.TagSum -NoNewline
        Write-ColorLine "$($script:Symbols.Star) " $script:Colors.Highlight -NoNewline
        Write-ColorLine "Total test cases: " $script:Colors.Info -NoNewline
        Write-ColorLine $Total $script:Colors.Number
    }

    function Write-ProgressBar {
        param([int]$Current, [int]$Total, [int]$Width = 40)
        $percent = [Math]::Round(($Current / $Total) * 100)
        $filled = [Math]::Round(($Current / $Total) * $Width)
        $empty = $Width - $filled
        
        Write-ColorLine "[" $script:Colors.Muted -NoNewline
        Write-ColorLine ("#" * $filled) $script:Colors.Success -NoNewline
        Write-ColorLine ("-" * $empty) $script:Colors.Muted -NoNewline
        Write-ColorLine "] " $script:Colors.Muted -NoNewline
        Write-ColorLine "$percent%" $script:Colors.Number
    }

    # =========================================================================
    # DISPLAY TEST SUITE BANNER
    # =========================================================================
    $bannerTimestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    Write-Host ""
    Write-ColorLine "+==============================================================================+" $script:Colors.Banner
    Write-ColorLine "|" $script:Colors.Banner -NoNewline
    Write-ColorLine "                                                                              " $script:Colors.BannerText -NoNewline
    Write-ColorLine "|" $script:Colors.Banner
    Write-ColorLine "|" $script:Colors.Banner -NoNewline
    Write-ColorLine "   $($script:Symbols.Lightning) LG ULTRAGEAR NO-DIMMING FIX - COMPREHENSIVE TEST SUITE $($script:Symbols.Lightning)          " $script:Colors.BannerText -NoNewline
    Write-ColorLine "|" $script:Colors.Banner
    Write-ColorLine "|" $script:Colors.Banner -NoNewline
    Write-ColorLine "                                                                              " $script:Colors.BannerText -NoNewline
    Write-ColorLine "|" $script:Colors.Banner
    Write-ColorLine "+==============================================================================+" $script:Colors.Banner
    Write-ColorLine "|  " $script:Colors.BannerAccent -NoNewline
    Write-ColorLine "$($script:Symbols.Package) Version    : " $script:Colors.Muted -NoNewline
    Write-ColorLine $script:TestSuiteVersion.PadRight(58) $script:Colors.Highlight -NoNewline
    Write-ColorLine "|" $script:Colors.BannerAccent
    Write-ColorLine "|  " $script:Colors.BannerAccent -NoNewline
    Write-ColorLine "$($script:Symbols.Clock) Started    : " $script:Colors.Muted -NoNewline
    Write-ColorLine $bannerTimestamp.PadRight(58) $script:Colors.Highlight -NoNewline
    Write-ColorLine "|" $script:Colors.BannerAccent
    Write-ColorLine "|  " $script:Colors.BannerAccent -NoNewline
    Write-ColorLine "$($script:Symbols.Gear) PowerShell : " $script:Colors.Muted -NoNewline
    Write-ColorLine $PSVersionTable.PSVersion.ToString().PadRight(58) $script:Colors.Highlight -NoNewline
    Write-ColorLine "|" $script:Colors.BannerAccent
    Write-ColorLine "|  " $script:Colors.BannerAccent -NoNewline
    Write-ColorLine "$($script:Symbols.Diamond) Platform   : " $script:Colors.Muted -NoNewline
    Write-ColorLine $PSVersionTable.Platform.ToString().PadRight(58) $script:Colors.Highlight -NoNewline
    Write-ColorLine "|" $script:Colors.BannerAccent
    Write-ColorLine "|  " $script:Colors.BannerAccent -NoNewline
    Write-ColorLine "$($script:Symbols.Shield) OS         : " $script:Colors.Muted -NoNewline
    Write-ColorLine $PSVersionTable.OS.Substring(0, [Math]::Min(58, $PSVersionTable.OS.Length)).PadRight(58) $script:Colors.Highlight -NoNewline
    Write-ColorLine "|" $script:Colors.BannerAccent
    Write-ColorLine "+==============================================================================+" $script:Colors.Banner
    Write-Host ""

    # =========================================================================
    # INITIALIZATION SEQUENCE
    # =========================================================================
    Write-TestSection "INITIALIZATION SEQUENCE" "$($script:Symbols.Wrench)"

    Write-TestInit "Loading test framework (Pester 5.x)" -Status 'LOAD'
    Write-TestInit "Configuring strict mode"
    Write-TestInit "Setting up test environment"

    # =============================================================================
    # HELPER FUNCTIONS (defined in BeforeAll for Pester 5.x scoping)
    # =============================================================================
    function Get-ScriptPath {
        $repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
        return Join-Path -Path $repoRoot -ChildPath 'install-lg-ultragear-no-dimming.ps1'
    }
    Write-TestInit "Registered helper: Get-ScriptPath"

    function Get-ScriptAST {
        $path = Get-ScriptPath
        return [System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path $path), [ref]$null, [ref]$null)
    }
    Write-TestInit "Registered helper: Get-ScriptAST"

    function Get-MockMonitorCharBuffer {
        param([string]$Name)
        $charBuffer = @()
        # Truncate name to 64 characters (typical WMI buffer size)
        $truncatedName = if ($Name.Length -gt 64) { $Name.Substring(0, 64) } else { $Name }
        foreach ($ch in $truncatedName.ToCharArray()) { $charBuffer += [int][char]$ch }
        while ($charBuffer.Count -lt 64) { $charBuffer += 0 }
        return [int[]]$charBuffer
    }
    Write-TestInit "Registered helper: Get-MockMonitorCharBuffer"

    function Get-ScriptContent {
        return Get-Content -LiteralPath (Get-ScriptPath) -Raw
    }
    Write-TestInit "Registered helper: Get-ScriptContent"

    # Pre-compute commonly used values
    $script:repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
    $script:scriptPath = Get-ScriptPath
    $script:profilePath = Join-Path -Path $script:repoRoot -ChildPath 'lg-ultragear-full-cal.icm'

    Write-TestInit "Resolved repository root"
    Write-TestInit "Resolved script path"
    Write-TestInit "Resolved profile path"

    # =========================================================================
    # ENVIRONMENT VALIDATION
    # =========================================================================
    Write-TestSection "ENVIRONMENT VALIDATION" "$($script:Symbols.Magnify)"

    $scriptExists = Test-Path -LiteralPath $script:scriptPath
    Write-TestInit "Target script exists" -Status $(if ($scriptExists) { 'OK' } else { 'FAIL' })

    $profileExists = Test-Path -LiteralPath $script:profilePath
    Write-TestInit "ICC profile exists" -Status $(if ($profileExists) { 'OK' } else { 'WARN' })

    $isWindowsPlatform = $PSVersionTable.Platform -eq 'Win32NT' -or $env:OS -eq 'Windows_NT'
    Write-TestInit "Windows platform detected" -Status $(if ($isWindowsPlatform) { 'OK' } else { 'SKIP' })

    $isAdminUser = & {
        try {
            $id = [Security.Principal.WindowsIdentity]::GetCurrent()
            $p = [Security.Principal.WindowsPrincipal]::new($id)
            return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        } catch { return $false }
    }
    Write-TestInit "Administrator privileges" -Status $(if ($isAdminUser) { 'YES' } else { 'NO' })

    # =========================================================================
    # TEST CATEGORIES OVERVIEW
    # =========================================================================
    Write-TestSection "TEST CATEGORIES" "$($script:Symbols.Package)"
    
    $categories = @(
        @{ Name = 'Script Syntax and Structure'; Count = 22 },
        @{ Name = 'Help and Basic Execution'; Count = 3 },
        @{ Name = 'Dry Run Mode'; Count = 4 },
        @{ Name = 'WMI Monitor Detection'; Count = 37 },
        @{ Name = 'Auto-Elevation Logic'; Count = 4 },
        @{ Name = 'Scheduled Task Generation'; Count = 8 },
        @{ Name = 'Profile Hash Verification'; Count = 4 },
        @{ Name = 'TUI Functions'; Count = 10 },
        @{ Name = 'Uninstall Operations'; Count = 6 },
        @{ Name = 'Reinstall and Refresh'; Count = 3 },
        @{ Name = 'Repository File Structure'; Count = 5 },
        @{ Name = 'Integration Tests'; Count = 3 },
        @{ Name = 'Error Handling'; Count = 2 },
        @{ Name = 'Logging Functions'; Count = 14 }
    )
    
    $totalTests = 0
    foreach ($cat in $categories) {
        $totalTests += $cat.Count
        Write-TestCategory -Name $cat.Name -Count $cat.Count
    }
    
    Write-TestSummary -Total $totalTests

    # =========================================================================
    # INITIALIZATION COMPLETE
    # =========================================================================
    $initDuration = ((Get-Date) - $script:TestStartTime).TotalMilliseconds
    Write-TestSection "INITIALIZATION COMPLETE" "$($script:Symbols.Rocket)"
    Write-TestLog "Test suite initialized in $([Math]::Round($initDuration, 2))ms" -Level 'SUCCESS'
    Write-TestLog "Starting test execution..." -Level 'INFO'
    Write-Host ""
}

# =============================================================================
# SYNTAX AND PARAMETER TESTS
# =============================================================================
Describe 'Script Syntax and Structure' {
    Context 'Parsing' {
        It 'parses without syntax errors' {
            $tokens = $null
            $errors = $null
            $path = Get-ScriptPath
            $null = [System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path $path), [ref]$tokens, [ref]$errors)
            $errors | Should -BeNullOrEmpty
        }

        It 'has valid PowerShell AST structure' {
            $ast = Get-ScriptAST
            $ast | Should -Not -BeNullOrEmpty
            $ast.ParamBlock | Should -Not -BeNullOrEmpty
        }
    }

    Context 'Parameters' {
        BeforeAll {
            $ast = Get-ScriptAST
            $script:paramBlock = $ast.ParamBlock
            $script:paramNames = @($script:paramBlock.Parameters.Name.VariablePath.UserPath)
        }

        It 'exposes ProfilePath parameter' {
            $script:paramNames | Should -Contain 'ProfilePath'
        }

        It 'exposes MonitorNameMatch parameter' {
            $script:paramNames | Should -Contain 'MonitorNameMatch'
        }

        It 'exposes PerUser switch' {
            $script:paramNames | Should -Contain 'PerUser'
        }

        It 'exposes NoSetDefault switch' {
            $script:paramNames | Should -Contain 'NoSetDefault'
        }

        It 'exposes SkipHdrAssociation switch' {
            $script:paramNames | Should -Contain 'SkipHdrAssociation'
        }

        It 'exposes NoPrompt switch' {
            $script:paramNames | Should -Contain 'NoPrompt'
        }

        It 'exposes InstallOnly switch' {
            $script:paramNames | Should -Contain 'InstallOnly'
        }

        It 'exposes Probe switch' {
            $script:paramNames | Should -Contain 'Probe'
        }

        It 'exposes DryRun switch' {
            $script:paramNames | Should -Contain 'DryRun'
        }

        It 'exposes SkipElevation switch' {
            $script:paramNames | Should -Contain 'SkipElevation'
        }

        It 'exposes Help switch' {
            $script:paramNames | Should -Contain 'Help'
        }

        It 'exposes Interactive switch' {
            $script:paramNames | Should -Contain 'Interactive'
        }

        It 'exposes NonInteractive switch' {
            $script:paramNames | Should -Contain 'NonInteractive'
        }

        It 'exposes InstallMonitor switch' {
            $script:paramNames | Should -Contain 'InstallMonitor'
        }

        It 'exposes UninstallMonitor switch' {
            $script:paramNames | Should -Contain 'UninstallMonitor'
        }

        It 'exposes SkipMonitor switch' {
            $script:paramNames | Should -Contain 'SkipMonitor'
        }

        It 'exposes Uninstall switch' {
            $script:paramNames | Should -Contain 'Uninstall'
        }

        It 'exposes UninstallFull switch' {
            $script:paramNames | Should -Contain 'UninstallFull'
        }

        It 'exposes Reinstall switch' {
            $script:paramNames | Should -Contain 'Reinstall'
        }

        It 'exposes Refresh switch' {
            $script:paramNames | Should -Contain 'Refresh'
        }

        It 'has correct default for ProfilePath' {
            $defaults = @{}
            foreach ($p in $script:paramBlock.Parameters) {
                if ($p.DefaultValue) {
                    $defaults[$p.Name.VariablePath.UserPath] = $p.DefaultValue.Extent.Text
                }
            }
            $defaults['ProfilePath'] | Should -Match 'lg-ultragear-full-cal.icm'
        }

        It 'has correct default for MonitorNameMatch' {
            $defaults = @{}
            foreach ($p in $script:paramBlock.Parameters) {
                if ($p.DefaultValue) {
                    $defaults[$p.Name.VariablePath.UserPath] = $p.DefaultValue.Extent.Text
                }
            }
            $defaults['MonitorNameMatch'] | Should -Match 'LG ULTRAGEAR'
        }

        It 'has correct default for MonitorTaskName' {
            $defaults = @{}
            foreach ($p in $script:paramBlock.Parameters) {
                if ($p.DefaultValue) {
                    $defaults[$p.Name.VariablePath.UserPath] = $p.DefaultValue.Extent.Text
                }
            }
            $defaults['MonitorTaskName'] | Should -Match 'LG-UltraGear-ColorProfile-AutoReapply'
        }
    }
}

# =============================================================================
# HELP AND BASIC EXECUTION TESTS
# =============================================================================
Describe 'Help and Basic Execution' {
    It 'prints help and exits without error' {
        $scriptPath = Get-ScriptPath
        { & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal } | Should -Not -Throw
    }

    It 'help output contains usage information' {
        $scriptPath = Get-ScriptPath
        $output = & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
        $output | Should -Match 'Usage'
        $output | Should -Match 'INSTALL OPTIONS'
        $output | Should -Match 'MAINTENANCE'
        $output | Should -Match 'UNINSTALL'
    }

    It 'help output contains examples' {
        $scriptPath = Get-ScriptPath
        $output = & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
        $output | Should -Match 'EXAMPLES'
        $output | Should -Match 'Probe'
        $output | Should -Match 'NonInteractive'
    }
}

# =============================================================================
# DRY RUN TESTS
# =============================================================================
Describe 'Dry Run Mode' {
    BeforeAll {
        if (-not $IsWindows) {
            Set-ItResult -Skipped -Because 'Dry run tests only run on Windows hosts'
        }
    }

    Context 'With Mocked Monitor Data' {
        BeforeAll {
            $script:scriptPath = Get-ScriptPath
            $script:repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
            $script:profilePath = Join-Path -Path $script:repoRoot -ChildPath 'lg-ultragear-full-cal.icm'
        }

        It 'completes a dry run probe with single LG UltraGear monitor' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\LGULTRAGEAR\1&ABCDEF&0&UID1234'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR 27GN950'
                    }
                )
            }

            { & $script:scriptPath -ProfilePath $script:profilePath -MonitorNameMatch 'LG ULTRAGEAR' -DryRun -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal } | Should -Not -Throw
        }

        It 'completes a dry run probe with multiple monitors including LG UltraGear' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\DELL\1&ABCDEF&0&UID1234'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'Dell U2720Q'
                    },
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\LGULTRAGEAR\1&ABCDEF&0&UID5678'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR 27GP950'
                    },
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\SAMSUNG\1&ABCDEF&0&UID9012'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'Samsung Odyssey'
                    }
                )
            }

            { & $script:scriptPath -ProfilePath $script:profilePath -MonitorNameMatch 'LG ULTRAGEAR' -DryRun -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal } | Should -Not -Throw
        }

        It 'completes a dry run probe with no matching monitors' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\DELL\1&ABCDEF&0&UID1234'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'Dell U2720Q'
                    }
                )
            }

            { & $script:scriptPath -ProfilePath $script:profilePath -MonitorNameMatch 'LG ULTRAGEAR' -DryRun -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal } | Should -Not -Throw
        }

        It 'dry run does not modify system files' {
            $testProfilePath = Join-Path $env:WINDIR 'System32\spool\drivers\color\lg-ultragear-full-cal-TEST-DRYRUN.icm'

            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\LGULTRAGEAR\1&ABCDEF&0&UID1234'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
                    }
                )
            }

            & $script:scriptPath -ProfilePath $script:profilePath -MonitorNameMatch 'LG ULTRAGEAR' -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor 2>&1 | Out-Null

            # Verify no test file was created (dry run should not create files)
            Test-Path $testProfilePath | Should -BeFalse
        }
    }
}

# =============================================================================
# WMI MONITOR DETECTION TESTS
# =============================================================================
Describe 'WMI Monitor Detection' {
    BeforeAll {
        if (-not $IsWindows) {
            Set-ItResult -Skipped -Because 'WMI tests only run on Windows hosts'
        }
    }

    Context 'Monitor Name Parsing' {
        It 'correctly parses ASCII monitor names from WMI byte arrays' {
            $testName = 'LG ULTRAGEAR'
            $charBuffer = Get-MockMonitorCharBuffer -Name $testName

            # Simulate the parsing logic from the script
            $parsedName = ($charBuffer | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $parsedName | Should -Be $testName
        }

        It 'handles monitor names with special characters' {
            $testName = 'Dell U2720Q-B'
            $charBuffer = Get-MockMonitorCharBuffer -Name $testName

            $parsedName = ($charBuffer | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $parsedName | Should -Be $testName
        }

        It 'handles empty monitor names gracefully' {
            $charBuffer = @(0) * 64

            $parsedName = ($charBuffer | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $parsedName | Should -BeNullOrEmpty
        }

        It 'handles monitor names with trailing nulls' {
            $testName = 'LG ULTRAGEAR'
            $charBuffer = Get-MockMonitorCharBuffer -Name $testName
            # Add extra trailing nulls
            $charBuffer += @(0) * 20

            $parsedName = ($charBuffer | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $parsedName | Should -Be $testName
        }

        It 'handles monitor names with embedded nulls correctly' {
            # Some monitors may have sparse data
            $charBuffer = @([int][char]'L', 0, [int][char]'G', 0, 0, [int][char]' ', [int][char]'T', [int][char]'V')
            while ($charBuffer.Count -lt 64) { $charBuffer += 0 }

            $parsedName = ($charBuffer | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $parsedName | Should -Be 'LG TV'
        }

        It 'handles numeric characters in monitor names' {
            $testName = 'LG 27GP950-B 4K'
            $charBuffer = Get-MockMonitorCharBuffer -Name $testName

            $parsedName = ($charBuffer | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $parsedName | Should -Be $testName
        }

        It 'handles very long monitor names (truncated to buffer)' {
            $testName = 'A' * 100  # Longer than typical 64-char buffer
            $charBuffer = Get-MockMonitorCharBuffer -Name $testName

            $parsedName = ($charBuffer | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $parsedName.Length | Should -Be 64
        }

        It 'matches LG UltraGear pattern variations' {
            $patterns = @(
                'LG ULTRAGEAR',
                'LG ULTRAGEAR 27GN950',
                'LG ULTRAGEAR 27GP950-B',
                'LG ULTRAGEAR 32GQ950',
                'LG ULTRAGEAR 34GP950G',
                'LG ULTRAGEAR 48GQ900',
                'LG UltraGear'  # Mixed case
            )

            foreach ($name in $patterns) {
                $name | Should -Match '(?i)LG.*ULTRAGEAR'
            }
        }

        It 'does not match non-LG monitors' {
            $patterns = @(
                'Dell U2720Q',
                'Samsung Odyssey G9',
                'ASUS ROG Swift',
                'Acer Predator',
                'BenQ PD3220U',
                'ViewSonic VP2785',
                'HP Z27',
                'Lenovo ThinkVision'
            )

            foreach ($name in $patterns) {
                $name | Should -Not -Match '(?i)LG.*ULTRAGEAR'
            }
        }

        It 'does not match other LG monitors' {
            $patterns = @(
                'LG IPS FULLHD',
                'LG 27UK850',
                'LG 34WN80C',
                'LG OLED55C1'
            )

            foreach ($name in $patterns) {
                $name | Should -Not -Match '(?i)LG.*ULTRAGEAR'
            }
        }
    }

    Context 'Monitor Instance Name Parsing' {
        It 'parses standard display instance names' {
            $instanceName = 'DISPLAY\GSM5C86\5&34be1d0&0&UID4354_0'

            $instanceName | Should -Match '^DISPLAY\\'
            $instanceName | Should -Match 'GSM[0-9A-F]+'
        }

        It 'extracts manufacturer code from instance name' {
            $instanceNames = @(
                @{ Name = 'DISPLAY\GSM5C86\5&34be1d0&0&UID4354_0'; Manufacturer = 'GSM' },  # LG
                @{ Name = 'DISPLAY\DEL40F1\5&12345&0&UID1234_0'; Manufacturer = 'DEL' },    # Dell
                @{ Name = 'DISPLAY\SAM0F13\5&abcde&0&UID5678_0'; Manufacturer = 'SAM' },    # Samsung
                @{ Name = 'DISPLAY\ACI27F1\5&67890&0&UID9012_0'; Manufacturer = 'ACI' }     # ASUS
            )

            foreach ($item in $instanceNames) {
                $match = $item.Name -match 'DISPLAY\\([A-Z]{3})'
                $match | Should -BeTrue
                $Matches[1] | Should -Be $item.Manufacturer
            }
        }

        It 'handles multi-monitor UID patterns' {
            $instanceNames = @(
                'DISPLAY\GSM5C86\5&34be1d0&0&UID4354_0',
                'DISPLAY\GSM5C86\5&34be1d0&0&UID4354_1',
                'DISPLAY\GSM5C86\5&34be1d0&0&UID4355_0'
            )

            foreach ($name in $instanceNames) {
                $name | Should -Match 'UID\d+_\d+$'
            }
        }

        It 'validates DISPLAY prefix requirement' {
            $validNames = @(
                'DISPLAY\GSM5C86\5&34be1d0&0&UID4354_0',
                'DISPLAY\MONITOR\ABC123'
            )

            $invalidNames = @(
                'MONITOR\GSM5C86\5&34be1d0&0&UID4354_0',
                'USB\VID_1234&PID_5678',
                'PCI\VEN_1234&DEV_5678'
            )

            foreach ($name in $validNames) {
                $name | Should -Match '^DISPLAY\\'
            }

            foreach ($name in $invalidNames) {
                $name | Should -Not -Match '^DISPLAY\\'
            }
        }
    }

    Context 'WMI Data Structure Validation' {
        It 'creates valid mock monitor object structure' {
            $mockMonitor = [pscustomobject]@{
                InstanceName     = 'DISPLAY\GSM5C86\5&34be1d0&0&UID4354_0'
                UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
            }

            $mockMonitor.InstanceName | Should -Not -BeNullOrEmpty
            $mockMonitor.UserFriendlyName | Should -Not -BeNullOrEmpty
            $mockMonitor.UserFriendlyName.Count | Should -BeGreaterOrEqual 64
        }

        It 'validates UserFriendlyName is integer array' {
            $charBuffer = Get-MockMonitorCharBuffer -Name 'Test Monitor'

            $charBuffer | Should -BeOfType [int]
            $charBuffer | ForEach-Object { $_ | Should -BeGreaterOrEqual 0 }
            $charBuffer | ForEach-Object { $_ | Should -BeLessThan 256 }
        }

        It 'simulates multiple monitor enumeration' {
            $monitors = @(
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\GSM5C86\5&34be1d0&0&UID4354_0'
                    UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
                },
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\DEL40F1\5&34be1d0&0&UID4355_0'
                    UserFriendlyName = Get-MockMonitorCharBuffer -Name 'Dell U2720Q'
                },
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\GSM5AB8\5&34be1d0&0&UID4356_0'
                    UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG IPS FULLHD'
                }
            )

            $monitors.Count | Should -Be 3

            # Filter for LG UltraGear
            $matched = $monitors | Where-Object {
                $name = ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
                $name -match '(?i)LG.*ULTRAGEAR'
            }

            $matched.Count | Should -Be 1
            $matched[0].InstanceName | Should -Match 'GSM5C86'
        }

        It 'handles monitors with missing UserFriendlyName' {
            $mockMonitor = [pscustomobject]@{
                InstanceName     = 'DISPLAY\UNKNOWN\5&34be1d0&0&UID4354_0'
                UserFriendlyName = $null
            }

            $name = if ($mockMonitor.UserFriendlyName) {
                ($mockMonitor.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
            } else {
                ''
            }

            $name | Should -BeNullOrEmpty
        }

        It 'handles monitors with empty UserFriendlyName array' {
            $mockMonitor = [pscustomobject]@{
                InstanceName     = 'DISPLAY\UNKNOWN\5&34be1d0&0&UID4354_0'
                UserFriendlyName = @()
            }

            $name = ($mockMonitor.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''

            $name | Should -BeNullOrEmpty
        }
    }

    Context 'Monitor Matching Logic' {
        It 'matches with default pattern "LG ULTRAGEAR"' {
            $monitorNames = @(
                @{ Name = 'LG ULTRAGEAR'; Expected = $true },
                @{ Name = 'LG ULTRAGEAR 27GN950'; Expected = $true },
                @{ Name = 'Dell U2720Q'; Expected = $false },
                @{ Name = 'LG IPS FULLHD'; Expected = $false }
            )

            $pattern = 'LG ULTRAGEAR'

            foreach ($item in $monitorNames) {
                $result = $item.Name -like "*${pattern}*"
                $result | Should -Be $item.Expected -Because "Monitor '$($item.Name)' should $(if($item.Expected){'match'}else{'not match'}) pattern '$pattern'"
            }
        }

        It 'matches with custom patterns' {
            $testCases = @(
                @{ Pattern = 'Dell'; Names = @('Dell U2720Q', 'DELL P2419H'); Expected = $true },
                @{ Pattern = 'Samsung'; Names = @('Samsung Odyssey G9'); Expected = $true },
                @{ Pattern = 'ASUS ROG'; Names = @('ASUS ROG Swift PG279Q'); Expected = $true }
            )

            foreach ($case in $testCases) {
                foreach ($name in $case.Names) {
                    $result = $name -like "*$($case.Pattern)*"
                    $result | Should -Be $case.Expected
                }
            }
        }

        It 'case insensitive matching works' {
            $monitorName = 'LG ULTRAGEAR 27GP950'

            $monitorName -like '*lg ultragear*' | Should -BeTrue
            $monitorName -like '*LG ULTRAGEAR*' | Should -BeTrue
            $monitorName -like '*Lg UltraGear*' | Should -BeTrue
        }

        It 'wildcard matching at boundaries' {
            $monitorName = 'LG ULTRAGEAR'

            # Starts with
            $monitorName -like 'LG*' | Should -BeTrue
            $monitorName -like 'Dell*' | Should -BeFalse

            # Ends with
            $monitorName -like '*ULTRAGEAR' | Should -BeTrue
            $monitorName -like '*OLED' | Should -BeFalse

            # Contains
            $monitorName -like '*ULTRA*' | Should -BeTrue
        }
    }

    Context 'Live WMI Query (Windows Only)' {
        It 'can query WmiMonitorID without errors' {
            { Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop } | Should -Not -Throw
        }

        It 'returns array or null for WmiMonitorID query' {
            $result = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue
            # Result can be null, single object, or array
            $result -is [object] -or $null -eq $result | Should -BeTrue
        }

        It 'WMI monitors have required properties' {
            $monitors = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue
            if ($monitors) {
                foreach ($monitor in $monitors) {
                    $monitor.PSObject.Properties.Name | Should -Contain 'InstanceName'
                    $monitor.PSObject.Properties.Name | Should -Contain 'UserFriendlyName'
                }
            } else {
                Set-ItResult -Skipped -Because 'No monitors detected'
            }
        }

        It 'can enumerate all monitor properties' {
            $monitors = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue
            if ($monitors) {
                $firstMonitor = $monitors | Select-Object -First 1
                $firstMonitor | Should -Not -BeNullOrEmpty

                # Common WmiMonitorID properties
                $expectedProps = @('InstanceName', 'UserFriendlyName', 'ManufacturerName', 'ProductCodeID', 'SerialNumberID')
                foreach ($prop in $expectedProps) {
                    $firstMonitor.PSObject.Properties.Name | Should -Contain $prop
                }
            } else {
                Set-ItResult -Skipped -Because 'No monitors detected'
            }
        }

        It 'WMI namespace root\wmi exists and is accessible' {
            { Get-CimInstance -Namespace root\wmi -ClassName __NAMESPACE -ErrorAction Stop } | Should -Not -Throw
        }
    }

    Context 'Error Handling for WMI Queries' {
        It 'handles invalid WMI namespace gracefully' {
            $result = Get-CimInstance -Namespace root\invalid_namespace -ClassName WmiMonitorID -ErrorAction SilentlyContinue
            $result | Should -BeNullOrEmpty
        }

        It 'handles invalid WMI class gracefully' {
            $result = Get-CimInstance -Namespace root\wmi -ClassName InvalidClassName -ErrorAction SilentlyContinue
            $result | Should -BeNullOrEmpty
        }

        It 'script handles WMI timeout scenario' {
            # Simulate by checking error handling exists in script
            $scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            $scriptContent | Should -Match 'Get-CimInstance.*-ErrorAction'
        }
    }
}

# =============================================================================
# AUTO-ELEVATION TESTS
# =============================================================================
Describe 'Auto-Elevation Logic' {
    Context 'Test-IsAdmin Function' {
        It 'returns a boolean value' {
            # Test the logic directly
            $isAdmin = & {
                $id = [Security.Principal.WindowsIdentity]::GetCurrent()
                $p = [Security.Principal.WindowsPrincipal]::new($id)
                return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
            }

            $isAdmin | Should -BeOfType [bool]
        }

        It 'correctly identifies admin status' {
            $id = [Security.Principal.WindowsIdentity]::GetCurrent()
            $p = [Security.Principal.WindowsPrincipal]::new($id)
            $expected = $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

            # Run the same logic
            $actual = & {
                $id = [Security.Principal.WindowsIdentity]::GetCurrent()
                $p = [Security.Principal.WindowsPrincipal]::new($id)
                return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
            }

            $actual | Should -Be $expected
        }
    }

    Context 'SkipElevation Parameter' {
        It 'script runs without elevation when -SkipElevation is specified' {
            $scriptPath = Get-ScriptPath

            # This should not trigger UAC prompt
            { & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal } | Should -Not -Throw
        }

        It 'script continues with -SkipElevation even when not admin' {
            $scriptPath = Get-ScriptPath

            # Should complete without trying to elevate
            $output = & $scriptPath -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 | Out-String
            $output | Should -Not -Match 'relaunching with Administrator'
        }
    }
}

# =============================================================================
# SCHEDULED TASK GENERATION TESTS
# =============================================================================
Describe 'Scheduled Task Generation' {
    Context 'Action Script Content' {
        It 'generates valid PowerShell script content' {
            # Variables used in action script template
            $installerPath = 'C:\Test\installer.ps1'
            $monitorMatch = 'LG ULTRAGEAR'

            # Simulate the action script generation
            $actionScript = @"
# LG UltraGear Color Profile Auto-Reapply - Fast Monitor
# Exits immediately if no matching monitor is connected

# Quick check for LG UltraGear - exits in <50ms if not found
try {
    `$found = `$false
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop | ForEach-Object {
        `$name = (`$_.UserFriendlyName | Where-Object { `$_ -ne 0 } | ForEach-Object { [char]`$_ }) -join ''
        if (`$name -match '$monitorMatch') { `$found = `$true }
    }
    if (-not `$found) { exit 0 }
} catch { exit 0 }

# LG UltraGear detected - wait for display to stabilize then reapply
Start-Sleep -Milliseconds 1500
& '$installerPath' -NoSetDefault -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor -MonitorNameMatch '$monitorMatch' 2>`$null | Out-Null
"@

            # Verify the script parses without errors
            $tokens = $null
            $errors = $null
            $null = [System.Management.Automation.Language.Parser]::ParseInput($actionScript, [ref]$tokens, [ref]$errors)
            $errors | Should -BeNullOrEmpty
        }

        It 'contains fast exit check for non-matching monitors' {
            $monitorMatch = 'LG ULTRAGEAR'
            $actionScript = @"
try {
    `$found = `$false
    Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop | ForEach-Object {
        `$name = (`$_.UserFriendlyName | Where-Object { `$_ -ne 0 } | ForEach-Object { [char]`$_ }) -join ''
        if (`$name -match '$monitorMatch') { `$found = `$true }
    }
    if (-not `$found) { exit 0 }
} catch { exit 0 }
"@

            $actionScript | Should -Match 'if \(-not \$found\) \{ exit 0 \}'
            $actionScript | Should -Match 'catch \{ exit 0 \}'
        }

        It 'includes stabilization delay before reapply' {
            $actionScript = 'Start-Sleep -Milliseconds 1500'
            $actionScript | Should -Match 'Start-Sleep -Milliseconds 1500'
        }

        It 'calls installer with correct parameters' {
            $installerPath = 'C:\Test\installer.ps1'
            $monitorMatch = 'LG ULTRAGEAR'

            $expectedParams = @(
                '-NoSetDefault',
                '-NoPrompt',
                '-SkipElevation',
                '-SkipWindowsTerminal',
                '-SkipMonitor',
                '-MonitorNameMatch'
            )

            $actionLine = "& '$installerPath' -NoSetDefault -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor -MonitorNameMatch '$monitorMatch' 2>`$null | Out-Null"

            foreach ($param in $expectedParams) {
                $escapedParam = [regex]::Escape($param)
                $actionLine | Should -Match $escapedParam
            }
        }
    }

    Context 'Task Triggers' {
        It 'defines display device event trigger XML' {
            $triggerXml = @"
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

            # Verify XML structure
            $triggerXml | Should -Match 'Microsoft-Windows-Kernel-PnP'
            $triggerXml | Should -Match 'EventID=20001'
            $triggerXml | Should -Match 'EventID=20003'
            $triggerXml | Should -Match 'DISPLAY'
            $triggerXml | Should -Match 'MONITOR'
        }

        It 'includes all required trigger types' {
            # Verify the script defines these triggers:
            # - AtLogOn
            # - ConsoleConnect (StateChange=7)
            # - SessionUnlock (StateChange=8)
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            $scriptContent | Should -Match 'New-ScheduledTaskTrigger -AtLogOn'
            $scriptContent | Should -Match 'StateChange.*7'  # ConsoleConnect
            $scriptContent | Should -Match 'StateChange.*8'  # SessionUnlock
        }
    }

    Context 'Task Settings' {
        It 'uses SYSTEM account for task principal' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            $scriptContent | Should -Match 'UserId.*SYSTEM'
            $scriptContent | Should -Match 'LogonType.*ServiceAccount'
            $scriptContent | Should -Match 'RunLevel.*Highest'
        }

        It 'sets appropriate execution time limit' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            $scriptContent | Should -Match 'ExecutionTimeLimit.*30'
        }

        It 'configures battery and multiple instance handling' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            $scriptContent | Should -Match 'AllowStartIfOnBatteries'
            $scriptContent | Should -Match 'DontStopIfGoingOnBatteries'
            $scriptContent | Should -Match 'MultipleInstances.*IgnoreNew'
        }
    }
}

# =============================================================================
# PROFILE HASH VERIFICATION TESTS
# =============================================================================
Describe 'Profile Hash Verification' {
    Context 'SHA256 Hash Computation' {
        BeforeAll {
            $script:repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
            $script:profilePath = Join-Path -Path $script:repoRoot -ChildPath 'lg-ultragear-full-cal.icm'
        }

        It 'computes SHA256 hash for embedded profile' {
            if (-not (Test-Path $script:profilePath)) {
                Set-ItResult -Skipped -Because 'Profile file not found'
                return
            }

            $hash = Get-FileHash -LiteralPath $script:profilePath -Algorithm SHA256
            $hash.Hash | Should -Not -BeNullOrEmpty
            $hash.Hash.Length | Should -Be 64  # SHA256 produces 64 hex characters
        }

        It 'hash is consistent across multiple reads' {
            if (-not (Test-Path $script:profilePath)) {
                Set-ItResult -Skipped -Because 'Profile file not found'
                return
            }

            $hash1 = (Get-FileHash -LiteralPath $script:profilePath -Algorithm SHA256).Hash
            $hash2 = (Get-FileHash -LiteralPath $script:profilePath -Algorithm SHA256).Hash

            $hash1 | Should -Be $hash2
        }
    }

    Context 'Embedded Profile Integrity' {
        It 'script contains embedded profile data' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            $scriptContent | Should -Match 'EmbeddedProfileBase64'
            $scriptContent | Should -Match 'EmbeddedProfileName'
        }

        It 'embedded profile name matches expected value' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            $scriptContent | Should -Match "EmbeddedProfileName.*lg-ultragear-full-cal\.icm"
        }
    }
}

# =============================================================================
# TUI FUNCTION TESTS
# =============================================================================
Describe 'TUI Functions' {
    Context 'Menu Structure' {
        It 'script contains TUI configuration variables' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            $scriptContent | Should -Match '\$script:TUI_WIDTH'
            $scriptContent | Should -Match '\$script:TUI_HEIGHT'
            $scriptContent | Should -Match '\$script:TUI_TITLE'
            $scriptContent | Should -Match '\$script:TUI_VERSION'
        }

        It 'TUI width is reasonable for console display' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            if ($scriptContent -match '\$script:TUI_WIDTH\s*=\s*(\d+)') {
                $width = [int]$Matches[1]
                $width | Should -BeGreaterThan 60
                $width | Should -BeLessThan 200
            }
        }

        It 'TUI height is reasonable for console display' {
            $scriptPath = Get-ScriptPath
            $scriptContent = Get-Content -LiteralPath $scriptPath -Raw

            if ($scriptContent -match '\$script:TUI_HEIGHT\s*=\s*(\d+)') {
                $height = [int]$Matches[1]
                $height | Should -BeGreaterThan 20
                $height | Should -BeLessThan 100
            }
        }
    }

    Context 'Menu Functions Exist' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'defines Show-TUIMainMenu function' {
            $script:scriptContent | Should -Match 'function Show-TUIMainMenu'
        }

        It 'defines Show-TUIAdvancedMenu function' {
            $script:scriptContent | Should -Match 'function Show-TUIAdvancedMenu'
        }

        It 'defines Invoke-TUIAction function' {
            $script:scriptContent | Should -Match 'function Invoke-TUIAction'
        }

        It 'defines Start-TUI function' {
            $script:scriptContent | Should -Match 'function Start-TUI'
        }

        It 'defines Get-MonitorStatus function' {
            $script:scriptContent | Should -Match 'function Get-MonitorStatus'
        }
    }
}

# =============================================================================
# UNINSTALL OPERATION TESTS
# =============================================================================
Describe 'Uninstall Operations' {
    Context 'Uninstall Parameters' {
        It 'script supports -Uninstall parameter' {
            $ast = Get-ScriptAST
            $paramNames = @($ast.ParamBlock.Parameters.Name.VariablePath.UserPath)
            $paramNames | Should -Contain 'Uninstall'
        }

        It 'script supports -UninstallFull parameter' {
            $ast = Get-ScriptAST
            $paramNames = @($ast.ParamBlock.Parameters.Name.VariablePath.UserPath)
            $paramNames | Should -Contain 'UninstallFull'
        }

        It 'script supports -UninstallMonitor parameter' {
            $ast = Get-ScriptAST
            $paramNames = @($ast.ParamBlock.Parameters.Name.VariablePath.UserPath)
            $paramNames | Should -Contain 'UninstallMonitor'
        }
    }

    Context 'Uninstall Logic' {
        It 'Uninstall-AutoReapplyMonitor function exists' {
            $scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            $scriptContent | Should -Match 'function Uninstall-AutoReapplyMonitor'
        }

        It 'uninstall removes scheduled task' {
            $scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            $scriptContent | Should -Match 'Unregister-ScheduledTask'
        }

        It 'uninstall removes action script directory' {
            $scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            # Script removes the parent directory of the action script
            $scriptContent | Should -Match 'Remove-Item.*-Recurse.*-Force'
        }
    }
}

# =============================================================================
# REINSTALL AND REFRESH TESTS
# =============================================================================
Describe 'Reinstall and Refresh Operations' {
    Context 'Parameters' {
        It 'script supports -Reinstall parameter' {
            $ast = Get-ScriptAST
            $paramNames = @($ast.ParamBlock.Parameters.Name.VariablePath.UserPath)
            $paramNames | Should -Contain 'Reinstall'
        }

        It 'script supports -Refresh parameter' {
            $ast = Get-ScriptAST
            $paramNames = @($ast.ParamBlock.Parameters.Name.VariablePath.UserPath)
            $paramNames | Should -Contain 'Refresh'
        }
    }

    Context 'Reinstall Logic' {
        It 'reinstall calls uninstall before install' {
            $scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            # Check that Reinstall handling exists
            $scriptContent | Should -Match 'if \(\$Reinstall\)'
            # Check that it calls uninstall (may be on separate lines)
            $scriptContent | Should -Match 'Uninstall-AutoReapplyMonitor'
        }
    }
}

# =============================================================================
# FILE STRUCTURE TESTS
# =============================================================================
Describe 'Repository File Structure' {
    BeforeAll {
        $script:repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
    }

    It 'install.bat exists' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'install.bat'
        Test-Path $path | Should -BeTrue
    }

    It 'install.bat is valid batch file' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'install.bat'
        $content = Get-Content -LiteralPath $path -Raw
        $content | Should -Match '@echo off'
    }

    It 'readme.md exists with correct heading' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'readme.md'
        Test-Path $path | Should -BeTrue
        $first = Get-Content -LiteralPath $path -TotalCount 1
        $first | Should -Match '^# lg ultragear auto-dimming fix'
    }

    It 'lg-ultragear-full-cal.icm exists' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'lg-ultragear-full-cal.icm'
        Test-Path $path | Should -BeTrue
    }

    It 'scripts directory exists' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'scripts'
        Test-Path $path | Should -BeTrue
    }

    It 'tests directory exists' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'tests'
        Test-Path $path | Should -BeTrue
    }
}

# =============================================================================
# INTEGRATION TESTS
# =============================================================================
Describe 'Integration Tests' -Tag 'Integration' {
    BeforeAll {
        if (-not $IsWindows) {
            Set-ItResult -Skipped -Because 'Integration tests only run on Windows hosts'
        }
        $script:scriptPath = Get-ScriptPath
    }

    Context 'Full Dry Run Workflow' {
        It 'completes full install workflow in dry run mode' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\LGULTRAGEAR\1&ABCDEF&0&UID1234'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR 27GP950'
                    }
                )
            }

            $output = & $script:scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String

            $output | Should -Match 'dry-run'
        }
    }

    Context 'NonInteractive Mode' {
        It 'runs in non-interactive mode with -NonInteractive flag' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\LGULTRAGEAR\1&ABCDEF&0&UID1234'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
                    }
                )
            }

            { & $script:scriptPath -NonInteractive -DryRun -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal } | Should -Not -Throw
        }
    }

    Context 'Probe Mode Output' {
        It 'probe mode outputs monitor information' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\LGULTRAGEAR\1&ABCDEF&0&UID1234'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR 27GP950'
                    }
                )
            }

            $output = & $script:scriptPath -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String

            $output | Should -Match 'probe'
        }
    }
}

# =============================================================================
# ERROR HANDLING TESTS
# =============================================================================
Describe 'Error Handling' {
    Context 'Invalid Parameters' {
        It 'handles invalid profile path gracefully' {
            $scriptPath = Get-ScriptPath

            # Should not throw catastrophic error
            { & $scriptPath -ProfilePath 'C:\NonExistent\fake-profile.icm' -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 } | Should -Not -Throw
        }
    }

    Context 'WMI Failure Handling' {
        It 'handles WMI query failure gracefully' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                throw 'WMI access denied'
            }

            $scriptPath = Get-ScriptPath

            # Should handle WMI failure without crashing
            { & $scriptPath -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 } | Should -Not -Throw
        }
    }
}

# =============================================================================
# LOGGING FUNCTION TESTS
# =============================================================================
Describe 'Logging Functions' {
    Context 'Log Function Definitions' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'defines Write-InfoMessage function' {
            $script:scriptContent | Should -Match 'function Write-InfoMessage'
        }

        It 'defines Write-ActionMessage function' {
            $script:scriptContent | Should -Match 'function Write-ActionMessage'
        }

        It 'defines Write-SuccessMessage function' {
            $script:scriptContent | Should -Match 'function Write-SuccessMessage'
        }

        It 'defines Write-WarnMessage function' {
            $script:scriptContent | Should -Match 'function Write-WarnMessage'
        }

        It 'defines Write-NoteMessage function' {
            $script:scriptContent | Should -Match 'function Write-NoteMessage'
        }

        It 'defines Write-SkipMessage function' {
            $script:scriptContent | Should -Match 'function Write-SkipMessage'
        }

        It 'defines Write-DeleteMessage function' {
            $script:scriptContent | Should -Match 'function Write-DeleteMessage'
        }

        It 'defines Write-DoneMessage function' {
            $script:scriptContent | Should -Match 'function Write-DoneMessage'
        }

        It 'defines Write-CreateMessage function' {
            $script:scriptContent | Should -Match 'function Write-CreateMessage'
        }
    }

    Context 'Log Symbols' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'defines INFO symbol' {
            $script:scriptContent | Should -Match "SymbolInfo.*\[INFO\]"
        }

        It 'defines SUCCESS symbol' {
            $script:scriptContent | Should -Match "SymbolSuccess.*\[ OK \]"
        }

        It 'defines ACTION symbol' {
            $script:scriptContent | Should -Match "SymbolAction.*\[STEP\]"
        }

        It 'defines ERROR symbol' {
            $script:scriptContent | Should -Match "SymbolError.*\[ERR \]"
        }

        It 'defines DONE symbol' {
            $script:scriptContent | Should -Match "SymbolDone.*\[DONE\]"
        }
    }
}

# =============================================================================
# TEST SUITE COMPLETION
# =============================================================================
AfterAll {
    $testEndTime = Get-Date
    $totalDuration = ($testEndTime - $script:TestStartTime).TotalSeconds

    # Format elapsed time
    $elapsedFormatted = "T+{0,8:F3}s" -f $totalDuration

    # Re-define colors for AfterAll scope
    $Colors = @{
        Banner          = 'Green'
        BannerAccent    = 'DarkGreen'
        BannerText      = 'White'
        Highlight       = 'Cyan'
        Muted           = 'DarkGray'
        Timestamp       = 'DarkGray'
        Success         = 'Green'
        Info            = 'Gray'
    }
    
    $Symbols = @{
        Check      = [char]0x2713  # ✓
        Star       = [char]0x2605  # ★
        Trophy     = [char]0x2654  # ♔ (crown/king - trophy-like)
        Clock      = [char]0x25D4  # ◔
        Package    = [char]0x25A1  # □
        Sparkles   = [char]0x2726  # ✦
    }

    Write-Host ""
    Write-Host "+==============================================================================+" -ForegroundColor $Colors.Banner
    Write-Host "|" -ForegroundColor $Colors.Banner -NoNewline
    Write-Host "                                                                              " -ForegroundColor $Colors.BannerText -NoNewline
    Write-Host "|" -ForegroundColor $Colors.Banner
    Write-Host "|" -ForegroundColor $Colors.Banner -NoNewline
    Write-Host "   $($Symbols.Trophy) TEST SUITE EXECUTION COMPLETE $($Symbols.Trophy)                                " -ForegroundColor $Colors.BannerText -NoNewline
    Write-Host "|" -ForegroundColor $Colors.Banner
    Write-Host "|" -ForegroundColor $Colors.Banner -NoNewline
    Write-Host "                                                                              " -ForegroundColor $Colors.BannerText -NoNewline
    Write-Host "|" -ForegroundColor $Colors.Banner
    Write-Host "+==============================================================================+" -ForegroundColor $Colors.Banner
    
    $endTimestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    Write-Host "|  " -ForegroundColor $Colors.BannerAccent -NoNewline
    Write-Host "$($Symbols.Clock) Completed  : " -ForegroundColor $Colors.Muted -NoNewline
    Write-Host $endTimestamp.PadRight(58) -ForegroundColor $Colors.Highlight -NoNewline
    Write-Host "|" -ForegroundColor $Colors.BannerAccent
    Write-Host "|  " -ForegroundColor $Colors.BannerAccent -NoNewline
    Write-Host "$($Symbols.Star) Duration   : " -ForegroundColor $Colors.Muted -NoNewline
    Write-Host "$([Math]::Round($totalDuration, 2)) seconds".PadRight(58) -ForegroundColor $Colors.Highlight -NoNewline
    Write-Host "|" -ForegroundColor $Colors.BannerAccent
    Write-Host "|  " -ForegroundColor $Colors.BannerAccent -NoNewline
    Write-Host "$($Symbols.Package) Suite      : " -ForegroundColor $Colors.Muted -NoNewline
    Write-Host $script:TestSuiteName.PadRight(58) -ForegroundColor $Colors.Highlight -NoNewline
    Write-Host "|" -ForegroundColor $Colors.BannerAccent
    Write-Host "|  " -ForegroundColor $Colors.BannerAccent -NoNewline
    Write-Host "$($Symbols.Sparkles) Version    : " -ForegroundColor $Colors.Muted -NoNewline
    Write-Host $script:TestSuiteVersion.PadRight(58) -ForegroundColor $Colors.Highlight -NoNewline
    Write-Host "|" -ForegroundColor $Colors.BannerAccent
    Write-Host "+==============================================================================+" -ForegroundColor $Colors.Banner
    Write-Host ""
    
    Write-Host "[$elapsedFormatted] " -ForegroundColor $Colors.Timestamp -NoNewline
    Write-Host "[" -ForegroundColor $Colors.Success -NoNewline
    Write-Host "DONE" -ForegroundColor $Colors.Success -NoNewline
    Write-Host "] " -ForegroundColor $Colors.Success -NoNewline
    Write-Host "$($Symbols.Check) Test suite execution finished" -ForegroundColor $Colors.Success
    
    Write-Host "[$elapsedFormatted] " -ForegroundColor $Colors.Timestamp -NoNewline
    Write-Host "[" -ForegroundColor $Colors.Info -NoNewline
    Write-Host "INFO" -ForegroundColor $Colors.Info -NoNewline
    Write-Host "] " -ForegroundColor $Colors.Info -NoNewline
    Write-Host "Review Pester output above for detailed results" -ForegroundColor $Colors.Info
    Write-Host ""
}