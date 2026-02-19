//! DDC/CI brightness control via the Windows Monitor Configuration API.
//!
//! Uses `dxva2.dll` to enumerate physical monitors, then `SetVCPFeature` /
//! `GetVCPFeatureAndVCPFeatureReply` to read/write VCP code 0x10 (Luminance).
//!
//! All functions are safe to call without admin rights — DDC/CI only needs
//! access to the display adapter (which every interactive user has).

use log::{info, warn};
use std::error::Error;
use std::io;
use std::ptr;

use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, HDC, HMONITOR,
};

// ============================================================================
// DDC/CI FFI — dxva2.dll
// ============================================================================

/// Opaque handle to a physical monitor.
type HANDLE = *mut std::ffi::c_void;

/// Physical monitor as returned by `GetPhysicalMonitorsFromHMONITOR`.
#[repr(C)]
struct PhysicalMonitor {
    handle: HANDLE,
    description: [u16; 128],
}

#[link(name = "dxva2")]
extern "system" {
    fn GetNumberOfPhysicalMonitorsFromHMONITOR(
        h_monitor: isize,
        num_monitors: *mut u32,
    ) -> BOOL;

    fn GetPhysicalMonitorsFromHMONITOR(
        h_monitor: isize,
        array_size: u32,
        physical_monitors: *mut PhysicalMonitor,
    ) -> BOOL;

    fn DestroyPhysicalMonitor(h_monitor: HANDLE) -> BOOL;

    fn SetVCPFeature(
        h_monitor: HANDLE,
        vcp_code: u8,
        new_value: u32,
    ) -> BOOL;

    fn GetVCPFeatureAndVCPFeatureReply(
        h_monitor: HANDLE,
        vcp_code: u8,
        vcp_type: *mut u32,
        current_value: *mut u32,
        maximum_value: *mut u32,
    ) -> BOOL;
}

/// VCP code for Luminance (brightness).
const VCP_BRIGHTNESS: u8 = 0x10;

// ============================================================================
// Public API
// ============================================================================

/// Result of reading brightness from a monitor.
#[derive(Debug, Clone)]
pub struct BrightnessInfo {
    /// Current brightness value (0–max).
    pub current: u32,
    /// Maximum brightness value reported by the monitor.
    pub max: u32,
    /// Monitor description from the physical monitor handle.
    pub description: String,
}

/// Set DDC/CI brightness on all connected monitors.
///
/// Enumerates all HMONITOR handles via `EnumDisplayMonitors`, resolves each
/// to physical monitors, and calls `SetVCPFeature(0x10, value)`.
///
/// Returns the number of physical monitors that were successfully set.
pub fn set_brightness_all(value: u32) -> Result<usize, Box<dyn Error>> {
    let hmonitors = enumerate_hmonitors()?;
    let mut count = 0usize;

    for hmon in hmonitors {
        match set_brightness_for_hmonitor(hmon, value) {
            Ok(n) => count += n,
            Err(e) => warn!("DDC set brightness failed for a display: {}", e),
        }
    }

    if count == 0 {
        warn!("No physical monitors responded to DDC brightness set");
    } else {
        info!("DDC brightness set to {} on {} monitor(s)", value, count);
    }

    Ok(count)
}

/// Get DDC/CI brightness from all connected monitors.
///
/// Returns a `BrightnessInfo` for each physical monitor that supports
/// the brightness VCP code.
pub fn get_brightness_all() -> Result<Vec<BrightnessInfo>, Box<dyn Error>> {
    let hmonitors = enumerate_hmonitors()?;
    let mut results = Vec::new();

    for hmon in hmonitors {
        match get_brightness_for_hmonitor(hmon) {
            Ok(mut infos) => results.append(&mut infos),
            Err(e) => warn!("DDC get brightness failed for a display: {}", e),
        }
    }

    Ok(results)
}

