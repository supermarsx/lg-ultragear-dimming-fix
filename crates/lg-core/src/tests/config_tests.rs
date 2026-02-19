use super::*;
use std::fs;

// ── Default values ───────────────────────────────────────────────

#[test]
fn default_config_has_expected_monitor_match() {
    let cfg = Config::default();
    assert_eq!(cfg.monitor_match, "LG ULTRAGEAR");
}

#[test]
fn default_config_has_expected_profile_name() {
    let cfg = Config::default();
    assert_eq!(cfg.profile_name, "lg-ultragear-full-cal.icm");
}

#[test]
fn default_config_toast_enabled() {
    let cfg = Config::default();
    assert!(cfg.toast_enabled);
}

#[test]
fn default_config_toast_title() {
    let cfg = Config::default();
    assert_eq!(cfg.toast_title, "LG UltraGear");
}

#[test]
fn default_config_toast_body() {
    let cfg = Config::default();
    assert_eq!(cfg.toast_body, "Color profile reapplied ✓");
}

#[test]
fn default_config_stabilize_delay() {
    let cfg = Config::default();
    assert_eq!(cfg.stabilize_delay_ms, 1500);
}

#[test]
fn default_config_toggle_delay() {
    let cfg = Config::default();
    assert_eq!(cfg.toggle_delay_ms, 100);
}

#[test]
fn default_config_reapply_delay() {
    let cfg = Config::default();
    assert_eq!(cfg.reapply_delay_ms, 12000);
}

#[test]
fn default_config_all_refresh_methods_enabled() {
    let cfg = Config::default();
    assert!(cfg.refresh_display_settings);
    assert!(cfg.refresh_broadcast_color);
    assert!(cfg.refresh_invalidate);
    assert!(cfg.refresh_calibration_loader);
}

#[test]
fn default_config_verbose_is_false() {
    let cfg = Config::default();
    assert!(!cfg.verbose);
}

#[test]
fn default_config_ddc_brightness_off() {
    let cfg = Config::default();
    assert!(!cfg.ddc_brightness_on_reapply);
}

#[test]
fn default_config_ddc_brightness_value_is_50() {
    let cfg = Config::default();
    assert_eq!(cfg.ddc_brightness_value, 50);
}

// ── TOML parsing ─────────────────────────────────────────────────

#[test]
fn parse_full_toml() {
    let toml_str = r#"
        monitor_match = "ASUS ROG"
        profile_name = "custom.icm"
        toast_enabled = false
        toast_title = "My Title"
        toast_body = "Done!"
        stabilize_delay_ms = 3000
        toggle_delay_ms = 200
        refresh_display_settings = false
        refresh_broadcast_color = false
        refresh_invalidate = false
        refresh_calibration_loader = false
        verbose = true
    "#;

    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.monitor_match, "ASUS ROG");
    assert_eq!(cfg.profile_name, "custom.icm");
    assert!(!cfg.toast_enabled);
    assert_eq!(cfg.toast_title, "My Title");
    assert_eq!(cfg.toast_body, "Done!");
    assert_eq!(cfg.stabilize_delay_ms, 3000);
    assert_eq!(cfg.toggle_delay_ms, 200);
    assert!(!cfg.refresh_display_settings);
    assert!(!cfg.refresh_broadcast_color);
    assert!(!cfg.refresh_invalidate);
    assert!(!cfg.refresh_calibration_loader);
    assert!(cfg.verbose);
}

#[test]
fn parse_partial_toml_fills_defaults() {
    let toml_str = r#"
        monitor_match = "DELL"
    "#;

    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.monitor_match, "DELL");
    assert_eq!(cfg.profile_name, "lg-ultragear-full-cal.icm");
    assert!(cfg.toast_enabled);
    assert_eq!(cfg.stabilize_delay_ms, 1500);
    assert_eq!(cfg.toggle_delay_ms, 100);
    assert!(cfg.refresh_display_settings);
    assert!(!cfg.verbose);
}

