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
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};

// ============================================================================
// DDC/CI FFI — dxva2.dll
// ============================================================================

/// Opaque handle to a physical monitor.
#[allow(clippy::upper_case_acronyms)]
type HANDLE = *mut std::ffi::c_void;

/// Physical monitor as returned by `GetPhysicalMonitorsFromHMONITOR`.
#[repr(C)]
struct PhysicalMonitor {
    handle: HANDLE,
    description: [u16; 128],
}

#[link(name = "dxva2")]
extern "system" {
    fn GetNumberOfPhysicalMonitorsFromHMONITOR(h_monitor: isize, num_monitors: *mut u32) -> BOOL;

    fn GetPhysicalMonitorsFromHMONITOR(
        h_monitor: isize,
        array_size: u32,
        physical_monitors: *mut PhysicalMonitor,
    ) -> BOOL;

    fn DestroyPhysicalMonitor(h_monitor: HANDLE) -> BOOL;

    fn SetVCPFeature(h_monitor: HANDLE, vcp_code: u8, new_value: u32) -> BOOL;

    fn GetVCPFeatureAndVCPFeatureReply(
        h_monitor: HANDLE,
        vcp_code: u8,
        vcp_type: *mut u32,
        current_value: *mut u32,
        maximum_value: *mut u32,
    ) -> BOOL;
}

// ============================================================================
// VCP code constants (MCCS standard)
// ============================================================================

/// VCP code for Luminance (brightness).  Range 0–100.
pub const VCP_BRIGHTNESS: u8 = 0x10;

/// VCP code for Contrast.  Range 0–100.
pub const VCP_CONTRAST: u8 = 0x12;

/// VCP code for Select Color Preset.
/// Values: 1=sRGB, 2=Native, 4=4000K, 5=5000K, 6=6500K, 8=7500K, 11=User1…
pub const VCP_COLOR_PRESET: u8 = 0x14;

/// VCP code for Video Gain (Drive) — Red.  Range 0–100.
pub const VCP_RED_GAIN: u8 = 0x16;

/// VCP code for Video Gain (Drive) — Green.  Range 0–100.
pub const VCP_GREEN_GAIN: u8 = 0x18;

/// VCP code for Video Gain (Drive) — Blue.  Range 0–100.
pub const VCP_BLUE_GAIN: u8 = 0x1A;

/// VCP code for Input Source Select.
/// Values: 1=VGA, 3=DVI, 15=DisplayPort, 17=HDMI1, 18=HDMI2.
pub const VCP_INPUT_SOURCE: u8 = 0x60;

/// VCP code for Speaker Volume.  Range 0–100.
pub const VCP_VOLUME: u8 = 0x62;

/// VCP code for Display Mode (picture mode preset — monitor-specific).
pub const VCP_DISPLAY_MODE: u8 = 0xDC;

/// VCP code for Power Mode.
/// Values: 1=On, 2=Standby, 4=Suspend, 5=Off.
pub const VCP_POWER_MODE: u8 = 0xD6;

/// VCP code for VCP Version (read-only).
pub const VCP_VERSION: u8 = 0xDF;

/// VCP code: Restore Factory Defaults.  Write 1 to trigger.
pub const VCP_FACTORY_RESET: u8 = 0x04;

/// VCP code: Restore Factory Luminance/Contrast.  Write 1 to trigger.
pub const VCP_RESET_BRIGHTNESS_CONTRAST: u8 = 0x06;

/// VCP code: Restore Factory Color Defaults.  Write 1 to trigger.
pub const VCP_RESET_COLOR: u8 = 0x0A;

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
            unsafe {
                let _ = DestroyPhysicalMonitor(p.handle);
            };
        }
        return Err(format!("SetVCPFeature(0x10, {}) failed: {}", value, err).into());
    }

    info!(
        "DDC brightness set to {} for monitor index {}",
        value, index
    );

    // Clean up all handles
    for p in &physicals {
        unsafe {
            let _ = DestroyPhysicalMonitor(p.handle);
        };
    }
    Ok(())
}

// ============================================================================
// Generic VCP get/set
// ============================================================================

