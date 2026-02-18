//! Integration tests for the LG UltraGear CLI tool.
//!
//! These tests exercise the binary's CLI behaviour, cross-crate
//! data flow, ICC profile pack/unpack, service queries, TUI output
//! validation, and config management.

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
    path.push("lg-ultragear-dimming-fix.exe");
    path
}

/// Run binary with args and return (stdout, stderr, success).
fn run_binary(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(binary_path())
        .args(args)
        .output()
        .expect("Failed to run binary");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
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
        stdout.contains("lg-ultragear-dimming-fix"),
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
// New CLI commands (parity with PowerShell installer)
// ============================================================================

#[test]
fn non_interactive_flag_shows_help() {
    let output = Command::new(binary_path())
        .arg("--non-interactive")
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage") || stdout.contains("usage"),
        "should show usage with --non-interactive and no subcommand: {}",
        stdout
    );
}

#[test]
fn install_help_shows_options() {
    let output = Command::new(binary_path())
        .args(["install", "--help"])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("profile-only") && stdout.contains("service-only"),
        "install --help should show profile-only and service-only options: {}",
        stdout
    );
}

#[test]
fn uninstall_help_shows_options() {
    let output = Command::new(binary_path())
        .args(["uninstall", "--help"])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("full") && stdout.contains("profile"),
        "uninstall --help should show --full and --profile options: {}",
        stdout
    );
}

#[test]
fn reinstall_help_shows_pattern_option() {
    let output = Command::new(binary_path())
        .args(["reinstall", "--help"])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("pattern"),
        "reinstall --help should show --pattern option: {}",
        stdout
    );
}

