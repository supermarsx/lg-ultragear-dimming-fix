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

#[test]
fn eventlog_reg_key_contains_service_name() {
    assert!(EVENTLOG_REG_KEY.contains("lg-ultragear-color-svc"));
}

#[test]
fn eventlog_reg_key_is_under_application_log() {
    assert!(EVENTLOG_REG_KEY.contains(r"EventLog\Application"));
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