/// Result of reading a VCP feature.
#[derive(Debug, Clone)]
pub struct VcpValue {
    /// The VCP code that was read.
    pub code: u8,
    /// Current value.
    pub current: u32,
    /// Maximum value (for continuous controls) or 0.
    pub max: u32,
    /// VCP type: 0 = Set Parameter (continuous), 1 = Momentary.
    pub vcp_type: u32,
}

/// Capability probe result for one VCP code on a monitor.
#[derive(Debug, Clone)]
pub struct VcpCapability {
    pub code: u8,
    pub label: &'static str,
    pub risky: bool,
    pub supported: bool,
    pub current: Option<u32>,
    pub max: Option<u32>,
    pub vcp_type: Option<u32>,
}

/// Capability map for one physical monitor.
#[derive(Debug, Clone)]
pub struct MonitorCapabilityMap {
    pub index: usize,
    pub name: String,
    pub capabilities: Vec<VcpCapability>,
}

/// Information about a physical monitor handle with its description.
#[derive(Debug)]
struct MonitorHandle {
    handle: HANDLE,
    description: String,
    hmonitor: isize,
}

/// Read a VCP feature from a specific physical monitor identified by
/// matching its description against `pattern` (case-insensitive contains).
///
/// If `pattern` is empty, uses the first physical monitor found.
pub fn get_vcp_by_pattern(pattern: &str, vcp_code: u8) -> Result<VcpValue, Box<dyn Error>> {
    let handle = find_monitor_by_pattern(pattern)?;
    let result = get_vcp_raw(handle.handle, vcp_code);
    unsafe {
        let _ = DestroyPhysicalMonitor(handle.handle);
    };
    result
}

/// Write a VCP feature to a specific physical monitor identified by
/// matching its description against `pattern` (case-insensitive contains).
///
/// If `pattern` is empty, uses the first physical monitor found.
pub fn set_vcp_by_pattern(pattern: &str, vcp_code: u8, value: u32) -> Result<(), Box<dyn Error>> {
    let handle = find_monitor_by_pattern(pattern)?;
    let result = set_vcp_raw(handle.handle, vcp_code, value);
    unsafe {
        let _ = DestroyPhysicalMonitor(handle.handle);
    };
    result
}

/// Read a VCP feature from a specific physical monitor by 0-based index.
///
/// The index corresponds to the order returned by `list_physical_monitors()`.
pub fn get_vcp_by_index(index: usize, vcp_code: u8) -> Result<VcpValue, Box<dyn Error>> {
    let handles = get_all_monitor_handles()?;
    if index >= handles.len() {
        for mh in &handles {
            unsafe {
                let _ = DestroyPhysicalMonitor(mh.handle);
            };
        }
        return Err(format!(
            "Monitor index {} out of range (found {} monitors)",
            index,
            handles.len()
        )
        .into());
    }
    let result = get_vcp_raw(handles[index].handle, vcp_code);
    for mh in &handles {
        unsafe {
            let _ = DestroyPhysicalMonitor(mh.handle);
        };
    }
    result
}

/// Write a VCP feature to a specific physical monitor by 0-based index.
///
/// The index corresponds to the order returned by `list_physical_monitors()`.
pub fn set_vcp_by_index(index: usize, vcp_code: u8, value: u32) -> Result<(), Box<dyn Error>> {
    let handles = get_all_monitor_handles()?;
    if index >= handles.len() {
        for mh in &handles {
            unsafe {
                let _ = DestroyPhysicalMonitor(mh.handle);
            };
        }
        return Err(format!(
            "Monitor index {} out of range (found {} monitors)",
            index,
            handles.len()
        )
        .into());
    }
    let result = set_vcp_raw(handles[index].handle, vcp_code, value);
    for mh in &handles {
        unsafe {
            let _ = DestroyPhysicalMonitor(mh.handle);
        };
    }
    result
}

/// Read a VCP feature from all physical monitors, returning results
/// paired with their descriptions.
pub fn get_vcp_all(vcp_code: u8) -> Result<Vec<(String, VcpValue)>, Box<dyn Error>> {
    let handles = get_all_monitor_handles()?;
    let mut results = Vec::new();

    for mh in &handles {
        let name = resolve_display_name(&mh.description, mh.hmonitor);
        match get_vcp_raw(mh.handle, vcp_code) {
            Ok(val) => results.push((name, val)),
            Err(e) => warn!(
                "VCP 0x{:02X} read failed for {}: {}",
                vcp_code,
                if name.is_empty() { "unknown" } else { &name },
                e
            ),
        }
    }

    // Cleanup
    for mh in &handles {
        unsafe {
            let _ = DestroyPhysicalMonitor(mh.handle);
        };
    }

    Ok(results)
}

