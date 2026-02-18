//! Windows Service implementation with device notification and session change handling.
//!
//! Architecture:
//!   - Service main thread registers with SCM via `windows-service` crate
//!   - Creates a hidden message-only window on a worker thread
//!   - Window receives `WM_DEVICECHANGE` (monitor plug/unplug) and
//!     `WM_WTSSESSION_CHANGE` (logon, unlock, console connect)
//!   - On relevant events, triggers the profile reapply pipeline
//!   - Service stop signal cleanly destroys the window and exits
//!
//! Also provides a `watch()` entry point for foreground console mode
//! (same event loop, Ctrl+C to stop).

use lg_core::config::{self, Config};
use log::{error, info, warn};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{mem, ptr, thread};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_ALL_SESSIONS,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

// ============================================================================
// Constants
// ============================================================================

const SERVICE_NAME: &str = "lg-ultragear-color-svc";
const SERVICE_DISPLAY_NAME: &str = "LG UltraGear Color Profile Service";
const SERVICE_DESCRIPTION: &str =
    "Monitors display connections and reapplies the LG UltraGear no-dimming ICC color profile.";

/// Registry key where we store the monitor match pattern (informational).
const CONFIG_REG_KEY: &str = r"SYSTEM\CurrentControlSet\Services\lg-ultragear-color-svc\Parameters";
const CONFIG_REG_VALUE: &str = "MonitorMatch";

/// Custom window message to signal shutdown.
const WM_QUIT_SERVICE: u32 = WM_USER + 1;

/// GUID for display device interface notifications.
/// GUID_DEVINTERFACE_MONITOR = {E6F07B5F-EE97-4a90-B076-33F57BF4EAA7}
const GUID_DEVINTERFACE_MONITOR: windows::core::GUID = windows::core::GUID::from_values(
    0xE6F07B5F,
    0xEE97,
    0x4A90,
    [0xB0, 0x76, 0x33, 0xF5, 0x7B, 0xF4, 0xEA, 0xA7],
);

/// WM_DEVICECHANGE constants.
const WM_DEVICECHANGE: u32 = 0x0219;
const DBT_DEVICEARRIVAL: u32 = 0x8000;
const DBT_DEVNODES_CHANGED: u32 = 0x0007;

/// WM_WTSSESSION_CHANGE constants.
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const WTS_CONSOLE_CONNECT: u32 = 0x1;
const WTS_SESSION_LOGON: u32 = 0x5;
const WTS_SESSION_UNLOCK: u32 = 0x8;

/// DEV_BROADCAST_DEVICEINTERFACE_W for RegisterDeviceNotificationW.
#[repr(C)]
struct DevBroadcastDeviceInterface {
    dbcc_size: u32,
    dbcc_devicetype: u32,
    dbcc_reserved: u32,
    dbcc_classguid: windows::core::GUID,
    dbcc_name: [u16; 1],
}

const DBT_DEVTYP_DEVICEINTERFACE: u32 = 5;
const DEVICE_NOTIFY_WINDOW_HANDLE: u32 = 0;

/// Debounce epoch counter — each new event increments this, and the handler
/// thread only proceeds if no newer event has arrived during the debounce window.
static DEBOUNCE_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Accumulated event flags during the current debounce window.
static DEBOUNCE_EVENTS: AtomicU8 = AtomicU8::new(0);

// ── Event type bitflags ──────────────────────────────────────────

/// A monitor device interface was plugged in (GUID-filtered).
const EVENT_DEVICE_ARRIVAL: u8 = 0b0000_0001;
/// Generic devnode topology change (could be any device class).
const EVENT_DEVNODES_CHANGED: u8 = 0b0000_0010;
/// User logged on to a new session.
const EVENT_SESSION_LOGON: u8 = 0b0000_0100;
/// User unlocked an existing session.
const EVENT_SESSION_UNLOCK: u8 = 0b0000_1000;
/// A console was connected (e.g. Remote Desktop switch).
const EVENT_CONSOLE_CONNECT: u8 = 0b0001_0000;

/// Mask: any device-related event.
const EVENT_MASK_DEVICE: u8 = EVENT_DEVICE_ARRIVAL | EVENT_DEVNODES_CHANGED;
/// Mask: any session-related event.
const EVENT_MASK_SESSION: u8 = EVENT_SESSION_LOGON | EVENT_SESSION_UNLOCK | EVENT_CONSOLE_CONNECT;

// FFI for RegisterDeviceNotificationW (not always in windows crate metadata)
#[link(name = "user32")]
extern "system" {
    fn RegisterDeviceNotificationW(
        recipient: HWND,
        notification_filter: *const DevBroadcastDeviceInterface,
        flags: u32,
    ) -> *mut std::ffi::c_void;

    fn UnregisterDeviceNotification(handle: *mut std::ffi::c_void) -> BOOL;
}

