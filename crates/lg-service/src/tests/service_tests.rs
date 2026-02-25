use super::*;
use chrono::Timelike;

// ── Constants ────────────────────────────────────────────────────

#[test]
fn service_name_is_expected() {
    assert_eq!(SERVICE_NAME, "lg-ultragear-color-svc");
}

#[test]
#[allow(clippy::const_is_empty)]
fn service_display_name_not_empty() {
    assert!(!SERVICE_DISPLAY_NAME.is_empty());
}

#[test]
#[allow(clippy::const_is_empty)]
fn service_description_not_empty() {
    assert!(!SERVICE_DESCRIPTION.is_empty());
}

#[test]
fn service_failure_reset_period_is_24h() {
    assert_eq!(SERVICE_FAILURE_RESET_SECS, 24 * 60 * 60);
}

#[test]
fn service_failure_restart_delays_are_expected() {
    assert_eq!(SERVICE_FAILURE_RESTART_DELAYS_SECS, [5, 15, 30]);
}

#[test]
fn default_service_failure_actions_restart_policy() {
    let policy = default_service_failure_actions();

    match policy.reset_period {
        ServiceFailureResetPeriod::After(d) => {
            assert_eq!(d, Duration::from_secs(SERVICE_FAILURE_RESET_SECS))
        }
        ServiceFailureResetPeriod::Never => panic!("reset period should not be Never"),
    }
    assert!(policy.reboot_msg.is_none());
    assert!(policy.command.is_none());

    let actions = policy
        .actions
        .expect("restart actions should be configured");
    assert_eq!(actions.len(), SERVICE_FAILURE_RESTART_DELAYS_SECS.len());

    for (idx, action) in actions.iter().enumerate() {
        assert_eq!(action.action_type, ServiceActionType::Restart);
        assert_eq!(
            action.delay,
            Duration::from_secs(SERVICE_FAILURE_RESTART_DELAYS_SECS[idx])
        );
    }
}

#[test]
fn config_reg_key_contains_service_name() {
    assert!(CONFIG_REG_KEY.contains("lg-ultragear-color-svc"));
}

#[test]
fn config_reg_value_is_monitor_match() {
    assert_eq!(CONFIG_REG_VALUE, "MonitorMatch");
}

#[test]
fn eventlog_reg_key_contains_service_name() {
    assert!(EVENTLOG_REG_KEY.contains("lg-ultragear-color-svc"));
}

#[test]
fn eventlog_reg_key_is_under_application_log() {
    assert!(EVENTLOG_REG_KEY.contains(r"EventLog\Application"));
}

// ── Preset selection ────────────────────────────────────────────

#[test]
fn effective_preset_for_mode_prefers_sdr_when_hdr_false() {
    let cfg = Config {
        icc_active_preset: "custom".to_string(),
        icc_sdr_preset: "gamma22".to_string(),
        icc_hdr_preset: "gamma24".to_string(),
        ..Config::default()
    };
    let preset = effective_preset_for_mode(&cfg, false);
    assert_eq!(preset, "gamma22");
}

#[test]
fn effective_preset_for_mode_prefers_hdr_when_hdr_true() {
    let cfg = Config {
        icc_active_preset: "custom".to_string(),
        icc_sdr_preset: "gamma22".to_string(),
        icc_hdr_preset: "gamma24".to_string(),
        ..Config::default()
    };
    let preset = effective_preset_for_mode(&cfg, true);
    assert_eq!(preset, "gamma24");
}

// ── Window message constants ─────────────────────────────────────

#[test]
fn wm_quit_service_is_above_wm_user() {
    let quit = WM_QUIT_SERVICE;
    let user = WM_USER;
    assert!(quit > user);
    assert_eq!(quit, user + 1);
}

#[test]
fn wm_devicechange_value() {
    assert_eq!(WM_DEVICECHANGE, 0x0219);
}