/// List all physical monitors with their descriptions and HMONITOR index.
/// Useful for the TUI to show what monitors are available via DDC.
///
/// If the DDC description is "Generic PnP Monitor", the GDI device string
/// is used instead so the real product name is shown (e.g. "LG ULTRAGEAR").
pub fn list_physical_monitors() -> Result<Vec<(usize, String)>, Box<dyn Error>> {
    let handles = get_all_monitor_handles()?;
    let result: Vec<(usize, String)> = handles
        .iter()
        .enumerate()
        .map(|(i, mh)| (i, resolve_display_name(&mh.description, mh.hmonitor)))
        .collect();

    // Cleanup
    for mh in &handles {
        unsafe {
            let _ = DestroyPhysicalMonitor(mh.handle);
        };
    }

    Ok(result)
}

/// Return known VCP codes with labels and a default risk marker.
pub fn known_vcp_codes() -> &'static [(u8, &'static str, bool)] {
    &[
        (VCP_BRIGHTNESS, "Brightness", false),
        (VCP_CONTRAST, "Contrast", false),
        (VCP_COLOR_PRESET, "Color Preset", false),
        (VCP_RED_GAIN, "Red Gain", false),
        (VCP_GREEN_GAIN, "Green Gain", false),
        (VCP_BLUE_GAIN, "Blue Gain", false),
        (VCP_INPUT_SOURCE, "Input Source", true),
        (VCP_VOLUME, "Volume", false),
        (VCP_POWER_MODE, "Power Mode", true),
        (VCP_DISPLAY_MODE, "Display Mode", false),
        (VCP_VERSION, "VCP Version", false),
        (VCP_FACTORY_RESET, "Factory Reset", true),
        (
            VCP_RESET_BRIGHTNESS_CONTRAST,
            "Reset Brightness/Contrast",
            true,
        ),
        (VCP_RESET_COLOR, "Reset Color", true),
    ]
}

