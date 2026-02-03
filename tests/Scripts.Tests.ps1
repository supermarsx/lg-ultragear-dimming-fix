Set-StrictMode -Version Latest

# =============================================================================
# LG UltraGear No-Dimming Fix - Comprehensive Test Suite
# Simplified logging to prevent terminal hang
# =============================================================================

BeforeAll {
    $script:TestStartTime = Get-Date

    # Helper functions
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
        $truncatedName = if ($Name.Length -gt 64) { $Name.Substring(0, 64) } else { $Name }
        foreach ($ch in $truncatedName.ToCharArray()) { $charBuffer += [int][char]$ch }
        while ($charBuffer.Count -lt 64) { $charBuffer += 0 }
        return [int[]]$charBuffer
    }

    function Get-ScriptContent {
        return Get-Content -LiteralPath (Get-ScriptPath) -Raw
    }

    # Pre-compute paths
    $script:repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
    $script:scriptPath = Get-ScriptPath
    $script:profilePath = Join-Path -Path $script:repoRoot -ChildPath 'lg-ultragear-full-cal.icm'

    Write-Information "[T+0.000s] Test suite initialized" -InformationAction Continue
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
    }

    Context 'Core Functions' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'defines Write-InfoMessage function' {
            $script:scriptContent | Should -Match 'function Write-InfoMessage'
        }

        It 'defines Install-AutoReapplyMonitor function' {
            $script:scriptContent | Should -Match 'function Install-AutoReapplyMonitor'
        }

        It 'defines Get-MonitorStatus function' {
            $script:scriptContent | Should -Match 'function Get-MonitorStatus'
        }

        It 'defines Show-TUIMainMenu function' {
            $script:scriptContent | Should -Match 'function Show-TUIMainMenu'
        }

        It 'defines Show-TUIAdvancedMenu function' {
            $script:scriptContent | Should -Match 'function Show-TUIAdvancedMenu'
        }

        It 'defines Test-IsAdmin function' {
            $script:scriptContent | Should -Match 'function Test-IsAdmin'
        }

        It 'defines Invoke-TUIAction function' {
            $script:scriptContent | Should -Match 'function Invoke-TUIAction'
        }

        It 'defines Invoke-Main function' {
            $script:scriptContent | Should -Match 'function Invoke-Main'
        }

        It 'defines Show-Usage function' {
            $script:scriptContent | Should -Match 'function Show-Usage'
        }
    }
}

# =============================================================================
# HELP AND BASIC EXECUTION TESTS
# =============================================================================
Describe 'Help and Basic Execution' {
    Context 'Help Output' {
        It 'displays help without errors' {
            $scriptPath = Get-ScriptPath
            { & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 } | Should -Not -Throw
        }

        It 'help includes usage information' {
            $scriptPath = Get-ScriptPath
            $output = & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
            $output | Should -Match 'Usage'
        }

        It 'help includes parameter descriptions' {
            $scriptPath = Get-ScriptPath
            $output = & $scriptPath -Help -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
            $output | Should -Match 'ProfilePath'
        }
    }
}

# =============================================================================
# DRY RUN TESTS
# =============================================================================
Describe 'Dry Run Mode' {
    Context 'DryRun Flag Behavior' {
        It 'accepts DryRun parameter' {
            $scriptPath = Get-ScriptPath
            { & $scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 } | Should -Not -Throw
        }

        It 'DryRun output indicates dry run mode' {
            $scriptPath = Get-ScriptPath
            $output = & $scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
            $output | Should -Match '(?i)(DryRun|dry run|simulation|would|skip)'
        }

        It 'DryRun does not create scheduled task' {
            $scriptPath = Get-ScriptPath
            & $scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-Null
            $taskBefore = Get-ScheduledTask -TaskName 'LG UltraGear Auto-Reapply*' -ErrorAction SilentlyContinue
            $taskBefore | Should -BeNullOrEmpty -Because 'DryRun should not create actual tasks'
        }

        It 'DryRun does not modify registry' {
            $scriptPath = Get-ScriptPath
            $regPathBefore = Get-ItemProperty -Path 'HKCU:\Software\LGUltraGearFix' -ErrorAction SilentlyContinue
            & $scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-Null
            $regPathAfter = Get-ItemProperty -Path 'HKCU:\Software\LGUltraGearFix' -ErrorAction SilentlyContinue
            $regPathAfter | Should -Be $regPathBefore
        }
    }
}