// ============================================================================
// Service dispatch (called by SCM)
// ============================================================================

/// Entry point when launched by the Service Control Manager.
pub fn run() -> Result<(), Box<dyn Error>> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

windows_service::define_windows_service!(ffi_service_main, service_main);

fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = run_service(arguments) {
        error!("Service error: {}", e);
    }
}

fn run_service(_arguments: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    // Load config from file (falls back to defaults)
    let cfg = Config::load();
    info!(
        "Service starting. Monitor pattern: \"{}\", toast: {}, profile: {}",
        cfg.monitor_match, cfg.toast_enabled, cfg.profile_name
    );

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let hwnd: Arc<std::sync::Mutex<Option<isize>>> = Arc::new(std::sync::Mutex::new(None));
    let hwnd_clone = hwnd.clone();

    // Register service control handler
    let status_handle = service_control_handler::register(
        SERVICE_NAME,
        move |control| -> ServiceControlHandlerResult {
            match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    info!("Service stop/shutdown requested");
                    running_clone.store(false, Ordering::SeqCst);

                    if let Some(h) = *hwnd_clone.lock().unwrap() {
                        unsafe {
                            let _ =
                                PostMessageW(HWND(h as _), WM_QUIT_SERVICE, WPARAM(0), LPARAM(0));
                        }
                    }

                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        },
    )?;

    // Report running
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    // Run the event loop
    let result = run_event_loop(&cfg, &running, &hwnd);

    if let Err(ref e) = result {
        error!("Event loop error: {}", e);
    }

    // Report stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    info!("Service stopped");
    Ok(())
}

// ============================================================================
// Watch mode (foreground console)
// ============================================================================

/// Run the event watcher in foreground console mode.
///
/// Listens for the same display and session events as the service,
/// but runs interactively with Ctrl+C to stop. Useful for testing.
pub fn watch(config: &Config) -> Result<(), Box<dyn Error>> {
    let running = Arc::new(AtomicBool::new(true));
    let running_for_handler = running.clone();
    let hwnd: Arc<std::sync::Mutex<Option<isize>>> = Arc::new(std::sync::Mutex::new(None));
    let hwnd_for_handler = hwnd.clone();

    ctrlc::set_handler(move || {
        println!("\n[WATCH] Shutting down...");
        running_for_handler.store(false, Ordering::SeqCst);
        if let Some(h) = *hwnd_for_handler.lock().unwrap() {
            unsafe {
                let _ = PostMessageW(HWND(h as _), WM_QUIT_SERVICE, WPARAM(0), LPARAM(0));
            }
        }
    })?;

    println!("[WATCH] Starting event watcher (Ctrl+C to stop)");
    println!(
        "[WATCH] Monitor: \"{}\"  Profile: {}  Toast: {}",
        config.monitor_match,
        config.profile_name,
        if config.toast_enabled { "on" } else { "off" }
    );
    println!();

    run_event_loop(config, &running, &hwnd)
}

// ============================================================================
// Event loop with hidden message window
// ============================================================================

// Thread-local storage for the config used by the window proc.
thread_local! {
    static THREAD_CONFIG: std::cell::RefCell<Config> = std::cell::RefCell::new(Config::default());
}

fn run_event_loop(
    config: &Config,
    running: &Arc<AtomicBool>,
    hwnd_out: &Arc<std::sync::Mutex<Option<isize>>>,
) -> Result<(), Box<dyn Error>> {
    // Set thread-local config for window proc
    THREAD_CONFIG.with(|c| *c.borrow_mut() = config.clone());

    // Register window class
    let class_name = to_wide("LGUltraGearColorSvcWnd");
    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wnd_proc),
        hInstance: unsafe {
            windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR(ptr::null()))?
        }
        .into(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };

    let atom = unsafe { RegisterClassExW(&wc) };
    if atom == 0 {
        return Err("Failed to register window class".into());
    }

    // Create message-only window (HWND_MESSAGE parent = invisible)
    let hwnd = unsafe {
        CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(class_name.as_ptr()),
            Default::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            wc.hInstance,
            None,
        )?
    };

    // Store handle for control/shutdown
    *hwnd_out.lock().unwrap() = Some(hwnd.0 as isize);

    // Register for device interface notifications (monitor connect/disconnect)
    let filter = DevBroadcastDeviceInterface {
        dbcc_size: mem::size_of::<DevBroadcastDeviceInterface>() as u32,
        dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE,
        dbcc_reserved: 0,
        dbcc_classguid: GUID_DEVINTERFACE_MONITOR,
        dbcc_name: [0],
    };

    let notify_handle =
        unsafe { RegisterDeviceNotificationW(hwnd, &filter, DEVICE_NOTIFY_WINDOW_HANDLE) };
    if notify_handle.is_null() {
        warn!("RegisterDeviceNotificationW failed — will rely on session events only");
    }

    // Register for session change notifications
    let session_registered =
        unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_ALL_SESSIONS).is_ok() };
    if !session_registered {
        warn!("WTSRegisterSessionNotification failed — will rely on device events only");
    }

    info!("Event loop started, listening for display and session events");

    // Initial profile apply on startup (no stabilize delay needed)
    handle_profile_reapply(config);

    // Message pump
    unsafe {
        let mut msg = MSG::default();
        while running.load(Ordering::SeqCst) {
            let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if ret == BOOL(0) || ret == BOOL(-1) {
                break;
            }
            if msg.message == WM_QUIT_SERVICE {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup
    if session_registered {
        let _ = unsafe { WTSUnRegisterSessionNotification(hwnd) };
    }
    if !notify_handle.is_null() {
        unsafe {
            let _ = UnregisterDeviceNotification(notify_handle);
        }
    }
    unsafe {
        let _ = DestroyWindow(hwnd);
        let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), wc.hInstance);
    }

    Ok(())
}

