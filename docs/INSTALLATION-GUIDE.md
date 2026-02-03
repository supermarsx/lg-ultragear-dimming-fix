# Installation Files Guide

## ⭐ Recommended Installation (Simplest)

### `install.bat`
**The all-in-one solution** - Single file, just double-click!

**What it does:**
1. Installs the color profile to fix auto-dimming
2. Creates auto-reapply monitor for persistence
3. Handles all elevation and dependencies
4. Embedded PowerShell - no separate scripts needed

**Usage:**
```batch
# Windows: Just double-click
install.bat
```

**Why use this:**
- ✅ Single file contains everything
- ✅ No dependencies (calls other scripts but self-contained logic)
- ✅ Simplest user experience
- ✅ Works on any Windows system

---

## Component Files (Advanced Users)

### `install-monitor.ps1`
**Standalone auto-reapply monitor** - Self-contained, no dependencies

**Usage:**
```powershell
# Install monitor
.\install-monitor.ps1

# Uninstall monitor
.\install-monitor.ps1 -Uninstall

# Custom installer path
.\install-monitor.ps1 -InstallerPath "C:\path\to\installer.ps1"

# Custom monitor name
.\install-monitor.ps1 -MonitorNameMatch "LG"
```

**Features:**
- Works independently
- Auto-detects installer location
- Can uninstall cleanly
- No external dependencies except the main installer for reapplication

---

## Which One Should You Use?

```
┌─────────────────────────────────────────────────────────────┐
│  Want automatic fix that persists?                          │
│  → USE: install.bat                                         │
│  ✓ Single file, just double-click                           │
│  ✓ Installs profile + auto-reapply                          │
│  ✓ One click, done forever                                  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Only want profile, no auto-reapply?                        │
│  → USE: install-lg-ultragear-no-dimming.ps1                 │
│  or:    install-full-auto.bat                               │
│  ✓ Installs profile only                                    │
│  ✗ Resets after monitor disconnect                          │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Need to add/remove just the monitor?                       │
│  → USE: install-monitor.ps1                                 │
│  ✓ Standalone monitor management                            │
│  ✓ Can install or uninstall independently                   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Need to check if it's working?                             │
│  → USE: check-monitor-status.ps1                            │
│  ✓ Shows task status                                        │
│  ✓ Verifies profile installation                            │
│  ✓ Lists LG UltraGear monitors detected                     │
└─────────────────────────────────────────────────────────────┘
```

---

## File Structure

```
install.bat (⭐ START HERE)
    ↓ contains embedded PowerShell that:
    ├─→ Calls install-lg-ultragear-no-dimming.ps1 (installs profile)
    └─→ Creates scheduled task directly (no separate script)
            ↓ task runs on events:
        reapply-profile.ps1 (stored in %ProgramData%)
            ↓ calls:
        install-lg-ultragear-no-dimming.ps1 (reapplies profile)

install-monitor.ps1 (standalone)
    ↓ self-contained monitor installer
    ├─→ Can install monitor independently
    └─→ Can uninstall with -Uninstall flag
```

---

## Quick Reference

| File | Purpose | Self-Contained? | User-Facing? |
|------|---------|-----------------|--------------|
| **`install.bat`** | ⭐ Main installer (embedded logic) | YES | **START HERE** |
| `install-monitor.ps1` | Standalone monitor manager | YES | For monitor only |
| `install-lg-ultragear-no-dimming.ps1` | Core profile installer | YES | Can use standalone |
| `check-monitor-status.ps1` | Diagnostic tool | YES | For verification |
| `install-complete.bat/ps1` | Legacy two-script installer | NO | Still works |
| `install-with-auto-reapply.bat` | Legacy two-step installer | NO | Still works |
| `install-full-auto.bat` | Basic profile-only | YES | No auto-reapply |

---

## Management Commands

```powershell
# Install everything (recommended)
install.bat

# Check if it's working
.\check-monitor-status.ps1

# Uninstall auto-reapply monitor
.\install-monitor.ps1 -Uninstall

# Test manual trigger
Start-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"

# View task in Task Scheduler
taskschd.msc
```