#[test]
fn parse_empty_toml_gives_defaults() {
    let cfg: Config = toml::from_str("").unwrap();
    let def = Config::default();
    assert_eq!(cfg.monitor_match, def.monitor_match);
    assert_eq!(cfg.profile_name, def.profile_name);
    assert_eq!(cfg.toast_enabled, def.toast_enabled);
    assert_eq!(cfg.stabilize_delay_ms, def.stabilize_delay_ms);
    assert_eq!(cfg.toggle_delay_ms, def.toggle_delay_ms);
}

#[test]
fn parse_malformed_toml_fails() {
    let toml_str = "this is not valid toml {{{{";
    let result = toml::from_str::<Config>(toml_str);
    assert!(result.is_err());
}

#[test]
fn parse_toml_with_extra_fields_is_ok() {
    let toml_str = r#"
        monitor_match = "LG"
        some_future_field = 42
        another_field = "hello"
    "#;
    // serde+toml behaviour with unknown fields — should not panic
    let _ = toml::from_str::<Config>(toml_str);
}

#[test]
fn parse_toml_wrong_type_for_field_fails() {
    let toml_str = r#"
        stabilize_delay_ms = "not a number"
    "#;
    let result = toml::from_str::<Config>(toml_str);
    assert!(result.is_err());
}

#[test]
fn parse_toml_negative_delay_fails() {
    let toml_str = r#"
        stabilize_delay_ms = -1
    "#;
    let result = toml::from_str::<Config>(toml_str);
    assert!(result.is_err());
}

// ── Serialization roundtrip ──────────────────────────────────────

#[test]
fn serialize_roundtrip() {
    let original = Config {
        monitor_match: "TestMonitor".to_string(),
        profile_name: "test.icm".to_string(),
        toast_enabled: false,
        toast_title: "T".to_string(),
        toast_body: "B".to_string(),
        stabilize_delay_ms: 999,
        toggle_delay_ms: 50,
        reapply_delay_ms: 8000,
        refresh_display_settings: false,
        refresh_broadcast_color: true,
        refresh_invalidate: false,
        refresh_calibration_loader: true,
        ddc_brightness_on_reapply: true,
        ddc_brightness_value: 75,
        verbose: true,
    };

    let toml_str = toml::to_string(&original).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();

    assert_eq!(parsed.monitor_match, original.monitor_match);
    assert_eq!(parsed.profile_name, original.profile_name);
    assert_eq!(parsed.toast_enabled, original.toast_enabled);
    assert_eq!(parsed.toast_title, original.toast_title);
    assert_eq!(parsed.toast_body, original.toast_body);
    assert_eq!(parsed.stabilize_delay_ms, original.stabilize_delay_ms);
    assert_eq!(parsed.toggle_delay_ms, original.toggle_delay_ms);
    assert_eq!(
        parsed.refresh_display_settings,
        original.refresh_display_settings
    );
    assert_eq!(
        parsed.refresh_broadcast_color,
        original.refresh_broadcast_color
    );
    assert_eq!(parsed.refresh_invalidate, original.refresh_invalidate);
    assert_eq!(
        parsed.refresh_calibration_loader,
        original.refresh_calibration_loader
    );
    assert_eq!(
        parsed.ddc_brightness_on_reapply,
        original.ddc_brightness_on_reapply
    );
    assert_eq!(
        parsed.ddc_brightness_value,
        original.ddc_brightness_value
    );
    assert_eq!(parsed.verbose, original.verbose);
}

// ── to_toml_commented ────────────────────────────────────────────

#[test]
fn to_toml_commented_contains_all_field_values() {
    let cfg = Config::default();
    let output = Config::to_toml_commented(&cfg);

    assert!(
        output.contains("LG ULTRAGEAR"),
        "should contain monitor_match"
    );
    assert!(
        output.contains("lg-ultragear-full-cal.icm"),
        "should contain profile_name"
    );
    assert!(
        output.contains("toast_enabled = true"),
        "should contain toast_enabled"
    );
    assert!(
        output.contains("stabilize_delay_ms = 1500"),
        "should contain stabilize_delay_ms"
    );
    assert!(
        output.contains("toggle_delay_ms = 100"),
        "should contain toggle_delay_ms"
    );
}

