Set-StrictMode -Version Latest

Describe 'install-lg-ultragear-no-dimming.ps1' {
  It 'parses without syntax errors' {
    $tokens = $null; $errors = $null
    $path = Join-Path $PSScriptRoot '..' 'install-lg-ultragear-no-dimming.ps1'
    $null = [System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path $path), [ref]$tokens, [ref]$errors)
    $errors | Should -BeNullOrEmpty
  }

  It 'exposes expected parameters and defaults' {
    $path = Join-Path $PSScriptRoot '..' 'install-lg-ultragear-no-dimming.ps1'
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

    $defaults = @{}
    foreach ($p in $paramBlock.Parameters) { if ($p.DefaultValue) { $defaults[$p.Name.VariablePath.UserPath] = $p.DefaultValue.Extent.Text } }
    $defaults['ProfilePath'] | Should -Match 'lg-ultragear-full-cal.icm'
    $defaults['MonitorNameMatch'] | Should -Match 'LG ULTRAGEAR'
  }
}

Describe 'install-full-auto.bat' {
  It 'exists and passes -NoPrompt' {
    $path = Join-Path $PSScriptRoot '..' 'install-full-auto.bat'
    Test-Path $path | Should -BeTrue
    $content = Get-Content -LiteralPath $path -Raw
    $content | Should -Match '\-NoPrompt'
  }
}

Describe 'readme.md' {
  It 'exists and is lowercase name with correct heading' {
    $path = Join-Path $PSScriptRoot '..' 'readme.md'
    Test-Path $path | Should -BeTrue
    $first = (Get-Content -LiteralPath $path -TotalCount 1)
    $first | Should -Match '^# lg ultragear auto-dimming fix'
  }
}