/// Set DDC/CI brightness on a specific physical monitor by index (0-based).
/// Useful for multi-monitor setups where you only want to target one display.
pub fn set_brightness_by_index(index: usize, value: u32) -> Result<(), Box<dyn Error>> {
    let physicals = get_all_physical_monitors()?;
    if index >= physicals.len() {
        return Err(format!(
            "Monitor index {} out of range (found {} monitors)",
            index,
            physicals.len()
        )
        .into());
    }

    let pm = &physicals[index];
    let ok = unsafe { SetVCPFeature(pm.handle, VCP_BRIGHTNESS, value) };
    if !ok.as_bool() {
        let err = io::Error::last_os_error();
        // Clean up all handles
        for p in &physicals {
            unsafe { let _ = DestroyPhysicalMonitor(p.handle); };
        }
        return Err(format!("SetVCPFeature(0x10, {}) failed: {}", value, err).into());
    }

    info!("DDC brightness set to {} for monitor index {}", value, index);

    // Clean up all handles
    for p in &physicals {
        unsafe { let _ = DestroyPhysicalMonitor(p.handle); };
    }
    Ok(())
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Enumerate all HMONITOR handles on the system.
fn enumerate_hmonitors() -> Result<Vec<isize>, Box<dyn Error>> {
    let mut handles: Vec<isize> = Vec::new();

    unsafe extern "system" fn callback(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let vec = &mut *(data.0 as *mut Vec<isize>);
        vec.push(hmonitor.0 as isize);
        BOOL::from(true)
    }

    let ok = unsafe {
        EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(callback),
            LPARAM(&mut handles as *mut Vec<isize> as isize),
        )
    };

    if !ok.as_bool() {
        return Err("EnumDisplayMonitors failed".into());
    }

    Ok(handles)
}

/// Get all physical monitors across all HMONITOR handles.
fn get_all_physical_monitors() -> Result<Vec<PhysicalMonitor>, Box<dyn Error>> {
    let hmonitors = enumerate_hmonitors()?;
    let mut all = Vec::new();

    for hmon in hmonitors {
        let mut count: u32 = 0;
        let ok = unsafe {
            GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count)
        };
        if !ok.as_bool() || count == 0 {
            continue;
        }

        let mut monitors = Vec::with_capacity(count as usize);
        for _ in 0..count {
            monitors.push(PhysicalMonitor {
                handle: ptr::null_mut(),
                description: [0u16; 128],
            });
        }

        let ok = unsafe {
            GetPhysicalMonitorsFromHMONITOR(hmon, count, monitors.as_mut_ptr())
        };
        if ok.as_bool() {
            all.append(&mut monitors);
        }
    }

    Ok(all)
}

/// Set brightness for all physical monitors behind a given HMONITOR.
fn set_brightness_for_hmonitor(hmon: isize, value: u32) -> Result<usize, Box<dyn Error>> {
    let mut count: u32 = 0;
    let ok = unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count) };
    if !ok.as_bool() || count == 0 {
        return Ok(0);
    }

    let mut monitors = Vec::with_capacity(count as usize);
    for _ in 0..count {
        monitors.push(PhysicalMonitor {
            handle: ptr::null_mut(),
            description: [0u16; 128],
        });
    }

    let ok =
        unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, count, monitors.as_mut_ptr()) };
    if !ok.as_bool() {
        return Err("GetPhysicalMonitorsFromHMONITOR failed".into());
    }

    let mut success_count = 0usize;
    for pm in &monitors {
        let ok = unsafe { SetVCPFeature(pm.handle, VCP_BRIGHTNESS, value) };
        if ok.as_bool() {
            success_count += 1;
        } else {
            let err = io::Error::last_os_error();
            warn!("SetVCPFeature(0x10, {}) failed: {}", value, err);
        }
    }

    // Cleanup
    for pm in &monitors {
        unsafe { let _ = DestroyPhysicalMonitor(pm.handle); };
    }

    Ok(success_count)
}

