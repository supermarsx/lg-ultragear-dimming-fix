# LG UltraGear Color Profile Service

A lightweight native Windows service (~1MB) that automatically reapplies the LG UltraGear no-dimming ICC color profile whenever displays are connected, sessions are unlocked, or users log on.

## Why a Service?

The PowerShell scheduled task approach had several reliability issues:
- **Session 0 isolation**: SYSTEM tasks can't access user display APIs
- **PowerShell startup overhead**: ~2 seconds per invocation
- **Script execution policy**: Can be blocked by group policy
- **Hang risk**: `SendMessageTimeout` can block in task context

This Rust service solves all of these by running as a proper Windows service with a message-only window that receives native device and session notifications.

## How It Works

1. Runs as a Windows service (LocalSystem)
2. Creates a hidden message-only window
3. Registers for `WM_DEVICECHANGE` via `RegisterDeviceNotificationW` (instant monitor plug/unplug)
4. Registers for `WM_WTSSESSION_CHANGE` via `WTSRegisterSessionNotification` (logon, unlock, console connect)
5. On trigger: enumerates monitors via WMI, toggles ICC profile (disassociate → reassociate), refreshes display

## Building

```powershell
# From the service/ directory
cargo build --release
```

The output binary will be at `target\release\lg-ultragear-color-svc.exe` (~1MB).

## Usage

All commands require Administrator privileges.

```powershell
# Install (default monitor pattern: "LG ULTRAGEAR")
.\lg-ultragear-color-svc.exe install

# Install with custom monitor pattern
.\lg-ultragear-color-svc.exe install "LG UltraGear"

# Start the service
.\lg-ultragear-color-svc.exe start
# or: sc start lg-ultragear-color-svc

# Check status
.\lg-ultragear-color-svc.exe status

# Stop the service
.\lg-ultragear-color-svc.exe stop

# Uninstall
.\lg-ultragear-color-svc.exe uninstall

# One-shot test (runs outside service context)
.\lg-ultragear-color-svc.exe run-once
.\lg-ultragear-color-svc.exe run-once "LG UltraGear"
```

## Event Triggers

| Event | Source | Delay |
|-------|--------|-------|
| Monitor plug/unplug | `RegisterDeviceNotificationW` (WM_DEVICECHANGE) | 1.5s |
| User logon | `WTSRegisterSessionNotification` (WTS_SESSION_LOGON) | 1.5s |
| Session unlock | `WTSRegisterSessionNotification` (WTS_SESSION_UNLOCK) | 1.5s |
| Console connect | `WTSRegisterSessionNotification` (WTS_CONSOLE_CONNECT) | 1.5s |

## Profile Reapply Pipeline

1. **Detect** — WMI query for matching monitors
2. **Disassociate** — `WcsDisassociateColorProfileFromDevice` (reverts to default)
3. **Pause** — 100ms for Windows to process
4. **Reassociate** — `WcsAssociateColorProfileWithDevice` (applies fix)
5. **Refresh** — `ChangeDisplaySettingsExW` + `WM_SETTINGCHANGE` + `InvalidateRect`
6. **Calibration** — Trigger `\Microsoft\Windows\WindowsColorSystem\Calibration Loader`

## Configuration

The monitor match pattern is stored in the registry:
```
HKLM\SYSTEM\CurrentControlSet\Services\lg-ultragear-color-svc\Parameters
  MonitorMatch = "LG ULTRAGEAR"
```

## Logs

Service events are written to the Windows Event Log under the `lg-ultragear-color-svc` source.
View with:
```powershell
Get-WinEvent -ProviderName lg-ultragear-color-svc -MaxEvents 20
```

## Resource Usage

- **Memory**: ~3MB working set
- **CPU**: 0% when idle (event-driven, no polling)
- **Disk**: ~1MB binary, zero temp files
