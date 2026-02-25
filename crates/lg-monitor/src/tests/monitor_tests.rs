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
    let input = vec![65, 66, 0, 67, 68];
    let result = decode_friendly_name(&Some(input));
    assert_eq!(result, "AB");
}

#[test]
fn decode_friendly_name_no_null_terminator() {
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
        serial: "ABC123".to_string(),
        manufacturer_id: "GSM".to_string(),
        product_code: "1234".to_string(),
    };
    let cloned = mon.clone();
    assert_eq!(cloned.name, "Test Monitor");
    assert_eq!(cloned.device_key, r"DISPLAY\TEST\001");
    assert_eq!(cloned.serial, "ABC123");
    assert_eq!(cloned.manufacturer_id, "GSM");
    assert_eq!(cloned.product_code, "1234");
}

#[test]
fn matched_monitor_debug_format() {
    let mon = MatchedMonitor {
        name: "X".to_string(),
        device_key: "Y".to_string(),
        serial: "S".to_string(),
        manufacturer_id: "M".to_string(),
        product_code: "P".to_string(),
    };
    let debug = format!("{:?}", mon);
    assert!(debug.contains("MatchedMonitor"));
    assert!(debug.contains("X"));
    assert!(debug.contains("Y"));
    assert!(debug.contains("S"));
}

// ── WmiMonitorId deserialization ─────────────────────────────────

#[test]
fn wmi_monitor_id_defaults() {
    let id = WmiMonitorId {
        user_friendly_name: None,
        instance_name: None,
        serial_number_id: None,
        manufacturer_name: None,
        product_code_id: None,
    };
    assert!(id.user_friendly_name.is_none());
    assert!(id.instance_name.is_none());
    assert!(id.serial_number_id.is_none());
    assert!(id.manufacturer_name.is_none());
    assert!(id.product_code_id.is_none());
}

#[test]
fn wmi_monitor_id_with_values() {
    let id = WmiMonitorId {
        user_friendly_name: Some(vec![76, 71, 0]),
        instance_name: Some("DISPLAY\\LGS\\001_0".to_string()),
        serial_number_id: Some(vec![65, 66, 67, 49, 50, 51, 0]),
        manufacturer_name: Some(vec![71, 83, 77, 0]),
        product_code_id: Some(vec![51, 50, 71, 80, 57, 53, 0]),
    };
    assert_eq!(decode_friendly_name(&id.user_friendly_name), "LG");
    assert_eq!(id.instance_name.unwrap(), "DISPLAY\\LGS\\001_0");
    assert_eq!(decode_wmi_u16_text(&id.serial_number_id), "ABC123");
    assert_eq!(decode_wmi_u16_text(&id.manufacturer_name), "GSM");
    assert_eq!(decode_wmi_u16_text(&id.product_code_id), "32GP95");
}

// ── Instance name trimming ───────────────────────────────────────

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
    assert!(monitor_name_matches(
        "LG ULTRAGEAR",
        "lg ultragear",
        MonitorMatchMode::Substring,
        None
    ));
}

#[test]
fn pattern_matching_partial() {
    assert!(monitor_name_matches(
        "LG ULTRAGEAR 27GP950",
        "ULTRAGEAR",
        MonitorMatchMode::Substring,
        None
    ));
}

#[test]
fn pattern_matching_no_match() {
    assert!(!monitor_name_matches(
        "DELL U2723QE",
        "LG ULTRAGEAR",
        MonitorMatchMode::Substring,
        None
    ));
}

#[test]
fn pattern_matching_empty_pattern_matches_all() {
    assert!(monitor_name_matches(
        "Any Monitor",
        "",
        MonitorMatchMode::Substring,
        None
    ));
}

#[test]
fn pattern_matching_regex_mode() {
    let regex = regex::RegexBuilder::new("^LG\\s+ULTRA.*")
        .case_insensitive(true)
        .build()
        .unwrap();
    assert!(monitor_name_matches(
        "LG UltraGear 27\"",
        "^LG\\s+ULTRA.*",
        MonitorMatchMode::Regex,
        Some(&regex)
    ));
}

#[test]
fn pattern_matching_regex_mode_no_regex_object_is_false() {
    assert!(!monitor_name_matches(
        "LG UltraGear 27\"",
        "^LG\\s+ULTRA.*",
        MonitorMatchMode::Regex,
        None
    ));
}

// ── Advanced color flag decoding ────────────────────────────────

#[test]
fn advanced_color_supported_reads_bit0() {
    assert!(advanced_color_supported(0b0001));
    assert!(!advanced_color_supported(0b0000));
    assert!(!advanced_color_supported(0b0010));
}

#[test]
fn advanced_color_enabled_reads_bit1() {
    assert!(advanced_color_enabled(0b0010));
    assert!(!advanced_color_enabled(0b0000));
    assert!(!advanced_color_enabled(0b0001));
}

#[test]
fn advanced_color_state_any_enabled_depends_on_enabled_paths() {
    let off = AdvancedColorState {
        active_paths: 1,
        supported_paths: 1,
        enabled_paths: 0,
    };
    let on = AdvancedColorState {
        active_paths: 1,
        supported_paths: 1,
        enabled_paths: 1,
    };
    assert!(!off.any_enabled());
    assert!(on.any_enabled());
}
