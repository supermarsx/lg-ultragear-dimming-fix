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
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
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

    let hwnd = Arc::new(AtomicIsize::new(0));
    let hwnd_clone = hwnd.clone();

    // Register service control handler
    let status_handle = service_control_handler::register(
        SERVICE_NAME,
        move |control| -> ServiceControlHandlerResult {
            match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    info!("Service stop/shutdown requested");
                    running_clone.store(false, Ordering::SeqCst);

                    let h = hwnd_clone.load(Ordering::SeqCst);
                    if h != 0 {
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
    let hwnd = Arc::new(AtomicIsize::new(0));
    let hwnd_for_handler = hwnd.clone();

    ctrlc::set_handler(move || {
        println!("\n[WATCH] Shutting down...");
        running_for_handler.store(false, Ordering::SeqCst);
        let h = hwnd_for_handler.load(Ordering::SeqCst);
        if h != 0 {
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

// Thread-local channel sender for the window proc to dispatch events
// to the single debounce worker thread (zero-allocation, lock-free dispatch).
thread_local! {
    static EVENT_SENDER: std::cell::RefCell<Option<mpsc::Sender<u8>>> =
        const { std::cell::RefCell::new(None) };
}

fn run_event_loop(
    config: &Config,
    running: &Arc<AtomicBool>,
    hwnd_out: &Arc<AtomicIsize>,
) -> Result<(), Box<dyn Error>> {
    // Create the debounce channel and a single worker thread.
    // Instead of spawning a new OS thread per event (old approach), all events
    // are dispatched via a lightweight channel send (a few nanoseconds) and
    // coalesced by one dedicated thread using recv_timeout — zero CPU when idle.
    let (tx, rx) = mpsc::channel::<u8>();
    EVENT_SENDER.with(|s| *s.borrow_mut() = Some(tx));

    let debounce_config = Arc::new(config.clone());
    let debounce_handle = {
        let cfg = debounce_config.clone();
        thread::Builder::new()
            .name("debounce-worker".into())
            .spawn(move || debounce_worker(rx, cfg))
            .map_err(|e| format!("failed to spawn debounce worker: {}", e))?
    };

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
        match CreateWindowExW(
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
        ) {
            Ok(h) => h,
            Err(e) => {
                // Clean up the registered window class before returning
                let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), wc.hInstance);
                return Err(format!("Failed to create message window: {}", e).into());
            }
        }
    };

    // Store handle for control/shutdown (lock-free atomic)
    hwnd_out.store(hwnd.0 as isize, Ordering::SeqCst);

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

    // Shutdown debounce worker: drop sender to close channel, then join thread
    EVENT_SENDER.with(|s| *s.borrow_mut() = None);
    let _ = debounce_handle.join();

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

/// Single-threaded debounce worker. Receives event flags from the message
/// loop via a channel, coalesces rapid events within the stabilize window,
/// validates with a WMI check, waits for display initialization, then
/// triggers the profile reapply pipeline.
///
/// Uses `recv_timeout` for efficient blocking — zero CPU when idle, no
/// thread-per-event spawning, fully interruptible on shutdown.
fn debounce_worker(rx: mpsc::Receiver<u8>, config: Arc<Config>) {
    while let Ok(flag) = rx.recv() {
        // Phase 1: Coalesce events within the stabilize window.
        // Any events arriving during this period are OR'd together.
        let mut accumulated = flag;
        let deadline = Instant::now() + Duration::from_millis(config.stabilize_delay_ms);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(f) => accumulated |= f,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return, // Shutdown
            }
        }

        let has_device = accumulated & EVENT_MASK_DEVICE != 0;
        let has_session = accumulated & EVENT_MASK_SESSION != 0;

        if !has_device && !has_session {
            continue;
        }

        info!(
            "Debounce settled: flags=0b{:08b}, device={}, session={}",
            accumulated, has_device, has_session
        );

        // Phase 2: For device-only events, validate monitors exist before the long wait
        if has_device && !has_session {
            match lg_monitor::find_matching_monitors(&config.monitor_match) {
                Ok(devices) if devices.is_empty() => {
                    info!("Post-debounce: no matching monitors found, skipping");
                    continue;
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

        // Phase 3: Post-settle delay for display initialization (interruptible)
        if config.reapply_delay_ms > 0 {
            info!(
                "Display settled, waiting {}ms for full initialization",
                config.reapply_delay_ms
            );
            match rx.recv_timeout(Duration::from_millis(config.reapply_delay_ms)) {
                Ok(_) => {
                    // New events during delay — drain and proceed with reapply
                    while rx.try_recv().is_ok() {}
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {} // Expected — delay completed
                Err(mpsc::RecvTimeoutError::Disconnected) => return, // Shutdown
            }
        }

        // Phase 4: Apply the profile
        handle_profile_reapply(&config);

        // Drain any events that queued during reapply to avoid redundant cycles
        while rx.try_recv().is_ok() {}
    }

    info!("Debounce worker stopped");
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
                EVENT_SENDER.with(|s| {
                    if let Some(tx) = s.borrow().as_ref() {
                        let _ = tx.send(f);
                    }
                });
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
                EVENT_SENDER.with(|s| {
                    if let Some(tx) = s.borrow().as_ref() {
                        let _ = tx.send(f);
                    }
                });
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
    // Auto-extract the embedded ICC profile if not already present
    if let Err(e) = lg_profile::ensure_profile_installed(&profile_path) {
        error!("Failed to extract ICC profile: {}", e);
        return;
    }
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
    // Copy the running binary to the install directory so the service
    // survives moves/deletes of the original file.
    let src_path = std::env::current_exe()?;
    let install_dir = config::config_dir();
    if !install_dir.exists() {
        std::fs::create_dir_all(&install_dir)?;
    }
    let dest_path = config::install_path();
    std::fs::copy(&src_path, &dest_path)?;
    info!("Binary copied to {}", dest_path.display());

    // Extract the embedded ICC profile to the Windows color store
    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    lg_profile::ensure_profile_installed(&profile_path)?;
    info!("ICC profile ensured at {}", profile_path.display());

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: dest_path,
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

    // Remove the installed binary (best-effort — may still be locked briefly)
    let install_bin = config::install_path();
    if install_bin.exists() {
        // Brief delay to let the process fully terminate
        thread::sleep(Duration::from_millis(500));
        match std::fs::remove_file(&install_bin) {
            Ok(()) => info!("Removed installed binary: {}", install_bin.display()),
            Err(e) => warn!(
                "Could not remove {}: {} (clean up manually)",
                install_bin.display(),
                e
            ),
        }
    }

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
    println!("Binary:  {}", config::install_path().display());
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