#[test]
fn dbt_device_arrival_value() {
    assert_eq!(DBT_DEVICEARRIVAL, 0x8000);
}

#[test]
fn dbt_devnodes_changed_value() {
    assert_eq!(DBT_DEVNODES_CHANGED, 0x0007);
}

#[test]
fn wm_wtssession_change_value() {
    assert_eq!(WM_WTSSESSION_CHANGE, 0x02B1);
}

#[test]
fn wts_console_connect_value() {
    assert_eq!(WTS_CONSOLE_CONNECT, 0x1);
}

#[test]
fn wts_session_logon_value() {
    assert_eq!(WTS_SESSION_LOGON, 0x5);
}

#[test]
fn wts_session_unlock_value() {
    assert_eq!(WTS_SESSION_UNLOCK, 0x8);
}

// ── GUID ─────────────────────────────────────────────────────────

#[test]
fn guid_devinterface_monitor_data1() {
    assert_eq!(GUID_DEVINTERFACE_MONITOR.data1, 0xE6F07B5F);
}

#[test]
fn guid_devinterface_monitor_data2() {
    assert_eq!(GUID_DEVINTERFACE_MONITOR.data2, 0xEE97);
}

#[test]
fn guid_devinterface_monitor_data3() {
    assert_eq!(GUID_DEVINTERFACE_MONITOR.data3, 0x4A90);
}

#[test]
fn guid_devinterface_monitor_data4() {
    assert_eq!(
        GUID_DEVINTERFACE_MONITOR.data4,
        [0xB0, 0x76, 0x33, 0xF5, 0x7B, 0xF4, 0xEA, 0xA7]
    );
}

// ── DevBroadcastDeviceInterface ──────────────────────────────────

#[test]
fn dev_broadcast_struct_size() {
    let size = std::mem::size_of::<DevBroadcastDeviceInterface>();
    assert!(size >= 28, "DevBroadcastDeviceInterface size: {}", size);
}

#[test]
fn dbt_devtyp_deviceinterface_value() {
    assert_eq!(DBT_DEVTYP_DEVICEINTERFACE, 5);
}

#[test]
fn device_notify_window_handle_value() {
    assert_eq!(DEVICE_NOTIFY_WINDOW_HANDLE, 0);
}

// ── to_wide helper ───────────────────────────────────────────────

#[test]
fn to_wide_service_class_name() {
    let result = to_wide("LGUltraGearColorSvcWnd");
    assert!(!result.is_empty());
    assert_eq!(*result.last().unwrap(), 0u16);
}

#[test]
fn to_wide_empty() {
    let result = to_wide("");
    assert_eq!(result, vec![0u16]);
}

#[test]
fn to_wide_backslash_path() {
    let result = to_wide(r"DISPLAY\LGS\001");
    assert_eq!(*result.last().unwrap(), 0u16);
    assert!(result.len() > 1);
}

// ── ServiceType constants ────────────────────────────────────────

#[test]
fn service_type_own_process() {
    let st = ServiceType::OWN_PROCESS;
    assert_eq!(st, ServiceType::OWN_PROCESS);
}

// ── query_service_info ───────────────────────────────────────────

#[test]
fn query_service_info_returns_tuple() {
    // Just verify it doesn't panic — actual result depends on system state
    let (installed, running) = query_service_info();
    // If not installed, it can't be running
    if !installed {
        assert!(!running, "service cannot be running if not installed");
    }
}

// ── AtomicBool running flag ──────────────────────────────────────

#[test]
fn running_flag_default_true() {
    let running = Arc::new(AtomicBool::new(true));
    assert!(running.load(Ordering::SeqCst));
}

#[test]
fn running_flag_can_be_set_false() {
    let running = Arc::new(AtomicBool::new(true));
    let clone = running.clone();
    clone.store(false, Ordering::SeqCst);
    assert!(!running.load(Ordering::SeqCst));
}

// ── Event sender thread-local ────────────────────────────────────

