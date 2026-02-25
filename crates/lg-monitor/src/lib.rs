//! Monitor detection via WMI + DDC/CI brightness control.
//!
//! Enumerates connected monitors using `WmiMonitorID` and matches against
//! a user-configured friendly name pattern (e.g. "LG ULTRAGEAR").
//!
//! The [`ddc`] module provides DDC/CI brightness reading and control via
//! the Windows Monitor Configuration API (`dxva2.dll`).

pub mod ddc;

use regex::RegexBuilder;
use serde::Deserialize;
use std::error::Error;
use wmi::{COMLibrary, WMIConnection};

use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};

const ADVANCED_COLOR_SUPPORTED_MASK: u32 = 0b0001;
const ADVANCED_COLOR_ENABLED_MASK: u32 = 0b0010;
const DISPLAY_CONFIG_QUERY_RETRIES: usize = 3;

/// A matched monitor with its friendly name and device instance path.
#[derive(Debug, Clone)]
pub struct MatchedMonitor {
    pub name: String,
    pub device_key: String,
    pub serial: String,
    pub manufacturer_id: String,
    pub product_code: String,
}

/// Pattern matching mode for monitor discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorMatchMode {
    Substring,
    Regex,
}

/// Aggregate advanced-color/HDR state for active display paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AdvancedColorState {
    pub active_paths: u32,
    pub supported_paths: u32,
    pub enabled_paths: u32,
}

impl AdvancedColorState {
    pub fn any_enabled(self) -> bool {
        self.enabled_paths > 0
    }
}

/// Raw WMI result from `WmiMonitorID`.
#[derive(Deserialize, Debug)]
#[serde(rename = "WmiMonitorID")]
#[serde(rename_all = "PascalCase")]
struct WmiMonitorId {
    user_friendly_name: Option<Vec<u16>>,
    instance_name: Option<String>,
    serial_number_id: Option<Vec<u16>>,
    manufacturer_name: Option<Vec<u16>>,
    product_code_id: Option<Vec<u16>>,
}

/// Find all connected monitors whose friendly name contains `pattern` (case-insensitive).
pub fn find_matching_monitors(pattern: &str) -> Result<Vec<MatchedMonitor>, Box<dyn Error>> {
    find_matching_monitors_with_mode(pattern, MonitorMatchMode::Substring)
}

/// Find all connected monitors whose friendly name matches `pattern` as case-insensitive regex.
pub fn find_matching_monitors_regex(pattern: &str) -> Result<Vec<MatchedMonitor>, Box<dyn Error>> {
    find_matching_monitors_with_mode(pattern, MonitorMatchMode::Regex)
}

/// Find monitors by pattern using either substring or regex mode.
pub fn find_matching_monitors_with_mode(
    pattern: &str,
    mode: MonitorMatchMode,
) -> Result<Vec<MatchedMonitor>, Box<dyn Error>> {
    let com = COMLibrary::new()?;
    let wmi = WMIConnection::with_namespace_path("root\\wmi", com)?;

    let monitors: Vec<WmiMonitorId> = wmi.raw_query(
        "SELECT UserFriendlyName, InstanceName, SerialNumberID, ManufacturerName, ProductCodeID \
         FROM WmiMonitorID",
    )?;
    let mut matched = Vec::with_capacity(2);

    let compiled_regex = if matches!(mode, MonitorMatchMode::Regex) && !pattern.is_empty() {
        Some(
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .map_err(|e| format!("invalid regex pattern \"{}\": {}", pattern, e))?,
        )
    } else {
        None
    };

    for mon in monitors {
        let name = decode_friendly_name(&mon.user_friendly_name);
        if monitor_name_matches(&name, pattern, mode, compiled_regex.as_ref()) {
            // Strip trailing "_0" from instance name to get the device key
            let device_key = mon
                .instance_name
                .as_deref()
                .unwrap_or("")
                .trim_end_matches("_0")
                .to_string();

            if !device_key.is_empty() {
                matched.push(MatchedMonitor {
                    name,
                    device_key,
                    serial: decode_wmi_u16_text(&mon.serial_number_id),
                    manufacturer_id: decode_wmi_u16_text(&mon.manufacturer_name),
                    product_code: decode_wmi_u16_text(&mon.product_code_id),
                });
            }
        }
    }

    Ok(matched)
}