/// Probe each monitor for support of common VCP codes and return a capability map.
pub fn probe_monitor_capabilities() -> Result<Vec<MonitorCapabilityMap>, Box<dyn Error>> {
    let handles = get_all_monitor_handles()?;
    let mut maps = Vec::new();

    for (idx, mh) in handles.iter().enumerate() {
        let mut capabilities = Vec::with_capacity(known_vcp_codes().len());
        for &(code, label, risky) in known_vcp_codes() {
            match get_vcp_raw(mh.handle, code) {
                Ok(v) => capabilities.push(VcpCapability {
                    code,
                    label,
                    risky,
                    supported: true,
                    current: Some(v.current),
                    max: Some(v.max),
                    vcp_type: Some(v.vcp_type),
                }),
                Err(_) => capabilities.push(VcpCapability {
                    code,
                    label,
                    risky,
                    supported: false,
                    current: None,
                    max: None,
                    vcp_type: None,
                }),
            }
        }

        maps.push(MonitorCapabilityMap {
            index: idx,
            name: resolve_display_name(&mh.description, mh.hmonitor),
            capabilities,
        });
    }

    for mh in &handles {
        unsafe {
            let _ = DestroyPhysicalMonitor(mh.handle);
        };
    }

    Ok(maps)
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Read a VCP code from a raw physical monitor handle.  Does NOT destroy it.
fn get_vcp_raw(handle: HANDLE, vcp_code: u8) -> Result<VcpValue, Box<dyn Error>> {
    let mut vcp_type: u32 = 0;
    let mut current: u32 = 0;
    let mut maximum: u32 = 0;

    let ok = unsafe {
        GetVCPFeatureAndVCPFeatureReply(handle, vcp_code, &mut vcp_type, &mut current, &mut maximum)
    };
    if !ok.as_bool() {
        let err = io::Error::last_os_error();
        return Err(format!(
            "GetVCPFeatureAndVCPFeatureReply(0x{:02X}) failed: {}",
            vcp_code, err
        )
        .into());
    }

    Ok(VcpValue {
        code: vcp_code,
        current,
        max: maximum,
        vcp_type,
    })
}

/// Write a VCP code to a raw physical monitor handle.  Does NOT destroy it.
fn set_vcp_raw(handle: HANDLE, vcp_code: u8, value: u32) -> Result<(), Box<dyn Error>> {
    let ok = unsafe { SetVCPFeature(handle, vcp_code, value) };
    if !ok.as_bool() {
        let err = io::Error::last_os_error();
        return Err(format!(
            "SetVCPFeature(0x{:02X}, {}) failed: {}",
            vcp_code, value, err
        )
        .into());
    }
    Ok(())
}

/// Find a single physical monitor whose description matches `pattern`.
///
/// Uses `EnumDisplayDevices` to get the GDI device string for each HMONITOR,
/// then checks both the physical monitor description (from `dxva2`) and the
/// GDI device string for a case-insensitive contains match.
///
/// This handles LG monitors that show up as "Generic PnP Monitor" in the
/// physical monitor description but have "LG" in the GDI display adapter info.
fn find_monitor_by_pattern(pattern: &str) -> Result<MonitorHandle, Box<dyn Error>> {
    let handles = get_all_monitor_handles()?;

    if handles.is_empty() {
        return Err("No physical monitors found via DDC/CI".into());
    }

    // If pattern is empty, return the first monitor
    if pattern.is_empty() {
        // Destroy all except first
        for mh in handles.iter().skip(1) {
            unsafe {
                let _ = DestroyPhysicalMonitor(mh.handle);
            };
        }
        let first = handles.into_iter().next().unwrap();
        return Ok(first);
    }

    let pat = pattern.to_uppercase();

    // First pass: match by DDC physical monitor description
    for mh in &handles {
        if mh.description.to_uppercase().contains(&pat) {
            let matched_handle = mh.handle;
            let matched_desc = mh.description.clone();
            let matched_hmon = mh.hmonitor;
            // Destroy all OTHER handles
            for other in &handles {
                if !std::ptr::eq(other.handle, matched_handle) {
                    unsafe {
                        let _ = DestroyPhysicalMonitor(other.handle);
                    };
                }
            }
            info!("DDC: matched monitor by description: {}", matched_desc);
            return Ok(MonitorHandle {
                handle: matched_handle,
                description: matched_desc,
                hmonitor: matched_hmon,
            });
        }
    }

    // Second pass: match by GDI device name (EnumDisplayDevices)
    // This catches monitors listed as "Generic PnP Monitor" by dxva2 but
    // with the real name in the GDI device string.
    for mh in &handles {
        let gdi_name = get_gdi_device_name(mh.hmonitor);
        if let Some(ref name) = gdi_name {
            if name.to_uppercase().contains(&pat) {
                let matched_handle = mh.handle;
                let matched_hmon = mh.hmonitor;
                // Prefer the GDI name so callers see the real product name
                let resolved = name.clone();
                for other in &handles {
                    if !std::ptr::eq(other.handle, matched_handle) {
                        unsafe {
                            let _ = DestroyPhysicalMonitor(other.handle);
                        };
                    }
                }
                info!(
                    "DDC: matched monitor by GDI device name: {} (DDC desc: {})",
                    name, mh.description
                );
                return Ok(MonitorHandle {
                    handle: matched_handle,
                    description: resolved,
                    hmonitor: matched_hmon,
                });
            }
        }
    }

    // No match — clean up all handles
    for mh in &handles {
        unsafe {
            let _ = DestroyPhysicalMonitor(mh.handle);
        };
    }

    let names: Vec<String> = handles
        .iter()
        .map(|m| resolve_display_name(&m.description, m.hmonitor))
        .collect();
    Err(format!(
        "No DDC/CI monitor matched pattern '{}'. Found: {}",
        pattern,
        names.join(", ")
    )
    .into())
}

/// Get all physical monitors with their handles and descriptions.
/// Caller is responsible for calling `DestroyPhysicalMonitor` on each handle.
fn get_all_monitor_handles() -> Result<Vec<MonitorHandle>, Box<dyn Error>> {
    let hmonitors = enumerate_hmonitors()?;
    let mut all = Vec::new();

    for hmon in hmonitors {
        let mut count: u32 = 0;
        let ok = unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count) };
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

        let ok = unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, count, monitors.as_mut_ptr()) };
        if ok.as_bool() {
            for pm in monitors {
                all.push(MonitorHandle {
                    handle: pm.handle,
                    description: decode_description(&pm.description),
                    hmonitor: hmon,
                });
            }
        }
    }

    Ok(all)
}

