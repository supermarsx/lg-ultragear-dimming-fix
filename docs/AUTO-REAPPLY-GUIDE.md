# Auto-Reapply Guide (Current Service Workflow)

Auto-reapply is now handled by the Windows service `lg-ultragear-color-svc`.
The older scheduled-task script flow is legacy-only.

## What Triggers Reapply

The service listens for:

- Display device arrival/removal events
- Display topology changes (`WM_DISPLAYCHANGE`)
- Session unlock/logon events

When triggered, it re-applies profile association using a toggle flow:

1. Resolve effective preset (schedule -> HDR/SDR mode -> active preset fallback)
2. Ensure dynamic ICC profile exists (and mode-specific variants as configured)
3. Disassociate/reassociate profile to force Windows color pipeline refresh
4. Run soft refresh steps and calibration loader trigger (config-controlled)

## Enable Auto-Reapply

```powershell
lg-ultragear-dimming-fix.exe install
```

or install service explicitly:

```powershell
lg-ultragear-dimming-fix.exe service install
lg-ultragear-dimming-fix.exe service start
```

## Check Service Health

```powershell
lg-ultragear-dimming-fix.exe service status
lg-ultragear-dimming-fix.exe probe
```

## Common Config Controls

In `%ProgramData%\LG-UltraGear-Monitor\config.toml`:

- `stabilize_delay_ms`
- `toggle_delay_ms`
- `reapply_delay_ms`
- `refresh_display_settings`
- `refresh_broadcast_color`
- `refresh_invalidate`
- `refresh_calibration_loader`
- `icc_sdr_preset`, `icc_hdr_preset`
- `icc_schedule_day_preset`, `icc_schedule_night_preset`
- `icc_tuning_preset`

## Troubleshooting

If reapply does not stick:

1. Confirm service is running: `lg-ultragear-dimming-fix.exe service status`
2. Confirm monitor match works: `lg-ultragear-dimming-fix.exe detect`
3. Manually force one cycle: `lg-ultragear-dimming-fix.exe apply`
4. Increase `reapply_delay_ms` (for slow wake/scaler init)
5. Keep `refresh_broadcast_color = true` and `refresh_calibration_loader = true`

If needed, perform a clean reset:

```powershell
lg-ultragear-dimming-fix.exe uninstall --full
lg-ultragear-dimming-fix.exe install
```
