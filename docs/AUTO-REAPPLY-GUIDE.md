# LG UltraGear Auto-Reapply Fix - Quick Reference

## The Problem
The color profile fix works but doesn't stay persistent after:
- Monitor disconnect/reconnect
- System sleep/wake  
- Display driver updates
- Graphics settings changes

## The Solution
Use the **event-driven scheduled task monitor** for automatic reapplication.

---

## Installation (Choose One)

### ✅ RECOMMENDED: Auto-Reapply (Persistent Fix)
```batch
install-with-auto-reapply.bat
```
This installs the profile AND creates a lightweight monitor that auto-reapplies it.

### Basic: One-Time Apply
```batch
install-full-auto.bat
```
⚠️ Applies once but won't auto-reapply on reconnection.

---

## How It Works

1. **Scheduled Task**: Creates `LG-UltraGear-ColorProfile-AutoReapply` task
2. **Event Triggers**: Monitors Windows Event Log for display device events
3. **Zero Overhead**: No polling, no background services - only activates on actual events
4. **Toggle Reapply**: Disassociates the profile first, then re-associates it to force Windows to refresh
5. **Runs as SYSTEM**: Highest privilege level ensures reliable color profile changes
6. **Triggers On**:
   - Display device plug/unplug (Event ID 20001, 20003)
   - System unlock/wake
   - User logon

---

## Management Commands

### Check if monitor is installed
```powershell
Get-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"
```

### Manually trigger (test)
```powershell
Start-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"
```

### View task history
1. Open Task Scheduler: `taskschd.msc`
2. Navigate to: Task Scheduler Library
3. Find: `LG-UltraGear-ColorProfile-AutoReapply`
4. Click "History" tab

### Uninstall monitor
```powershell
.\install-monitor-watcher.ps1 -Uninstall
```

---

## Technical Details

**Location of files:**
- Profile: `%WINDIR%\System32\spool\drivers\color\lg-ultragear-full-cal.icm`
- Reapply script: `%ProgramData%\LG-UltraGear-Monitor\reapply-profile.ps1`
- Scheduled task: `LG-UltraGear-ColorProfile-AutoReapply`

**Runs as:** Administrators group (user session, elevated) - required to access display APIs

**Execution:** Hidden window, no user interruption

**Performance:** Event-driven only - zero CPU/memory when idle

---

## Troubleshooting

### Profile still resets?
1. Verify task exists: `Get-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"`
2. Check task history in Task Scheduler
3. Manually trigger: `Start-ScheduledTask -TaskName "LG-UltraGear-ColorProfile-AutoReapply"`
4. Reinstall: run `install-with-auto-reapply.bat` again

### Still experiencing dimming?
- Check monitor OSD for "Energy Saving" options (disable them)
- Ensure Windows HDR is OFF for SDR content
- Some models have firmware-level dimming that can't be fully disabled

---

## Why This Works Better Than Other Solutions

❌ **Manual reapplication**: Too tedious  
❌ **Polling scripts**: Wastes resources checking constantly  
❌ **Login scripts only**: Misses reconnection events  
✅ **Event-driven task**: Lightweight, automatic, reliable
