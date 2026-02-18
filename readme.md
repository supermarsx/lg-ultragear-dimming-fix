<img width="350" height="350" alt="lg-ultragear-dimming-fix" src="https://github.com/user-attachments/assets/4530aed6-d98c-423a-932b-defb3655cffe" />

# LG UltraGear Auto-Dimming Fix | Stop Screen Dimming on Gaming Monitors

[![stars](https://img.shields.io/github/stars/supermarsx/lg-ultragear-dimming-fix?style=flat-square&color=ffd700)](https://github.com/supermarsx/lg-ultragear-dimming-fix/stargazers)
[![watchers](https://img.shields.io/github/watchers/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/watchers)
[![forks](https://img.shields.io/github/forks/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/forks)
[![issues](https://img.shields.io/github/issues/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/issues)
[![downloads](https://img.shields.io/github/downloads/supermarsx/lg-ultragear-dimming-fix/total?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/releases)
[![built with](https://img.shields.io/badge/built%20with-powershell-5391FE?style=flat-square)](#)
[![made for](https://img.shields.io/badge/made%20for-windows-0078D6?style=flat-square)](#)
[![license](https://img.shields.io/github/license/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](license.md)

[![download-latest](https://img.shields.io/badge/Download-Latest%20Release-2ea44f?style=for-the-badge&logo=github)](https://github.com/supermarsx/lg-ultragear-dimming-fix/releases/latest)
[![download-binary](https://img.shields.io/badge/Download-One%20Click%20Installer-2ea44f?style=for-the-badge&logo=windows)](https://github.com/supermarsx/lg-ultragear-dimming-fix/releases/latest/download/install.bat)

> 💡 **Quick Start:** Download and run **`install.bat`** - that's it!

## Fix LG UltraGear Monitor Auto-Dimming Problems

The **lg-ultragear-dimming-fix** application helps you solve auto-dimming problems with LG UltraGear LCD monitors. If your screen dims unexpectedly while gaming or working, this tool can help maintain a consistent brightness level, enhancing your viewing experience.

### What This Tool Does

- **Purpose**: Stop LG UltraGear gaming monitors from dimming under static or semi-static content by constraining the panel's effective luminance range.
- **Method**: Apply and set a custom ICC/ICM color profile that limits the tone response so the monitor's firmware auto-dimming heuristic doesn't trigger.
- **Platform**: Windows 10/11 using Windows Color System (WCS).
- **Compatible Models**: Works with most LG UltraGear series monitors including 27GL850, 27GN950, 38GN950, 34GN850, and many others experiencing unexpected dimming.

### Common Problems This Fixes

- ✅ Screen dims when displaying bright or white content
- ✅ Monitor brightness fluctuates during gaming sessions
- ✅ Unexpected darkening with static images or productivity apps
- ✅ ABL (Automatic Brightness Limiting) cannot be disabled in OSD
- ✅ Brightness inconsistency affecting competitive gaming
- ✅ Eye strain from constant brightness changes


## why this works

Many LG UltraGear models use firmware-level auto-dimming (ABL - Automatic Brightness Limiting) that activates when:
- Average Picture Level (APL) stays high
- Content appears static or semi-static
- Bright colors dominate the screen

This dimming behavior is frustrating for gamers and professionals because:
- It reduces visibility in competitive gaming
- Creates inconsistent viewing experience
- Cannot be disabled through monitor OSD settings
- Persists even with power-saving options disabled

**Our solution** uses a custom ICC color profile that constrains the effective luminance range Windows sends to the display. By limiting the tone response curve, the monitor's firmware never reaches the threshold that triggers auto-dimming, maintaining consistent brightness.

### Software-Only Fix - No Hardware Modifications

- Nothing on the monitor is flashed or permanently changed
- You can revert at any time by removing the color profile
- Works immediately without firmware updates
- Safe for your monitor warranty

### Why Other Solutions Don't Work

**❌ Disabling OSD power-saving options** - Often does not fully disable firmware-level dimming on UltraGear models  
**❌ Windows/GPU settings changes** - Disabling adaptive brightness, CABC, or toggling HDR commonly fails to stop firmware behavior  
**❌ Waiting for firmware updates** - Many screens either have no user-accessible updates, or LG hasn't released fixes  
**✅ Color profile approach** - Works immediately with current firmware, no waiting required


## what's in this repo

### Core Files
- **`install.bat`** — ⭐ One-click launcher (just double-click!)
- **`install-lg-ultragear-no-dimming.ps1`** — Complete installer (profile + auto-reapply monitor)
- **`lg-ultragear-full-cal.icm`** — The color profile that fixes dimming

### CLI Tool (Rust Workspace)
- **`crates/lg-cli`** — Full CLI binary (`lg-ultragear.exe`): detect, apply, watch, config, service
- **`crates/lg-core`** — Shared config loading/saving (TOML)
- **`crates/lg-monitor`** — WMI-based monitor detection
- **`crates/lg-profile`** — Color profile WCS APIs (install, associate, refresh)
- **`crates/lg-notify`** — Toast notifications (PowerShell + Session 0 fallback)
- **`crates/lg-service`** — Windows service runtime + foreground watch mode

### Documentation
- `docs/` — Additional guides and documentation

### Build Tools
- `scripts/` — Helper scripts:
  - `scripts/local-ci.ps1` — run format, lint, test, build locally (skips steps if tools not installed)
  - `scripts/service-ci.ps1` — Rust workspace CI: fmt → clippy → test → build
  - `scripts/service-ci-parallel.ps1` — Parallel Rust CI (fmt + clippy + test in parallel, then build)
  - `scripts/clean.ps1` — clean common build/test artifacts (dist, logs, coverage, etc.)
  - `scripts/embedder.ps1` — regenerate and embed the profile (Base64 + SHA256) into the installer


## quick start

### ⚡ one-click installation

**`install.bat`** — Double-click and done!

Installs:
1. ✅ Color profile fix
2. ✅ Auto-reapply monitor (persists after reconnect/sleep/reboot)

**PowerShell commands:**
```powershell
# Full install (profile + auto-reapply)
.\install-lg-ultragear-no-dimming.ps1

# Profile only (no auto-reapply)
.\install-lg-ultragear-no-dimming.ps1 -SkipMonitor

# Uninstall auto-reapply monitor
.\install-lg-ultragear-no-dimming.ps1 -UninstallMonitor

# Check what monitors are detected
.\install-lg-ultragear-no-dimming.ps1 -Probe
```

---

**option a - one-click batch**
- double-click `install-full-auto.bat` (or run from command prompt). it will:
  - request admin (uac),
  - run powershell with `-executionpolicy bypass` (no permanent policy change),
  - call the installer with defaults.
  - at the end, it will prompt: "press enter to exit...".
  - ⚠️ Note: This applies the fix once but won't auto-reapply on reconnection.

**option b - powershell (manual)**
- open powershell in the repo folder, then run:

  ```powershell
  Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
  ./install-lg-ultragear-no-dimming.ps1
  ```

  - the installer auto‑elevates if needed and, by default, prompts: "press enter to exit...". add `-NoPrompt` to skip.
  - tip: you can probe first with:

  ```powershell
  ./install-lg-ultragear-no-dimming.ps1 -Probe
  ```

**option c - single executable (no powershell needed)**
- download `install-lg-ultragear-no-dimming.exe` from the releases page and run it (uac prompt will appear). it behaves like the powershell script and will prompt to press enter at the end.

  note: when using the executable, open powershell as administrator and run it from your downloads folder so elevation works reliably:

  ```powershell
  # start powershell as administrator, then:
  cd $env:USERPROFILE\Downloads
  .\install-lg-ultragear-no-dimming.exe
  ```

**what happens**
- the profile is copied (or refreshed in‑place) into `%WINDIR%\System32\spool\drivers\color`.
- displays with friendly name containing "lg ultragear" are discovered via wmi.
- the profile is associated with each matched display and set as default.
- the sdr pipeline receives an explicit default association (hdr/advanced-color is also updated unless `-SkipHdrAssociation`).
- system color settings are refreshed.


## manual install (no scripts)

if you prefer not to run any scripts, you can apply the profile manually using the built‑in color management tool.

**pre‑requisites**
- get the `lg-ultragear-full-cal.icm` file from this repo (clone or download the zip).

**optional: copy to the system color store (admin)**
- open file explorer as administrator.
- navigate to `%windir%\system32\spool\drivers\color`.
- copy `lg-ultragear-full-cal.icm` into that folder. (you can also keep the file anywhere; the ui lets you browse.)

**associate the profile with your lg ultragear**
- press `win + r`, type `colorcpl`, press enter.
- go to the “devices” tab.
- from the display dropdown, select your lg ultragear monitor. if you have multiple displays, ensure you select the correct one (use windows settings → system → display → “identify” to confirm which is which).
- check “use my settings for this device”.
- if `lg-ultragear-full-cal.icm` is not listed, click “add…”, then:
  - if you placed the file in the system store, pick it from the list; or
  - click “browse…” and select the `.icm` file from wherever you saved it.
- select `lg-ultragear-full-cal.icm` and click “set as default profile”.
- repeat for any other lg ultragear displays you want to fix.

**optional: set system‑wide default (all users)**
- in `colorcpl`, click “change system defaults…”.
- repeat the same steps under the “devices” tab for the target display(s).

**finish and verify**
- close the dialogs. some apps pick up changes immediately; others may need a restart or sign‑out/in.
- to verify, open `colorcpl` again and confirm the profile is the default for your lg ultragear.


## script usage

- default: `./install-lg-ultragear-no-dimming.ps1`
- match different text: `./install-lg-ultragear-no-dimming.ps1 -monitornamematch "lg"`
- use a specific file: `./install-lg-ultragear-no-dimming.ps1 -profilepath ./lg-ultragear-full-cal.icm`
- also associate per-user: `./install-lg-ultragear-no-dimming.ps1 -peruser`
- do not set as default: `./install-lg-ultragear-no-dimming.ps1 -nosetdefault`
- skip hdr association: `./install-lg-ultragear-no-dimming.ps1 -skiphdrassociation`
 - install only (no device association): `./install-lg-ultragear-no-dimming.ps1 -installonly`
 - probe only (no changes, list monitors and matches): `./install-lg-ultragear-no-dimming.ps1 -probe`
 - dry run (simulate actions; same as -WhatIf): `./install-lg-ultragear-no-dimming.ps1 -dryrun`

**console output**
- the installer prints clear, colored progress with console-safe labels (e.g., [STEP], [INFO], [OK]), lists all detected monitors, and highlights which ones match the friendly-name filter (default: "lg ultragear").

## cli arguments

- `-ProfilePath <path>`: path to ICC/ICM file. Default: `./lg-ultragear-full-cal.icm`.
- note: the installer uses the embedded profile at runtime; the `-ProfilePath` argument is kept for compatibility but is ignored during install/association.
- `-MonitorNameMatch <string>`: substring to match monitor friendly names. Default: `LG ULTRAGEAR`.
- `-PerUser`: also associate the profile in current-user scope.
- `-NoSetDefault`: associate only; do not set as default.
- `-SkipHdrAssociation`: skip the HDR/advanced-color association API.
- `-InstallOnly`: install/copy the profile into the system store without associating.
- `-Probe`: list detected and matched monitors; no changes.
- `-DryRun`: simulate operations (equivalent to WhatIf for actions).
- `-NoPrompt`: do not wait for Enter before exiting.
- `-SkipElevation`: do not auto-elevate (useful for CI/testing).
- `-Help` (aliases: `-h`, `-?`): show usage and exit.

## dev scripts (local)

**local dev helpers (PowerShell)**

```powershell
# run all (format, lint, test, build)
pwsh -File scripts/local-ci.ps1

# treat linter warnings as errors
pwsh -File scripts/local-ci.ps1 -Strict

# clean artifacts
pwsh -File scripts/clean.ps1

# update embedded profile (base64 + sha256) inside the installer
pwsh -File scripts/embedder.ps1 -ProfilePath .\lg-ultragear-full-cal.icm

# or specify a different main script path
pwsh -File scripts/embedder.ps1 -ProfilePath C:\path\to\your.icm -MainScriptPath .\install-lg-ultragear-no-dimming.ps1
```

**Rust workspace helpers**

```powershell
# full Rust CI: format → clippy → test → release build
pwsh -File scripts/service-ci.ps1

# parallel Rust CI (faster)
pwsh -File scripts/service-ci-parallel.ps1

# individual steps
pwsh -File scripts/service-format.ps1        # check formatting
pwsh -File scripts/service-format.ps1 -Fix   # auto-fix formatting
pwsh -File scripts/service-lint.ps1           # clippy lint
pwsh -File scripts/service-test.ps1           # run tests
pwsh -File scripts/service-build.ps1          # release build → dist/lg-ultragear.exe
```

**idempotency**
- if the profile file is already present in the system color store, the installer compares hashes and overwrites only when content differs; no duplicate files are created.
- associations and "set default" operations are safe to repeat; the script aims to leave a single effective default profile per device.


## verification

- classic ui: press `win+r`, run `colorcpl` -> devices tab -> select your lg ultragear -> ensure `lg-ultragear-full-cal.icm` is present and set as default.
- settings: system -> display -> advanced display -> pick the lg; verify behavior under hdr if applicable.
- visual check: leave a bright, mostly static window; dimming should be gone or greatly reduced.

## troubleshooting common lg ultragear dimming issues

### The fix doesn't stay persistent (profile resets after reconnection)
- **Solution:** Use `install-with-auto-reapply.bat` instead of the basic installer. This creates a scheduled task that automatically reapplies the profile when the monitor reconnects.
- **Why this happens:** Windows sometimes resets color profiles on display device events (disconnect/reconnect, sleep/wake, driver updates)
- **Check if auto-reapply is working:** Open Task Scheduler (`taskschd.msc`) → Task Scheduler Library → look for `LG-UltraGear-ColorProfile-AutoReapply`

### Verify the monitor task is installed
```powershell
Get-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"
```

### Manually trigger the task to test
```powershell
Start-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"
```

### Check task history
- Open Task Scheduler (`taskschd.msc`)
- Navigate to the task
- Click the "History" tab to see when it last ran

### The profile is applied but dimming still occurs
- Some LG UltraGear models have multiple dimming mechanisms
- Try also disabling any "Energy Saving" or "Smart Energy Saving" options in the monitor's OSD menu
- Ensure HDR is disabled in Windows settings if you're using SDR content

### Screen dims only on certain colors or scenes
- This is normal behavior with the profile - it prevents aggressive dimming but may not eliminate all dimming
- Try adjusting monitor brightness and contrast settings in OSD
- Some models have more aggressive ABL that requires additional tuning

### Gaming performance concerns
- This fix does NOT impact gaming performance or FPS
- Works alongside G-SYNC, FreeSync, and other monitor features
- No input lag or response time changes

### To completely uninstall
```powershell
# Remove the auto-reapply monitor
.\install-monitor-watcher.ps1 -Uninstall

# Remove the color profile (manual)
# Open colorcpl → Devices → select your monitor → remove the profile
```


## rollback / revert

- switch back: `colorcpl` -> devices -> select display -> choose another default or uncheck "use my settings for this device".
- remove association: in `colorcpl`, select the profile and click remove.
- remove file: delete it from `%windir%\system32\spool\drivers\color` (admin required).


## how it works (technical)

- uses wcs (`mscms.dll`) via p/invoke:
  - `installcolorprofile` to install the icc/icm (or we overwrite if already present),
  - `wcsassociatecolorprofilewithdevice` to bind it to the display's device key,
  - `wcssetdefaultcolorprofile` to make it default,
  - `colorprofileadddisplayassociation` best-effort for hdr/advanced-color displays.
- monitors are discovered via `wmimonitorid` to match user-friendly names like "lg ultragear 27gn950".
- a `wm_settingchange` broadcast prompts windows to refresh color.


## security / permissions

- installing into the system color store and setting defaults requires administrator. both the batch and the powershell installer auto-elevate (uac prompt shown if needed).


## license

See [license.md](license.md) for licensing details.

## downloads & releases

Automated CI produces versioned releases named `yyyy.n` (example: `2025.1`). `n` increments for each release within the year.

Each release includes:
- `lg-ultragear-dimming-fix.zip` (scripts, exe, profile, readme, license)
- `install-lg-ultragear-no-dimming.exe` (standalone executable)

## frequently asked questions (faq)

### Does this work with all LG UltraGear monitors?
This fix has been tested and confirmed working on many LG UltraGear models including:
- 27GL850, 27GL83A
- 27GN950, 27GN850, 27GN800
- 34GN850, 38GN950
- 32GN650, 32GP850
- And many other models experiencing auto-dimming

If your model isn't listed, try it - the fix is safe and reversible.

### Will this affect color accuracy?
The profile constrains luminance range slightly to prevent dimming triggers. Most users report no noticeable color difference in normal use. For professional color work, you may want to use a different profile or revert when color accuracy is critical.

### Does this work with HDR content?
This fix primarily targets SDR (Standard Dynamic Range) content. For HDR content, the behavior may vary depending on your monitor model. Some users report success with `-EnableHdrAssociation` flag.

### Can I use this with NVIDIA or AMD graphics cards?
Yes! This fix works with any graphics card brand (NVIDIA, AMD, Intel) because it operates at the Windows color management level, independent of GPU driver settings.

### Will this void my monitor warranty?
No. This is a software-only fix that doesn't modify your monitor's firmware or hardware. It simply changes how Windows sends color data to the display.

### My monitor still dims after applying the fix
- Ensure you used `install-with-auto-reapply.bat` for persistent fixing
- Check Task Scheduler to verify the auto-reapply task is active
- Disable "Energy Saving" features in your monitor's OSD menu
- Some extreme ABL cases may require additional monitor OSD adjustments

### Can I uninstall this easily?
Yes! Simply run `.\install-monitor-watcher.ps1 -Uninstall` and remove the color profile through Windows Color Management (`colorcpl`).

## keywords & search terms

LG UltraGear dimming fix, LG monitor auto dimming, LG UltraGear brightness problem, stop LG monitor from dimming, LG ABL disable, LG automatic brightness limiting, gaming monitor dimming issue, LG 27GL850 dimming, LG 27GN950 dimming, LG UltraGear screen darkening, fix monitor brightness fluctuation, LG gaming monitor dimming Windows 10, LG gaming monitor dimming Windows 11, disable ABL LG UltraGear, ICC profile fix dimming, color profile dimming solution
