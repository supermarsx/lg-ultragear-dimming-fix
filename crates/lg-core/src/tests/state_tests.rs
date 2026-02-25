use super::*;
use std::sync::{Mutex, OnceLock};

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct FileBackup {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

impl FileBackup {
    fn capture(path: PathBuf) -> Self {
        let original = fs::read(&path).ok();
        Self { path, original }
    }
}

impl Drop for FileBackup {
    fn drop(&mut self) {
        if let Some(bytes) = &self.original {
            let _ = ensure_parent(&self.path);
            let _ = fs::write(&self.path, bytes);
        } else if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[test]
fn risky_vcp_codes_parses_hex_and_decimal() {
    let codes = risky_vcp_codes_from_csv("04,0x06,10,0A,0XDC");
    // Parser prefers hexadecimal when the token is valid hex.
    assert_eq!(codes, vec![0x04, 0x06, 0x10, 0x0A, 0xDC]);
}

#[test]
fn risky_vcp_codes_ignores_invalid_values() {
    let codes = risky_vcp_codes_from_csv(",,bad,0xGG, 14 , 0x10");
    assert_eq!(codes, vec![0x14, 0x10]);
}

#[test]
fn automation_config_sanitized_clamps_expected_ranges() {
    let cfg = AutomationConfig {
        ambient: AmbientAutomationConfig {
            sensor_poll_interval_ms: 10,
            sensor_timeout_ms: 1,
            sensor_smoothing_alpha: 99.0,
            lux_hysteresis: -4.0,
            ..AmbientAutomationConfig::default()
        },
        app_rules: AppRulesConfig {
            poll_interval_ms: 1,
            ..AppRulesConfig::default()
        },
        tray: TrayQuickConfig {
            brightness_step: 999,
            ..TrayQuickConfig::default()
        },
        ddc_safety: DdcSafetyConfig {
            rollback_timeout_ms: 12,
            ..DdcSafetyConfig::default()
        },
        metrics: MetricsConfig {
            rolling_window: 0,
            ..MetricsConfig::default()
        },
        ..AutomationConfig::default()
    }
    .sanitized();

    assert_eq!(cfg.ambient.sensor_poll_interval_ms, 500);
    assert_eq!(cfg.ambient.sensor_timeout_ms, 100);
    assert_eq!(cfg.ambient.sensor_smoothing_alpha, 1.0);
    assert_eq!(cfg.ambient.lux_hysteresis, 0.0);
    assert_eq!(cfg.app_rules.poll_interval_ms, 500);
    assert_eq!(cfg.tray.brightness_step, 100);
    assert_eq!(cfg.ddc_safety.rollback_timeout_ms, 1000);
    assert_eq!(cfg.metrics.rolling_window, 1);
}

#[test]
fn load_automation_config_invalid_toml_falls_back_to_default() {
    let _guard = test_lock().lock().expect("lock");
    let path = automation_config_path();
    let _backup = FileBackup::capture(path.clone());
    let _ = ensure_parent(&path);
    fs::write(&path, "[").expect("write invalid automation toml");

    let loaded = load_automation_config();
    let default_cfg = AutomationConfig::default();
    assert_eq!(
        loaded.ambient.sensor_method,
        default_cfg.ambient.sensor_method
    );
    assert_eq!(
        loaded.metrics.rolling_window,
        default_cfg.metrics.rolling_window
    );
}

#[test]
fn save_and_load_automation_config_roundtrip_core_fields() {
    let _guard = test_lock().lock().expect("lock");
    let path = automation_config_path();
    let _backup = FileBackup::capture(path);
    let cfg = AutomationConfig {
        ambient: AmbientAutomationConfig {
            enabled: true,
            sensor_method: "simulated".to_string(),
            sensor_command: "250".to_string(),
            day_preset: "gamma24".to_string(),
            ..AmbientAutomationConfig::default()
        },
        app_rules: AppRulesConfig {
            enabled: true,
            match_mode: "regex".to_string(),
            case_sensitive: true,
            rules: vec![AppProfileRule {
                name: "Game".to_string(),
                process_pattern: "^game.*".to_string(),
                preset: "reader".to_string(),
                ..AppProfileRule::default()
            }],
            ..AppRulesConfig::default()
        },
        tray: TrayQuickConfig {
            enabled: true,
            brightness_step: 7,
            ..TrayQuickConfig::default()
        },
        ..AutomationConfig::default()
    };
    save_automation_config(&cfg).expect("save automation config");
    let loaded = load_automation_config();
    assert!(loaded.ambient.enabled);
    assert_eq!(loaded.ambient.sensor_method, "simulated");
    assert!(loaded.app_rules.enabled);
    assert_eq!(loaded.app_rules.match_mode, "regex");
    assert_eq!(loaded.app_rules.rules.len(), 1);
    assert!(loaded.tray.enabled);
    assert_eq!(loaded.tray.brightness_step, 7);
}

#[test]
fn compute_apply_latency_metrics_empty_is_default() {
    let _guard = test_lock().lock().expect("lock");
    let path = diagnostics_log_path();
    let _backup = FileBackup::capture(path.clone());
    if path.exists() {
        fs::remove_file(&path).expect("remove diagnostics log");
    }
    let metrics = compute_apply_latency_metrics(20);
    assert_eq!(metrics.samples, 0);
    assert_eq!(metrics.success_count, 0);
    assert_eq!(metrics.failure_count, 0);
    assert_eq!(metrics.avg_ms, 0.0);
    assert_eq!(metrics.p95_ms, 0);
    assert_eq!(metrics.last_ms, 0);
}

#[test]
fn compute_apply_latency_metrics_parses_window_and_percentile() {
    let _guard = test_lock().lock().expect("lock");
    let path = diagnostics_log_path();
    let _backup = FileBackup::capture(path.clone());
    ensure_parent(&path).expect("ensure diagnostics parent");

    let lines = [
        "2026-01-01T00:00:00Z\tservice\tINFO\tapply_latency\tms=120 success=1 trigger=event",
        "2026-01-01T00:00:01Z\tservice\tINFO\tapply_latency\tms=80 success=0 trigger=event",
        "2026-01-01T00:00:02Z\tservice\tINFO\tapply_latency\tms=220 success=1 trigger=event",
        "2026-01-01T00:00:03Z\tservice\tINFO\tapply_success\tapplied profiles to 1 monitor(s)",
    ]
    .join("\n");
    fs::write(&path, format!("{}\n", lines)).expect("write diagnostics");

    let metrics = compute_apply_latency_metrics(3);
    assert_eq!(metrics.samples, 3);
    assert_eq!(
        metrics.success_count + metrics.failure_count,
        metrics.samples
    );
    assert_eq!(metrics.last_ms, 220);
    assert!(metrics.avg_ms > 100.0);
    assert!(metrics.p95_ms >= 120);
}
