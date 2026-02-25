# LG UltraGear Dimming Fix - Installation Guide (Current)

This project is now a single native executable workflow. Legacy batch/PowerShell installers are kept under `legacy/` for reference only.

## Recommended Install

1. Download `lg-ultragear-dimming-fix.exe` from Releases.
2. Run it as Administrator.
3. In TUI mode, choose `1` (Install profile + service), or run:

```powershell
lg-ultragear-dimming-fix.exe install
```

This installs:

- Dynamic ICC profile(s) in `%WINDIR%\System32\spool\drivers\color`
- Windows service `lg-ultragear-color-svc`
- Config at `%ProgramData%\LG-UltraGear-Monitor\config.toml`

## Install Variants

```powershell
# Profile only
lg-ultragear-dimming-fix.exe install --profile-only

# Service only
lg-ultragear-dimming-fix.exe install --service-only

# Custom monitor matching
lg-ultragear-dimming-fix.exe install --pattern "LG ULTRAGEAR"
lg-ultragear-dimming-fix.exe install --pattern "27G.*" --regex
```

## Verify Installation

```powershell
lg-ultragear-dimming-fix.exe detect
lg-ultragear-dimming-fix.exe service status
lg-ultragear-dimming-fix.exe config show
```

Optional Windows UI check:

- Run `colorcpl`
- Devices tab -> select your LG monitor
- Confirm generated profile is associated (for example `lg-ultragear-gamma22-cmx.icm`)

## Uninstall / Reinstall

```powershell
# Remove service only
lg-ultragear-dimming-fix.exe uninstall

# Full cleanup (service + profiles + config)
lg-ultragear-dimming-fix.exe uninstall --full

# Fresh reinstall
lg-ultragear-dimming-fix.exe reinstall
```

## Notes

- Admin elevation is required for service and color-store operations.
- Service mode is event-driven (display/session events), not polling.
- For current tuning and preset behavior, see `readme.md` -> "Preset System (Current Behavior)".
