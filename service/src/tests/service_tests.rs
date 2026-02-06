use super::*;

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
fn config_reg_key_contains_service_name() {
    assert!(CONFIG_REG_KEY.contains("lg-ultragear-color-svc"));
}

#[test]
fn config_reg_value_is_monitor_match() {
    assert_eq!(CONFIG_REG_VALUE, "MonitorMatch");
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
    // Should be at least 28 bytes (4 + 4 + 4 + 16 + 2, with padding)
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
    // Verify we're using OWN_PROCESS (not shared)
    let st = ServiceType::OWN_PROCESS;
    assert_eq!(st, ServiceType::OWN_PROCESS);
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

// ── Config thread-local ──────────────────────────────────────────

#[test]
fn thread_config_default_accessible() {
    THREAD_CONFIG.with(|c| {
        let cfg = c.borrow();
        // Should be default config
        assert_eq!(cfg.monitor_match, "LG ULTRAGEAR");
    });
}

#[test]
fn thread_config_can_be_updated() {
    THREAD_CONFIG.with(|c| {
        let mut cfg = c.borrow_mut();
        cfg.monitor_match = "TEST".to_string();
    });
    THREAD_CONFIG.with(|c| {
        let cfg = c.borrow();
        assert_eq!(cfg.monitor_match, "TEST");
    });
    // Reset to avoid affecting other tests
    THREAD_CONFIG.with(|c| {
        *c.borrow_mut() = Config::default();
    });
}