#[test]
fn event_sender_defaults_to_none() {
    EVENT_SENDER.with(|s| {
        assert!(s.borrow().is_none());
    });
}

#[test]
fn event_sender_can_be_set_and_cleared() {
    let (tx, _rx) = mpsc::channel::<u8>();
    EVENT_SENDER.with(|s| *s.borrow_mut() = Some(tx));
    EVENT_SENDER.with(|s| assert!(s.borrow().is_some()));
    EVENT_SENDER.with(|s| *s.borrow_mut() = None);
    EVENT_SENDER.with(|s| assert!(s.borrow().is_none()));
}

// ── Channel-based debounce ───────────────────────────────────────

#[test]
fn channel_event_send_receive() {
    let (tx, rx) = mpsc::channel::<u8>();
    tx.send(EVENT_DEVICE_ARRIVAL).unwrap();
    let received = rx.recv().unwrap();
    assert_eq!(received, EVENT_DEVICE_ARRIVAL);
}

#[test]
fn channel_coalesces_multiple_events() {
    let (tx, rx) = mpsc::channel::<u8>();
    tx.send(EVENT_DEVICE_ARRIVAL).unwrap();
    tx.send(EVENT_DEVNODES_CHANGED).unwrap();
    tx.send(EVENT_SESSION_UNLOCK).unwrap();
    let mut accumulated: u8 = 0;
    while let Ok(f) = rx.try_recv() {
        accumulated |= f;
    }
    assert_ne!(accumulated & EVENT_MASK_DEVICE, 0);
    assert_ne!(accumulated & EVENT_MASK_SESSION, 0);
}

#[test]
fn channel_recv_timeout_returns_on_timeout() {
    let (_tx, rx) = mpsc::channel::<u8>();
    let start = Instant::now();
    let result = rx.recv_timeout(Duration::from_millis(50));
    assert!(result.is_err());
    assert!(start.elapsed() >= Duration::from_millis(40));
}

#[test]
fn channel_disconnects_on_sender_drop() {
    let (tx, rx) = mpsc::channel::<u8>();
    drop(tx);
    assert!(rx.recv().is_err());
}

#[test]
fn channel_try_recv_drains_queue() {
    let (tx, rx) = mpsc::channel::<u8>();
    tx.send(EVENT_DEVICE_ARRIVAL).unwrap();
    tx.send(EVENT_SESSION_LOGON).unwrap();
    drop(tx);
    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn channel_recv_timeout_interruptible_on_disconnect() {
    let (tx, rx) = mpsc::channel::<u8>();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        drop(tx);
    });
    let result = rx.recv_timeout(Duration::from_secs(5));
    assert!(matches!(result, Err(mpsc::RecvTimeoutError::Disconnected)));
    handle.join().unwrap();
}

// ── Event flag constants ─────────────────────────────────────────

#[test]
fn event_flags_are_distinct_bits() {
    let all = [
        EVENT_DEVICE_ARRIVAL,
        EVENT_DEVNODES_CHANGED,
        EVENT_SESSION_LOGON,
        EVENT_SESSION_UNLOCK,
        EVENT_CONSOLE_CONNECT,
        EVENT_AUTOMATION_POLL,
    ];
    for (i, &a) in all.iter().enumerate() {
        assert!(a.count_ones() == 1, "Flag 0b{:08b} is not a single bit", a);
        for &b in &all[i + 1..] {
            assert_eq!(a & b, 0, "Flags 0b{:08b} and 0b{:08b} overlap", a, b);
        }
    }
}

#[test]
fn event_mask_device_covers_device_flags() {
    assert_ne!(EVENT_MASK_DEVICE & EVENT_DEVICE_ARRIVAL, 0);
    assert_ne!(EVENT_MASK_DEVICE & EVENT_DEVNODES_CHANGED, 0);
    assert_eq!(EVENT_MASK_DEVICE & EVENT_SESSION_LOGON, 0);
    assert_eq!(EVENT_MASK_DEVICE & EVENT_SESSION_UNLOCK, 0);
    assert_eq!(EVENT_MASK_DEVICE & EVENT_CONSOLE_CONNECT, 0);
}

