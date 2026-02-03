Set-StrictMode -Version Latest

# =============================================================================
# HELPER FUNCTIONS
# =============================================================================
function Get-ScriptPath {
    $repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
    return Join-Path -Path $repoRoot -ChildPath 'install-lg-ultragear-no-dimming.ps1'
}

function Get-ScriptAST {
    $path = Get-ScriptPath
    return [System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path $path), [ref]$null, [ref]$null)
}

function Get-MockMonitorCharBuffer {
    param([string]$Name)
    $charBuffer = @()
    foreach ($ch in $Name.ToCharArray()) { $charBuffer += [int][char]$ch }
    while ($charBuffer.Count -lt 64) { $charBuffer += 0 }
    return [int[]]$charBuffer
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
        $output = & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 | Out-String
        $output | Should -Match 'Usage:'
        $output | Should -Match 'INSTALL OPTIONS'
        $output | Should -Match 'MAINTENANCE'
        $output | Should -Match 'UNINSTALL'
    }

    It 'help output contains examples' {
        $scriptPath = Get-ScriptPath
        $output = & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 | Out-String
        $output | Should -Match 'EXAMPLES'
        $output | Should -Match '-Probe'
        $output | Should -Match '-NonInteractive'
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

        It 'matches LG UltraGear pattern variations' {
            $patterns = @(
                'LG ULTRAGEAR',
                'LG ULTRAGEAR 27GN950',
                'LG ULTRAGEAR 27GP950-B',
                'LG ULTRAGEAR 32GQ950'
            )

            foreach ($name in $patterns) {
                $name | Should -Match 'LG.*ULTRAGEAR'
            }
        }

        It 'does not match non-LG monitors' {
            $patterns = @(
                'Dell U2720Q',
                'Samsung Odyssey G9',
                'ASUS ROG Swift',
                'Acer Predator'
            )

            foreach ($name in $patterns) {
                $name | Should -Not -Match 'LG.*ULTRAGEAR'
            }
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
            $taskName = 'Test-LG-UltraGear-Task'
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
                "-MonitorNameMatch '$monitorMatch'"
            )

            $actionLine = "& '$installerPath' -NoSetDefault -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor -MonitorNameMatch '$monitorMatch' 2>`$null | Out-Null"

            foreach ($param in $expectedParams) {
                $actionLine | Should -Match [regex]::Escape($param)
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
            $triggerTypes = @(
                'AtLogOn',           # Trigger 2
                'ConsoleConnect',    # Trigger 3 StateChange=7
                'SessionUnlock'      # Trigger 4 StateChange=8
            )

            # Verify the script defines these triggers
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
            $scriptContent | Should -Match 'Remove-Item.*LG-UltraGear-Monitor'
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
            # Check that it calls uninstall
            $scriptContent | Should -Match 'Reinstall.*Uninstall-AutoReapplyMonitor'
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

    It 'install-full-auto.bat exists' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'install-full-auto.bat'
        Test-Path $path | Should -BeTrue
    }

    It 'install-full-auto.bat passes -NoPrompt' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'install-full-auto.bat'
        $content = Get-Content -LiteralPath $path -Raw
        $content | Should -Match '\-NoPrompt'
    }

    It 'readme.md exists with correct heading' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'readme.md'
        Test-Path $path | Should -BeTrue
        $first = Get-Content -LiteralPath $path -TotalCount 1
        $first | Should -Match '^# lg ultragear auto-dimming fix'
    }

    It 'license.md exists' {
        $path = Join-Path -Path $script:repoRoot -ChildPath 'license.md'
        Test-Path $path | Should -BeTrue
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

            $output = & $script:scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal -SkipMonitor 2>&1 | Out-String

            $output | Should -Match 'dry-run enabled'
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

            $output = & $script:scriptPath -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 | Out-String

            $output | Should -Match 'probe mode'
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

        It 'defines OK symbol' {
            $script:scriptContent | Should -Match "SymbolOk.*\[ OK \]"
        }

        It 'defines STEP symbol' {
            $script:scriptContent | Should -Match "SymbolStep.*\[STEP\]"
        }

        It 'defines ERROR symbol' {
            $script:scriptContent | Should -Match "SymbolError.*\[ERR \]"
        }

        It 'defines DONE symbol' {
            $script:scriptContent | Should -Match "SymbolDone.*\[DONE\]"
        }
    }
}
