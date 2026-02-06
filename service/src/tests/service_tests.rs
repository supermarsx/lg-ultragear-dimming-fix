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

// ── Debounce epoch counter ───────────────────────────────────────

#[test]
fn debounce_epoch_starts_at_zero() {
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
    assert_eq!(DEBOUNCE_EPOCH.load(Ordering::SeqCst), 0);
}

#[test]
fn debounce_epoch_fetch_add_returns_previous() {
    DEBOUNCE_EPOCH.store(5, Ordering::SeqCst);
    let prev = DEBOUNCE_EPOCH.fetch_add(1, Ordering::SeqCst);
    assert_eq!(prev, 5);
    assert_eq!(DEBOUNCE_EPOCH.load(Ordering::SeqCst), 6);
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
}

#[test]
fn debounce_epoch_increments_sequentially() {
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
    for i in 1..=5 {
        let epoch = DEBOUNCE_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(epoch, i);
    }
    assert_eq!(DEBOUNCE_EPOCH.load(Ordering::SeqCst), 5);
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
}

#[test]
fn debounce_stale_epoch_detected() {
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
    let epoch_a = DEBOUNCE_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
    let _epoch_b = DEBOUNCE_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
    let current = DEBOUNCE_EPOCH.load(Ordering::SeqCst);
    assert_ne!(epoch_a, current, "Stale epoch should differ from current");
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
}

#[test]
fn debounce_latest_epoch_proceeds() {
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
    let epoch = DEBOUNCE_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
    let current = DEBOUNCE_EPOCH.load(Ordering::SeqCst);
    assert_eq!(epoch, current, "Latest epoch should match current");
    DEBOUNCE_EPOCH.store(0, Ordering::SeqCst);
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
    ];
    // Each flag should be a single bit, no overlaps
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
    // Should not cover session flags
    assert_eq!(EVENT_MASK_DEVICE & EVENT_SESSION_LOGON, 0);
    assert_eq!(EVENT_MASK_DEVICE & EVENT_SESSION_UNLOCK, 0);
    assert_eq!(EVENT_MASK_DEVICE & EVENT_CONSOLE_CONNECT, 0);
}

#[test]
fn event_mask_session_covers_session_flags() {
    assert_ne!(EVENT_MASK_SESSION & EVENT_SESSION_LOGON, 0);
    assert_ne!(EVENT_MASK_SESSION & EVENT_SESSION_UNLOCK, 0);
    assert_ne!(EVENT_MASK_SESSION & EVENT_CONSOLE_CONNECT, 0);
    // Should not cover device flags
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

// ── Debounce event accumulator ───────────────────────────────────

#[test]
fn debounce_events_starts_empty() {
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    assert_eq!(DEBOUNCE_EVENTS.load(Ordering::SeqCst), 0);
}

#[test]
fn debounce_events_accumulates_single_flag() {
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVICE_ARRIVAL, Ordering::SeqCst);
    let events = DEBOUNCE_EVENTS.load(Ordering::SeqCst);
    assert_ne!(events & EVENT_DEVICE_ARRIVAL, 0);
    assert_eq!(events & EVENT_SESSION_LOGON, 0);
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
}

#[test]
fn debounce_events_accumulates_multiple_flags() {
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVICE_ARRIVAL, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVNODES_CHANGED, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_SESSION_UNLOCK, Ordering::SeqCst);
    let events = DEBOUNCE_EVENTS.load(Ordering::SeqCst);
    assert_ne!(events & EVENT_MASK_DEVICE, 0, "Should have device events");
    assert_ne!(events & EVENT_MASK_SESSION, 0, "Should have session events");
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
}

#[test]
fn debounce_events_or_is_idempotent() {
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVICE_ARRIVAL, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVICE_ARRIVAL, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVICE_ARRIVAL, Ordering::SeqCst);
    let events = DEBOUNCE_EVENTS.load(Ordering::SeqCst);
    assert_eq!(
        events, EVENT_DEVICE_ARRIVAL,
        "ORing same flag should be idempotent"
    );
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
}

#[test]
fn debounce_events_swap_drains_all() {
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVICE_ARRIVAL | EVENT_SESSION_LOGON, Ordering::SeqCst);
    let drained = DEBOUNCE_EVENTS.swap(0, Ordering::SeqCst);
    assert_ne!(drained & EVENT_DEVICE_ARRIVAL, 0);
    assert_ne!(drained & EVENT_SESSION_LOGON, 0);
    assert_eq!(
        DEBOUNCE_EVENTS.load(Ordering::SeqCst),
        0,
        "Should be empty after drain"
    );
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
}

#[test]
fn debounce_events_device_only_no_session() {
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(
        EVENT_DEVICE_ARRIVAL | EVENT_DEVNODES_CHANGED,
        Ordering::SeqCst,
    );
    let events = DEBOUNCE_EVENTS.swap(0, Ordering::SeqCst);
    let has_device = events & EVENT_MASK_DEVICE != 0;
    let has_session = events & EVENT_MASK_SESSION != 0;
    assert!(has_device);
    assert!(
        !has_session,
        "Device-only burst should not have session flag"
    );
}

#[test]
fn debounce_events_session_only_no_device() {
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_SESSION_UNLOCK, Ordering::SeqCst);
    let events = DEBOUNCE_EVENTS.swap(0, Ordering::SeqCst);
    let has_device = events & EVENT_MASK_DEVICE != 0;
    let has_session = events & EVENT_MASK_SESSION != 0;
    assert!(
        !has_device,
        "Session-only event should not have device flag"
    );
    assert!(has_session);
}

#[test]
fn debounce_events_mixed_storm() {
    // Simulate: monitor plug + session unlock happening together
    DEBOUNCE_EVENTS.store(0, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVICE_ARRIVAL, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_DEVNODES_CHANGED, Ordering::SeqCst);
    DEBOUNCE_EVENTS.fetch_or(EVENT_SESSION_UNLOCK, Ordering::SeqCst);
    let events = DEBOUNCE_EVENTS.swap(0, Ordering::SeqCst);
    let has_device = events & EVENT_MASK_DEVICE != 0;
    let has_session = events & EVENT_MASK_SESSION != 0;
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
        dbcc_devicetype: 99, // Not DBT_DEVTYP_DEVICEINTERFACE
        dbcc_reserved: 0,
        dbcc_classguid: GUID_DEVINTERFACE_MONITOR,
        dbcc_name: [0],
    };
    let result = unsafe { is_monitor_device_event(LPARAM(&filter as *const _ as isize)) };
    assert!(!result, "Wrong device type should not match");
}
