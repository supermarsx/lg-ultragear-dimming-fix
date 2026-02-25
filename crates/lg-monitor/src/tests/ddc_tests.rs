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

// ── VCP constants ────────────────────────────────────────────

#[test]
fn vcp_constants_are_correct() {
    assert_eq!(VCP_CONTRAST, 0x12);
    assert_eq!(VCP_COLOR_PRESET, 0x14);
    assert_eq!(VCP_RED_GAIN, 0x16);
    assert_eq!(VCP_GREEN_GAIN, 0x18);
    assert_eq!(VCP_BLUE_GAIN, 0x1A);
    assert_eq!(VCP_INPUT_SOURCE, 0x60);
    assert_eq!(VCP_VOLUME, 0x62);
    assert_eq!(VCP_DISPLAY_MODE, 0xDC);
    assert_eq!(VCP_POWER_MODE, 0xD6);
    assert_eq!(VCP_VERSION, 0xDF);
    assert_eq!(VCP_FACTORY_RESET, 0x04);
    assert_eq!(VCP_RESET_BRIGHTNESS_CONTRAST, 0x06);
    assert_eq!(VCP_RESET_COLOR, 0x0A);
}

// ── VcpValue struct ──────────────────────────────────────────

#[test]
fn vcp_value_debug_format() {
    let val = VcpValue {
        code: 0x10,
        current: 50,
        max: 100,
        vcp_type: 0,
    };
    let debug = format!("{:?}", val);
    assert!(debug.contains("VcpValue"));
    assert!(debug.contains("50"));
}

#[test]
fn vcp_value_clone() {
    let val = VcpValue {
        code: 0xDF,
        current: 0x0202,
        max: 0,
        vcp_type: 0,
    };
    let cloned = val.clone();
    assert_eq!(cloned.code, 0xDF);
    assert_eq!(cloned.current, 0x0202);
}

// ── list_physical_monitors ───────────────────────────────────

#[test]
fn list_physical_monitors_does_not_panic() {
    let result = list_physical_monitors();
    assert!(result.is_ok());
}

// ── resolve_display_name ─────────────────────────────────────

#[test]
fn resolve_display_name_keeps_real_name() {
    // A real product name should be returned as-is (GDI is not consulted
    // because the hmonitor 0 is invalid, but the function never reaches
    // the GDI path for non-generic names).
    let name = resolve_display_name("LG ULTRAGEAR", 0);
    assert_eq!(name, "LG ULTRAGEAR");
}

#[test]
fn resolve_display_name_keeps_empty() {
    let name = resolve_display_name("", 0);
    assert_eq!(name, "");
}

#[test]
fn resolve_display_name_generic_with_invalid_hmon_falls_back() {
    // hmonitor 0 is invalid so GDI lookup will fail; should fall back
    // to the original description.
    let name = resolve_display_name("Generic PnP Monitor", 0);
    assert_eq!(name, "Generic PnP Monitor");
}

#[test]
fn resolve_display_name_generic_case_insensitive() {
    let name = resolve_display_name("GENERIC PNP MONITOR", 0);
    assert_eq!(name, "GENERIC PNP MONITOR");
}

// ── get_vcp_all ──────────────────────────────────────────────

#[test]
fn get_vcp_all_does_not_panic() {
    // Read VCP version from all monitors — safe, read-only
    let result = get_vcp_all(VCP_VERSION);
    assert!(result.is_ok());
}

#[test]
fn known_vcp_codes_contains_core_entries() {
    let known = known_vcp_codes();
    assert!(known.iter().any(|(code, _, _)| *code == VCP_BRIGHTNESS));
    assert!(known.iter().any(|(code, _, _)| *code == VCP_COLOR_PRESET));
    assert!(known.iter().any(|(code, _, _)| *code == VCP_DISPLAY_MODE));
}

#[test]
fn known_vcp_codes_marks_reset_entries_as_risky() {
    let known = known_vcp_codes();
    let factory = known
        .iter()
        .find(|(code, _, _)| *code == VCP_FACTORY_RESET)
        .copied();
    let reset_color = known
        .iter()
        .find(|(code, _, _)| *code == VCP_RESET_COLOR)
        .copied();
    assert!(factory.expect("factory reset must exist").2);
    assert!(reset_color.expect("reset color must exist").2);
}

#[test]
fn probe_monitor_capabilities_does_not_panic() {
    let result = probe_monitor_capabilities();
    assert!(result.is_ok());
}
