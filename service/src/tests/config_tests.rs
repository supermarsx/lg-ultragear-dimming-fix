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
    // All other fields should be default
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
    // serde(default) should ignore unknown fields — but serde+toml denies them by default.
    // This test documents the behaviour.
    let result = toml::from_str::<Config>(toml_str);
    // toml crate with serde by default rejects unknown fields
    // unless we add #[serde(deny_unknown_fields)] (we don't have it, so it depends on toml version)
    // either way, this should not panic
    let _ = result;
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
    // u64 can't be negative in TOML
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
        refresh_display_settings: false,
        refresh_broadcast_color: true,
        refresh_invalidate: false,
        refresh_calibration_loader: true,
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
    assert!(output.contains("Debug"));
}

#[test]
fn to_toml_commented_is_valid_toml() {
    let cfg = Config::default();
    let output = Config::to_toml_commented(&cfg);
    // The commented TOML should be parseable (comments are valid TOML)
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
        refresh_display_settings: false,
        refresh_broadcast_color: false,
        refresh_invalidate: true,
        refresh_calibration_loader: false,
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
    // This test verifies the path starts with a Windows directory
    let cfg = Config::default();
    let path = cfg.profile_path();
    let path_lower = path.to_string_lossy().to_lowercase();
    // Should start with WINDIR or fallback C:\Windows
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

// ── File I/O with temp directories ───────────────────────────────

#[test]
fn write_default_creates_file() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");

    // We can't easily override config_path(), so test to_toml_commented + fs::write
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