fn monitor_name_matches(
    name: &str,
    pattern: &str,
    mode: MonitorMatchMode,
    regex: Option<&regex::Regex>,
) -> bool {
    if pattern.is_empty() {
        return true;
    }
    match mode {
        MonitorMatchMode::Substring => name.to_uppercase().contains(&pattern.to_uppercase()),
        MonitorMatchMode::Regex => regex.is_some_and(|r| r.is_match(name)),
    }
}

/// Query active displays and summarize advanced-color/HDR state.
pub fn query_advanced_color_state() -> Result<AdvancedColorState, Box<dyn Error>> {
    let paths = query_active_display_paths()?;
    let mut state = AdvancedColorState {
        active_paths: paths.len() as u32,
        ..AdvancedColorState::default()
    };

    for path in paths {
        let mut info = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO::default();
        info.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO;
        info.header.size = std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32;
        info.header.adapterId = path.targetInfo.adapterId;
        info.header.id = path.targetInfo.id;

        let status = unsafe { DisplayConfigGetDeviceInfo(&mut info.header) };
        if status != ERROR_SUCCESS.0 as i32 {
            continue;
        }

        let flags = unsafe { info.Anonymous.value };
        if advanced_color_supported(flags) {
            state.supported_paths += 1;
        }
        if advanced_color_enabled(flags) {
            state.enabled_paths += 1;
        }
    }

    Ok(state)
}

/// True if any active display path currently has advanced-color/HDR enabled.
pub fn is_any_display_hdr_enabled() -> Result<bool, Box<dyn Error>> {
    Ok(query_advanced_color_state()?.any_enabled())
}

fn query_active_display_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>, Box<dyn Error>> {
    for _ in 0..DISPLAY_CONFIG_QUERY_RETRIES {
        let mut path_count = 0u32;
        let mut mode_count = 0u32;

        let size_status = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if size_status != ERROR_SUCCESS {
            return Err(format!("GetDisplayConfigBufferSizes failed: {}", size_status.0).into());
        }
        if path_count == 0 {
            return Ok(Vec::new());
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let mut queried_paths = path_count;
        let mut queried_modes = mode_count;

        let query_status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut queried_paths,
                paths.as_mut_ptr(),
                &mut queried_modes,
                modes.as_mut_ptr(),
                None,
            )
        };

        if query_status == ERROR_SUCCESS {
            paths.truncate(queried_paths as usize);
            return Ok(paths);
        }

        if query_status != ERROR_INSUFFICIENT_BUFFER {
            return Err(format!("QueryDisplayConfig failed: {}", query_status.0).into());
        }
    }

    Err("QueryDisplayConfig repeatedly returned insufficient buffer".into())
}

fn advanced_color_supported(flags: u32) -> bool {
    flags & ADVANCED_COLOR_SUPPORTED_MASK != 0
}

fn advanced_color_enabled(flags: u32) -> bool {
    flags & ADVANCED_COLOR_ENABLED_MASK != 0
}

/// Decode the `UserFriendlyName` field from WMI (array of u16 code points, null-terminated).
fn decode_friendly_name(raw: &Option<Vec<u16>>) -> String {
    match raw {
        Some(chars) => chars
            .iter()
            .take_while(|&&c| c != 0)
            .filter_map(|&c| char::from_u32(c as u32))
            .collect(),
        None => String::new(),
    }
}

fn decode_wmi_u16_text(raw: &Option<Vec<u16>>) -> String {
    let decoded = decode_friendly_name(raw);
    decoded.trim().to_string()
}

#[cfg(test)]
#[path = "tests/monitor_tests.rs"]
mod tests;
