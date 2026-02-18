<img width="350" height="350" alt="lg-ultragear-dimming-fix" src="https://github.com/user-attachments/assets/4530aed6-d98c-423a-932b-defb3655cffe" />

# LG UltraGear Auto-Dimming Fix | Stop Screen Dimming on Gaming Monitors

[![stars](https://img.shields.io/github/stars/supermarsx/lg-ultragear-dimming-fix?style=flat-square&color=ffd700)](https://github.com/supermarsx/lg-ultragear-dimming-fix/stargazers)
[![watchers](https://img.shields.io/github/watchers/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/watchers)
[![forks](https://img.shields.io/github/forks/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/forks)
[![issues](https://img.shields.io/github/issues/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/issues)
[![downloads](https://img.shields.io/github/downloads/supermarsx/lg-ultragear-dimming-fix/total?style=flat-square)](https://github.com/supermarsx/lg-ultragear-dimming-fix/releases)
[![built with](https://img.shields.io/badge/built%20with-rust-DEA584?style=flat-square&logo=rust)](#)
[![made for](https://img.shields.io/badge/made%20for-windows-0078D6?style=flat-square)](#)
[![license](https://img.shields.io/github/license/supermarsx/lg-ultragear-dimming-fix?style=flat-square)](license.md)

[![download-latest](https://img.shields.io/badge/Download-Latest%20Release-2ea44f?style=for-the-badge&logo=github)](https://github.com/supermarsx/lg-ultragear-dimming-fix/releases/latest)

> 💡 **Quick Start:** Download `lg-ultragear-dimming-fix.exe`, run it, and the interactive menu takes care of the rest.

## Fix LG UltraGear Monitor Auto-Dimming Problems

**lg-ultragear-dimming-fix** is a single native Windows binary that stops auto-dimming on LG UltraGear LCD monitors. It installs a calibrated ICC color profile, detects your monitors via WMI, and can run as a Windows service to automatically reapply the fix whenever displays reconnect, the session unlocks, or the system wakes from sleep.

No PowerShell. No scripts. No dependencies. Just one `.exe`.

### What This Tool Does

- **Purpose**: Stop LG UltraGear gaming monitors from dimming under static or semi-static content by constraining the panel's effective luminance range.
- **Method**: Apply and set a custom ICC/ICM color profile that limits the tone response so the monitor's firmware auto-dimming heuristic doesn't trigger.
- **Persistence**: A Windows service monitors display-connect, session-unlock, and logon events and reapplies the profile automatically — surviving reconnects, sleep/wake, and reboots.
- **Platform**: Windows 10/11 — native Win32/WinRT APIs, no runtime dependencies.
- **Compatible Models**: Works with most LG UltraGear series monitors including 27GL850, 27GN950, 38GN950, 34GN850, and many others experiencing unexpected dimming.

### Common Problems This Fixes

- ✅ Screen dims when displaying bright or white content
- ✅ Monitor brightness fluctuates during gaming sessions
- ✅ Unexpected darkening with static images or productivity apps
- ✅ ABL (Automatic Brightness Limiting) cannot be disabled in OSD
- ✅ Brightness inconsistency affecting competitive gaming
- ✅ Eye strain from constant brightness changes
- ✅ Profile resets after monitor reconnection, sleep, or reboot


## Why This Works

Many LG UltraGear models use firmware-level auto-dimming (ABL — Automatic Brightness Limiting) that activates when:
- Average Picture Level (APL) stays high
- Content appears static or semi-static
- Bright colors dominate the screen

This dimming behavior is frustrating for gamers and professionals because:
- It reduces visibility in competitive gaming
- Creates inconsistent viewing experience
- Cannot be disabled through monitor OSD settings
- Persists even with power-saving options disabled

**The solution** uses a custom ICC color profile that constrains the effective luminance range Windows sends to the display. By limiting the tone response curve, the monitor's firmware never reaches the threshold that triggers auto-dimming, maintaining consistent brightness.

### Software-Only Fix — No Hardware Modifications

- Nothing on the monitor is flashed or permanently changed
- You can revert at any time by removing the color profile
- Works immediately without firmware updates
- Safe for your monitor warranty

### Why Other Solutions Don't Work

**❌ Disabling OSD power-saving options** — Often does not fully disable firmware-level dimming on UltraGear models
**❌ Windows/GPU settings changes** — Disabling adaptive brightness, CABC, or toggling HDR commonly fails to stop firmware behavior
**❌ Waiting for firmware updates** — Many screens either have no user-accessible updates, or LG hasn't released fixes
**✅ Color profile approach** — Works immediately with current firmware, no waiting required


## Quick Start

### Download and Run

1. Download **`lg-ultragear-dimming-fix.exe`** from the [latest release](https://github.com/supermarsx/lg-ultragear-dimming-fix/releases/latest)
2. Run it (a UAC prompt will appear — admin is required for color profile and service installation)
3. The interactive TUI opens automatically — press **1** for a full install (profile + service)

That's it. The service handles persistence from here.

### Interactive TUI

When run without arguments, the tool opens an interactive terminal menu:

```
╔════════════════════════════════════════════════════════════════════════════╗
║                    LG UltraGear Auto-Dimming Fix                           ║
╟────────────────────────────────────────────────────────────────────────────╢
║  Status: Profile ✓  Service ✓  Running ✓  Monitors: 1                     ║
╟────────────────────────────────────────────────────────────────────────────╢
║  1. Install (profile + service)     6. Detect monitors                     ║
║  2. Install profile only            7. Remove service                      ║
║  3. Install service only            8. Remove profile                      ║
║  4. Refresh / reapply profile       9. Full uninstall                      ║
║  5. Reinstall everything            A. Advanced options                    ║
║                                     Q. Quit                                ║
╚════════════════════════════════════════════════════════════════════════════╝
```

Advanced options let you toggle toast notifications, dry-run mode, and verbose output.

### CLI Mode

For scripting, automation, or headless environments, use subcommands directly:

```powershell
# Install profile + service (detects LG UltraGear monitors automatically)
lg-ultragear-dimming-fix.exe install

# Install profile only, no service
lg-ultragear-dimming-fix.exe install --profile-only

# Install service only
lg-ultragear-dimming-fix.exe install --service-only

# Detect monitors matching a pattern
lg-ultragear-dimming-fix.exe detect
lg-ultragear-dimming-fix.exe detect --pattern "LG"

# One-shot profile reapply
lg-ultragear-dimming-fix.exe apply

# Run event watcher in foreground (Ctrl+C to stop)
lg-ultragear-dimming-fix.exe watch

# Uninstall service
lg-ultragear-dimming-fix.exe uninstall

# Full uninstall (service + profile + config)
lg-ultragear-dimming-fix.exe uninstall --full

# Reinstall everything
lg-ultragear-dimming-fix.exe reinstall

# View / manage configuration
lg-ultragear-dimming-fix.exe config show
lg-ultragear-dimming-fix.exe config path
lg-ultragear-dimming-fix.exe config reset

# Windows service control (advanced)
lg-ultragear-dimming-fix.exe service install
lg-ultragear-dimming-fix.exe service start
lg-ultragear-dimming-fix.exe service stop
lg-ultragear-dimming-fix.exe service status
lg-ultragear-dimming-fix.exe service uninstall
```

### CLI Reference

#### Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output |
| `--dry-run` | | Simulate operations without making changes |
| `--non-interactive` | | Force CLI mode (skip TUI even if a terminal is attached) |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |

#### Commands

**Install / Uninstall / Reinstall**

| Command | Flags | Description |
|---------|-------|-------------|
| `install` | `--pattern <TEXT>` `-p` | Install color profile and/or service. Pattern overrides the default monitor name match. |
| | `--profile-only` | Install ICC profile only (no service) |
| | `--service-only` | Install service only (skip profile extraction) |
| `uninstall` | | Uninstall service |
| | `--full` | Remove everything (service + profile + config) |
| | `--profile` | Also remove the ICC profile from the color store |
| `reinstall` | `--pattern <TEXT>` `-p` | Clean reinstall (uninstall then install) |

**Monitor & Profile**

| Command | Flags | Description |
|---------|-------|-------------|
| `detect` | `--pattern <TEXT>` `-p` | Detect connected monitors matching a pattern |
| `apply` | `--pattern <TEXT>` `-p` | One-shot profile reapply for matching monitors |
| `watch` | `--pattern <TEXT>` `-p` | Run event watcher in foreground (Ctrl+C to stop) |

**Configuration**

| Command | Flags | Description |
|---------|-------|-------------|
| `config show` | | Show current configuration |
| `config path` | | Print config file path |
| `config reset` | | Reset config to defaults |

**Service Management**

| Command | Flags | Description |
|---------|-------|-------------|
| `service install` | `--pattern <TEXT>` `-p` | Install the Windows service |
| `service uninstall` | | Uninstall the Windows service |
| `service start` | | Start the service |
| `service stop` | | Stop the service |
| `service status` | | Show service status |


## Manual Install (No Tool)

If you prefer not to run any executables, you can apply the profile manually:

1. Get `lg-ultragear-full-cal.icm` from this repo or the release zip
2. Copy it to `%WINDIR%\System32\spool\drivers\color` (requires admin)
3. Press `Win+R`, type `colorcpl`, press Enter
4. Go to the **Devices** tab, select your LG UltraGear monitor
5. Check **"Use my settings for this device"**
6. Click **Add…**, browse to the `.icm` file, select it
7. Click **Set as Default Profile**

> ⚠️ This manual method does **not** persist across reconnections. Use the tool with the service for automatic reapplication.


## How It Works (Technical)

### Architecture

The tool is built as a Rust workspace with six crates:

| Crate | Purpose |
|-------|---------|
| **lg-cli** | CLI entry point (clap) + interactive TUI (crossterm) |
| **lg-core** | Shared configuration (TOML-based, stored in `%ProgramData%`) |
| **lg-monitor** | WMI-based monitor discovery (`WmiMonitorId` queries) |
| **lg-profile** | ICC profile management via Windows Color System (`mscms.dll`) |
| **lg-notify** | Toast notifications via WinRT (`ToastNotificationManager`) |
| **lg-service** | Windows service runtime (SCM, device notifications, session events) |

### Profile Installation

- The ICC profile (`lg-ultragear-full-cal.icm`) is **embedded in the binary** via `include_bytes!` — no external files needed at runtime
- On install, the profile is extracted to `%WINDIR%\System32\spool\drivers\color`
- Profile is associated with matching display device keys via `WcsAssociateColorProfileWithDevice` / `WcsDisassociateColorProfileFromDevice`
- Display settings are refreshed and the Calibration Loader task is triggered via COM Task Scheduler

### Monitor Detection

- Uses WMI `WmiMonitorId` to enumerate connected displays
- Matches by user-friendly name (case-insensitive substring, default: `"LG ULTRAGEAR"`)
- Override with `--pattern` flag or `monitor_name_match` in config

### Service Mode

The Windows service (`lg-ultragear-color-svc`) listens for:
- **Device interface notifications** (`DBT_DEVICEARRIVAL` / `DBT_DEVICEREMOVECOMPLETE`) — monitor connect/disconnect
- **Session change events** (`WTS_SESSION_UNLOCK`, `WTS_SESSION_LOGON`) — session unlock, logon
- **Display change messages** (`WM_DISPLAYCHANGE`) — resolution/display topology changes

Events are debounced and trigger a profile reapply cycle: disassociate → reassociate → refresh → trigger Calibration Loader.

### Configuration

Configuration is stored at `%ProgramData%\LG-UltraGear-Monitor\config.toml`:

```toml
monitor_name_match = "LG ULTRAGEAR"
verbose = false
dry_run = false
toast_enabled = true
toast_title = "LG UltraGear"
toast_body = "Color profile reapplied"
refresh_enabled = true
invalidate_enabled = true
calibration_loader_enabled = true
debounce_ms = 3000
```

### File Locations

| Item | Path |
|------|------|
| Binary | `%ProgramData%\LG-UltraGear-Monitor\lg-ultragear-dimming-fix.exe` |
| Config | `%ProgramData%\LG-UltraGear-Monitor\config.toml` |
| Profile | `%WINDIR%\System32\spool\drivers\color\lg-ultragear-full-cal.icm` |


## Security / Permissions

- Installing into the system color store and registering a Windows service requires **administrator** privileges
- A UAC prompt will appear when the tool needs elevation
- The service runs as `LocalSystem` for access to the color store and device notifications
- No network access, no telemetry, no external dependencies


## Verification

- **Color Management UI**: Press `Win+R`, run `colorcpl` → Devices tab → select your LG UltraGear → confirm `lg-ultragear-full-cal.icm` is present and set as default
- **Service status**: Run `lg-ultragear-dimming-fix.exe service status` or check in `services.msc`
- **Visual check**: Leave a bright, mostly static window open — dimming should be gone or greatly reduced
- **Monitor detection**: Run `lg-ultragear-dimming-fix.exe detect` to see matched displays


## Troubleshooting

### The profile resets after reconnection or sleep
- Install the service: run the tool and press **1** (Install profile + service), or use `lg-ultragear-dimming-fix.exe install`
- Verify the service is running: `lg-ultragear-dimming-fix.exe service status`

### The profile is applied but dimming still occurs
- Some LG UltraGear models have multiple dimming mechanisms
- Disable "Energy Saving" or "Smart Energy Saving" in the monitor's OSD menu
- Ensure HDR is disabled in Windows settings if you're using SDR content
- Some models have more aggressive ABL that requires additional OSD tuning

### Screen dims only on certain colors or scenes
- The profile prevents aggressive dimming but may not eliminate all firmware-level ABL
- Try adjusting monitor brightness and contrast in OSD
- Some models have more aggressive ABL than others

### Gaming performance concerns
- This fix does **not** impact gaming performance or FPS
- Works alongside G-SYNC, FreeSync, and other monitor features
- No input lag or response time changes

### Completely uninstall everything

```powershell
lg-ultragear-dimming-fix.exe uninstall --full
```

This removes the service, the ICC profile from the color store, and the config file.

### Rollback / revert (manual)
- `colorcpl` → Devices → select display → choose another default or uncheck "Use my settings for this device"
- Delete the profile from `%WINDIR%\System32\spool\drivers\color` (admin required)


## Building from Source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- Windows 10/11 SDK (for Win32/WinRT bindings)

### Build

```powershell
# Debug build
cargo build

# Release build (optimized, LTO, stripped)
cargo build --release

# Run tests (317 tests across all crates)
cargo test --all-targets

# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --all-targets --all-features -- -D warnings
```

The release binary is at `target\release\lg-ultragear-dimming-fix.exe`.

### Project Structure

```
crates/
  lg-cli/        CLI entry point + interactive TUI
  lg-core/       Configuration management (TOML)
  lg-monitor/    WMI monitor detection
  lg-profile/    ICC profile + WCS APIs
  lg-notify/     WinRT toast notifications
  lg-service/    Windows service runtime
legacy/          Original PowerShell/batch installer (archived)
docs/            Additional guides
```


## Downloads & Releases

Automated CI produces versioned releases. Each release includes:

- **`lg-ultragear-dimming-fix.exe`** — standalone native binary (everything included)
- **`lg-ultragear-dimming-fix.zip`** — binary + ICM profile + readme + license


## License

See [license.md](license.md) for licensing details.


## FAQ

### Does this work with all LG UltraGear monitors?
Tested and confirmed on many models including 27GL850, 27GL83A, 27GN950, 27GN850, 27GN800, 34GN850, 38GN950, 32GN650, 32GP850, and others. If your model isn't listed, try it — the fix is safe and reversible.

### Will this affect color accuracy?
The profile constrains the luminance range slightly to prevent dimming triggers. Most users report no noticeable color difference in normal use. For professional color-critical work, you may want to revert the profile.

### Does this work with HDR content?
This fix primarily targets SDR content. HDR behavior varies by model.

### Can I use this with NVIDIA or AMD graphics cards?
Yes. The fix operates at the Windows Color System level, independent of GPU vendor or driver.

### Will this void my monitor warranty?
No. This is a software-only fix that does not modify firmware or hardware.

### What's the difference between "watch" and the service?
`watch` runs the event watcher in the foreground (Ctrl+C to stop). The service runs in the background permanently, starting automatically with Windows.

### Can I uninstall this easily?
Yes. Run `lg-ultragear-dimming-fix.exe uninstall --full` to remove everything (service, profile, config).


## Keywords

LG UltraGear dimming fix, LG monitor auto dimming, LG UltraGear brightness problem, stop LG monitor from dimming, LG ABL disable, LG automatic brightness limiting, gaming monitor dimming issue, LG 27GL850 dimming, LG 27GN950 dimming, LG UltraGear screen darkening, fix monitor brightness fluctuation, LG gaming monitor dimming Windows 10, LG gaming monitor dimming Windows 11, disable ABL LG UltraGear, ICC profile fix dimming, color profile dimming solution