#[test]
fn event_mask_session_covers_session_flags() {
    assert_ne!(EVENT_MASK_SESSION & EVENT_SESSION_LOGON, 0);
    assert_ne!(EVENT_MASK_SESSION & EVENT_SESSION_UNLOCK, 0);
    assert_ne!(EVENT_MASK_SESSION & EVENT_CONSOLE_CONNECT, 0);
    assert_eq!(EVENT_MASK_SESSION & EVENT_DEVICE_ARRIVAL, 0);
    assert_eq!(EVENT_MASK_SESSION & EVENT_DEVNODES_CHANGED, 0);
}

#[test]
fn event_masks_are_disjoint() {
    assert_eq!(
        EVENT_MASK_DEVICE & EVENT_MASK_SESSION,
        0,
        "Device and session masks must not overlap"
    );
}

// ── Event flag accumulation ──────────────────────────────────────

#[test]
fn event_accumulation_single_flag() {
    let mut accumulated: u8 = 0;
    accumulated |= EVENT_DEVICE_ARRIVAL;
    assert_ne!(accumulated & EVENT_DEVICE_ARRIVAL, 0);
    assert_eq!(accumulated & EVENT_SESSION_LOGON, 0);
}

#[test]
fn event_accumulation_multiple_flags() {
    let mut accumulated: u8 = 0;
    accumulated |= EVENT_DEVICE_ARRIVAL;
    accumulated |= EVENT_DEVNODES_CHANGED;
    accumulated |= EVENT_SESSION_UNLOCK;
    assert_ne!(accumulated & EVENT_MASK_DEVICE, 0);
    assert_ne!(accumulated & EVENT_MASK_SESSION, 0);
}

#[test]
fn event_accumulation_or_is_idempotent() {
    let mut accumulated: u8 = 0;
    accumulated |= EVENT_DEVICE_ARRIVAL;
    accumulated |= EVENT_DEVICE_ARRIVAL;
    accumulated |= EVENT_DEVICE_ARRIVAL;
    assert_eq!(accumulated, EVENT_DEVICE_ARRIVAL);
}

#[test]
fn event_accumulation_device_only() {
    let mut accumulated: u8 = 0;
    accumulated |= EVENT_DEVICE_ARRIVAL | EVENT_DEVNODES_CHANGED;
    let has_device = accumulated & EVENT_MASK_DEVICE != 0;
    let has_session = accumulated & EVENT_MASK_SESSION != 0;
    assert!(has_device);
    assert!(
        !has_session,
        "Device-only burst should not have session flag"
    );
}

#[test]
fn event_accumulation_session_only() {
    let mut accumulated: u8 = 0;
    accumulated |= EVENT_SESSION_UNLOCK;
    let has_device = accumulated & EVENT_MASK_DEVICE != 0;
    let has_session = accumulated & EVENT_MASK_SESSION != 0;
    assert!(
        !has_device,
        "Session-only event should not have device flag"
    );
    assert!(has_session);
}

#[test]
fn event_accumulation_mixed_storm() {
    let mut accumulated: u8 = 0;
    accumulated |= EVENT_DEVICE_ARRIVAL;
    accumulated |= EVENT_DEVNODES_CHANGED;
    accumulated |= EVENT_SESSION_UNLOCK;
    let has_device = accumulated & EVENT_MASK_DEVICE != 0;
    let has_session = accumulated & EVENT_MASK_SESSION != 0;
    assert!(
        has_device && has_session,
        "Mixed storm should have both flags"
    );
}

// ── Device event filtering ───────────────────────────────────────

#[test]
fn is_monitor_device_event_null_lparam_is_false() {
    let result = unsafe { is_monitor_device_event(LPARAM(0)) };
    assert!(
        !result,
        "Null LPARAM should not be treated as monitor event"
    );
}