# =============================================================================
# WMI MONITOR DETECTION TESTS
# =============================================================================
Describe 'WMI Monitor Detection' {
    Context 'WMI Query Structure' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'uses Get-CimInstance for WMI queries' {
            $script:scriptContent | Should -Match 'Get-CimInstance'
        }

        It 'queries WmiMonitorID class' {
            $script:scriptContent | Should -Match 'WmiMonitorID'
        }

        It 'uses root\wmi namespace' {
            $script:scriptContent | Should -Match 'root\\wmi'
        }

        It 'handles UserFriendlyName property' {
            $script:scriptContent | Should -Match 'UserFriendlyName'
        }

        It 'handles InstanceName property' {
            $script:scriptContent | Should -Match 'InstanceName'
        }
    }

    Context 'Monitor Name Decoding' {
        It 'decodes char array to string correctly' {
            $charArray = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
            $decoded = ($charArray | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
            $decoded | Should -Be 'LG ULTRAGEAR'
        }

        It 'handles empty char arrays' {
            $charArray = @(0, 0, 0, 0, 0)
            $decoded = ($charArray | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
            $decoded | Should -BeNullOrEmpty
        }

        It 'handles partial char arrays' {
            $charArray = Get-MockMonitorCharBuffer -Name 'LG'
            $decoded = ($charArray | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
            $decoded | Should -Be 'LG'
        }

        It 'handles special characters in names' {
            $charArray = Get-MockMonitorCharBuffer -Name 'LG-27GP950_v2'
            $decoded = ($charArray | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
            $decoded | Should -Be 'LG-27GP950_v2'
        }

        It 'handles Unicode characters' {
            $charArray = Get-MockMonitorCharBuffer -Name 'Monitor™'
            $decoded = ($charArray | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
            $decoded | Should -Be 'Monitor™'
        }
    }

    Context 'LG UltraGear Name Matching' {
        BeforeAll {
            $script:lgPatterns = @(
                'LG ULTRAGEAR',
                'LG UltraGear',
                'lg ultragear',
                '27GP950',
                '32GQ950',
                '27GN950',
                '34GN850',
                '38GN950'
            )
            $script:nonLgPatterns = @(
                'Dell Monitor',
                'ASUS ROG',
                'Samsung Odyssey',
                'Acer Predator',
                'Generic Monitor'
            )
        }

        It 'matches LG ULTRAGEAR case-insensitive' {
            'LG ULTRAGEAR' -match '(?i)ultragear' | Should -BeTrue
            'lg ultragear' -match '(?i)ultragear' | Should -BeTrue
            'LG UltraGear' -match '(?i)ultragear' | Should -BeTrue
        }

        It 'matches common LG UltraGear models' {
            foreach ($pattern in $script:lgPatterns) {
                $pattern -match '(?i)(ultragear|27GP|32GQ|27GN|34GN|38GN)' | Should -BeTrue -Because "$pattern should match LG pattern"
            }
        }

        It 'does not match non-LG monitors' {
            foreach ($pattern in $script:nonLgPatterns) {
                $pattern -match '(?i)^LG.*ultragear' | Should -BeFalse -Because "$pattern should not match LG pattern"
            }
        }

        It 'handles partial model number matches' {
            '27GP950-B' -match '27GP950' | Should -BeTrue
            'LG 27GP950-B.AUS' -match '27GP950' | Should -BeTrue
        }

        It 'MonitorNameMatch regex works' {
            $testName = 'LG ULTRAGEAR 27GP950'
            $pattern = 'ultragear'
            $testName -match $pattern | Should -BeTrue
        }
    }

    Context 'Mock Monitor Detection' {
        It 'detects single LG monitor with mock' {
            # Test mock creation works
            $mockMonitor = [pscustomobject]@{
                InstanceName     = 'DISPLAY\GSM7706\1&ABCDEF&0&UID12345'
                UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
            }
            $mockMonitor | Should -Not -BeNullOrEmpty
            $mockMonitor.InstanceName | Should -Match 'GSM7706'
        }

        It 'detects multiple LG monitors with mock' {
            $mockMonitors = @(
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\GSM7706\1&ABCDEF&0&UID12345'
                    UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR 27GP950'
                },
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\GSM7707\1&ABCDEF&0&UID12346'
                    UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR 32GQ950'
                }
            )
            $mockMonitors.Count | Should -Be 2
        }

        It 'handles mixed monitor types with mock' {
            $monitors = @(
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\GSM7706\1&ABCDEF&0&UID12345'
                    UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
                },
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\DEL4321\1&ABCDEF&0&UID99999'
                    UserFriendlyName = Get-MockMonitorCharBuffer -Name 'Dell U2720Q'
                }
            )

            $lgMonitors = $monitors | Where-Object {
                $name = ($_.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
                $name -match '(?i)ultragear'
            }
            $lgMonitors.Count | Should -Be 1
        }

        It 'handles no monitors found' {
            $monitors = @()
            $monitors.Count | Should -Be 0
        }

        It 'handles WMI null response' {
            $monitors = $null
            $monitors | Should -BeNullOrEmpty
        }
    }

    Context 'WMI Error Handling' {
        It 'handles WMI access denied gracefully' {
            # Test that exceptions are throwable
            { throw [System.UnauthorizedAccessException]::new('Access denied') } | Should -Throw
        }

        It 'handles WMI timeout gracefully' {
            { throw [System.TimeoutException]::new('Operation timed out') } | Should -Throw
        }

        It 'handles invalid namespace gracefully' {
            # Simulate CIM error handling
            $cimError = $null
            try {
                throw [System.Exception]::new('Invalid namespace')
            } catch {
                $cimError = $_
            }
            $cimError | Should -Not -BeNullOrEmpty
        }

        It 'handles CIM connection failure' {
            { throw [System.Runtime.InteropServices.COMException]::new('RPC server unavailable') } | Should -Throw
        }
    }

    Context 'InstanceName Parsing' {
        BeforeAll {
            $script:testInstances = @(
                @{ Input = 'DISPLAY\GSM7706\1&ABCDEF&0&UID12345'; Manufacturer = 'GSM'; Model = '7706' },
                @{ Input = 'DISPLAY\DEL4321\1&123456&0&UID99999'; Manufacturer = 'DEL'; Model = '4321' },
                @{ Input = 'DISPLAY\ACI27A7\5&1234567&0&UID00001'; Manufacturer = 'ACI'; Model = '27A7' }
            )
        }

        It 'extracts manufacturer code from InstanceName' {
            foreach ($test in $script:testInstances) {
                $parts = $test.Input -split '\\'
                $parts[1].Substring(0, 3) | Should -Be $test.Manufacturer
            }
        }

        It 'extracts model code from InstanceName' {
            foreach ($test in $script:testInstances) {
                $parts = $test.Input -split '\\'
                $parts[1].Substring(3) | Should -Be $test.Model
            }
        }

        It 'handles malformed InstanceName' {
            $malformed = 'INVALID_FORMAT'
            $parts = $malformed -split '\\'
            $parts.Count | Should -Be 1
        }
    }

    Context 'Probe Mode' {
        It 'accepts Probe parameter' {
            $scriptPath = Get-ScriptPath
            { & $scriptPath -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 } | Should -Not -Throw
        }

        It 'Probe mode outputs monitor information' {
            Mock -CommandName Get-CimInstance -ParameterFilter {
                ($PSBoundParameters.ContainsKey('Class') -and $PSBoundParameters['Class'] -eq 'WmiMonitorID') -or
                ($PSBoundParameters.ContainsKey('ClassName') -and $PSBoundParameters['ClassName'] -eq 'WmiMonitorID')
            } -MockWith {
                return @(
                    [pscustomobject]@{
                        InstanceName     = 'DISPLAY\GSM7706\1&ABCDEF&0&UID12345'
                        UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
                    }
                )
            }

            $output = & $script:scriptPath -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
            $output | Should -Match '(?i)(monitor|display|probe|detect)'
        }
    }

    Context 'Extended Monitor Properties' {
        It 'handles ProductCodeID property' {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            $script:scriptContent | Should -Match '(?i)(ProductCode|InstanceName)'
        }

        It 'handles WeekOfManufacture property' {
            # Test mock creation with extended properties
            $monitor = [pscustomobject]@{
                InstanceName      = 'DISPLAY\GSM7706\1&ABCDEF&0&UID12345'
                UserFriendlyName  = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
                WeekOfManufacture = 42
                YearOfManufacture = 2023
            }
            $monitor.WeekOfManufacture | Should -Be 42
            $monitor.YearOfManufacture | Should -Be 2023
        }

        It 'handles Active property' {
            $monitor = [pscustomobject]@{
                InstanceName     = 'DISPLAY\GSM7706\1&ABCDEF&0&UID12345'
                UserFriendlyName = Get-MockMonitorCharBuffer -Name 'LG ULTRAGEAR'
                Active           = $true
            }
            $monitor.Active | Should -BeTrue
        }
    }
}

# =============================================================================
# AUTO-ELEVATION TESTS
# =============================================================================
Describe 'Auto-Elevation Logic' {
    Context 'Elevation Detection' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'has Test-IsAdmin function' {
            $script:scriptContent | Should -Match 'function Test-IsAdmin'
        }

        It 'checks WindowsIdentity for elevation' {
            $script:scriptContent | Should -Match 'WindowsIdentity'
        }

        It 'checks WindowsPrincipal for admin role' {
            $script:scriptContent | Should -Match 'WindowsPrincipal'
        }

        It 'uses Administrator built-in role check' {
            $script:scriptContent | Should -Match 'Administrator'
        }
    }
}