/// Try to get the GDI display device name for an HMONITOR.
///
/// Uses `GetMonitorInfoW` + `EnumDisplayDevicesW` to resolve the monitor
/// adapter name, then the monitor device string which may contain the
/// real product name (e.g. "LG ULTRAGEAR") even when dxva2 only reports
/// "Generic PnP Monitor".
fn get_gdi_device_name(hmon: isize) -> Option<String> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFOEXA};

    let mut mi = MONITORINFOEXA::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXA>() as u32;

    let ok = unsafe {
        GetMonitorInfoW(
            HMONITOR(hmon as *mut std::ffi::c_void),
            &mut mi as *mut MONITORINFOEXA as *mut _,
        )
    };
    if !ok.as_bool() {
        return None;
    }

    let device_bytes = &mi.szDevice;
    let device: String = device_bytes
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8 as char)
        .collect();

    // Now use EnumDisplayDevicesW with the adapter name to get the monitor name
    use windows::Win32::Graphics::Gdi::EnumDisplayDevicesA;
    use windows::Win32::Graphics::Gdi::DISPLAY_DEVICEA;

    let mut dd = DISPLAY_DEVICEA {
        cb: std::mem::size_of::<DISPLAY_DEVICEA>() as u32,
        ..Default::default()
    };

    let device_cstr: Vec<u8> = device.bytes().chain(std::iter::once(0)).collect();
    let device_pcstr = windows::core::PCSTR::from_raw(device_cstr.as_ptr());

    let ok = unsafe { EnumDisplayDevicesA(device_pcstr, 0, &mut dd, 0) };
    if !ok.as_bool() {
        return None;
    }

    let monitor_str: String = dd
        .DeviceString
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8 as char)
        .collect();

    if monitor_str.is_empty() {
        None
    } else {
        Some(monitor_str)
    }
}

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
        let ok = unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count) };
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

        let ok = unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, count, monitors.as_mut_ptr()) };
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

    let ok = unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, count, monitors.as_mut_ptr()) };
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
        unsafe {
            let _ = DestroyPhysicalMonitor(pm.handle);
        };
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

    let ok = unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, count, monitors.as_mut_ptr()) };
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
            let display = resolve_display_name(&desc, hmon);
            results.push(BrightnessInfo {
                current,
                max: maximum,
                description: display,
            });
        } else {
            let err = io::Error::last_os_error();
            warn!("GetVCPFeatureAndVCPFeatureReply(0x10) failed: {}", err);
        }
    }

    // Cleanup
    for pm in &monitors {
        unsafe {
            let _ = DestroyPhysicalMonitor(pm.handle);
        };
    }

    Ok(results)
}

/// Return the best human-readable name for a physical monitor.
///
/// If the DDC description is the unhelpful "Generic PnP Monitor", we fall
/// back to the GDI device string (via `EnumDisplayDevicesA`) which usually
/// contains the real product name (e.g. "LG ULTRAGEAR").
fn resolve_display_name(ddc_desc: &str, hmon: isize) -> String {
    const GENERIC: &str = "generic pnp monitor";
    if !ddc_desc.trim().eq_ignore_ascii_case(GENERIC) {
        return ddc_desc.to_string();
    }
    // DDC gave us the generic name — try GDI
    if let Some(gdi) = get_gdi_device_name(hmon) {
        if !gdi.trim().is_empty() {
            return gdi;
        }
    }
    ddc_desc.to_string()
}

/// Decode the physical monitor description from a null-terminated UTF-16 array.
fn decode_description(raw: &[u16; 128]) -> String {
    raw.iter()
        .take_while(|&&c| c != 0)
        .filter_map(|&c| char::from_u32(c as u32))
        .collect()
}

#[cfg(test)]
#[path = "tests/ddc_tests.rs"]
mod tests;
