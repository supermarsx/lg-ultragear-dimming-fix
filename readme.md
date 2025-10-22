# lg ultragear auto-dimming fix

[![stars](https://img.shields.io/github/stars/supermarsx/lg-ultragear-dimming-fix?style=flat&color=ffd700)](https://github.com/supermarsx/lg-ultragear-dimming-fix/stargazers)
[![watchers](https://img.shields.io/github/watchers/supermarsx/lg-ultragear-dimming-fix?style=flat)](https://github.com/supermarsx/lg-ultragear-dimming-fix/watchers)
[![forks](https://img.shields.io/github/forks/supermarsx/lg-ultragear-dimming-fix?style=flat)](https://github.com/supermarsx/lg-ultragear-dimming-fix/forks)
[![downloads](https://img.shields.io/github/downloads/supermarsx/lg-ultragear-dimming-fix/total?style=flat)](https://github.com/supermarsx/lg-ultragear-dimming-fix/releases)
[![built with](https://img.shields.io/badge/built%20with-powershell-5391FE)](#)
[![made for](https://img.shields.io/badge/made%20for-windows-0078D6)](#)
[![license](https://img.shields.io/github/license/supermarsx/lg-ultragear-dimming-fix?style=flat)](license.md)



- purpose: stop lg ultragear gaming monitors from dimming under static or semi-static content by constraining the panel's effective luminance range.
- method: apply and set a custom icc/icm color profile that limits the tone response so the monitor's firmware auto-dimming heuristic doesn't trigger.
- platform: windows 10/11 using windows color system (wcs).


## why this works

- many ultragear models dim when average picture level stays high or content looks static.
- a tailored color profile reduces the effective range windows drives the display through, preventing the firmware condition that triggers dimming.
- software-only mitigation: nothing on the monitor is flashed or permanently changed; you can revert at any time.

**limitations of other approaches**
- disabling power-saving, abl or similar osd options often does not fully disable dimming on ultragear models.
- changing windows/gpu settings (disabling adaptive brightness, cabc, toggling hdr, etc.) commonly fails to stop the firmware-level behavior.
- firmware updates would be ideal, but many screens either have no user-feasible updates or the update tools are not publicly available. the profile-based approach works immediately without firmware changes.


## what's in this repo

- `lg-ultragear-full-cal.icm` - custom icc/icm profile that constrains luminance.
- `install-lg-ultragear-no-dimming.ps1` - installer that finds "lg ultragear" displays, installs the profile, associates it, and sets it as default.
- `install-full-auto.bat` - one-click bridge: auto-elevates, uses a temporary execution policy bypass, and runs the installer end-to-end.
- release artifacts - packaged zip and a single-file executable built from the installer for easy distribution.


## quick start

**option a - one-click batch (recommended)**
- double-click `install-full-auto.bat` (or run from command prompt). it will:
  - request admin (uac),
  - run powershell with `-executionpolicy bypass` (no permanent policy change),
  - call the installer with defaults.
  - at the end, it will prompt: "press enter to exit...".

**option b - powershell (manual)**
- open powershell in the repo folder, then run:
  - `set-executionpolicy -scope process -executionpolicy bypass -force`
  - `./install-lg-ultragear-no-dimming.ps1 -verbose`
  - the installer auto-elevates if needed and, by default, prompts: "press enter to exit...". add `-noprompt` to skip.
  - tip: you can probe first with `./install-lg-ultragear-no-dimming.ps1 -probe` to see detected and matched monitors.

**option c - single executable (no powershell needed)**
- download `install-lg-ultragear-no-dimming.exe` from the releases page and run it (uac prompt will appear). it behaves like the powershell script and will prompt to press enter at the end.

**what happens**
- the profile is copied (or refreshed in-place) into `%windir%\system32\spool\drivers\color`.
- displays with friendly name containing "lg ultragear" are discovered via wmi.
- the profile is associated with each matched display and set as default.
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
- verbose output: `./install-lg-ultragear-no-dimming.ps1 -verbose`
 - install only (no device association): `./install-lg-ultragear-no-dimming.ps1 -installonly`
 - probe only (no changes, list monitors and matches): `./install-lg-ultragear-no-dimming.ps1 -probe`
 - dry run (simulate actions; same as -WhatIf): `./install-lg-ultragear-no-dimming.ps1 -dryrun`

**console output**
- the installer prints clear, colored progress with emojis (e.g., probing displays, installing profile, associating per display), lists all detected monitors, and highlights which ones match the friendly-name filter (default: "lg ultragear").

**idempotency**
- if the profile file is already present in the system color store, the installer compares hashes and overwrites only when content differs; no duplicate files are created.
- associations and "set default" operations are safe to repeat; the script aims to leave a single effective default profile per device.


## verification

- classic ui: press `win+r`, run `colorcpl` -> devices tab -> select your lg ultragear -> ensure `lg-ultragear-full-cal.icm` is present and set as default.
- settings: system -> display -> advanced display -> pick the lg; verify behavior under hdr if applicable.
- visual check: leave a bright, mostly static window; dimming should be gone or greatly reduced.


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

- see `license.md` for licensing details.
- downloads & releases

- automated ci produces versioned releases named `yyyy.n` (example: `2025.1`). `n` increments for each release within the year.
- each release includes:
  - `lg-ultragear-dimming-fix.zip` (scripts, exe, profile, readme, license)
  - `install-lg-ultragear-no-dimming.exe` (standalone executable)