#[test]
fn to_toml_commented_contains_section_headers() {
    let cfg = Config::default();
    let output = Config::to_toml_commented(&cfg);

    assert!(output.contains("Monitor Detection"));
    assert!(output.contains("Toast Notifications"));
    assert!(output.contains("Timing"));
    assert!(output.contains("Refresh Methods"));
    assert!(output.contains("DDC/CI Brightness"));
    assert!(output.contains("Debug"));
}

#[test]
fn to_toml_commented_is_valid_toml() {
    let cfg = Config::default();
    let output = Config::to_toml_commented(&cfg);
    let parsed: Result<Config, _> = toml::from_str(&output);
    assert!(
        parsed.is_ok(),
        "Commented TOML should be valid: {:?}",
        parsed.err()
    );
}

#[test]
fn to_toml_commented_roundtrip_preserves_values() {
    let original = Config {
        monitor_match: "Custom Monitor".to_string(),
        profile_name: "custom.icm".to_string(),
        toast_enabled: false,
        toast_title: "Custom".to_string(),
        toast_body: "Applied".to_string(),
        stabilize_delay_ms: 5000,
        toggle_delay_ms: 250,
        reapply_delay_ms: 15000,
        refresh_display_settings: false,
        refresh_broadcast_color: false,
        refresh_invalidate: true,
        refresh_calibration_loader: false,
        ddc_brightness_on_reapply: true,
        ddc_brightness_value: 80,
        verbose: true,
    };

    let commented = Config::to_toml_commented(&original);
    let parsed: Config = toml::from_str(&commented).unwrap();

    assert_eq!(parsed.monitor_match, original.monitor_match);
    assert_eq!(parsed.profile_name, original.profile_name);
    assert_eq!(parsed.toast_enabled, original.toast_enabled);
    assert_eq!(parsed.stabilize_delay_ms, original.stabilize_delay_ms);
    assert_eq!(parsed.toggle_delay_ms, original.toggle_delay_ms);
    assert_eq!(parsed.verbose, original.verbose);
}

// ── profile_path ─────────────────────────────────────────────────

#[test]
fn profile_path_contains_color_directory() {
    let cfg = Config::default();
    let path = cfg.profile_path();
    let path_str = path.to_string_lossy().to_lowercase();
    assert!(path_str.contains("spool"));
    assert!(path_str.contains("drivers"));
    assert!(path_str.contains("color"));
}

#[test]
fn profile_path_ends_with_profile_name() {
    let cfg = Config {
        profile_name: "my-custom-profile.icm".to_string(),
        ..Config::default()
    };
    let path = cfg.profile_path();
    assert!(path.ends_with("my-custom-profile.icm"));
}

#[test]
fn profile_path_uses_windir_env() {
    let cfg = Config::default();
    let path = cfg.profile_path();
    let path_lower = path.to_string_lossy().to_lowercase();
    assert!(
        path_lower.contains("windows"),
        "Path should reference Windows dir: {}",
        path.display()
    );
}

// ── config_dir / config_path ─────────────────────────────────────

#[test]
fn config_dir_contains_lg_folder() {
    let dir = config_dir();
    let dir_str = dir.to_string_lossy();
    assert!(dir_str.contains("LG-UltraGear-Monitor"));
}

#[test]
fn config_path_ends_with_toml() {
    let path = config_path();
    assert_eq!(path.extension().unwrap(), "toml");
}

#[test]
fn config_path_is_inside_config_dir() {
    let dir = config_dir();
    let path = config_path();
    assert!(path.starts_with(&dir));
}

// ── install_path ─────────────────────────────────────────────────

#[test]
fn install_path_is_inside_config_dir() {
    let dir = config_dir();
    let path = install_path();
    assert!(path.starts_with(&dir));
}

#[test]
fn install_path_ends_with_exe() {
    let path = install_path();
    assert_eq!(path.extension().unwrap(), "exe");
}

#[test]
fn install_path_has_expected_filename() {
    let path = install_path();
    assert_eq!(path.file_name().unwrap(), "lg-ultragear-dimming-fix.exe");
}

// ── File I/O with temp directories ───────────────────────────────