#[test]
fn is_monitor_device_event_monitor_guid_is_true() {
    let filter = DevBroadcastDeviceInterface {
        dbcc_size: std::mem::size_of::<DevBroadcastDeviceInterface>() as u32,
        dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE,
        dbcc_reserved: 0,
        dbcc_classguid: GUID_DEVINTERFACE_MONITOR,
        dbcc_name: [0],
    };
    let result = unsafe { is_monitor_device_event(LPARAM(&filter as *const _ as isize)) };
    assert!(result, "Monitor GUID should match");
}

#[test]
fn is_monitor_device_event_wrong_guid_is_false() {
    let filter = DevBroadcastDeviceInterface {
        dbcc_size: std::mem::size_of::<DevBroadcastDeviceInterface>() as u32,
        dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE,
        dbcc_reserved: 0,
        dbcc_classguid: windows::core::GUID::from_values(0x12345678, 0, 0, [0; 8]),
        dbcc_name: [0],
    };
    let result = unsafe { is_monitor_device_event(LPARAM(&filter as *const _ as isize)) };
    assert!(!result, "Non-monitor GUID should not match");
}

#[test]
fn is_monitor_device_event_wrong_device_type_is_false() {
    let filter = DevBroadcastDeviceInterface {
        dbcc_size: std::mem::size_of::<DevBroadcastDeviceInterface>() as u32,
        dbcc_devicetype: 99,
        dbcc_reserved: 0,
        dbcc_classguid: GUID_DEVINTERFACE_MONITOR,
        dbcc_name: [0],
    };
    let result = unsafe { is_monitor_device_event(LPARAM(&filter as *const _ as isize)) };
    assert!(!result, "Wrong device type should not match");
}

// ── Automation helper logic ──────────────────────────────────────

struct FileBackup {
    path: std::path::PathBuf,
    original: Option<Vec<u8>>,
}

