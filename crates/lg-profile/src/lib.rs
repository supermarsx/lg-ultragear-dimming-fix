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
use std::path::Path;
use std::{ptr, thread, time::Duration};
use windows::core::{BSTR, HSTRING, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{ChangeDisplaySettingsExW, InvalidateRect};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::TaskScheduler::{ITaskService, TaskScheduler};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

// ============================================================================
// Embedded ICC profile
// ============================================================================

/// The ICC profile bytes, embedded at compile time from the repo root.
const EMBEDDED_ICM: &[u8] = include_bytes!("../../../lg-ultragear-full-cal.icm");

/// Size of the embedded ICC profile in bytes (useful for tests).
pub const EMBEDDED_ICM_SIZE: usize = EMBEDDED_ICM.len();

/// Ensure the ICC profile is installed in the Windows color store.
///
/// If the file already exists and matches the embedded size, this is a no-op.
/// Otherwise, writes (or overwrites) the embedded profile to the color directory.
///
/// Returns `Ok(true)` if a new file was written, `Ok(false)` if already present.
pub fn ensure_profile_installed(profile_path: &Path) -> Result<bool, Box<dyn Error>> {
    // Check if it already exists with the correct size
    if let Ok(meta) = std::fs::metadata(profile_path) {
        if meta.len() == EMBEDDED_ICM.len() as u64 {
            info!("ICC profile already installed: {}", profile_path.display());
            return Ok(false);
        }
    }

    // Ensure the parent directory exists
    if let Some(parent) = profile_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(profile_path, EMBEDDED_ICM)?;
    info!("ICC profile extracted to {}", profile_path.display());
    Ok(true)
}

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

/// Remove the ICC profile from the Windows color store.
///
/// Returns `Ok(true)` if the file was removed, `Ok(false)` if it didn't exist.
pub fn remove_profile(profile_path: &Path) -> Result<bool, Box<dyn Error>> {
    if !profile_path.exists() {
        info!("ICC profile not present: {}", profile_path.display());
        return Ok(false);
    }
    std::fs::remove_file(profile_path)?;
    info!("ICC profile removed: {}", profile_path.display());
    Ok(true)
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

    let profile_wide: Vec<u16> = profile_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let device_wide = to_wide(device_key);

    unsafe {
        // Step 1: Disassociate (reverts to default profile)
        // Failure here is non-fatal — the profile may not be currently associated.
        let result = WcsDisassociateColorProfileFromDevice(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
        );
        if !result.as_bool() {
            warn!(
                "WcsDisassociateColorProfileFromDevice failed for {} (non-fatal)",
                device_key
            );
        }

        // Step 2: Configurable pause to let Windows process the change
        thread::sleep(Duration::from_millis(toggle_delay_ms));

        // Step 3: Re-associate (applies the fix profile)
        // Failure here IS fatal — the profile was NOT applied.
        let result = WcsAssociateColorProfileWithDevice(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
        );
        if !result.as_bool() {
            return Err(format!(
                "WcsAssociateColorProfileWithDevice failed for {}",
                device_key
            )
            .into());
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
/// Uses the COM Task Scheduler API directly (no external process spawning).
/// If `enabled` is false, returns immediately.
pub fn trigger_calibration_loader(enabled: bool) {
    if !enabled {
        return;
    }

    match run_calibration_loader_task() {
        Ok(()) => info!("Calibration Loader task triggered"),
        Err(e) => warn!("Calibration Loader task trigger failed: {}", e),
    }
}

/// Run the Windows Calibration Loader scheduled task via COM Task Scheduler API.
fn run_calibration_loader_task() -> Result<(), Box<dyn Error>> {
    // Initialize COM on this thread (balanced with CoUninitialize below).
    // ok() ignores S_FALSE (already initialized with same apartment model).
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED).ok();
    }

    let result = (|| -> Result<(), Box<dyn Error>> {
        let service: ITaskService =
            unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)? };

        // Connect to local Task Scheduler with current credentials
        let empty = windows::core::VARIANT::default();
        unsafe {
            service.Connect(&empty, &empty, &empty, &empty)?;
        }

        let folder =
            unsafe { service.GetFolder(&BSTR::from(r"\Microsoft\Windows\WindowsColorSystem"))? };

        let task = unsafe { folder.GetTask(&BSTR::from("Calibration Loader"))? };

        let _ = unsafe { task.Run(&windows::core::VARIANT::default())? };

        Ok(())
    })();

    unsafe {
        CoUninitialize();
    }

    result
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