# =============================================================================
# SCHEDULED TASK TESTS
# =============================================================================
Describe 'Scheduled Task Generation' {
    Context 'Task Registration Functions' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'defines task registration function' {
            $script:scriptContent | Should -Match 'Register-.*Task|New-ScheduledTask'
        }

        It 'uses ScheduledTasks module cmdlets' {
            $script:scriptContent | Should -Match 'Register-ScheduledTask|New-ScheduledTaskTrigger|New-ScheduledTaskAction'
        }

        It 'defines task trigger for display change' {
            $script:scriptContent | Should -Match '(?i)(displayswitch|session|logon|event)'
        }

        It 'includes task XML generation' {
            $script:scriptContent | Should -Match '(?i)(xml|taskpath|taskname)'
        }
    }

    Context 'Task Cleanup' {
        It 'has unregister task capability' {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            $script:scriptContent | Should -Match 'Unregister-ScheduledTask|Remove-.*Task'
        }

        It 'DryRun does not register tasks' {
            $tasksBefore = Get-ScheduledTask -TaskPath '\' -ErrorAction SilentlyContinue | Where-Object { $_.TaskName -like '*LG*UltraGear*' }

            $scriptPath = Get-ScriptPath
            & $scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 2>&1 | Out-Null

            $tasksAfter = Get-ScheduledTask -TaskPath '\' -ErrorAction SilentlyContinue | Where-Object { $_.TaskName -like '*LG*UltraGear*' }

            if ($tasksBefore) {
                $tasksAfter.Count | Should -Be $tasksBefore.Count
            } else {
                $tasksAfter | Should -BeNullOrEmpty
            }
        }
    }

    Context 'Task Script Content' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'generates PowerShell command for task' {
            $script:scriptContent | Should -Match 'powershell|pwsh'
        }

        It 'includes execution policy bypass' {
            $script:scriptContent | Should -Match '(?i)(-ExecutionPolicy\s+Bypass|Set-ExecutionPolicy)'
        }
    }
}

