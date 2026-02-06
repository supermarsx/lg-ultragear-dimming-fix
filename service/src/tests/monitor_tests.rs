use super::*;

// ── decode_friendly_name ─────────────────────────────────────────

#[test]
fn decode_friendly_name_basic_ascii() {
    let input: Vec<u16> = "LG ULTRAGEAR"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let result = decode_friendly_name(&Some(input));
    assert_eq!(result, "LG ULTRAGEAR");
}

#[test]
fn decode_friendly_name_none_returns_empty() {
    let result = decode_friendly_name(&None);
    assert_eq!(result, "");
}

#[test]
fn decode_friendly_name_empty_vec() {
    let result = decode_friendly_name(&Some(vec![]));
    assert_eq!(result, "");
}

#[test]
fn decode_friendly_name_only_null() {
    let result = decode_friendly_name(&Some(vec![0]));
    assert_eq!(result, "");
}

#[test]
fn decode_friendly_name_null_terminated_mid_string() {
    // "AB\0CD" should produce "AB"
    let input = vec![65, 66, 0, 67, 68];
    let result = decode_friendly_name(&Some(input));
    assert_eq!(result, "AB");
}

#[test]
fn decode_friendly_name_no_null_terminator() {
    // Without null, should decode all characters
    let input: Vec<u16> = "HELLO".encode_utf16().collect();
    let result = decode_friendly_name(&Some(input));
    assert_eq!(result, "HELLO");
}

#[test]
fn decode_friendly_name_unicode() {
    let input: Vec<u16> = "モニター"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let result = decode_friendly_name(&Some(input));
    assert_eq!(result, "モニター");
}

#[test]
fn decode_friendly_name_with_spaces_and_numbers() {
    let input: Vec<u16> = "ASUS ROG PG279Q 2023"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let result = decode_friendly_name(&Some(input));
    assert_eq!(result, "ASUS ROG PG279Q 2023");
}

#[test]
fn decode_friendly_name_single_char() {
    let result = decode_friendly_name(&Some(vec![65, 0]));
    assert_eq!(result, "A");
}

#[test]
fn decode_friendly_name_all_nulls() {
    let result = decode_friendly_name(&Some(vec![0, 0, 0, 0]));
    assert_eq!(result, "");
}

// ── MatchedMonitor struct ────────────────────────────────────────

#[test]
fn matched_monitor_clone() {
    let mon = MatchedMonitor {
        name: "Test Monitor".to_string(),
        device_key: r"DISPLAY\TEST\001".to_string(),
    };
    let cloned = mon.clone();
    assert_eq!(cloned.name, "Test Monitor");
    assert_eq!(cloned.device_key, r"DISPLAY\TEST\001");
}

#[test]
fn matched_monitor_debug_format() {
    let mon = MatchedMonitor {
        name: "X".to_string(),
        device_key: "Y".to_string(),
    };
    let debug = format!("{:?}", mon);
    assert!(debug.contains("MatchedMonitor"));
    assert!(debug.contains("X"));
    assert!(debug.contains("Y"));
}

// ── WmiMonitorId deserialization ─────────────────────────────────

#[test]
fn wmi_monitor_id_defaults() {
    // Ensure the struct can be created with None fields
    let id = WmiMonitorId {
        user_friendly_name: None,
        instance_name: None,
    };
    assert!(id.user_friendly_name.is_none());
    assert!(id.instance_name.is_none());
}

#[test]
fn wmi_monitor_id_with_values() {
    let id = WmiMonitorId {
        user_friendly_name: Some(vec![76, 71, 0]),
        instance_name: Some("DISPLAY\\LGS\\001_0".to_string()),
    };
    assert_eq!(decode_friendly_name(&id.user_friendly_name), "LG");
    assert_eq!(id.instance_name.unwrap(), "DISPLAY\\LGS\\001_0");
}

// ── Instance name trimming (logic from find_matching_monitors) ──

#[test]
fn instance_name_trim_trailing_zero() {
    let instance = "DISPLAY\\LGS\\001_0";
    let trimmed = instance.trim_end_matches("_0");
    assert_eq!(trimmed, "DISPLAY\\LGS\\001");
}

#[test]
fn instance_name_no_trailing_zero_unchanged() {
    let instance = "DISPLAY\\LGS\\001";
    let trimmed = instance.trim_end_matches("_0");
    assert_eq!(trimmed, "DISPLAY\\LGS\\001");
}

#[test]
fn instance_name_multiple_trailing_zeros() {
    let instance = "DISPLAY\\LGS\\001_0_0";
    let trimmed = instance.trim_end_matches("_0");
    assert_eq!(trimmed, "DISPLAY\\LGS\\001");
}

// ── Pattern matching logic ───────────────────────────────────────

#[test]
fn pattern_matching_case_insensitive() {
    let name = "LG ULTRAGEAR";
    let pattern = "lg ultragear";
    assert!(name.to_uppercase().contains(&pattern.to_uppercase()));
}

#[test]
fn pattern_matching_partial() {
    let name = "LG ULTRAGEAR 27GP950";
    let pattern = "ULTRAGEAR";
    assert!(name.to_uppercase().contains(&pattern.to_uppercase()));
}

#[test]
fn pattern_matching_no_match() {
    let name = "DELL U2723QE";
    let pattern = "LG ULTRAGEAR";
    assert!(!name.to_uppercase().contains(&pattern.to_uppercase()));
}

#[test]
fn pattern_matching_empty_pattern_matches_all() {
    let name = "Any Monitor";
    let pattern = "";
    assert!(name.to_uppercase().contains(&pattern.to_uppercase()));
}