/// Check if a `DBT_DEVICEARRIVAL` event is for a monitor device interface.
unsafe fn is_monitor_device_event(lparam: LPARAM) -> bool {
    if lparam.0 == 0 {
        return false;
    }
    let header = lparam.0 as *const DevBroadcastDeviceInterface;
    (*header).dbcc_devicetype == DBT_DEVTYP_DEVICEINTERFACE
        && (*header).dbcc_classguid == GUID_DEVINTERFACE_MONITOR
}

/// Schedule a profile reapply with event-aware debounce.
///
/// Each call accumulates the event type into `DEBOUNCE_EVENTS` and increments
/// the epoch counter. The spawned thread sleeps for the debounce window, then:
///   1. Checks epoch — if a newer event arrived, this thread exits (superseded).
///   2. Drains accumulated event flags and validates them.
///   3. If validated, waits `reapply_delay_ms` for display to fully initialize.
///   4. Runs the reapply pipeline.
fn schedule_reapply(config: &Config, event_flag: u8) {
    DEBOUNCE_EVENTS.fetch_or(event_flag, Ordering::SeqCst);
    let epoch = DEBOUNCE_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
    info!(
        "Event received (flag=0b{:08b}), debounce epoch={}",
        event_flag, epoch
    );

    let cfg = config.clone();
    thread::spawn(move || {
        // Phase 1: Debounce window
        thread::sleep(Duration::from_millis(cfg.stabilize_delay_ms));

        let current = DEBOUNCE_EPOCH.load(Ordering::SeqCst);
        if current != epoch {
            info!(
                "Debounce: epoch {} superseded by {}, skipping",
                epoch, current
            );
            return;
        }

        // Phase 2: Drain accumulated events and validate
        let events = DEBOUNCE_EVENTS.swap(0, Ordering::SeqCst);
        let has_device = events & EVENT_MASK_DEVICE != 0;
        let has_session = events & EVENT_MASK_SESSION != 0;

        info!(
            "Debounce settled (epoch={}): flags=0b{:08b}, device={}, session={}",
            epoch, events, has_device, has_session
        );

        if !has_device && !has_session {
            info!("No actionable events accumulated, skipping");
            return;
        }

        // For device-only events, do a quick WMI check BEFORE the long wait
        if has_device && !has_session {
            match lg_monitor::find_matching_monitors(&cfg.monitor_match) {
                Ok(devices) if devices.is_empty() => {
                    info!("Post-debounce: device event but no matching monitors found, skipping");
                    return;
                }
                Ok(devices) => {
                    info!(
                        "Post-debounce: {} matching monitor(s) confirmed",
                        devices.len()
                    );
                }
                Err(e) => {
                    warn!(
                        "Post-debounce monitor check failed: {}, proceeding anyway",
                        e
                    );
                }
            }
        }

        // Phase 3: Post-settle delay
        if cfg.reapply_delay_ms > 0 {
            info!(
                "Display settled, waiting {}ms for full initialization",
                cfg.reapply_delay_ms
            );
            thread::sleep(Duration::from_millis(cfg.reapply_delay_ms));
        }

        // Phase 4: Apply the profile
        handle_profile_reapply(&cfg);
    });
}

