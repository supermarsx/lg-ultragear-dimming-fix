//! Monitor detection via WMI + DDC/CI brightness control.
//!
//! Enumerates connected monitors using `WmiMonitorID` and matches against
//! a user-configured friendly name pattern (e.g. "LG ULTRAGEAR").
//!
//! The [`ddc`] module provides DDC/CI brightness reading and control via
//! the Windows Monitor Configuration API (`dxva2.dll`).

pub mod ddc;

use serde::Deserialize;
use std::error::Error;
use wmi::{COMLibrary, WMIConnection};

/// A matched monitor with its friendly name and device instance path.
#[derive(Debug, Clone)]
pub struct MatchedMonitor {
    pub name: String,
    pub device_key: String,
}

/// Raw WMI result from `WmiMonitorID`.
#[derive(Deserialize, Debug)]
#[serde(rename = "WmiMonitorID")]
#[serde(rename_all = "PascalCase")]
struct WmiMonitorId {
    user_friendly_name: Option<Vec<u16>>,
    instance_name: Option<String>,
}

/// Find all connected monitors whose friendly name contains `pattern` (case-insensitive).
pub fn find_matching_monitors(pattern: &str) -> Result<Vec<MatchedMonitor>, Box<dyn Error>> {
    let com = COMLibrary::new()?;
    let wmi = WMIConnection::with_namespace_path("root\\wmi", com)?;

    let monitors: Vec<WmiMonitorId> =
        wmi.raw_query("SELECT UserFriendlyName, InstanceName FROM WmiMonitorID")?;
    let pattern_upper = pattern.to_uppercase();
    let mut matched = Vec::with_capacity(2);

    for mon in monitors {
        let name = decode_friendly_name(&mon.user_friendly_name);
        if name.to_uppercase().contains(&pattern_upper) {
            // Strip trailing "_0" from instance name to get the device key
            let device_key = mon
                .instance_name
                .as_deref()
                .unwrap_or("")
                .trim_end_matches("_0")
                .to_string();

            if !device_key.is_empty() {
                matched.push(MatchedMonitor { name, device_key });
            }
        }
    }

    Ok(matched)
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

#[cfg(test)]
#[path = "tests/monitor_tests.rs"]
mod tests;