#[test]
fn write_default_creates_file() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");

    let cfg = Config::default();
    let content = Config::to_toml_commented(&cfg);
    fs::write(&cfg_path, &content).unwrap();

    assert!(cfg_path.exists());
    let read_back = fs::read_to_string(&cfg_path).unwrap();
    assert!(read_back.contains("monitor_match"));
}

#[test]
fn write_and_read_config_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");

    let original = Config {
        monitor_match: "Roundtrip Test".to_string(),
        profile_name: "roundtrip.icm".to_string(),
        toast_enabled: false,
        stabilize_delay_ms: 2222,
        toggle_delay_ms: 333,
        verbose: true,
        ..Config::default()
    };

    let content = Config::to_toml_commented(&original);
    fs::write(&cfg_path, &content).unwrap();

    let read_back = fs::read_to_string(&cfg_path).unwrap();
    let parsed: Config = toml::from_str(&read_back).unwrap();

    assert_eq!(parsed.monitor_match, "Roundtrip Test");
    assert_eq!(parsed.profile_name, "roundtrip.icm");
    assert!(!parsed.toast_enabled);
    assert_eq!(parsed.stabilize_delay_ms, 2222);
    assert_eq!(parsed.toggle_delay_ms, 333);
    assert!(parsed.verbose);
}

// ── Edge cases ───────────────────────────────────────────────────

#[test]
fn config_with_unicode_monitor_match() {
    let toml_str = r#"
        monitor_match = "モニター"
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.monitor_match, "モニター");
}

#[test]
fn config_with_empty_string_fields() {
    let toml_str = r#"
        monitor_match = ""
        profile_name = ""
        toast_title = ""
        toast_body = ""
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.monitor_match, "");
    assert_eq!(cfg.profile_name, "");
}

#[test]
fn config_with_zero_delays() {
    let toml_str = r#"
        stabilize_delay_ms = 0
        toggle_delay_ms = 0
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.stabilize_delay_ms, 0);
    assert_eq!(cfg.toggle_delay_ms, 0);
}

#[test]
fn config_with_large_delays() {
    let toml_str = r#"
        stabilize_delay_ms = 999999999
        toggle_delay_ms = 999999999
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.stabilize_delay_ms, 999999999);
    assert_eq!(cfg.toggle_delay_ms, 999999999);
}

#[test]
fn config_clone_is_equal() {
    let original = Config::default();
    let cloned = original.clone();
    assert_eq!(original.monitor_match, cloned.monitor_match);
    assert_eq!(original.profile_name, cloned.profile_name);
    assert_eq!(original.toast_enabled, cloned.toast_enabled);
    assert_eq!(original.stabilize_delay_ms, cloned.stabilize_delay_ms);
}

#[test]
fn config_debug_format() {
    let cfg = Config::default();
    let debug_str = format!("{:?}", cfg);
    assert!(debug_str.contains("Config"));
    assert!(debug_str.contains("monitor_match"));
}

// ── escape_toml_string ───────────────────────────────────────────

#[test]
fn escape_toml_string_plain_text_unchanged() {
    assert_eq!(escape_toml_string("hello world"), "hello world");
}

#[test]
fn escape_toml_string_escapes_backslash() {
    assert_eq!(escape_toml_string(r"C:\path"), r"C:\\path");
}