impl FileBackup {
    fn capture(path: std::path::PathBuf) -> Self {
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
fn parse_first_number_extracts_first_numeric_token() {
    assert_eq!(parse_first_number("lux=123.5 cd"), Some(123.5));
    assert_eq!(parse_first_number("hello 42 world"), Some(42.0));
    assert_eq!(parse_first_number("no numbers"), None);
}

#[test]
fn parse_hhmm_uses_fallback_on_invalid_input() {
    let t = parse_hhmm("invalid", "08:30");
    assert_eq!(t.hour(), 8);
    assert_eq!(t.minute(), 30);
}

#[test]
fn time_in_range_supports_wraparound_ranges() {
    let start = chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap();
    let end = chrono::NaiveTime::from_hms_opt(6, 0, 0).unwrap();
    let at_night = chrono::NaiveTime::from_hms_opt(23, 30, 0).unwrap();
    let before_end = chrono::NaiveTime::from_hms_opt(5, 30, 0).unwrap();
    let daytime = chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    assert!(time_in_range(at_night, start, end));
    assert!(time_in_range(before_end, start, end));
    assert!(!time_in_range(daytime, start, end));
}

#[test]
fn ambient_band_from_lux_respects_thresholds() {
    let cfg = app_state::AmbientAutomationConfig {
        lux_day_threshold: 220.0,
        lux_night_threshold: 80.0,
        lux_hysteresis: 10.0,
        ..app_state::AmbientAutomationConfig::default()
    };
    assert_eq!(ambient_band_from_lux(300.0, &cfg, None), "day");
    assert_eq!(ambient_band_from_lux(20.0, &cfg, None), "night");
    assert_eq!(ambient_band_from_lux(130.0, &cfg, None), "evening");
    assert_eq!(ambient_band_from_lux(212.0, &cfg, Some("day")), "day");
    assert_eq!(ambient_band_from_lux(88.0, &cfg, Some("night")), "night");
}

#[test]
fn process_matches_rule_supports_contains_exact_and_regex() {
    assert!(process_matches_rule("game.exe", "game", "contains", false));
    assert!(!process_matches_rule("game.exe", "GAME", "contains", true));
    assert!(process_matches_rule("game.exe", "game.exe", "exact", true));
    assert!(process_matches_rule(
        "eldenring.exe",
        "^elden.*\\.exe$",
        "regex",
        false
    ));
    assert!(!process_matches_rule("tool.exe", "^game.*", "regex", false));
}

#[test]
fn read_ambient_sensor_raw_lux_env_method_reads_variable() {
    std::env::set_var("LG_TEST_AMBIENT_LUX", "345.6");
    let cfg = app_state::AmbientAutomationConfig {
        enabled: true,
        sensor_enabled: true,
        sensor_method: "env".to_string(),
        sensor_command: "LG_TEST_AMBIENT_LUX".to_string(),
        ..app_state::AmbientAutomationConfig::default()
    };
    let value = read_ambient_sensor_raw_lux(&cfg);
    std::env::remove_var("LG_TEST_AMBIENT_LUX");
    assert_eq!(value, Some(345.6));
}

#[test]
fn evaluate_ambient_decision_simulated_day_and_night() {
    let day_cfg = app_state::AmbientAutomationConfig {
        enabled: true,
        sensor_enabled: true,
        sensor_method: "simulated".to_string(),
        sensor_command: "999".to_string(),
        sensor_smoothing_alpha: 1.0,
        lux_day_threshold: 220.0,
        lux_night_threshold: 80.0,
        day_preset: "gamma24".to_string(),
        ..app_state::AmbientAutomationConfig::default()
    };
    let day = evaluate_ambient_decision(&day_cfg).expect("ambient day decision");
    assert_eq!(day.preset.as_deref(), Some("gamma24"));

    let night_cfg = app_state::AmbientAutomationConfig {
        enabled: true,
        sensor_enabled: true,
        sensor_method: "simulated".to_string(),
        sensor_command: "0".to_string(),
        sensor_smoothing_alpha: 1.0,
        lux_day_threshold: 220.0,
        lux_night_threshold: 80.0,
        night_preset: "reader".to_string(),
        ..app_state::AmbientAutomationConfig::default()
    };
    let night = evaluate_ambient_decision(&night_cfg).expect("ambient night decision");
    assert_eq!(night.preset.as_deref(), Some("reader"));
}

#[test]
fn compute_automation_fingerprint_contains_key_dimensions() {
    let fp = compute_automation_fingerprint("reader", "reader_balanced", 150.0, Some(55));
    assert!(fp.contains("preset=reader"));
    assert!(fp.contains("tuning=reader_balanced"));
    assert!(fp.contains("luminance=150.00"));
    assert!(fp.contains("ddc=55"));
}

#[test]
fn should_skip_automation_apply_detects_repeated_poll_value() {
    let unique = format!(
        "fp-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    assert!(!should_skip_automation_apply(&unique, "automation_poll"));
    assert!(should_skip_automation_apply(&unique, "automation_poll"));
    assert!(!should_skip_automation_apply(
        &(unique.clone() + "-2"),
        "automation_poll"
    ));
}

#[test]
fn automation_poll_interval_uses_min_enabled_interval() {
    let path = app_state::automation_config_path();
    let _backup = FileBackup::capture(path);
    let cfg = app_state::AutomationConfig {
        ambient: app_state::AmbientAutomationConfig {
            enabled: true,
            sensor_poll_interval_ms: 4500,
            ..app_state::AmbientAutomationConfig::default()
        },
        app_rules: app_state::AppRulesConfig {
            enabled: true,
            poll_interval_ms: 2100,
            ..app_state::AppRulesConfig::default()
        },
        ..app_state::AutomationConfig::default()
    };
    app_state::save_automation_config(&cfg).expect("save automation config");
    assert_eq!(automation_poll_interval_ms(), Some(2100));
}
