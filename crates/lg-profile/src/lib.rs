//! Color profile management — disassociate, reassociate, and refresh.
//!
//! Uses `mscms.dll` (WCS APIs) directly via the `windows` crate for reliable
//! color profile toggling, plus display refresh via `user32.dll`.
//!
//! All functions take raw parameters (no Config dependency) so this crate
//! can be used independently.

use log::{info, warn};
use std::error::Error;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::{ptr, thread, time::Duration};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{ChangeDisplaySettingsExW, InvalidateRect};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

// ============================================================================
// mscms.dll FFI — WCS color profile APIs
// ============================================================================

/// Scope constant for system-wide color profile operations.
const WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE: u32 = 2;

// These are not in the `windows` crate metadata, so we link manually.
#[link(name = "mscms")]
extern "system" {
    fn WcsAssociateColorProfileWithDevice(
        scope: u32,
        profile_name: PCWSTR,
        device_name: PCWSTR,
    ) -> BOOL;

    fn WcsDisassociateColorProfileFromDevice(
        scope: u32,
        profile_name: PCWSTR,
        device_name: PCWSTR,
    ) -> BOOL;
}

/// Check if the ICC profile is installed at the given path.
pub fn is_profile_installed(profile_path: &Path) -> bool {
    profile_path.exists()
}

/// Reapply the color profile for a single monitor device key using the toggle
/// approach: disassociate (reverts to default) → pause → reassociate (applies fix).
/// This forces Windows to actually reload the ICC profile.
///
/// # Arguments
/// * `device_key` — WMI device instance path (e.g. `DISPLAY\LGS\001`)
/// * `profile_path` — Full path to the ICC profile file
/// * `toggle_delay_ms` — Pause between disassociate and reassociate (ms)
pub fn reapply_profile(
    device_key: &str,
    profile_path: &Path,
    toggle_delay_ms: u64,
) -> Result<(), Box<dyn Error>> {
    if !profile_path.exists() {
        return Err(format!("Profile not found: {}", profile_path.display()).into());
    }

    let profile_str = profile_path.to_string_lossy().to_string();
    let profile_wide = to_wide(&profile_str);
    let device_wide = to_wide(device_key);

    unsafe {
        // Step 1: Disassociate (reverts to default profile)
        let result = WcsDisassociateColorProfileFromDevice(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
        );
        if !result.as_bool() {
            warn!(
                "WcsDisassociateColorProfileFromDevice failed for {}",
                device_key
            );
        }

        // Step 2: Configurable pause to let Windows process the change
        thread::sleep(Duration::from_millis(toggle_delay_ms));

        // Step 3: Re-associate (applies the fix profile)
        let result = WcsAssociateColorProfileWithDevice(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
        );
        if !result.as_bool() {
            warn!(
                "WcsAssociateColorProfileWithDevice failed for {}",
                device_key
            );
        }
    }

    info!("Profile toggled for device: {}", device_key);
    Ok(())
}

/// Force display refresh using the specified Windows APIs.
///
/// # Arguments
/// * `display_settings` — Call `ChangeDisplaySettingsExW` (full display refresh)
/// * `broadcast_color` — Broadcast `WM_SETTINGCHANGE` with "Color" parameter
/// * `invalidate` — Call `InvalidateRect` to force window repaint
pub fn refresh_display(display_settings: bool, broadcast_color: bool, invalidate: bool) {
    unsafe {
        // Method 1: ChangeDisplaySettingsEx with null — triggers full display mode refresh
        if display_settings {
            let _ = ChangeDisplaySettingsExW(
                PCWSTR(ptr::null()),
                None,
                HWND::default(),
                Default::default(),
                None,
            );
        }

        // Method 2: Broadcast WM_SETTINGCHANGE with "Color" parameter
        if broadcast_color {
            let color = HSTRING::from("Color");
            let mut _result = 0usize;
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(color.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                2000,
                Some(&mut _result),
            );
        }

        // Method 3: Invalidate all windows to force repaint
        if invalidate {
            let _ = InvalidateRect(HWND::default(), None, true);
        }
    }

    info!("Display refresh broadcast sent");
}

/// Trigger the built-in Windows Calibration Loader scheduled task.
///
/// If `enabled` is false, returns immediately.
pub fn trigger_calibration_loader(enabled: bool) {
    if !enabled {
        return;
    }

    // Use schtasks.exe — simplest reliable way from a service context
    let result = std::process::Command::new("schtasks.exe")
        .args([
            "/Run",
            "/TN",
            r"\Microsoft\Windows\WindowsColorSystem\Calibration Loader",
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    match result {
        Ok(output) if output.status.success() => {
            info!("Calibration Loader task triggered");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Calibration Loader task trigger failed: {}", stderr.trim());
        }
        Err(e) => {
            warn!("Failed to run schtasks.exe: {}", e);
        }
    }
}

/// Convert a Rust string to a null-terminated wide string (UTF-16).
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
#[path = "tests/profile_tests.rs"]
mod tests;
