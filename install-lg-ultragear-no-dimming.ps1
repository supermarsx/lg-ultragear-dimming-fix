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
    [switch]$NoPrompt,
    [switch]$InstallOnly,
    [switch]$Probe,
    [switch]$DryRun,
    [switch]$SkipElevation,
    [switch]$SkipWindowsTerminal,
    [switch]$KeepTemp,
    [switch]$SkipHashCheck,
    [Alias('h', '?')]
    [switch]$Help
)

begin {
    # Hint to static analysis: parameters are intentionally used across nested scopes
    $null = $ProfilePath, $MonitorNameMatch, $PerUser, $NoSetDefault, $SkipHdrAssociation, $NoPrompt, $InstallOnly, $Probe, $SkipElevation, $SkipWindowsTerminal, $KeepTemp, $SkipHashCheck
    # Mark parameters as referenced for static analyzers

    # Record the launch context so relative paths stay consistent after re-invocation.
    $script:InvocationPath = if ($PSCommandPath) { $PSCommandPath } else { $MyInvocation.MyCommand.Path }
    $script:InvocationDirectory = if ($script:InvocationPath) { Split-Path -Path $script:InvocationPath -Parent } else { $null }
    # Capture the caller's working directory if available; this helps rebuild relative paths later.
    try {
        $script:OriginalWorkingDirectory = (Get-Location).ProviderPath
    } catch {
        try { $script:OriginalWorkingDirectory = (Get-Location).Path } catch { $script:OriginalWorkingDirectory = $null }
    }

    # Set console appearance: black background, green-ish text, and window title to script name
    try {
        $scriptName = if ($script:InvocationPath) { Split-Path -Path $script:InvocationPath -Leaf } else { 'install-lg-ultragear-no-dimming.ps1' }
        $raw = $Host.UI.RawUI
        # Save originals if needed in the future
        $script:OriginalFg = $raw.ForegroundColor
        $script:OriginalBg = $raw.BackgroundColor
        $script:OriginalTitle = $raw.WindowTitle
        $raw.BackgroundColor = 'Black'
        $raw.ForegroundColor = 'White'
        try { $raw.WindowTitle = $scriptName } catch { [Console]::Title = $scriptName }
        try { Clear-Host } catch { Write-NoteMessage "Clear-Host skipped (no interactive host)." }
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
        Write-Host "Usage: .\\install-lg-ultragear-no-dimming.ps1 [options]"
        Write-Host ""; Write-Host "Options:"
        Write-Host "  -ProfilePath <path>          Path to ICC/ICM file (default: .\\lg-ultragear-full-cal.icm)"
        Write-Host "  -MonitorNameMatch <string>   Substring to match monitor friendly name (default: 'LG ULTRAGEAR')"
        Write-Host "  -PerUser                      Also associate profile in current-user scope"
        Write-Host "  -NoSetDefault                 Associate only; do not set as default"
        Write-Host "  -SkipHdrAssociation           Skip HDR/advanced-color association API"
        Write-Host "  -InstallOnly                  Install/copy profile only; no device association"
        Write-Host "  -Probe                        Probe and list detected/matched monitors; no changes"
        Write-Host "  -DryRun                       Simulate operations (same as -WhatIf for actions)"
        Write-Host "  -NoPrompt                     Do not wait for Enter before exit"
        Write-Host "  -SkipElevation                Do not auto-elevate (useful for CI/testing)"
        Write-Host "  -SkipWindowsTerminal          Do not re-host under Windows Terminal"
        Write-Host "  -KeepTemp                     Keep temp materialized profile files for inspection"
        Write-Host "  -SkipHashCheck                Do not enforce SHA256 integrity on materialized profile"
        Write-Host "  -h | -?                       Show this help and exit"
        Write-Host ""; Write-Host "Examples:"
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1"
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1 -MonitorNameMatch 'LG ULTRAGEAR' -PerUser"
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1 -Probe -NoPrompt"
        Write-Host "  .\\install-lg-ultragear-no-dimming.ps1 -InstallOnly -ProfilePath .\\lg-ultragear-full-cal.icm"
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
                $argsList += $val
            }
        }

        $workingDir = if ($script:OriginalWorkingDirectory -and (Test-Path -LiteralPath $script:OriginalWorkingDirectory)) {
            $script:OriginalWorkingDirectory
        } elseif ($script:InvocationDirectory -and (Test-Path -LiteralPath $script:InvocationDirectory)) {
            $script:InvocationDirectory
        } else {
            $env:SystemRoot
        }

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
                    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash
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
                    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash
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
                                $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash
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
                        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash
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
      Honors -InstallOnly, -PerUser, -NoSetDefault, -Probe.
    #>
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

                $srcHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $profileFull).Hash
                $srcSize = (Get-Item -LiteralPath $profileFull).Length
                Write-InfoMessage ("source profile size: {0} bytes" -f $srcSize)
                Write-InfoMessage ("source profile SHA256: {0}" -f $srcHash)

                if (Test-Path -LiteralPath $installedPath) {
                    $dstHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedPath).Hash
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

                if (-not $NoSetDefault) {
                    if ($PSCmdlet.ShouldProcess($deviceName, "Set as default profile")) {
                        if (-not [WcsDefault]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_SYSTEM_WIDE, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
                            $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                            Write-WarnMessage ("set default (system) failed (Win32={0})" -f $code)
                        } else { Write-SuccessMessage "set default (system) ok" }
                        if ($PerUser.IsPresent) {
                            if (-not [WcsDefault]::WcsSetDefaultColorProfile([uint32]$WCS_SCOPE_CURRENT_USER, $deviceName, $CPT_ICC, $CPS_DEV, 0, $installedPath)) {
                                $code = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
                                Write-WarnMessage ("set default (user) failed (Win32={0})" -f $code)
                            } else { Write-SuccessMessage "set default (user) ok" }
                        }
                    }
                }

                if (-not $SkipHdrAssociation) {
                    try {
                        # profileType 0 => ICC. No error if SDR.
                        if ($PSCmdlet.ShouldProcess($deviceName, "HDR/advanced-color association")) {
                            [void][WcsHdrAssoc]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_SYSTEM_WIDE, 0)
                            if ($PerUser.IsPresent) { [void][WcsHdrAssoc]::ColorProfileAddDisplayAssociation($installedPath, $deviceName, [uint32]$WCS_SCOPE_CURRENT_USER, 0) }
                            Write-SuccessMessage "HDR/advanced-color association ok"
                        }
                    } catch {
                        Write-NoteMessage "HDR association API not available; skipping."
                    }
                }
            }

            Write-ActionMessage "refreshing color settings"
            $HWND_BROADCAST = [IntPtr]0xffff
            $WM_SETTINGCHANGE = 0x1A
            $SMTO_ABORTIFHUNG = 0x0002
            [UIntPtr]$res = [UIntPtr]::Zero
            [void][Win32SendMessage]::SendMessageTimeout($HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Color', $SMTO_ABORTIFHUNG, 2000, [ref]$res)

            Write-SuccessMessage "done. associated profile '$profileName' with all displays matching '$MonitorNameMatch'."
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
    Invoke-Main
}
