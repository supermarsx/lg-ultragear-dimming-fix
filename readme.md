**lg ultragear auto-dimming fix**

- purpose: stop lg ultragear gaming monitors from dimming under static or semi-static content by constraining the panel's effective luminance range.
- method: apply and set a custom icc/icm color profile that limits the tone response so the monitor's firmware auto-dimming heuristic doesn't trigger.
- platform: windows 10/11 using windows color system (wcs).


**why this works**

- many ultragear models dim when average picture level stays high or content looks static.
- a tailored color profile reduces the effective range windows drives the display through, preventing the firmware condition that triggers dimming.
- software-only mitigation: nothing on the monitor is flashed or permanently changed; you can revert at any time.

limitations of other approaches
- disabling power-saving, abl or similar osd options often does not fully disable dimming on ultragear models.
- changing windows/gpu settings (disabling adaptive brightness, cabc, toggling hdr, etc.) commonly fails to stop the firmware-level behavior.
- firmware updates would be ideal, but many screens either have no user-feasible updates or the update tools are not publicly available. the profile-based approach works immediately without firmware changes.


**what's in this repo**

- `lg-ultragear-full-cal.icm` - custom icc/icm profile that constrains luminance.
- `install-lg-ultragear-no-dimming.ps1` - installer that finds "lg ultragear" displays, installs the profile, associates it, and sets it as default.
- `install-full-auto.bat` - one-click bridge: auto-elevates, uses a temporary execution policy bypass, and runs the installer end-to-end.
- release artifacts - packaged zip and a single-file executable built from the installer for easy distribution.

note: the previous generic reference installer was removed after this tailored script was added.


**quick start**

option a - one-click batch (recommended)
- double-click `install-full-auto.bat` (or run from command prompt). it will:
  - request admin (uac),
  - run powershell with `-executionpolicy bypass` (no permanent policy change),
  - call the installer with defaults.
  - at the end, it will prompt: "press enter to exit...".

option b - powershell (manual)
- open powershell in the repo folder, then run:
  - `set-executionpolicy -scope process -executionpolicy bypass -force`
  - `./install-lg-ultragear-no-dimming.ps1 -verbose`
  - the installer auto-elevates if needed and, by default, prompts: "press enter to exit...". add `-noprompt` to skip.
  - tip: you can probe first with `./install-lg-ultragear-no-dimming.ps1 -probe` to see detected and matched monitors.

option c - single executable (no powershell needed)
- download `install-lg-ultragear-no-dimming.exe` from the releases page and run it (uac prompt will appear). it behaves like the powershell script and will prompt to press enter at the end.

what happens
- the profile is copied (or refreshed in-place) into `%windir%\system32\spool\drivers\color`.
- displays with friendly name containing "lg ultragear" are discovered via wmi.
- the profile is associated with each matched display and set as default.
- system color settings are refreshed.


**script usage**

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

console output
- the installer prints clear, colored progress with emojis (e.g., probing displays, installing profile, associating per display), lists all detected monitors, and highlights which ones match the friendly-name filter (default: "lg ultragear").

idempotency
- if the profile file is already present in the system color store, the installer compares hashes and overwrites only when content differs; no duplicate files are created.
- associations and "set default" operations are safe to repeat; the script aims to leave a single effective default profile per device.


**verification**

- classic ui: press `win+r`, run `colorcpl` -> devices tab -> select your lg ultragear -> ensure `lg-ultragear-full-cal.icm` is present and set as default.
- settings: system -> display -> advanced display -> pick the lg; verify behavior under hdr if applicable.
- visual check: leave a bright, mostly static window; dimming should be gone or greatly reduced.


**rollback / revert**

- switch back: `colorcpl` -> devices -> select display -> choose another default or uncheck "use my settings for this device".
- remove association: in `colorcpl`, select the profile and click remove.
- remove file: delete it from `%windir%\system32\spool\drivers\color` (admin required).


**how it works (technical)**

- uses wcs (`mscms.dll`) via p/invoke:
  - `installcolorprofile` to install the icc/icm (or we overwrite if already present),
  - `wcsassociatecolorprofilewithdevice` to bind it to the display's device key,
  - `wcssetdefaultcolorprofile` to make it default,
  - `colorprofileadddisplayassociation` best-effort for hdr/advanced-color displays.
- monitors are discovered via `wmimonitorid` to match user-friendly names like "lg ultragear 27gn950".
- a `wm_settingchange` broadcast prompts windows to refresh color.


**security / permissions**

- installing into the system color store and setting defaults requires administrator. both the batch and the powershell installer auto-elevate (uac prompt shown if needed).


**license**

- see `license.md` for licensing details.
- downloads & releases

- automated ci produces versioned releases named `yyyy.n` (example: `2025.1`). `n` increments for each release within the year.
- each release includes:
  - `lg-ultragear-dimming-fix.zip` (scripts, exe, profile, readme, license)
  - `install-lg-ultragear-no-dimming.exe` (standalone executable)
