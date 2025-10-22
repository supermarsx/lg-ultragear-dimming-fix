Set-StrictMode -Version Latest

Describe 'install-lg-ultragear-no-dimming.ps1' {
    It 'parses without syntax errors' {
        $tokens = $null
        $errors = $null
        $repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
        $path = Join-Path -Path $repoRoot -ChildPath 'install-lg-ultragear-no-dimming.ps1'
        $null = [System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path $path), [ref]$tokens, [ref]$errors)
        $errors | Should -BeNullOrEmpty
    }

    It 'exposes expected parameters and defaults' {
        $repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
        $path = Join-Path -Path $repoRoot -ChildPath 'install-lg-ultragear-no-dimming.ps1'
        $ast = [System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path $path), [ref]$null, [ref]$null)
        $paramBlock = $ast.ParamBlock
        $names = @($paramBlock.Parameters.Name.VariablePath.UserPath)
        $names | Should -Contain 'ProfilePath'
        $names | Should -Contain 'MonitorNameMatch'
        $names | Should -Contain 'PerUser'
        $names | Should -Contain 'NoSetDefault'
        $names | Should -Contain 'SkipHdrAssociation'
        $names | Should -Contain 'NoPrompt'
        $names | Should -Contain 'InstallOnly'
        $names | Should -Contain 'Probe'
        $names | Should -Contain 'DryRun'
        $names | Should -Contain 'SkipElevation'

        $defaults = @{}
        foreach ($p in $paramBlock.Parameters) {
            if ($p.DefaultValue) {
                $defaults[$p.Name.VariablePath.UserPath] = $p.DefaultValue.Extent.Text
            }
        }
        $defaults['ProfilePath'] | Should -Match 'lg-ultragear-full-cal.icm'
        $defaults['MonitorNameMatch'] | Should -Match 'LG ULTRAGEAR'
    }
}

Describe 'install-lg-ultragear-no-dimming.ps1 execution (mocked)' {
    It 'completes a dry run with mocked monitor data' {
        if (-not $IsWindows) {
            Set-ItResult -Skipped -Because 'Execution test only runs on Windows hosts'
            return
        }

        $repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
        $scriptPath = Join-Path -Path $repoRoot -ChildPath 'install-lg-ultragear-no-dimming.ps1'
        $profilePath = Join-Path -Path $repoRoot -ChildPath 'lg-ultragear-full-cal.icm'

        $monitorName = 'LG ULTRAGEAR 27GN950'
        $charBuffer = @()
        foreach ($ch in $monitorName.ToCharArray()) { $charBuffer += [int][char]$ch }
        while ($charBuffer.Count -lt 64) { $charBuffer += 0 }

        Mock -CommandName Get-CimInstance -ParameterFilter { $Class -eq 'WmiMonitorID' } -MockWith {
            @(
                [pscustomobject]@{
                    InstanceName     = 'DISPLAY\LGULTRAGEAR\1&ABCDEF&0&UID1234'
                    UserFriendlyName = [int[]]$charBuffer
                }
            )
        }

        { & $scriptPath -ProfilePath $profilePath -MonitorNameMatch 'LG ULTRAGEAR' -DryRun -NoPrompt -SkipElevation } | Should -Not -Throw
    }
}

Describe 'install-full-auto.bat' {
    It 'exists and passes -NoPrompt' {
        $repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
        $path = Join-Path -Path $repoRoot -ChildPath 'install-full-auto.bat'
        Test-Path $path | Should -BeTrue
        $content = Get-Content -LiteralPath $path -Raw
        $content | Should -Match '\-NoPrompt'
    }
}

Describe 'readme.md' {
    It 'exists and is lowercase name with correct heading' {
        $repoRoot = [System.IO.Path]::GetFullPath((Join-Path -Path $PSScriptRoot -ChildPath '..'))
        $path = Join-Path -Path $repoRoot -ChildPath 'readme.md'
        Test-Path $path | Should -BeTrue
        $first = Get-Content -LiteralPath $path -TotalCount 1
        $first | Should -Match '^# lg ultragear auto-dimming fix'
    }
}
