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
use std::io;
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
/// After the file is placed, calls `InstallColorProfileW` to register the
/// profile with the Windows Color System — the WCS APIs (`WcsAssociate…`,
/// `WcsDisassociate…`) require profiles to be registered, not merely present
/// on disk.
///
/// Returns `Ok(true)` if a new file was written, `Ok(false)` if already present.
pub fn ensure_profile_installed(profile_path: &Path) -> Result<bool, Box<dyn Error>> {
    // Check if it already exists with the correct size
    if let Ok(meta) = std::fs::metadata(profile_path) {
        if meta.len() == EMBEDDED_ICM.len() as u64 {
            info!("ICC profile already installed: {}", profile_path.display());
            // Even when the file exists, ensure it is registered with WCS.
            register_color_profile(profile_path)?;
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

    // Register with WCS so WcsAssociateColorProfileWithDevice will succeed.
    register_color_profile(profile_path)?;

    Ok(true)
}

/// Register an ICC profile with the Windows Color System via
/// `InstallColorProfileW` (mscms.dll).
///
/// This lets the WCS association/disassociation APIs find the profile.
/// Calling it on an already-registered profile is harmless.
pub fn register_color_profile(profile_path: &Path) -> Result<(), Box<dyn Error>> {
    let path_wide: Vec<u16> = profile_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let ok = unsafe { InstallColorProfileW(PCWSTR(ptr::null()), PCWSTR(path_wide.as_ptr())) };

    if !ok.as_bool() {
        let code = io::Error::last_os_error();
        // Non-fatal: log a warning but do not block the install pipeline.
        // Common failure: running without admin rights on a system-wide path.
        warn!(
            "InstallColorProfileW returned false for {} ({})",
            profile_path.display(),
            code
        );
    } else {
        info!(
            "Profile registered with WCS: {}",
            profile_path.display()
        );
    }

    Ok(())
}

// ============================================================================
// mscms.dll FFI — WCS color profile APIs
// ============================================================================

/// Scope constant for system-wide color profile operations.
const WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE: u32 = 2;

/// Scope constant for per-user (current user) color profile operations.
const WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER: u32 = 1;

/// Color Profile Type: ICC profile.
const CPT_ICC: i32 = 1; // COLORPROFILETYPE::CPT_ICC

/// Color Profile Subtype: device default.
const CPST_NONE: i32 = 1; // COLORPROFILESUBTYPE::CPST_NONE

/// SDR profile type for `ColorProfileSetDisplayDefaultAssociation`.
const COLOR_PROFILE_TYPE_SDR: u32 = 0;

/// SDR profile subtype for `ColorProfileSetDisplayDefaultAssociation`.
const COLOR_PROFILE_SUBTYPE_SDR: u32 = 0;

// These are not in the `windows` crate metadata, so we link manually.
#[link(name = "mscms")]
extern "system" {
    fn InstallColorProfileW(machine_name: PCWSTR, profile_name: PCWSTR) -> BOOL;

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

    fn WcsSetDefaultColorProfile(
        scope: u32,
        device_name: PCWSTR,
        cpt: i32,
        cpst: i32,
        profile_id: u32,
        profile_name: PCWSTR,
    ) -> BOOL;

    /// Modern Win10+ API: sets the SDR default profile for a display.
    /// This is what the Color Management control panel calls when you
    /// select a profile — it triggers the WCS engine to actually apply
    /// the profile to the display pipeline.
    fn ColorProfileSetDisplayDefaultAssociation(
        profile_name: PCWSTR,
        device_name: PCWSTR,
        scope: u32,
        profile_type: u32,
        profile_sub_type: u32,
        profile_id: u32,
    ) -> BOOL;

    /// Modern Win10+ API: adds a profile to the HDR/advanced-color association
    /// for a display.
    fn ColorProfileAddDisplayAssociation(
        profile_name: PCWSTR,
        device_name: PCWSTR,
        scope: u32,
        profile_type: u32,
    ) -> BOOL;
}

/// Check if the ICC profile is installed at the given path.
pub fn is_profile_installed(profile_path: &Path) -> bool {
    profile_path.exists()
}

/// Remove the ICC profile from the Windows color store.
///
/// Retries with exponential back-off if the file is locked (e.g. by the WCS
/// engine or the service process).  After all retries, schedules the file for
/// deletion on next reboot via `MoveFileExW(MOVEFILE_DELAY_UNTIL_REBOOT)`.
///
/// Returns `Ok(true)` if the file was removed (or scheduled for removal),
/// `Ok(false)` if it didn't exist.
pub fn remove_profile(profile_path: &Path) -> Result<bool, Box<dyn Error>> {
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};

    if !profile_path.exists() {
        info!("ICC profile not present: {}", profile_path.display());
        return Ok(false);
    }

    // Retry up to 5 times with increasing back-off (total ~3 s).
    let delays_ms: &[u64] = &[0, 200, 500, 1000, 1500];
    for (attempt, &ms) in delays_ms.iter().enumerate() {
        if ms > 0 {
            thread::sleep(Duration::from_millis(ms));
        }
        match std::fs::remove_file(profile_path) {
            Ok(()) => {
                info!("ICC profile removed: {} (attempt {})", profile_path.display(), attempt + 1);
                return Ok(true);
            }
            Err(e) if e.raw_os_error() == Some(32) => {
                // ERROR_SHARING_VIOLATION — file is locked, retry.
                info!(
                    "Profile locked (attempt {}): {} — retrying",
                    attempt + 1,
                    profile_path.display()
                );
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    // Last resort: schedule for deletion on next reboot.
    let wide: Vec<u16> = profile_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe { MoveFileExW(PCWSTR(wide.as_ptr()), None, MOVEFILE_DELAY_UNTIL_REBOOT) };
    match ok {
        Ok(()) => {
            warn!(
                "ICC profile locked — scheduled for deletion on reboot: {}",
                profile_path.display()
            );
            Ok(true)
        }
        Err(e) => Err(format!(
            "Could not remove or schedule {} for deletion: {}",
            profile_path.display(),
            e
        )
        .into()),
    }
}

/// Reapply the color profile for a single monitor device key using the toggle
/// approach: disassociate (reverts to default) → pause → reassociate (applies fix).
/// This forces Windows to actually reload the ICC profile.
///
/// # Arguments
/// * `device_key` — WMI device instance path (e.g. `DISPLAY\LGS\001`)
/// * `profile_path` — Full path to the ICC profile file
/// * `toggle_delay_ms` — Pause between disassociate and reassociate (ms)
/// * `per_user` — If true, also perform per-user scope operations
pub fn reapply_profile(
    device_key: &str,
    profile_path: &Path,
    toggle_delay_ms: u64,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    if !profile_path.exists() {
        return Err(format!("Profile not found: {}", profile_path.display()).into());
    }

    // WCS association APIs expect just the filename, not the full path.
    // The profile must already be registered via InstallColorProfileW.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
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
            let err = io::Error::last_os_error();
            warn!(
                "WcsDisassociateColorProfileFromDevice failed for {} (Win32={}) (non-fatal)",
                device_key, err
            );
        }

        // Per-user disassociate (non-fatal)
        if per_user {
            let result = WcsDisassociateColorProfileFromDevice(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(profile_wide.as_ptr()),
                PCWSTR(device_wide.as_ptr()),
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "WcsDisassociateColorProfileFromDevice (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            }
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
            let err = io::Error::last_os_error();
            return Err(format!(
                "WcsAssociateColorProfileWithDevice failed for {} (Win32={})",
                device_key, err
            )
            .into());
        }

        // Per-user associate
        if per_user {
            let result = WcsAssociateColorProfileWithDevice(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(profile_wide.as_ptr()),
                PCWSTR(device_wide.as_ptr()),
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "WcsAssociateColorProfileWithDevice (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            }
        }

        // Step 4: Tell the WCS display pipeline to use this profile (SDR default).
        // This is the modern Win10+ equivalent of what the Color Management
        // control panel does when you select a profile for a display.
        let result = ColorProfileSetDisplayDefaultAssociation(
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            COLOR_PROFILE_TYPE_SDR,
            COLOR_PROFILE_SUBTYPE_SDR,
            0,
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            warn!(
                "ColorProfileSetDisplayDefaultAssociation (system) failed for {} (Win32={}) (non-fatal)",
                device_key, err
            );
        } else {
            info!("SDR display default association set (system) for {}", device_key);
        }

        if per_user {
            let result = ColorProfileSetDisplayDefaultAssociation(
                PCWSTR(profile_wide.as_ptr()),
                PCWSTR(device_wide.as_ptr()),
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                COLOR_PROFILE_TYPE_SDR,
                COLOR_PROFILE_SUBTYPE_SDR,
                0,
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "ColorProfileSetDisplayDefaultAssociation (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            } else {
                info!("SDR display default association set (per-user) for {}", device_key);
            }
        }
    }

    info!("Profile toggled for device: {}", device_key);
    Ok(())
}

/// Set the profile as the generic default using the legacy `WcsSetDefaultColorProfile` API.
///
/// This is an optional operation — some systems or monitors benefit from having the
/// profile also registered as the generic ICC default, but it is NOT required for the
/// dimming fix to work.
///
/// # Arguments
/// * `device_key` — WMI device instance path
/// * `profile_path` — Full path to the ICC profile file
/// * `per_user` — If true, also set the per-user generic default
pub fn set_generic_default(
    device_key: &str,
    profile_path: &Path,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    // WCS APIs expect just the filename, not the full path.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let device_wide = to_wide(device_key);

    unsafe {
        // System-wide generic default
        let result = WcsSetDefaultColorProfile(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(device_wide.as_ptr()),
            CPT_ICC,
            CPST_NONE,
            0,
            PCWSTR(profile_wide.as_ptr()),
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            warn!(
                "WcsSetDefaultColorProfile (system) failed for {} (Win32={}) (non-fatal)",
                device_key, err
            );
        } else {
            info!("Generic default profile set (system) for {}", device_key);
        }

        // Per-user generic default
        if per_user {
            let result = WcsSetDefaultColorProfile(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(device_wide.as_ptr()),
                CPT_ICC,
                CPST_NONE,
                0,
                PCWSTR(profile_wide.as_ptr()),
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "WcsSetDefaultColorProfile (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            } else {
                info!("Generic default profile set (per-user) for {}", device_key);
            }
        }
    }

    Ok(())
}

/// Set the SDR display-default association for a display device.
///
/// Calls `ColorProfileSetDisplayDefaultAssociation` (Win10+) which is the
/// modern API that the Color Management control panel uses.  This tells the
/// WCS display pipeline to actually apply the profile.
///
/// # Arguments
/// * `device_key` — WMI device instance path
/// * `profile_path` — Full path to the ICC profile file
/// * `per_user` — If true, also set the per-user association
pub fn set_display_default_association(
    device_key: &str,
    profile_path: &Path,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    // WCS APIs expect just the filename, not the full path.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let device_wide = to_wide(device_key);

    unsafe {
        let result = ColorProfileSetDisplayDefaultAssociation(
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            COLOR_PROFILE_TYPE_SDR,
            COLOR_PROFILE_SUBTYPE_SDR,
            0,
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            warn!(
                "ColorProfileSetDisplayDefaultAssociation (system) failed for {} (Win32={}) (non-fatal)",
                device_key, err
            );
        } else {
            info!("SDR display default association set (system) for {}", device_key);
        }

        if per_user {
            let result = ColorProfileSetDisplayDefaultAssociation(
                PCWSTR(profile_wide.as_ptr()),
                PCWSTR(device_wide.as_ptr()),
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                COLOR_PROFILE_TYPE_SDR,
                COLOR_PROFILE_SUBTYPE_SDR,
                0,
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "ColorProfileSetDisplayDefaultAssociation (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            } else {
                info!("SDR display default association set (per-user) for {}", device_key);
            }
        }
    }

    Ok(())
}

/// Add the profile to the HDR/advanced-color association for a display device.
///
/// Calls `ColorProfileAddDisplayAssociation` (Win10+).
/// This is an opt-in operation for HDR displays.
///
/// # Arguments
/// * `device_key` — WMI device instance path
/// * `profile_path` — Full path to the ICC profile file
/// * `per_user` — If true, also add the per-user association
pub fn add_hdr_display_association(
    device_key: &str,
    profile_path: &Path,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    // WCS APIs expect just the filename, not the full path.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let device_wide = to_wide(device_key);

    unsafe {
        let result = ColorProfileAddDisplayAssociation(
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            0, // advanced-color / HDR profile type
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            warn!(
                "ColorProfileAddDisplayAssociation (system) failed for {} (Win32={}) (non-fatal)",
                device_key, err
            );
        } else {
            info!("HDR display association added (system) for {}", device_key);
        }

        if per_user {
            let result = ColorProfileAddDisplayAssociation(
                PCWSTR(profile_wide.as_ptr()),
                PCWSTR(device_wide.as_ptr()),
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                0,
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "ColorProfileAddDisplayAssociation (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            } else {
                info!("HDR display association added (per-user) for {}", device_key);
            }
        }
    }

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