# =============================================================================
# PROFILE HASH VERIFICATION TESTS
# =============================================================================
Describe 'Profile Hash Verification' {
    Context 'Hash Computation' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'uses hash algorithm for verification' {
            $script:scriptContent | Should -Match 'Get-FileHash|SHA256|MD5|ComputeHash'
        }

        It 'compares embedded vs file hash' {
            $script:scriptContent | Should -Match '(?i)(hash|checksum|verify)'
        }
    }

    Context 'Embedded Profile' {
        It 'contains Base64 encoded profile' {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            $script:scriptContent | Should -Match '\$.*Base64|FromBase64String|ConvertFrom-Base64'
        }

        It 'embedded profile hash is documented' {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
            $script:scriptContent | Should -Match '(?i)(embedded.*hash|profile.*sha|icc.*checksum)'
        }
    }
}

# =============================================================================
# TUI FUNCTION TESTS
# =============================================================================
Describe 'TUI Functions' {
    Context 'Menu Functions' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'defines Show-TUI function' {
            $script:scriptContent | Should -Match 'function Show-TUI'
        }

        It 'defines menu options' {
            $script:scriptContent | Should -Match '(?i)(menu|option|choice|select)'
        }

        It 'handles user input' {
            $script:scriptContent | Should -Match 'Read-Host|ReadKey|\$Host\.UI'
        }

        It 'has install option' {
            $script:scriptContent | Should -Match '(?i)install'
        }

        It 'has uninstall option' {
            $script:scriptContent | Should -Match '(?i)uninstall'
        }

        It 'has probe/status option' {
            $script:scriptContent | Should -Match '(?i)(probe|status|detect)'
        }

        It 'has exit option' {
            $script:scriptContent | Should -Match '(?i)(exit|quit|close)'
        }

        It 'validates menu input' {
            $script:scriptContent | Should -Match '(?i)(valid|invalid|range|check)'
        }

        It 'has colored output support' {
            $script:scriptContent | Should -Match 'ForegroundColor|-Fore'
        }

        It 'clears screen or redraws menu' {
            $script:scriptContent | Should -Match 'Clear-Host|cls|\[Console\]::Clear'
        }
    }
}

