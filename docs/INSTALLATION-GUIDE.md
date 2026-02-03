# Installation Files Guide

## Recommended Installation (Simplest)

### 🌟 `install-complete.bat` + `install-complete.ps1`
**The all-in-one solution** - Just double-click and you're done!

**What it does:**
1. Installs the color profile to fix auto-dimming
2. Creates auto-reapply monitor for persistence
3. Handles all elevation and dependencies

**Usage:**
```batch
# Windows: Just double-click
install-complete.bat

# PowerShell:
.\install-complete.ps1

# Options:
.\install-complete.ps1 -SkipMonitor          # Profile only, no auto-reapply
.\install-complete.ps1 -UninstallMonitor     # Remove auto-reapply monitor
```

---

## Component Files

### Core Components
- **`install-lg-ultragear-no-dimming.ps1`** - Main installer (called by other scripts)
- **`install-monitor-watcher.ps1`** - Creates the auto-reapply scheduled task
- **`lg-ultragear-full-cal.icm`** - The color profile that fixes dimming

### Status & Diagnostics
- **`check-monitor-status.ps1`** - Verify installation and check if auto-reapply is working

---

## Alternative Installers (Legacy)

### `install-with-auto-reapply.bat`
Two-step installer that runs both profile installer and monitor watcher sequentially.
- Still works fine
- More verbose output
- Use `install-complete.bat` instead for cleaner experience

### `install-full-auto.bat`
Basic installer - **profile only, no auto-reapply**
- Use when you only want one-time profile installation
- Won't persist after monitor reconnection
- Not recommended unless you have specific needs

---

## Which One Should You Use?

```
┌─────────────────────────────────────────────────────────────┐
│  Want automatic fix that persists?                          │
│  → USE: install-complete.bat                                │
│  ✓ Installs profile                                         │
│  ✓ Auto-reapplies on reconnection                           │
│  ✓ One click, done forever                                  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Only want profile, no auto-reapply?                        │
│  → USE: install-complete.ps1 -SkipMonitor                   │
│  or:    install-full-auto.bat                               │
│  ✓ Installs profile only                                    │
│  ✗ Resets after monitor disconnect                          │
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

## File Relationships

```
install-complete.bat
    ↓ launches
install-complete.ps1
    ↓ calls
    ├─→ install-lg-ultragear-no-dimming.ps1 (installs profile)
    └─→ install-monitor-watcher.ps1 (creates auto-reapply task)
            ↓ creates scheduled task that runs
        reapply-profile.ps1 (stored in %ProgramData%)
            ↓ calls
        install-lg-ultragear-no-dimming.ps1 (reapplies profile)
```

---

## Quick Reference

| File | Purpose | User-facing? |
|------|---------|-------------|
| `install-complete.bat` | Main entry point (launcher) | ⭐ YES - START HERE |
| `install-complete.ps1` | All-in-one installer | YES |
| `install-lg-ultragear-no-dimming.ps1` | Core profile installer | Usually called by others |
| `install-monitor-watcher.ps1` | Auto-reapply setup | Can use standalone |
| `check-monitor-status.ps1` | Diagnostic tool | YES - for verification |
| `install-with-auto-reapply.bat` | Legacy two-step installer | Still works, but use complete |
| `install-full-auto.bat` | Basic profile-only installer | Use if you don't want auto-reapply |

---

## Management Commands

```powershell
# Install everything (recommended)
.\install-complete.bat

# Check if it's working
.\check-monitor-status.ps1

# Uninstall auto-reapply monitor
.\install-complete.ps1 -UninstallMonitor

# Test manual trigger
Start-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"

# View task in Task Scheduler
taskschd.msc
```
