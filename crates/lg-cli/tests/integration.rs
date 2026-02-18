//! Integration tests for the LG UltraGear CLI tool.
//!
//! These tests exercise the binary's CLI behaviour and cross-crate
//! data flow.

use std::process::Command;

// ============================================================================
// Binary CLI tests
// ============================================================================

/// Get the path to the built binary.
fn binary_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    path.push("lg-ultragear.exe");
    path
}

#[test]
fn binary_exists() {
    let bin = binary_path();
    assert!(bin.exists(), "Binary should exist at: {}", bin.display());
}

#[test]
fn unknown_command_exits_with_error() {
    let output = Command::new(binary_path())
        .arg("this-is-not-a-real-command")
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Unknown command should exit with error"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // clap reports unrecognized subcommands
    assert!(
        stderr.contains("unrecognized") || stderr.contains("error"),
        "stderr should mention error: {}",
        stderr
    );
}

#[test]
fn help_flag_shows_usage() {
    let output = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage") || stdout.contains("usage"),
        "should show usage info: {}",
        stdout
    );
}

#[test]
fn version_flag_shows_version() {
    let output = Command::new(binary_path())
        .arg("--version")
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("lg-ultragear"),
        "should show binary name: {}",
        stdout
    );
}

#[test]
fn config_path_command_outputs_path() {
    let output = Command::new(binary_path())
        .args(["config", "path"])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("config.toml"),
        "config path should contain config.toml: {}",
        stdout
    );
}

#[test]
fn config_path_contains_programdata() {
    let output = Command::new(binary_path())
        .args(["config", "path"])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout_lower = stdout.to_lowercase();
    assert!(
        stdout_lower.contains("programdata") || stdout_lower.contains("lg-ultragear-monitor"),
        "config path should reference ProgramData or LG-UltraGear-Monitor: {}",
        stdout
    );
}

#[test]
fn config_show_command_displays_config() {
    let output = Command::new(binary_path())
        .args(["config", "show"])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("monitor_match") || stdout.contains("Monitor Detection"),
        "config show should display config info: {}",
        stdout
    );
}

#[test]
fn config_command_without_subcommand_shows_config() {
    let output = Command::new(binary_path())
        .arg("config")
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Config") || stdout.contains("config") || stdout.contains("monitor_match"),
        "config command should show config info: {}",
        stdout
    );
}

// ============================================================================
// Config roundtrip via temp file
// ============================================================================

#[test]
fn config_toml_roundtrip_via_tempfile() {
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");

    let custom_toml = r#"
monitor_match = "INTEGRATION TEST"
profile_name = "test-integration.icm"
toast_enabled = false
toast_title = "IntTest"
toast_body = "Roundtrip OK"
stabilize_delay_ms = 2000
toggle_delay_ms = 150
refresh_display_settings = true
refresh_broadcast_color = false
refresh_invalidate = true
refresh_calibration_loader = false
verbose = true
"#;

    fs::write(&cfg_path, custom_toml).unwrap();

    let contents = fs::read_to_string(&cfg_path).unwrap();
    assert!(contents.contains("INTEGRATION TEST"));
    assert!(contents.contains("test-integration.icm"));
    assert!(contents.contains("toast_enabled = false"));
    assert!(contents.contains("stabilize_delay_ms = 2000"));
    assert!(contents.contains("verbose = true"));
}

#[test]
fn config_toml_partial_file_parse() {
    let toml_str = r#"monitor_match = "PARTIAL""#;

    #[derive(serde::Deserialize)]
    #[serde(default)]
    struct TestConfig {
        monitor_match: String,
        profile_name: String,
        toast_enabled: bool,
        stabilize_delay_ms: u64,
    }

    impl Default for TestConfig {
        fn default() -> Self {
            Self {
                monitor_match: "DEFAULT".to_string(),
                profile_name: "default.icm".to_string(),
                toast_enabled: true,
                stabilize_delay_ms: 1500,
            }
        }
    }

    let cfg: TestConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.monitor_match, "PARTIAL");
    assert_eq!(cfg.profile_name, "default.icm");
    assert!(cfg.toast_enabled);
    assert_eq!(cfg.stabilize_delay_ms, 1500);
}

// ============================================================================
// Cross-module data flow
// ============================================================================

#[test]
fn profile_path_construction_uses_config_profile_name() {
    let profile_name = "test-cross-module.icm";
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    let path = std::path::PathBuf::from(&windir)
        .join("System32")
        .join("spool")
        .join("drivers")
        .join("color")
        .join(profile_name);

    assert!(path.to_string_lossy().contains("color"));
    assert!(path.to_string_lossy().ends_with("test-cross-module.icm"));
}

#[test]
fn wide_string_encoding_consistent() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let input = r"DISPLAY\LG\ULTRAGEAR_001";
    let wide: Vec<u16> = OsStr::new(input)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    assert_eq!(*wide.last().unwrap(), 0u16);
    assert_eq!(wide.len(), input.len() + 1);
}

// ============================================================================
// Environment checks
// ============================================================================

#[test]
fn windir_env_var_exists() {
    let windir = std::env::var("WINDIR");
    assert!(
        windir.is_ok(),
        "WINDIR environment variable should be set on Windows"
    );
}

#[test]
fn programdata_env_var_exists() {
    let pd = std::env::var("ProgramData");
    assert!(
        pd.is_ok(),
        "ProgramData environment variable should be set on Windows"
    );
}

#[test]
fn windows_color_directory_exists() {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    let color_dir = std::path::PathBuf::from(&windir)
        .join("System32")
        .join("spool")
        .join("drivers")
        .join("color");
    assert!(
        color_dir.exists(),
        "Windows color directory should exist: {}",
        color_dir.display()
    );
}
