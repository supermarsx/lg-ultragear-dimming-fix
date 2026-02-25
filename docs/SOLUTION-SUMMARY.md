# Solution Summary (Current Architecture)

## Problem Addressed

Some LG UltraGear displays dim under static or bright content. Manual ICC assignment often resets after reconnect/sleep/logon.

## Current Solution

The tool now uses a native Rust executable plus a Windows service:

- lg-ultragear-dimming-fix.exe for install/config/apply/diagnostics
- lg-ultragear-color-svc for persistent event-driven reapply

## How Reapply Works

On display/session events, the service:

1. Resolves active profile preset (schedule -> mode -> fallback)
2. Resolves tuning preset (anti-dim/color-space/unyellow/contrast families)
3. Ensures dynamic ICC profile artifacts exist
4. Reapplies profile association using disassociate/reassociate toggle
5. Runs configured refresh hooks (soft color broadcast, optional display refresh, calibration loader)

## Why This Is Reliable

- Event-driven workflow instead of polling
- Native Windows color APIs (WCS + display association)
- Persistent service lifecycle managed by SCM
- Per-monitor/profile generation support
- Configurable delays for wake/reconnect stabilization

## Key Paths

- Config: %ProgramData%\\LG-UltraGear-Monitor\\config.toml
- Binary (installed): %ProgramData%\\LG-UltraGear-Monitor\\lg-ultragear-dimming-fix.exe
- Profiles: %WINDIR%\\System32\\spool\\drivers\\color\\

## Operational Commands

lg-ultragear-dimming-fix.exe install
lg-ultragear-dimming-fix.exe service status
lg-ultragear-dimming-fix.exe detect
lg-ultragear-dimming-fix.exe apply
lg-ultragear-dimming-fix.exe config show

For full command and preset docs, see readme.md.