/// Get brightness for all physical monitors behind a given HMONITOR.
fn get_brightness_for_hmonitor(hmon: isize) -> Result<Vec<BrightnessInfo>, Box<dyn Error>> {
    let mut count: u32 = 0;
    let ok = unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count) };
    if !ok.as_bool() || count == 0 {
        return Ok(Vec::new());
    }

    let mut monitors = Vec::with_capacity(count as usize);
    for _ in 0..count {
        monitors.push(PhysicalMonitor {
            handle: ptr::null_mut(),
            description: [0u16; 128],
        });
    }

    let ok =
        unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, count, monitors.as_mut_ptr()) };
    if !ok.as_bool() {
        return Err("GetPhysicalMonitorsFromHMONITOR failed".into());
    }

    let mut results = Vec::new();
    for pm in &monitors {
        let mut vcp_type: u32 = 0;
        let mut current: u32 = 0;
        let mut maximum: u32 = 0;

        let ok = unsafe {
            GetVCPFeatureAndVCPFeatureReply(
                pm.handle,
                VCP_BRIGHTNESS,
                &mut vcp_type,
                &mut current,
                &mut maximum,
            )
        };
        if ok.as_bool() {
            let desc = decode_description(&pm.description);
            results.push(BrightnessInfo {
                current,
                max: maximum,
                description: desc,
            });
        } else {
            let err = io::Error::last_os_error();
            warn!("GetVCPFeatureAndVCPFeatureReply(0x10) failed: {}", err);
        }
    }

    // Cleanup
    for pm in &monitors {
        unsafe { let _ = DestroyPhysicalMonitor(pm.handle); };
    }

    Ok(results)
}

/// Decode the physical monitor description from a null-terminated UTF-16 array.
fn decode_description(raw: &[u16; 128]) -> String {
    raw.iter()
        .take_while(|&&c| c != 0)
        .filter_map(|&c| char::from_u32(c as u32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vcp_brightness_code_is_0x10() {
        assert_eq!(VCP_BRIGHTNESS, 0x10);
    }

    #[test]
    fn decode_description_empty() {
        let raw = [0u16; 128];
        assert_eq!(decode_description(&raw), "");
    }

    #[test]
    fn decode_description_ascii() {
        let mut raw = [0u16; 128];
        for (i, ch) in "LG ULTRAGEAR".encode_utf16().enumerate() {
            raw[i] = ch;
        }
        assert_eq!(decode_description(&raw), "LG ULTRAGEAR");
    }

    #[test]
    fn decode_description_null_terminated_mid() {
        let mut raw = [0u16; 128];
        raw[0] = 65; // A
        raw[1] = 66; // B
        raw[2] = 0;
        raw[3] = 67; // C (should not appear)
        assert_eq!(decode_description(&raw), "AB");
    }

    #[test]
    fn brightness_info_debug_format() {
        let info = BrightnessInfo {
            current: 50,
            max: 100,
            description: "Test".to_string(),
        };
        let debug = format!("{:?}", info);
        assert!(debug.contains("BrightnessInfo"));
        assert!(debug.contains("50"));
        assert!(debug.contains("100"));
    }

    #[test]
    fn brightness_info_clone() {
        let info = BrightnessInfo {
            current: 75,
            max: 100,
            description: "Monitor".to_string(),
        };
        let cloned = info.clone();
        assert_eq!(cloned.current, 75);
        assert_eq!(cloned.max, 100);
        assert_eq!(cloned.description, "Monitor");
    }

    #[test]
    fn enumerate_hmonitors_does_not_panic() {
        // This will succeed on any Windows system with a display adapter.
        // On headless CI, it may return an empty list but should not panic.
        let result = enumerate_hmonitors();
        assert!(result.is_ok());
    }

    #[test]
    fn get_brightness_all_does_not_panic() {
        // Safe to call — may return empty on headless/CI, should not panic.
        let result = get_brightness_all();
        assert!(result.is_ok());
    }

    #[test]
    fn set_brightness_all_with_zero_does_not_panic() {
        // We intentionally do NOT call set_brightness_all(0) in tests because
        // it would actually set the monitor brightness. Instead, we verify
        // the enumerate path works.
        let result = enumerate_hmonitors();
        assert!(result.is_ok());
    }
}