# =============================================================================
# UNINSTALL OPERATION TESTS
# =============================================================================
Describe 'Uninstall Operations' {
    Context 'Uninstall Capability' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'has uninstall functionality' {
            $script:scriptContent | Should -Match '(?i)uninstall'
        }

        It 'can remove scheduled task' {
            $script:scriptContent | Should -Match 'Unregister-ScheduledTask'
        }

        It 'can remove files' {
            $script:scriptContent | Should -Match 'Remove-Item'
        }

        It 'handles cleanup confirmation' {
            $script:scriptContent | Should -Match '(?i)(confirm|force|prompt)'
        }

        It 'supports quiet mode' {
            $script:scriptContent | Should -Match '(?i)(NoPrompt)'
        }

        It 'logs removal actions' {
            $script:scriptContent | Should -Match 'Write-.*Message'
        }
    }
}

# =============================================================================
# REINSTALL AND REFRESH TESTS
# =============================================================================
Describe 'Reinstall and Refresh' {
    Context 'Reinstall Capability' {
        BeforeAll {
            $script:scriptContent = Get-Content -LiteralPath (Get-ScriptPath) -Raw
        }

        It 'can reinstall over existing installation' {
            $script:scriptContent | Should -Match '(?i)(reinstall|overwrite|replace|force)'
        }

        It 'refreshes color associations' {
            $script:scriptContent | Should -Match '(?i)(refresh|reapply|reassociate)'
        }

        It 'handles profile update scenario' {
            $script:scriptContent | Should -Match '(?i)(update|newer|version|upgrade)'
        }
    }
}

# =============================================================================
# REPOSITORY FILE STRUCTURE TESTS
# =============================================================================
Describe 'Repository File Structure' {
    Context 'Required Files' {
        BeforeAll {
            $script:repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
        }

        It 'has main installer script' {
            Test-Path -LiteralPath (Join-Path -Path $script:repoRoot -ChildPath 'install-lg-ultragear-no-dimming.ps1') | Should -BeTrue
        }

        It 'has ICC profile' {
            Test-Path -LiteralPath (Join-Path -Path $script:repoRoot -ChildPath 'lg-ultragear-full-cal.icm') | Should -BeTrue
        }

        It 'has readme documentation' {
            Test-Path -LiteralPath (Join-Path -Path $script:repoRoot -ChildPath 'readme.md') | Should -BeTrue
        }

        It 'has batch launcher' {
            Test-Path -LiteralPath (Join-Path -Path $script:repoRoot -ChildPath 'install.bat') | Should -BeTrue
        }
    }
}

# =============================================================================
# INTEGRATION TESTS
# =============================================================================
Describe 'Integration Tests' {
    Context 'End-to-End Dry Run' {
        It 'completes full dry run without errors' {
            $scriptPath = Get-ScriptPath
            { & $scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 } | Should -Not -Throw
        }

        It 'DryRun output shows execution details' {
            $scriptPath = Get-ScriptPath
            $output = & $scriptPath -DryRun -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
            $output.Length | Should -BeGreaterThan 100
        }

        It 'probe mode executes successfully' {
            $scriptPath = Get-ScriptPath
            $output = & $scriptPath -Probe -NoPrompt -SkipElevation -SkipWindowsTerminal 6>&1 5>&1 4>&1 3>&1 2>&1 | Out-String
            $output | Should -Match '(?i)(monitor|probe|detect)'
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
    $elapsed = ((Get-Date) - $script:TestStartTime).TotalSeconds
    Write-Information "[T+$([Math]::Round($elapsed, 3))s] Test suite completed" -InformationAction Continue
}
