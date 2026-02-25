use super::*;

struct FileBackup {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

impl FileBackup {
    fn capture(path: PathBuf) -> Self {
        let original = std::fs::read(&path).ok();
        Self { path, original }
    }
}

impl Drop for FileBackup {
    fn drop(&mut self) {
        if let Some(bytes) = &self.original {
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&self.path, bytes);
        } else if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[test]
fn parse_hex_u8_supports_prefixed_and_plain_values() {
    assert_eq!(parse_hex_u8("0x10").unwrap(), 0x10);
    assert_eq!(parse_hex_u8("dc").unwrap(), 0xDC);
    assert!(parse_hex_u8("xyz").is_err());
}

#[test]
fn parse_hex_bytes_handles_common_separators() {
    assert_eq!(
        parse_hex_bytes("AA BB-CC:DD").unwrap(),
        vec![0xAA, 0xBB, 0xCC, 0xDD]
    );
    assert!(parse_hex_bytes("A").is_err());
}

#[test]
fn color_preset_name_maps_known_values() {
    assert_eq!(color_preset_name(1), "sRGB");
    assert_eq!(color_preset_name(11), "User 1");
    assert_eq!(color_preset_name(999), "Unknown");
}

#[test]
fn is_risky_vcp_write_uses_configured_csv() {
    let cfg = app_state::AutomationConfig {
        ddc_safety: app_state::DdcSafetyConfig {
            risky_vcp_codes: "04,10,DC".to_string(),
            ..app_state::DdcSafetyConfig::default()
        },
        ..app_state::AutomationConfig::default()
    };
    assert!(is_risky_vcp_write(0x10, &cfg));
    assert!(!is_risky_vcp_write(0x12, &cfg));
}

#[test]
fn emit_apply_latency_cli_writes_event_when_metrics_enabled() {
    let auto_path = app_state::automation_config_path();
    let diag_path = app_state::diagnostics_log_path();
    let _auto_backup = FileBackup::capture(auto_path);
    let _diag_backup = FileBackup::capture(diag_path.clone());
    let cfg = app_state::AutomationConfig {
        metrics: app_state::MetricsConfig {
            enabled: true,
            collect_latency: true,
            collect_success_rate: true,
            rolling_window: 20,
        },
        ..app_state::AutomationConfig::default()
    };
    app_state::save_automation_config(&cfg).expect("save automation");
    emit_apply_latency_cli(Instant::now(), true, "test_case=emit_cli");
    let events = app_state::read_recent_diagnostic_events(16).expect("read diagnostics");
    assert!(
        events.iter().any(|e| {
            e.source == "cli"
                && e.event == "apply_latency"
                && e.details.contains("test_case=emit_cli")
        }),
        "expected cli apply_latency event in diagnostics"
    );
}

#[test]
fn tuning_for_reader_uses_selected_tuning_preset() {
    let cfg = Config {
        icc_tuning_preset: "anti_dim_night".to_string(),
        icc_tuning_overlay_manual: false,
        ..Config::default()
    };

    let resolved = tuning_for_active_preset(&cfg, "reader");
    let expected =
        lg_profile::dynamic_icc_tuning_for_preset(lg_profile::DynamicIccTuningPreset::AntiDimNight);

    assert!(
        (resolved.black_lift - expected.black_lift).abs() < 1e-9,
        "reader preset should keep selected tuning preset values"
    );
    assert!(
        (resolved.midtone_boost - expected.midtone_boost).abs() < 1e-9,
        "reader preset should keep selected tuning preset values"
    );
    assert!(
        (resolved.white_compression - expected.white_compression).abs() < 1e-9,
        "reader preset should keep selected tuning preset values"
    );
}

#[test]
fn tuning_for_reader_keeps_manual_overlay_when_enabled() {
    let cfg = Config {
        icc_tuning_preset: "anti_dim_night".to_string(),
        icc_tuning_overlay_manual: true,
        icc_white_compression: 0.11,
        ..Config::default()
    };

    let resolved = tuning_for_active_preset(&cfg, "reader");
    assert!(
        (resolved.white_compression - 0.11).abs() < 1e-9,
        "manual overlay should still apply on reader preset"
    );
}