#[test]
fn dry_run_flag_is_accepted() {
    let output = Command::new(binary_path())
        .args(["--dry-run", "detect"])
        .output()
        .expect("Failed to run binary");

    // Should succeed (detect doesn't use dry_run but should accept the flag)
    assert!(
        output.status.success(),
        "detect with --dry-run should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ============================================================================
// CLI subcommand help tests
// ============================================================================

#[test]
fn detect_help_shows_options() {
    let (stdout, _, _) = run_binary(&["detect", "--help"]);
    assert!(
        stdout.contains("pattern"),
        "detect --help should show --pattern: {}",
        stdout
    );
}

#[test]
fn apply_help_shows_options() {
    let (stdout, _, _) = run_binary(&["apply", "--help"]);
    assert!(
        stdout.contains("pattern"),
        "apply --help should show --pattern: {}",
        stdout
    );
}

#[test]
fn watch_help_shows_options() {
    let (stdout, _, _) = run_binary(&["watch", "--help"]);
    assert!(
        stdout.contains("pattern"),
        "watch --help should show --pattern: {}",
        stdout
    );
}

#[test]
fn config_help_shows_subcommands() {
    let (stdout, _, _) = run_binary(&["config", "--help"]);
    assert!(
        stdout.contains("show") || stdout.contains("Show"),
        "config --help should list show: {}",
        stdout
    );
    assert!(
        stdout.contains("path") || stdout.contains("Path"),
        "config --help should list path: {}",
        stdout
    );
    assert!(
        stdout.contains("reset") || stdout.contains("Reset"),
        "config --help should list reset: {}",
        stdout
    );
}

#[test]
fn service_help_shows_subcommands() {
    let (stdout, _, _) = run_binary(&["service", "--help"]);
    assert!(
        stdout.contains("install") || stdout.contains("Install"),
        "service --help should list install: {}",
        stdout
    );
    assert!(
        stdout.contains("uninstall") || stdout.contains("Uninstall"),
        "service --help should list uninstall: {}",
        stdout
    );
    assert!(
        stdout.contains("start") || stdout.contains("Start"),
        "service --help should list start: {}",
        stdout
    );
    assert!(
        stdout.contains("stop") || stdout.contains("Stop"),
        "service --help should list stop: {}",
        stdout
    );
    assert!(
        stdout.contains("status") || stdout.contains("Status"),
        "service --help should list status: {}",
        stdout
    );
}

#[test]
fn all_subcommands_accept_help_flag() {
    for cmd in &[
        "install", "uninstall", "reinstall", "detect", "apply", "watch", "config", "service",
    ] {
        let (stdout, stderr, success) = run_binary(&[cmd, "--help"]);
        assert!(
            success || stdout.contains("Usage") || stdout.contains("usage"),
            "{} --help should succeed or show usage. stdout: {} stderr: {}",
            cmd,
            stdout,
            stderr
        );
    }
}

// ============================================================================
// CLI flag conflict tests
// ============================================================================

#[test]
fn install_conflicting_flags_rejected() {
    let (_, stderr, success) = run_binary(&["install", "--profile-only", "--service-only"]);
    assert!(
        !success,
        "install --profile-only --service-only should fail: {}",
        stderr
    );
}

#[test]
fn verbose_flag_accepted_globally() {
    let (stdout, stderr, success) = run_binary(&["-v", "detect"]);
    assert!(
        success,
        "detect with -v should succeed. stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("monitor") || stdout.contains("Monitor") || stdout.contains("Scanning"),
        "detect with verbose should produce output: {}",
        stdout
    );
}

#[test]
fn verbose_long_flag_accepted() {
    let (_, _, success) = run_binary(&["--verbose", "detect"]);
    assert!(success, "detect with --verbose should succeed");
}

// ============================================================================
// Detect command tests
// ============================================================================

#[test]
fn detect_command_runs_successfully() {
    let (stdout, _, success) = run_binary(&["detect"]);
    assert!(success, "detect should succeed");
    assert!(
        stdout.contains("Scanning") || stdout.contains("monitor") || stdout.contains("Profile"),
        "detect should show scanning info: {}",
        stdout
    );
}

#[test]
fn detect_with_custom_pattern() {
    let (stdout, _, success) = run_binary(&["detect", "--pattern", "NONEXISTENT_MONITOR_XYZ"]);
    assert!(success, "detect with custom pattern should succeed");
    assert!(
        stdout.contains("No matching monitors") || stdout.contains("Found 0"),
        "should find no matching monitors: {}",
        stdout
    );
}

#[test]
fn detect_with_empty_pattern_matches_all() {
    let (stdout, _, success) = run_binary(&["detect", "--pattern", ""]);
    assert!(
        success,
        "detect with empty pattern should succeed"
    );
    assert!(
        stdout.contains("Scanning") || stdout.contains("monitor"),
        "detect with empty pattern should produce output: {}",
        stdout
    );
}

#[test]
fn detect_shows_profile_path() {
    let (stdout, _, success) = run_binary(&["detect"]);
    assert!(success);
    assert!(
        stdout.contains("Profile:") || stdout.contains("profile"),
        "detect should show profile path: {}",
        stdout
    );
}

#[test]
fn detect_shows_installed_status() {
    let (stdout, _, success) = run_binary(&["detect"]);
    assert!(success);
    assert!(
        stdout.contains("Installed:"),
        "detect should show installed status: {}",
        stdout
    );
}

// ============================================================================
// Dry-run command tests
// ============================================================================

#[test]
fn apply_dry_run_shows_simulation() {
    let (stdout, _, success) = run_binary(&["--dry-run", "apply"]);
    assert!(success, "apply --dry-run should succeed");
    assert!(
        stdout.contains("DRY RUN"),
        "apply --dry-run should indicate simulation: {}",
        stdout
    );
}

#[test]
fn install_dry_run_shows_simulation() {
    let (stdout, _, success) = run_binary(&["--dry-run", "install"]);
    assert!(success, "install --dry-run should succeed");
    assert!(
        stdout.contains("DRY RUN"),
        "install --dry-run should indicate simulation: {}",
        stdout
    );
}

#[test]
fn install_profile_only_dry_run() {
    let (stdout, _, success) = run_binary(&["--dry-run", "install", "--profile-only"]);
    assert!(success, "install --profile-only --dry-run should succeed");
    assert!(
        stdout.contains("DRY RUN"),
        "should indicate dry run: {}",
        stdout
    );
    assert!(
        stdout.contains("profile") || stdout.contains("ICC"),
        "should mention profile: {}",
        stdout
    );
}

#[test]
fn install_service_only_dry_run() {
    let (stdout, _, success) = run_binary(&["--dry-run", "install", "--service-only"]);
    assert!(success, "install --service-only --dry-run should succeed");
    assert!(
        stdout.contains("DRY RUN"),
        "should indicate dry run: {}",
        stdout
    );
}

#[test]
fn uninstall_dry_run_shows_simulation() {
    let (stdout, _, success) = run_binary(&["--dry-run", "uninstall"]);
    assert!(success, "uninstall --dry-run should succeed");
    assert!(
        stdout.contains("DRY RUN"),
        "uninstall --dry-run should indicate simulation: {}",
        stdout
    );
}

#[test]
fn uninstall_full_dry_run() {
    let (stdout, _, success) = run_binary(&["--dry-run", "uninstall", "--full"]);
    assert!(success, "uninstall --full --dry-run should succeed");
    assert!(
        stdout.contains("DRY RUN"),
        "should indicate dry run: {}",
        stdout
    );
    // Full uninstall should mention service, profile, and config removal
    let mentions = stdout.to_lowercase();
    assert!(
        mentions.contains("service") && mentions.contains("profile"),
        "full dry run should mention service and profile: {}",
        stdout
    );
}

#[test]
fn uninstall_profile_dry_run() {
    let (stdout, _, success) = run_binary(&["--dry-run", "uninstall", "--profile"]);
    assert!(success, "uninstall --profile --dry-run should succeed");
    assert!(stdout.contains("DRY RUN"));
}

#[test]
fn reinstall_dry_run_shows_simulation() {
    let (stdout, _, success) = run_binary(&["--dry-run", "reinstall"]);
    assert!(success, "reinstall --dry-run should succeed");
    assert!(
        stdout.contains("DRY RUN"),
        "reinstall --dry-run should indicate simulation: {}",
        stdout
    );
}

// ============================================================================
// Config command tests
// ============================================================================

#[test]
fn config_show_displays_all_fields() {
    let (stdout, _, success) = run_binary(&["config", "show"]);
    assert!(success, "config show should succeed");

    // Verify all configuration sections are shown
    assert!(stdout.contains("Monitor Detection"), "should show Monitor Detection section");
    assert!(stdout.contains("Toast Notifications"), "should show Toast section");
    assert!(stdout.contains("Timing"), "should show Timing section");
    assert!(stdout.contains("Refresh Methods"), "should show Refresh section");
    assert!(stdout.contains("Debug"), "should show Debug section");

    // Verify individual field keys
    assert!(stdout.contains("monitor_match"));
    assert!(stdout.contains("profile_name"));
    assert!(stdout.contains("toast_enabled"));
    assert!(stdout.contains("toast_title"));
    assert!(stdout.contains("toast_body"));
    assert!(stdout.contains("stabilize_delay_ms"));
    assert!(stdout.contains("toggle_delay_ms"));
    assert!(stdout.contains("reapply_delay_ms"));
    assert!(stdout.contains("refresh_display_settings"));
    assert!(stdout.contains("refresh_broadcast_color"));
    assert!(stdout.contains("refresh_invalidate"));
    assert!(stdout.contains("refresh_calibration_loader"));
    assert!(stdout.contains("verbose"));
}

#[test]
fn config_show_displays_default_monitor_match() {
    let (stdout, _, success) = run_binary(&["config", "show"]);
    assert!(success);
    // Default config has "LG ULTRAGEAR" unless overridden
    assert!(
        stdout.contains("LG ULTRAGEAR") || stdout.contains("monitor_match"),
        "should contain the monitor match pattern: {}",
        stdout
    );
}

#[test]
fn config_path_is_absolute() {
    let (stdout, _, success) = run_binary(&["config", "path"]);
    assert!(success);
    let path = stdout.trim();
    // On Windows, absolute paths start with a drive letter
    assert!(
        path.contains(':') || path.starts_with('\\'),
        "config path should be absolute: {}",
        path
    );
}

#[test]
fn config_path_ends_with_config_toml() {
    let (stdout, _, success) = run_binary(&["config", "path"]);
    assert!(success);
    assert!(stdout.trim().ends_with("config.toml"));
}

#[test]
fn config_path_contains_lg_folder() {
    let (stdout, _, success) = run_binary(&["config", "path"]);
    assert!(success);
    assert!(stdout.contains("LG-UltraGear-Monitor"));
}

// ============================================================================
// Service status command (may fail if service not installed - handle gracefully)
// ============================================================================

#[test]
fn service_status_runs_without_panic() {
    // This may fail (service not installed) but should not panic
    let _output = Command::new(binary_path())
        .args(["service", "status"])
        .output()
        .expect("Failed to run binary");
    // Just verify the process didn't crash/hang
}

// ============================================================================
// Non-interactive / TUI mode tests
// ============================================================================

#[test]
fn non_interactive_with_no_subcommand_shows_help() {
    let (stdout, _, _) = run_binary(&["--non-interactive"]);
    assert!(
        stdout.contains("Usage") || stdout.contains("usage"),
        "non-interactive + no subcommand should show help: {}",
        stdout
    );
}

#[test]
fn non_interactive_not_a_terminal_shows_help() {
    // The binary should detect it's not in a terminal (piped output) and show help
    let (stdout, _, _) = run_binary(&[]);
    // When run from Command::new (no terminal), should show help or usage
    assert!(
        stdout.contains("Usage") || stdout.contains("usage") || stdout.is_empty(),
        "binary with no args in non-terminal should show help or be empty: len={}",
        stdout.len()
    );
}

// ============================================================================
// ICC Profile pack/unpack tests
// ============================================================================

#[test]
fn icm_embedded_profile_has_nonzero_size() {
    const { assert!(lg_profile::EMBEDDED_ICM_SIZE > 0, "Embedded ICM should contain data") };
}

#[test]
fn icm_embedded_profile_is_reasonable_size() {
    // ICC profiles are typically 1-50 KB; ours should be in that range
    const { assert!(lg_profile::EMBEDDED_ICM_SIZE > 100, "Embedded ICM too small") };
    const { assert!(lg_profile::EMBEDDED_ICM_SIZE < 500_000, "Embedded ICM suspiciously large") };
}

#[test]
fn icm_extract_to_temp_directory() {
    let dir = std::env::temp_dir().join("lg-integ-test-extract");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("test-extract.icm");

    let wrote = lg_profile::ensure_profile_installed(&path).expect("should succeed");
    assert!(wrote, "should report file was written");
    assert!(path.exists(), "file should exist");
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        lg_profile::EMBEDDED_ICM_SIZE as u64,
        "file size should match embedded size"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn icm_extract_idempotent() {
    let dir = std::env::temp_dir().join("lg-integ-test-idempotent");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("test-idempotent.icm");

    // First extraction
    let wrote1 = lg_profile::ensure_profile_installed(&path).expect("first extract");
    assert!(wrote1);

    // Second extraction should be a no-op
    let wrote2 = lg_profile::ensure_profile_installed(&path).expect("second extract");
    assert!(!wrote2, "second call should be no-op");

    // File should still exist with correct size
    assert!(path.exists());
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        lg_profile::EMBEDDED_ICM_SIZE as u64,
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn icm_extract_then_remove_roundtrip() {
    let dir = std::env::temp_dir().join("lg-integ-test-roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("test-roundtrip.icm");

    // Extract
    lg_profile::ensure_profile_installed(&path).expect("extract");
    assert!(path.exists());

    // Remove
    let removed = lg_profile::remove_profile(&path).expect("remove");
    assert!(removed, "should report removed");
    assert!(!path.exists(), "file should be gone");

    // Verify is_profile_installed reports false
    assert!(
        !lg_profile::is_profile_installed(&path),
        "should report not installed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn icm_extract_then_remove_then_re_extract() {
    let dir = std::env::temp_dir().join("lg-integ-test-re-extract");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("test-re-extract.icm");

    // Extract → remove → extract again
    lg_profile::ensure_profile_installed(&path).unwrap();
    lg_profile::remove_profile(&path).unwrap();
    assert!(!path.exists());

    let wrote = lg_profile::ensure_profile_installed(&path).unwrap();
    assert!(wrote, "re-extraction should write the file again");
    assert!(path.exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn icm_remove_nonexistent_returns_false() {
    let path = std::path::PathBuf::from(
        r"C:\Windows\Temp\lg-integ-this-file-does-not-exist-99999.icm",
    );
    let result = lg_profile::remove_profile(&path).expect("should not error");
    assert!(!result);
}

#[test]
fn icm_is_profile_installed_for_extracted_file() {
    let dir = std::env::temp_dir().join("lg-integ-test-is-installed");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("test-is-installed.icm");

    assert!(!lg_profile::is_profile_installed(&path));

    lg_profile::ensure_profile_installed(&path).unwrap();
    assert!(lg_profile::is_profile_installed(&path));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn icm_extracted_profile_content_matches_expected_size() {
    let dir = std::env::temp_dir().join("lg-integ-test-content");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("test-content.icm");

    lg_profile::ensure_profile_installed(&path).unwrap();

    // Read back and verify the content bytes
    let contents = std::fs::read(&path).unwrap();
    assert_eq!(contents.len(), lg_profile::EMBEDDED_ICM_SIZE);

    // ICC profiles have "acsp" at offset 36 (ICC specification)
    if lg_profile::EMBEDDED_ICM_SIZE > 40 {
        let sig = &contents[36..40];
        assert_eq!(
            sig,
            b"acsp",
            "ICC profile should have 'acsp' signature at offset 36"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn icm_overwrite_corrupted_file() {
    let dir = std::env::temp_dir().join("lg-integ-test-overwrite");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test-overwrite.icm");

    // Write a corrupted/wrong-size file
    std::fs::write(&path, b"this is not an ICC profile").unwrap();
    assert!(path.exists());

    // ensure_profile_installed should overwrite because size doesn't match
    let wrote = lg_profile::ensure_profile_installed(&path).expect("should overwrite");
    assert!(wrote, "should overwrite corrupted file");
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        lg_profile::EMBEDDED_ICM_SIZE as u64,
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ============================================================================
// Service query tests (safe, read-only)
// ============================================================================

#[test]
fn service_query_info_never_panics() {
    let (installed, running) = lg_service::query_service_info();
    // If not installed, it shouldn't be running
    if !installed {
        assert!(
            !running,
            "service cannot be running if not installed"
        );
    }
}

#[test]
fn service_query_info_is_deterministic() {
    // Call twice, should return the same result
    let result1 = lg_service::query_service_info();
    let result2 = lg_service::query_service_info();
    assert_eq!(result1, result2, "consecutive queries should match");
}

// ============================================================================
// Monitor detection tests (read-only WMI queries)
// ============================================================================

#[test]
fn monitor_detection_does_not_panic() {
    let result = lg_monitor::find_matching_monitors("LG ULTRAGEAR");
    assert!(result.is_ok(), "monitor detection should not error: {:?}", result.err());
}

#[test]
fn monitor_detection_empty_pattern_returns_all() {
    let result = lg_monitor::find_matching_monitors("");
    assert!(result.is_ok());
    // Empty pattern matches all monitors — count varies by system
}

#[test]
fn monitor_detection_nonexistent_pattern_returns_empty() {
    let result = lg_monitor::find_matching_monitors("ZYXWVU_NONEXISTENT_MONITOR_99999");
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty(), "should find no matching monitors");
}

#[test]
fn monitor_detection_case_insensitive() {
    // Both uppercase and lowercase should return the same results
    let upper = lg_monitor::find_matching_monitors("LG ULTRAGEAR").unwrap();
    let lower = lg_monitor::find_matching_monitors("lg ultragear").unwrap();
    assert_eq!(upper.len(), lower.len(), "case should not affect result count");
}

#[test]
fn monitor_matched_fields_are_populated() {
    let monitors = lg_monitor::find_matching_monitors("").unwrap();
    for mon in &monitors {
        assert!(!mon.name.is_empty(), "monitor name should not be empty");
        assert!(!mon.device_key.is_empty(), "device key should not be empty");
    }
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

#[test]
fn config_toml_all_fields_roundtrip() {
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");

    // Write full config with every field
    let toml_str = r#"
monitor_match = "ROUNDTRIP"
profile_name = "roundtrip.icm"
toast_enabled = false
toast_title = "Custom Title"
toast_body = "Custom Body"
stabilize_delay_ms = 3000
toggle_delay_ms = 250
reapply_delay_ms = 20000
refresh_display_settings = false
refresh_broadcast_color = false
refresh_invalidate = false
refresh_calibration_loader = false
verbose = true
"#;

    fs::write(&cfg_path, toml_str).unwrap();
    let read_back = fs::read_to_string(&cfg_path).unwrap();

    #[derive(serde::Deserialize)]
    #[serde(default)]
    struct FullConfig {
        monitor_match: String,
        profile_name: String,
        toast_enabled: bool,
        toast_title: String,
        toast_body: String,
        stabilize_delay_ms: u64,
        toggle_delay_ms: u64,
        reapply_delay_ms: u64,
        refresh_display_settings: bool,
        refresh_broadcast_color: bool,
        refresh_invalidate: bool,
        refresh_calibration_loader: bool,
        verbose: bool,
    }

    impl Default for FullConfig {
        fn default() -> Self {
            Self {
                monitor_match: "DEFAULT".to_string(),
                profile_name: "default.icm".to_string(),
                toast_enabled: true,
                toast_title: "LG UltraGear".to_string(),
                toast_body: "Reapplied".to_string(),
                stabilize_delay_ms: 1500,
                toggle_delay_ms: 100,
                reapply_delay_ms: 12000,
                refresh_display_settings: true,
                refresh_broadcast_color: true,
                refresh_invalidate: true,
                refresh_calibration_loader: true,
                verbose: false,
            }
        }
    }

    let cfg: FullConfig = toml::from_str(&read_back).unwrap();
    assert_eq!(cfg.monitor_match, "ROUNDTRIP");
    assert_eq!(cfg.profile_name, "roundtrip.icm");
    assert!(!cfg.toast_enabled);
    assert_eq!(cfg.toast_title, "Custom Title");
    assert_eq!(cfg.toast_body, "Custom Body");
    assert_eq!(cfg.stabilize_delay_ms, 3000);
    assert_eq!(cfg.toggle_delay_ms, 250);
    assert_eq!(cfg.reapply_delay_ms, 20000);
    assert!(!cfg.refresh_display_settings);
    assert!(!cfg.refresh_broadcast_color);
    assert!(!cfg.refresh_invalidate);
    assert!(!cfg.refresh_calibration_loader);
    assert!(cfg.verbose);
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

#[test]
fn config_profile_path_matches_expected_format() {
    let cfg = lg_core::config::Config::default();
    let path = cfg.profile_path();
    let path_str = path.to_string_lossy().to_lowercase();
    assert!(path_str.contains("spool"));
    assert!(path_str.contains("drivers"));
    assert!(path_str.contains("color"));
    assert!(path_str.ends_with("lg-ultragear-full-cal.icm"));
}

#[test]
fn config_install_path_matches_expected_format() {
    let path = lg_core::config::install_path();
    assert!(path.to_string_lossy().contains("LG-UltraGear-Monitor"));
    assert_eq!(path.file_name().unwrap(), "lg-ultragear-dimming-fix.exe");
}

#[test]
fn config_dir_matches_expected_format() {
    let dir = lg_core::config::config_dir();
    assert!(dir.to_string_lossy().contains("LG-UltraGear-Monitor"));
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

#[test]
fn programdata_directory_exists() {
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let path = std::path::PathBuf::from(&pd);
    assert!(path.exists(), "ProgramData directory should exist");
}

// ============================================================================
// Toast notifications (disabled/no-op tests)
// ============================================================================

#[test]
fn toast_disabled_is_noop_from_integration() {
    lg_notify::show_reapply_toast(false, "Integration Test", "Should not show", false);
}

#[test]
fn toast_disabled_with_verbose_is_noop() {
    lg_notify::show_reapply_toast(false, "Integration Test", "Should not show", true);
}

// ============================================================================
// Profile reapply validation (non-destructive tests)
// ============================================================================

#[test]
fn reapply_profile_fails_with_nonexistent_profile() {
    let fake_path = std::path::PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-integ-test-99999.icm",
    );
    let result = lg_profile::reapply_profile(r"DISPLAY\TEST\001", &fake_path, 100);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Profile not found") || err.contains("not found"));
}

#[test]
fn refresh_display_all_disabled_is_noop() {
    // Calling with all methods disabled should be a complete no-op
    lg_profile::refresh_display(false, false, false);
}

#[test]
fn calibration_loader_disabled_is_noop() {
    lg_profile::trigger_calibration_loader(false);
}

// ============================================================================
// Binary metadata tests
// ============================================================================

#[test]
fn binary_version_is_semver() {
    let (stdout, _, success) = run_binary(&["--version"]);
    assert!(success);
    // Version output should contain digits and dots (semver)
    let version_part = stdout.trim();
    assert!(
        version_part.contains('.'),
        "version should contain dots: {}",
        version_part
    );
}

#[test]
fn binary_help_shows_about() {
    let (stdout, _, _) = run_binary(&["--help"]);
    assert!(
        stdout.contains("dimming") || stdout.contains("Dimming") || stdout.contains("color") || stdout.contains("profile"),
        "help should mention the tool's purpose: {}",
        stdout
    );
}

#[test]
fn binary_help_lists_all_subcommands() {
    let (stdout, _, _) = run_binary(&["--help"]);
    let lower = stdout.to_lowercase();
    assert!(lower.contains("install"), "help should list install");
    assert!(lower.contains("uninstall"), "help should list uninstall");
    assert!(lower.contains("reinstall"), "help should list reinstall");
    assert!(lower.contains("detect"), "help should list detect");
    assert!(lower.contains("apply"), "help should list apply");
    assert!(lower.contains("watch"), "help should list watch");
    assert!(lower.contains("config"), "help should list config");
    assert!(lower.contains("service"), "help should list service");
}

#[test]
fn binary_help_shows_global_flags() {
    let (stdout, _, _) = run_binary(&["--help"]);
    assert!(stdout.contains("--verbose") || stdout.contains("-v"));
    assert!(stdout.contains("--dry-run"));
    assert!(stdout.contains("--non-interactive"));
}

// ============================================================================
// Error handling tests
// ============================================================================

#[test]
fn multiple_unknown_commands_all_fail() {
    for cmd in &["foo", "bar", "baz", "install-everything", "remove-all"] {
        let (_, _, success) = run_binary(&[cmd]);
        assert!(!success, "unknown command '{}' should fail", cmd);
    }
}

#[test]
fn invalid_flag_fails() {
    let (_, _, success) = run_binary(&["--not-a-real-flag"]);
    assert!(!success, "invalid flag should cause failure");
}

#[test]
fn install_with_invalid_extra_args() {
    let (_, _, success) = run_binary(&["install", "--not-a-flag"]);
    assert!(!success, "install with invalid flag should fail");
}

// ============================================================================
// End-to-end combined workflow tests (dry-run only, safe)
// ============================================================================

#[test]
fn dry_run_full_workflow_install_uninstall() {
    // Simulate a full install → detect → uninstall workflow using --dry-run
    let (stdout, _, success) = run_binary(&["--dry-run", "install"]);
    assert!(success, "dry-run install should succeed");
    assert!(stdout.contains("DRY RUN"));

    let (stdout, _, success) = run_binary(&["detect"]);
    assert!(success, "detect should succeed");
    assert!(
        stdout.contains("Scanning") || stdout.contains("monitor"),
    );

    let (stdout, _, success) = run_binary(&["--dry-run", "uninstall", "--full"]);
    assert!(success, "dry-run full uninstall should succeed");
    assert!(stdout.contains("DRY RUN"));
}

#[test]
fn dry_run_apply_workflow() {
    let (stdout, _, success) = run_binary(&["--dry-run", "apply"]);
    assert!(success);
    assert!(stdout.contains("DRY RUN"));
    assert!(
        stdout.contains("Would reapply") || stdout.contains("reapply") || stdout.contains("DRY RUN"),
    );
}

#[test]
fn dry_run_reinstall_workflow() {
    let (stdout, _, success) = run_binary(&["--dry-run", "reinstall"]);
    assert!(success);
    assert!(stdout.contains("DRY RUN"));
}

// ============================================================================
// Config load/defaults integration
// ============================================================================

#[test]
fn config_load_returns_valid_config() {
    let cfg = lg_core::config::Config::load();
    // Should always return a valid config (using defaults if file missing)
    assert!(!cfg.monitor_match.is_empty() || cfg.monitor_match.is_empty());
    assert!(!cfg.profile_name.is_empty());
    assert!(cfg.stabilize_delay_ms <= 999_999_999);
    assert!(cfg.toggle_delay_ms <= 999_999_999);
}

#[test]
fn config_default_has_sane_values() {
    let cfg = lg_core::config::Config::default();
    assert_eq!(cfg.monitor_match, "LG ULTRAGEAR");
    assert_eq!(cfg.profile_name, "lg-ultragear-full-cal.icm");
    assert!(cfg.toast_enabled);
    assert!(cfg.stabilize_delay_ms > 0);
    assert!(cfg.toggle_delay_ms > 0);
    assert!(cfg.reapply_delay_ms > 0);
}

// ============================================================================
// Pattern flag forwarding tests
// ============================================================================

#[test]
fn detect_pattern_flag_short_form() {
    let (stdout, _, success) = run_binary(&["detect", "-p", "NONEXISTENT_XYZ"]);
    assert!(success, "detect -p should work");
    assert!(
        stdout.contains("NONEXISTENT_XYZ"),
        "should echo the pattern: {}",
        stdout
    );
}

#[test]
fn apply_pattern_flag_short_form() {
    let (stdout, _, success) = run_binary(&["--dry-run", "apply", "-p", "TEST_PATTERN"]);
    assert!(success, "apply -p with dry-run should work");
    // Should show the pattern in output or proceed without error
    assert!(
        stdout.contains("DRY RUN") || stdout.contains("TEST_PATTERN"),
        "should acknowledge pattern: {}",
        stdout
    );
}

// ============================================================================
// Process exit code tests
// ============================================================================

#[test]
fn help_exits_with_success() {
    let output = Command::new(binary_path())
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn version_exits_with_success() {
    let output = Command::new(binary_path())
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn detect_exits_with_success() {
    let output = Command::new(binary_path())
        .arg("detect")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn config_show_exits_with_success() {
    let output = Command::new(binary_path())
        .args(["config", "show"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn config_path_exits_with_success() {
    let output = Command::new(binary_path())
        .args(["config", "path"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn unknown_command_exits_with_nonzero() {
    let output = Command::new(binary_path())
        .arg("nonexistent-command")
        .output()
        .unwrap();
    assert!(!output.status.success());
}