/// Window procedure — handles device change and session change messages.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DEVICECHANGE => {
            let event = wparam.0 as u32;
            let flag = match event {
                DBT_DEVICEARRIVAL if is_monitor_device_event(lparam) => Some(EVENT_DEVICE_ARRIVAL),
                DBT_DEVNODES_CHANGED => Some(EVENT_DEVNODES_CHANGED),
                _ => None,
            };
            if let Some(f) = flag {
                info!("Device change detected (event=0x{:04X})", event);
                THREAD_CONFIG.with(|c| schedule_reapply(&c.borrow(), f));
            }
            LRESULT(0)
        }

        WM_WTSSESSION_CHANGE => {
            let session_event = wparam.0 as u32;
            let flag = match session_event {
                WTS_CONSOLE_CONNECT => Some(EVENT_CONSOLE_CONNECT),
                WTS_SESSION_LOGON => Some(EVENT_SESSION_LOGON),
                WTS_SESSION_UNLOCK => Some(EVENT_SESSION_UNLOCK),
                _ => None,
            };
            if let Some(f) = flag {
                info!("Session change detected (event=0x{:04X})", session_event);
                THREAD_CONFIG.with(|c| schedule_reapply(&c.borrow(), f));
            }
            LRESULT(0)
        }

        WM_QUIT_SERVICE => {
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Detect matching monitors and reapply the profile, then refresh and toast.
fn handle_profile_reapply(config: &Config) {
    if config.monitor_match.is_empty() {
        warn!("Monitor match pattern is empty, skipping reapply");
        return;
    }
    if config.profile_name.is_empty() {
        warn!("Profile name is empty, skipping reapply");
        return;
    }

    let profile_path = config.profile_path();
    if !lg_profile::is_profile_installed(&profile_path) {
        warn!(
            "ICC profile not found: {}, skipping reapply",
            profile_path.display()
        );
        return;
    }

    match lg_monitor::find_matching_monitors(&config.monitor_match) {
        Ok(devices) if devices.is_empty() => {
            info!("No matching monitors found, skipping");
        }
        Ok(devices) => {
            for device in &devices {
                info!(
                    "Reapplying profile for: {} ({})",
                    device.name, device.device_key
                );
                if let Err(e) = lg_profile::reapply_profile(
                    &device.device_key,
                    &profile_path,
                    config.toggle_delay_ms,
                ) {
                    error!("Failed to reapply for {}: {}", device.name, e);
                }
            }
            lg_profile::refresh_display(
                config.refresh_display_settings,
                config.refresh_broadcast_color,
                config.refresh_invalidate,
            );
            lg_profile::trigger_calibration_loader(config.refresh_calibration_loader);
            lg_notify::show_reapply_toast(
                config.toast_enabled,
                &config.toast_title,
                &config.toast_body,
                config.verbose,
            );
            info!("Profile reapply complete for {} monitor(s)", devices.len());
        }
        Err(e) => {
            error!("Monitor enumeration failed: {}", e);
        }
    }
}

// ============================================================================
// Service install/uninstall/start/stop/status
// ============================================================================

pub fn install(monitor_match: &str) -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    let exe_path = std::env::current_exe()?;

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path,
        // Tell SCM to pass "service run" so clap dispatches to service mode
        launch_arguments: vec!["service".into(), "run".into()],
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };

    let service = manager.create_service(
        &service_info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START,
    )?;
    service.set_description(SERVICE_DESCRIPTION)?;

    // Store monitor match pattern in registry (informational)
    write_monitor_match(monitor_match)?;

    info!("Service installed successfully");
    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service =
        manager.open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::DELETE)?;

    // Try to stop first
    let _ = service.stop();
    thread::sleep(Duration::from_secs(1));

    service.delete()?;
    info!("Service uninstalled");
    Ok(())
}

pub fn start_service() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(SERVICE_NAME, ServiceAccess::START)?;
    service.start::<&str>(&[])?;
    Ok(())
}

pub fn stop_service() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(SERVICE_NAME, ServiceAccess::STOP)?;
    service.stop()?;
    Ok(())
}

pub fn print_status() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)?;
    let status = service.query_status()?;
    let cfg = Config::load();
    println!("Service: {}", SERVICE_NAME);
    println!("State:   {:?}", status.current_state);
    println!("PID:     {:?}", status.process_id);
    println!("Config:  {}", config::config_path().display());
    println!("Monitor: {}", cfg.monitor_match);
    println!("Profile: {}", cfg.profile_name);
    println!("Toast:   {}", if cfg.toast_enabled { "on" } else { "off" });
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

fn write_monitor_match(pattern: &str) -> Result<(), Box<dyn Error>> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm.create_subkey(CONFIG_REG_KEY)?;
    key.set_value(CONFIG_REG_VALUE, &pattern)?;
    Ok(())
}

/// Convert a Rust string to a null-terminated wide string (UTF-16).
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
#[path = "tests/service_tests.rs"]
mod tests;