#[test]
fn escape_toml_string_escapes_double_quote() {
    assert_eq!(escape_toml_string(r#"say "hi""#), r#"say \"hi\""#);
}

#[test]
fn escape_toml_string_escapes_newline() {
    assert_eq!(escape_toml_string("line1\nline2"), r"line1\nline2");
}

#[test]
fn escape_toml_string_escapes_tab() {
    assert_eq!(escape_toml_string("col1\tcol2"), r"col1\tcol2");
}

#[test]
fn escape_toml_string_preserves_unicode() {
    assert_eq!(
        escape_toml_string("Color profile reapplied ✓"),
        "Color profile reapplied ✓"
    );
}

#[test]
fn escape_toml_string_empty() {
    assert_eq!(escape_toml_string(""), "");
}

#[test]
fn escape_toml_string_combined() {
    assert_eq!(escape_toml_string("a\\b\"c\nd"), r#"a\\b\"c\nd"#);
}

// ── TOML injection prevention ────────────────────────────────────

#[test]
fn to_toml_commented_with_quotes_in_values_is_valid() {
    let cfg = Config {
        monitor_match: r#"LG "ULTRAGEAR""#.to_string(),
        toast_title: r#"Title with "quotes""#.to_string(),
        toast_body: "Body with 'apostrophes'".to_string(),
        ..Config::default()
    };
    let output = Config::to_toml_commented(&cfg);
    let parsed: Result<Config, _> = toml::from_str(&output);
    assert!(
        parsed.is_ok(),
        "TOML with escaped quotes should parse: {:?}",
        parsed.err()
    );
    let parsed = parsed.unwrap();
    assert_eq!(parsed.monitor_match, r#"LG "ULTRAGEAR""#);
    assert_eq!(parsed.toast_title, r#"Title with "quotes""#);
}

#[test]
fn to_toml_commented_with_backslashes_in_values_is_valid() {
    let cfg = Config {
        monitor_match: r"DISPLAY\LG\001".to_string(),
        profile_name: r"path\to\file.icm".to_string(),
        ..Config::default()
    };
    let output = Config::to_toml_commented(&cfg);
    let parsed: Result<Config, _> = toml::from_str(&output);
    assert!(
        parsed.is_ok(),
        "TOML with escaped backslashes should parse: {:?}",
        parsed.err()
    );
    let parsed = parsed.unwrap();
    assert_eq!(parsed.monitor_match, r"DISPLAY\LG\001");
    assert_eq!(parsed.profile_name, r"path\to\file.icm");
}

#[test]
fn to_toml_commented_with_newlines_in_values_is_valid() {
    let cfg = Config {
        toast_body: "Line 1\nLine 2".to_string(),
        ..Config::default()
    };
    let output = Config::to_toml_commented(&cfg);
    let parsed: Result<Config, _> = toml::from_str(&output);
    assert!(
        parsed.is_ok(),
        "TOML with escaped newlines should parse: {:?}",
        parsed.err()
    );
    let parsed = parsed.unwrap();
    assert_eq!(parsed.toast_body, "Line 1\nLine 2");
}

// ── parse_full_toml reapply_delay_ms coverage ────────────────────

#[test]
fn parse_toml_with_reapply_delay() {
    let toml_str = r#"
        reapply_delay_ms = 15000
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.reapply_delay_ms, 15000);
}

#[test]
fn parse_full_toml_including_reapply_delay() {
    let toml_str = r#"
        monitor_match = "TEST"
        profile_name = "test.icm"
        toast_enabled = false
        toast_title = "T"
        toast_body = "B"
        stabilize_delay_ms = 2000
        toggle_delay_ms = 200
        reapply_delay_ms = 18000
        refresh_display_settings = false
        refresh_broadcast_color = false
        refresh_invalidate = false
        refresh_calibration_loader = false
        verbose = true
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.reapply_delay_ms, 18000);
    assert_eq!(cfg.stabilize_delay_ms, 2000);
    assert_eq!(cfg.toggle_delay_ms, 200);
}

// ── DDC brightness TOML parsing ──────────────────────────────────

#[test]
fn parse_toml_with_ddc_brightness() {
    let toml_str = r#"
        ddc_brightness_on_reapply = true
        ddc_brightness_value = 80
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(cfg.ddc_brightness_on_reapply);
    assert_eq!(cfg.ddc_brightness_value, 80);
}

#[test]
fn parse_toml_ddc_brightness_defaults_when_omitted() {
    let toml_str = r#"
        monitor_match = "TEST"
    "#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(!cfg.ddc_brightness_on_reapply);
    assert_eq!(cfg.ddc_brightness_value, 50);
}

#[test]
fn to_toml_commented_contains_ddc_section() {
    let cfg = Config::default();
    let output = Config::to_toml_commented(&cfg);
    assert!(output.contains("DDC/CI Brightness"), "should contain DDC section header");
    assert!(output.contains("ddc_brightness_on_reapply = false"), "should contain ddc toggle");
    assert!(output.contains("ddc_brightness_value = 50"), "should contain ddc value");
}
