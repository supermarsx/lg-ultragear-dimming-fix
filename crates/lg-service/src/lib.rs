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

use chrono::{Local, NaiveTime};
use lg_core::config::{self, Config};
use lg_core::state as app_state;
use log::{error, info, warn};
use regex::RegexBuilder;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use std::{mem, ptr, thread};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_ALL_SESSIONS,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceControl, ServiceControlAccept,
    ServiceErrorControl, ServiceExitCode, ServiceFailureActions, ServiceFailureResetPeriod,
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

const SERVICE_FAILURE_RESET_SECS: u64 = 24 * 60 * 60;
const SERVICE_FAILURE_RESTART_DELAYS_SECS: [u64; 3] = [5, 15, 30];

/// Registry key where we store the monitor match pattern (informational).
const CONFIG_REG_KEY: &str = r"SYSTEM\CurrentControlSet\Services\lg-ultragear-color-svc\Parameters";
const CONFIG_REG_VALUE: &str = "MonitorMatch";

/// Registry base key for Windows Event Log sources.
const EVENTLOG_REG_KEY: &str =
    r"SYSTEM\CurrentControlSet\Services\EventLog\Application\lg-ultragear-color-svc";

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
/// Periodic automation poll timer.
const EVENT_AUTOMATION_POLL: u8 = 0b0010_0000;

/// Mask: any device-related event.
const EVENT_MASK_DEVICE: u8 = EVENT_DEVICE_ARRIVAL | EVENT_DEVNODES_CHANGED;
/// Mask: any session-related event.
const EVENT_MASK_SESSION: u8 = EVENT_SESSION_LOGON | EVENT_SESSION_UNLOCK | EVENT_CONSOLE_CONNECT;

#[derive(Debug, Clone, Default)]
struct AmbientMemory {
    smoothed_lux: Option<f64>,
    band: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct AutomationDecision {
    source: String,
    preset: Option<String>,
    tuning_preset: Option<String>,
    luminance_cd_m2: Option<f64>,
    ddc_brightness: Option<u32>,
    details: String,
}

static AMBIENT_MEMORY: OnceLock<Mutex<AmbientMemory>> = OnceLock::new();
static LAST_AUTOMATION_FINGERPRINT: OnceLock<Mutex<String>> = OnceLock::new();

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
        "Service starting. Monitor pattern: \"{}\" ({:?}), toast: {}, profile: {}",
        cfg.monitor_match,
        if cfg.monitor_match_regex {
            lg_monitor::MonitorMatchMode::Regex
        } else {
            lg_monitor::MonitorMatchMode::Substring
        },
        cfg.toast_enabled,
        cfg.profile_name
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

    // Run the event loop. A normal stop/shutdown should return Ok(()).
    // Unexpected errors must map to a non-zero service exit code so SCM
    // recovery actions (restart) can trigger.
    let result = run_event_loop(&cfg, &running, &hwnd);
    let exit_code = match &result {
        Ok(()) => ServiceExitCode::NO_ERROR,
        Err(e) => {
            error!("Event loop error: {}", e);
            ServiceExitCode::ServiceSpecific(1)
        }
    };

    // Report stopped with a meaningful exit code.
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    match result {
        Ok(()) => {
            info!("Service stopped cleanly");
            Ok(())
        }
        Err(e) => Err(e),
    }
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
        "[WATCH] Monitor: \"{}\" ({})  Profile: {}  Toast: {}",
        config.monitor_match,
        if config.monitor_match_regex {
            "regex"
        } else {
            "substring"
        },
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

fn tuning_from_config(config: &Config) -> lg_profile::DynamicIccTuning {
    let manual = lg_profile::DynamicIccTuning {
        black_lift: config.icc_black_lift,
        midtone_boost: config.icc_midtone_boost,
        white_compression: config.icc_white_compression,
        gamma_r: config.icc_gamma_r,
        gamma_g: config.icc_gamma_g,
        gamma_b: config.icc_gamma_b,
        vcgt_enabled: config.icc_vcgt_enabled,
        vcgt_strength: config.icc_vcgt_strength,
        target_black_cd_m2: config.icc_target_black_cd_m2,
        include_media_black_point: config.icc_include_media_black_point,
        include_device_descriptions: config.icc_include_device_descriptions,
        include_characterization_target: config.icc_include_characterization_target,
        include_viewing_cond_desc: config.icc_include_viewing_cond_desc,
        technology_signature: lg_profile::parse_icc_signature_or_zero(
            &config.icc_technology_signature,
        ),
        ciis_signature: lg_profile::parse_icc_signature_or_zero(&config.icc_ciis_signature),
        cicp_enabled: config.icc_cicp_enabled,
        cicp_color_primaries: config.icc_cicp_primaries,
        cicp_transfer_characteristics: config.icc_cicp_transfer,
        cicp_matrix_coefficients: config.icc_cicp_matrix,
        cicp_full_range: config.icc_cicp_full_range,
        metadata_enabled: config.icc_metadata_enabled,
        include_calibration_datetime: config.icc_include_calibration_datetime,
        include_chromatic_adaptation: config.icc_include_chromatic_adaptation,
        include_chromaticity: config.icc_include_chromaticity,
        include_measurement: config.icc_include_measurement,
        include_viewing_conditions: config.icc_include_viewing_conditions,
        include_spectral_scaffold: config.icc_include_spectral_scaffold,
    };
    lg_profile::resolve_dynamic_icc_tuning(
        manual,
        &config.icc_tuning_preset,
        config.icc_tuning_overlay_manual,
    )
}

fn effective_preset_for_mode(config: &Config, hdr_mode: bool) -> String {
    lg_profile::select_effective_preset(
        &config.icc_active_preset,
        &config.icc_sdr_preset,
        &config.icc_hdr_preset,
        &config.icc_schedule_day_preset,
        &config.icc_schedule_night_preset,
        hdr_mode,
    )
}

fn detect_hdr_mode() -> bool {
    match lg_monitor::is_any_display_hdr_enabled() {
        Ok(enabled) => enabled,
        Err(e) => {
            warn!("HDR detection failed (falling back to SDR preset): {}", e);
            false
        }
    }
}

fn monitor_match_mode(config: &Config) -> lg_monitor::MonitorMatchMode {
    if config.monitor_match_regex {
        lg_monitor::MonitorMatchMode::Regex
    } else {
        lg_monitor::MonitorMatchMode::Substring
    }
}

fn find_matching_monitors_for_config(
    config: &Config,
) -> Result<Vec<lg_monitor::MatchedMonitor>, Box<dyn Error>> {
    lg_monitor::find_matching_monitors_with_mode(&config.monitor_match, monitor_match_mode(config))
}

fn monitor_identity_from_match(
    mon: &lg_monitor::MatchedMonitor,
) -> lg_profile::DynamicMonitorIdentity {
    lg_profile::DynamicMonitorIdentity {
        monitor_name: mon.name.clone(),
        device_key: mon.device_key.clone(),
        serial_number: mon.serial.clone(),
        manufacturer_id: mon.manufacturer_id.clone(),
        product_code: mon.product_code.clone(),
    }
}

fn parse_first_number(text: &str) -> Option<f64> {
    static NUMBER_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = NUMBER_RE.get_or_init(|| {
        regex::Regex::new(r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?").expect("valid number regex")
    });
    re.find(text).and_then(|m| m.as_str().parse::<f64>().ok())
}

fn parse_hhmm(value: &str, fallback: &str) -> NaiveTime {
    NaiveTime::parse_from_str(value.trim(), "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(fallback, "%H:%M"))
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(0, 0, 0).unwrap_or(NaiveTime::MIN))
}

fn time_in_range(now: NaiveTime, start: NaiveTime, end: NaiveTime) -> bool {
    if start <= end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

fn ambient_band_from_time(cfg: &app_state::AmbientAutomationConfig) -> String {
    let now = Local::now().time();
    let day_start = parse_hhmm(&cfg.day_start, "08:00");
    let evening_start = parse_hhmm(&cfg.evening_start, "18:00");
    let night_start = parse_hhmm(&cfg.night_start, "22:30");

    if time_in_range(now, day_start, evening_start) {
        "day".to_string()
    } else if time_in_range(now, evening_start, night_start) {
        "evening".to_string()
    } else {
        "night".to_string()
    }
}

fn read_ambient_sensor_raw_lux(cfg: &app_state::AmbientAutomationConfig) -> Option<f64> {
    if !cfg.sensor_enabled {
        return None;
    }
    let method = cfg.sensor_method.trim().to_ascii_lowercase();
    match method.as_str() {
        "ddc_brightness" | "ddc" => {
            let values = lg_monitor::ddc::get_brightness_all().ok()?;
            if values.is_empty() {
                return None;
            }
            let mut total = 0.0;
            let mut count = 0usize;
            for value in values {
                if value.max == 0 {
                    continue;
                }
                let pct = (value.current as f64 / value.max as f64) * 100.0;
                total += pct;
                count += 1;
            }
            if count == 0 {
                None
            } else {
                // Normalized pseudo-lux estimate from monitor luminance setting.
                Some((total / count as f64) * 3.0)
            }
        }
        "powershell" | "pwsh" => {
            if cfg.sensor_command.trim().is_empty() {
                return None;
            }
            let output = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &cfg.sensor_command])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            parse_first_number(&text)
        }
        "command" | "cmd" => {
            if cfg.sensor_command.trim().is_empty() {
                return None;
            }
            let output = std::process::Command::new("cmd")
                .args(["/C", &cfg.sensor_command])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            parse_first_number(&text)
        }
        "env" => {
            let key = if cfg.sensor_command.trim().is_empty() {
                "LG_AMBIENT_LUX"
            } else {
                cfg.sensor_command.trim()
            };
            std::env::var(key)
                .ok()
                .and_then(|v| v.trim().parse::<f64>().ok())
        }
        "simulated" | "manual" => {
            if !cfg.sensor_command.trim().is_empty() {
                cfg.sensor_command.trim().parse::<f64>().ok()
            } else {
                std::env::var("LG_AMBIENT_LUX")
                    .ok()
                    .and_then(|v| v.trim().parse::<f64>().ok())
            }
        }
        _ => None,
    }
}

fn ambient_band_from_lux(
    lux: f64,
    cfg: &app_state::AmbientAutomationConfig,
    previous: Option<&str>,
) -> String {
    let hysteresis = cfg.lux_hysteresis.max(0.0);
    if matches!(previous, Some("day")) && lux >= cfg.lux_day_threshold - hysteresis {
        return "day".to_string();
    }
    if matches!(previous, Some("night")) && lux <= cfg.lux_night_threshold + hysteresis {
        return "night".to_string();
    }
    if lux >= cfg.lux_day_threshold {
        "day".to_string()
    } else if lux <= cfg.lux_night_threshold {
        "night".to_string()
    } else {
        "evening".to_string()
    }
}

fn evaluate_ambient_decision(
    cfg: &app_state::AmbientAutomationConfig,
) -> Option<AutomationDecision> {
    if !cfg.enabled {
        return None;
    }

    let mut memory = AMBIENT_MEMORY
        .get_or_init(|| Mutex::new(AmbientMemory::default()))
        .lock()
        .ok()?;
    let (band, details) = if let Some(raw_lux) = read_ambient_sensor_raw_lux(cfg) {
        let scaled = raw_lux * cfg.sensor_scale + cfg.sensor_offset;
        let smoothed = if let Some(prev) = memory.smoothed_lux {
            prev + (scaled - prev) * cfg.sensor_smoothing_alpha.clamp(0.0, 1.0)
        } else {
            scaled
        };
        memory.smoothed_lux = Some(smoothed);
        let band = ambient_band_from_lux(smoothed, cfg, memory.band.as_deref());
        let details = format!(
            "method={} raw_lux={:.1} scaled_lux={:.1} band={}",
            cfg.sensor_method, raw_lux, smoothed, band
        );
        (band, details)
    } else if cfg.allow_time_fallback {
        let band = ambient_band_from_time(cfg);
        let details = format!("method=time_fallback band={}", band);
        (band, details)
    } else {
        (
            "unknown".to_string(),
            "method=sensor_unavailable".to_string(),
        )
    };

    memory.band = Some(band.clone());

    let preset = match band.as_str() {
        "day" => cfg.day_preset.trim(),
        "evening" => cfg.evening_preset.trim(),
        "night" => cfg.night_preset.trim(),
        _ => cfg.unknown_sensor_preset.trim(),
    };

    let luminance = match band.as_str() {
        "day" => cfg.day_luminance_cd_m2,
        "evening" => cfg.evening_luminance_cd_m2,
        "night" => cfg.night_luminance_cd_m2,
        _ => None,
    };

    Some(AutomationDecision {
        source: "ambient".to_string(),
        preset: if preset.is_empty() {
            None
        } else {
            Some(preset.to_string())
        },
        tuning_preset: None,
        luminance_cd_m2: luminance,
        ddc_brightness: None,
        details,
    })
}

fn running_process_names() -> Vec<String> {
    let output = match std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut names = Vec::new();
    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let first = line
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"');
        if first.is_empty() {
            continue;
        }
        let lower = first.to_ascii_lowercase();
        if !names.iter().any(|n| n == &lower) {
            names.push(lower);
        }
    }
    names
}

fn process_matches_rule(
    process_name: &str,
    pattern: &str,
    match_mode: &str,
    case_sensitive: bool,
) -> bool {
    if pattern.trim().is_empty() {
        return false;
    }
    match match_mode.trim().to_ascii_lowercase().as_str() {
        "exact" => {
            if case_sensitive {
                process_name == pattern
            } else {
                process_name.eq_ignore_ascii_case(pattern)
            }
        }
        "regex" => RegexBuilder::new(pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .map(|re| re.is_match(process_name))
            .unwrap_or(false),
        _ => {
            if case_sensitive {
                process_name.contains(pattern)
            } else {
                process_name
                    .to_ascii_lowercase()
                    .contains(&pattern.to_ascii_lowercase())
            }
        }
    }
}

fn evaluate_app_rule_decision(cfg: &app_state::AppRulesConfig) -> Option<AutomationDecision> {
    if !cfg.enabled {
        return None;
    }
    let mut rules = cfg
        .rules
        .iter()
        .filter(|r| r.enabled && !r.process_pattern.trim().is_empty())
        .collect::<Vec<_>>();
    if rules.is_empty() {
        return None;
    }
    rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    let processes = running_process_names();
    if processes.is_empty() {
        return None;
    }

    for rule in rules {
        let mut matched_process = None;
        for proc_name in &processes {
            if process_matches_rule(
                proc_name,
                &rule.process_pattern,
                &cfg.match_mode,
                cfg.case_sensitive,
            ) {
                matched_process = Some(proc_name.clone());
                break;
            }
        }
        if let Some(proc_name) = matched_process {
            return Some(AutomationDecision {
                source: format!("app_rule:{}", rule.name),
                preset: if rule.preset.trim().is_empty() {
                    None
                } else {
                    Some(rule.preset.trim().to_string())
                },
                tuning_preset: if rule.tuning_preset.trim().is_empty() {
                    None
                } else {
                    Some(rule.tuning_preset.trim().to_string())
                },
                luminance_cd_m2: rule.luminance_override,
                ddc_brightness: rule.ddc_brightness,
                details: format!(
                    "rule=\"{}\" process=\"{}\" priority={} mode={}",
                    rule.name, proc_name, rule.priority, cfg.match_mode
                ),
            });
        }
    }
    None
}

fn resolve_automation_decision(event_flags: u8) -> AutomationDecision {
    let cfg = app_state::load_automation_config();
    let mut resolved = AutomationDecision::default();
    let mut details = Vec::new();

    if let Some(ambient) = evaluate_ambient_decision(&cfg.ambient) {
        details.push(ambient.details.clone());
        resolved = ambient;
    }

    if let Some(rule_decision) = evaluate_app_rule_decision(&cfg.app_rules) {
        let override_ambient = cfg.app_rules.override_ambient;
        details.push(rule_decision.details.clone());
        if override_ambient {
            resolved = rule_decision;
        } else {
            if resolved.preset.is_none() {
                resolved.preset = rule_decision.preset;
            }
            if resolved.tuning_preset.is_none() {
                resolved.tuning_preset = rule_decision.tuning_preset;
            }
            if resolved.luminance_cd_m2.is_none() {
                resolved.luminance_cd_m2 = rule_decision.luminance_cd_m2;
            }
            if resolved.ddc_brightness.is_none() {
                resolved.ddc_brightness = rule_decision.ddc_brightness;
            }
            if resolved.source.is_empty() {
                resolved.source = rule_decision.source;
            } else {
                resolved.source = format!("{},{}", resolved.source, rule_decision.source);
            }
        }
    }

    if resolved.source.is_empty() {
        resolved.source = if event_flags & EVENT_AUTOMATION_POLL != 0 {
            "automation_poll".to_string()
        } else {
            "base".to_string()
        };
    }
    resolved.details = details.join(" | ");
    resolved
}

fn maybe_run_self_heal(config: &Config, effective_preset: &str, trigger: &str, event_flags: u8) {
    let cfg = app_state::load_automation_config();
    let health = &cfg.health;
    if !health.enabled {
        return;
    }
    let should_run = match trigger {
        "startup" => health.startup_self_heal,
        "event" => {
            health.run_every_event
                || (health.wake_self_heal && (event_flags & EVENT_MASK_SESSION != 0))
        }
        "automation_poll" => health.run_every_event,
        _ => health.run_every_event,
    };
    if !should_run {
        return;
    }

    let color_dir = lg_profile::color_directory();
    let profile_path =
        lg_profile::resolve_active_profile_path(&color_dir, effective_preset, &config.profile_name);
    let mut repaired = false;
    if health.verify_profile_presence
        && !lg_profile::is_profile_installed(&profile_path)
        && health.regenerate_if_missing
    {
        if lg_profile::ensure_active_profile_installed_tuned(
            &color_dir,
            effective_preset,
            &config.profile_name,
            config.icc_gamma,
            config.icc_luminance_cd_m2,
            config.icc_generate_specialized_profiles,
            tuning_from_config(config),
        )
        .is_ok()
        {
            repaired = true;
            app_state::append_diagnostic_event(
                "service",
                "WARN",
                "self_heal",
                &format!(
                    "regenerated_missing_profile preset={} trigger={}",
                    effective_preset, trigger
                ),
            );
        }
    }

    if health.cleanup_stale_profiles {
        let expected = profile_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| config.profile_name.clone());
        let removed = lg_profile::cleanup_stale_profiles(&expected);
        if !removed.is_empty() {
            repaired = true;
            app_state::append_diagnostic_event(
                "service",
                "INFO",
                "self_heal",
                &format!("removed_stale_profiles={}", removed.len()),
            );
        }
    }

    if repaired {
        info!("Self-heal applied corrections");
    }
}

fn compute_automation_fingerprint(
    preset: &str,
    tuning_preset: &str,
    luminance_cd_m2: f64,
    ddc_brightness: Option<u32>,
) -> String {
    format!(
        "preset={} tuning={} luminance={:.2} ddc={}",
        preset,
        tuning_preset,
        luminance_cd_m2,
        ddc_brightness
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    )
}

fn should_skip_automation_apply(fingerprint: &str, trigger: &str) -> bool {
    if trigger != "automation_poll" {
        let mut state = LAST_AUTOMATION_FINGERPRINT
            .get_or_init(|| Mutex::new(String::new()))
            .lock()
            .ok();
        if let Some(ref mut current) = state {
            **current = fingerprint.to_string();
        }
        return false;
    }
    let mut state = match LAST_AUTOMATION_FINGERPRINT
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
    {
        Ok(s) => s,
        Err(_) => return false,
    };
    if *state == fingerprint {
        true
    } else {
        *state = fingerprint.to_string();
        false
    }
}

fn emit_apply_latency(source: &str, started: Instant, success: bool, details: &str) {
    let metrics_cfg = app_state::load_automation_config().metrics;
    if !metrics_cfg.enabled || !metrics_cfg.collect_latency {
        return;
    }
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let line = if details.trim().is_empty() {
        format!("ms={} success={}", elapsed_ms, if success { 1 } else { 0 })
    } else {
        format!(
            "ms={} success={} {}",
            elapsed_ms,
            if success { 1 } else { 0 },
            details
        )
    };
    app_state::append_diagnostic_event(source, "INFO", "apply_latency", &line);
}

fn automation_poll_interval_ms() -> Option<u64> {
    let cfg = app_state::load_automation_config();
    let mut intervals = Vec::new();
    if cfg.ambient.enabled {
        intervals.push(cfg.ambient.sensor_poll_interval_ms.max(500));
    }
    if cfg.app_rules.enabled {
        intervals.push(cfg.app_rules.poll_interval_ms.max(500));
    }
    intervals.into_iter().min()
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
    EVENT_SENDER.with(|s| *s.borrow_mut() = Some(tx.clone()));

    let debounce_config = Arc::new(config.clone());
    let debounce_handle = {
        let cfg = debounce_config.clone();
        thread::Builder::new()
            .name("debounce-worker".into())
            .spawn(move || debounce_worker(rx, cfg))
            .map_err(|e| format!("failed to spawn debounce worker: {}", e))?
    };
    let automation_poller = {
        let poll_interval = automation_poll_interval_ms();
        poll_interval.map(|interval_ms| {
            let tx = tx.clone();
            let running = running.clone();
            thread::Builder::new()
                .name("automation-poller".into())
                .spawn(move || {
                    while running.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(interval_ms));
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        if tx.send(EVENT_AUTOMATION_POLL).is_err() {
                            break;
                        }
                    }
                })
        })
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
    handle_profile_reapply(config, "startup", 0);

    let mut message_loop_error: Option<String> = None;

    // Message pump
    unsafe {
        let mut msg = MSG::default();
        while running.load(Ordering::SeqCst) {
            let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if ret == BOOL(0) {
                break;
            }
            if ret == BOOL(-1) {
                message_loop_error = Some("GetMessageW failed in service message loop".to_string());
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
    if let Some(handle) = automation_poller {
        if let Ok(join_handle) = handle {
            let _ = join_handle.join();
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

    if let Some(err) = message_loop_error {
        return Err(err.into());
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
        let has_poll = accumulated & EVENT_AUTOMATION_POLL != 0;

        if !has_device && !has_session && !has_poll {
            continue;
        }

        info!(
            "Debounce settled: flags=0b{:08b}, device={}, session={}, poll={}",
            accumulated, has_device, has_session, has_poll
        );
        app_state::append_diagnostic_event(
            "service",
            "INFO",
            "event_debounce",
            &format!(
                "flags=0b{:08b} device={} session={} poll={}",
                accumulated, has_device, has_session, has_poll
            ),
        );

        // Phase 2: For device-only events, validate monitors exist before the long wait
        if has_device && !has_session {
            match find_matching_monitors_for_config(&config) {
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
        if (has_device || has_session) && config.reapply_delay_ms > 0 {
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
        let trigger = if has_poll && !has_device && !has_session {
            "automation_poll"
        } else {
            "event"
        };
        handle_profile_reapply(&config, trigger, accumulated);

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
fn handle_profile_reapply(config: &Config, trigger: &str, event_flags: u8) {
    let started = Instant::now();
    let mut effective_cfg = config.clone();
    let decision = resolve_automation_decision(event_flags);
    let forced_preset = decision.preset.clone().filter(|p| !p.trim().is_empty());
    let hdr_mode_active = detect_hdr_mode();

    if let Some(ref tuning) = decision.tuning_preset {
        effective_cfg.icc_tuning_preset = tuning.clone();
    }
    if let Some(luminance) = decision.luminance_cd_m2 {
        effective_cfg.icc_luminance_cd_m2 = lg_profile::sanitize_dynamic_luminance_cd_m2(luminance);
    }
    let active_preset = forced_preset
        .clone()
        .unwrap_or_else(|| effective_preset_for_mode(&effective_cfg, hdr_mode_active));
    let sdr_preset = forced_preset
        .clone()
        .unwrap_or_else(|| effective_preset_for_mode(&effective_cfg, false));
    let hdr_preset = forced_preset
        .clone()
        .unwrap_or_else(|| effective_preset_for_mode(&effective_cfg, true));

    if !decision.details.trim().is_empty() {
        app_state::append_diagnostic_event(
            "service",
            "INFO",
            "automation_decision",
            &format!(
                "trigger={} source={} preset={} details={}",
                trigger, decision.source, active_preset, decision.details
            ),
        );
    }

    maybe_run_self_heal(&effective_cfg, &active_preset, trigger, event_flags);

    let desired_ddc_brightness = decision.ddc_brightness.or_else(|| {
        if effective_cfg.ddc_brightness_on_reapply {
            Some(effective_cfg.ddc_brightness_value.min(100))
        } else {
            None
        }
    });

    let fingerprint = compute_automation_fingerprint(
        &active_preset,
        &effective_cfg.icc_tuning_preset,
        effective_cfg.icc_luminance_cd_m2,
        desired_ddc_brightness,
    );
    if should_skip_automation_apply(&fingerprint, trigger) {
        app_state::append_diagnostic_event(
            "service",
            "INFO",
            "apply_skip",
            "automation poll: no change in active automation decision",
        );
        emit_apply_latency("service", started, true, "skip=automation_no_change");
        return;
    }

    app_state::append_diagnostic_event(
        "service",
        "INFO",
        "apply_begin",
        &format!(
            "trigger={} pattern=\"{}\" mode={} preset={} source={}",
            trigger,
            effective_cfg.monitor_match,
            if effective_cfg.monitor_match_regex {
                "regex"
            } else {
                "substring"
            },
            active_preset,
            decision.source
        ),
    );

    let success = (|| -> bool {
        if effective_cfg.monitor_match.is_empty() {
            warn!("Monitor match pattern is empty, skipping reapply");
            app_state::append_diagnostic_event(
                "service",
                "WARN",
                "apply_skip",
                "monitor match pattern is empty",
            );
            return false;
        }
        if matches!(
            lg_profile::parse_dynamic_icc_preset(&active_preset),
            lg_profile::DynamicIccPreset::Custom
        ) && effective_cfg.profile_name.trim().is_empty()
        {
            warn!("Profile name is empty, skipping reapply");
            app_state::append_diagnostic_event(
                "service",
                "WARN",
                "apply_skip",
                "custom preset selected but profile_name is empty",
            );
            return false;
        }

        let color_dir = lg_profile::color_directory();
        let shared_mode_paths = if effective_cfg.icc_per_monitor_profiles {
            None
        } else {
            match lg_profile::ensure_mode_profiles_installed_tuned(
                &color_dir,
                &sdr_preset,
                &hdr_preset,
                &effective_cfg.profile_name,
                effective_cfg.icc_gamma,
                effective_cfg.icc_luminance_cd_m2,
                effective_cfg.icc_generate_specialized_profiles,
                tuning_from_config(&effective_cfg),
            ) {
                Ok((sdr_path, hdr_path)) => {
                    if !lg_profile::is_profile_installed(&sdr_path)
                        || !lg_profile::is_profile_installed(&hdr_path)
                    {
                        warn!(
                            "ICC mode profile not found (sdr={}, hdr={}), skipping reapply",
                            sdr_path.display(),
                            hdr_path.display()
                        );
                        return false;
                    }
                    Some((sdr_path, hdr_path))
                }
                Err(e) => {
                    error!("Failed to generate/install active ICC profile: {}", e);
                    app_state::append_diagnostic_event(
                        "service",
                        "ERROR",
                        "apply_error",
                        &format!("failed to generate active profile: {}", e),
                    );
                    return false;
                }
            }
        };

        match find_matching_monitors_for_config(&effective_cfg) {
            Ok(devices) if devices.is_empty() => {
                info!("No matching monitors found, skipping");
                app_state::append_diagnostic_event(
                    "service",
                    "WARN",
                    "apply_skip",
                    "no matching monitors",
                );
                false
            }
            Ok(devices) => {
                let mut applied_count = 0usize;
                let mut last_applied_profile: Option<std::path::PathBuf> = None;
                for device in &devices {
                    let (sdr_profile_path, hdr_profile_path) =
                        if let Some(paths) = &shared_mode_paths {
                            paths.clone()
                        } else {
                            let identity = monitor_identity_from_match(device);
                            match lg_profile::ensure_mode_profiles_installed_tuned_for_monitor(
                                &color_dir,
                                &sdr_preset,
                                &hdr_preset,
                                &effective_cfg.profile_name,
                                effective_cfg.icc_gamma,
                                effective_cfg.icc_luminance_cd_m2,
                                effective_cfg.icc_generate_specialized_profiles,
                                tuning_from_config(&effective_cfg),
                                &identity,
                            ) {
                                Ok(path) => path,
                                Err(e) => {
                                    error!(
                                        "Failed to generate monitor-scoped ICC for {}: {}",
                                        device.name, e
                                    );
                                    app_state::append_diagnostic_event(
                                        "service",
                                        "ERROR",
                                        "apply_error",
                                        &format!(
                                            "monitor-scoped profile generation failed for {}: {}",
                                            device.name, e
                                        ),
                                    );
                                    continue;
                                }
                            }
                        };
                    let active_profile_path = if hdr_mode_active {
                        &hdr_profile_path
                    } else {
                        &sdr_profile_path
                    };
                    info!(
                        "Reapplying mode profiles for: {} ({}) active={} sdr={} hdr={}",
                        device.name,
                        device.device_key,
                        active_profile_path.display(),
                        sdr_profile_path.display(),
                        hdr_profile_path.display()
                    );
                    if let Err(e) = lg_profile::reapply_profile_with_mode_associations(
                        &device.device_key,
                        active_profile_path,
                        &sdr_profile_path,
                        &hdr_profile_path,
                        effective_cfg.toggle_delay_ms,
                        false,
                    ) {
                        error!("Failed to reapply for {}: {}", device.name, e);
                        app_state::append_diagnostic_event(
                            "service",
                            "ERROR",
                            "apply_error",
                            &format!("reapply failed for {}: {}", device.name, e),
                        );
                    } else {
                        applied_count += 1;
                        last_applied_profile = Some(active_profile_path.clone());
                    }
                }
                // Keep periodic/event-driven reapply refresh non-disruptive.
                // Hard refresh is escalated internally only when verification fails.
                lg_profile::refresh_display(
                    false,
                    effective_cfg.refresh_broadcast_color,
                    effective_cfg.refresh_invalidate,
                );
                lg_profile::trigger_calibration_loader(effective_cfg.refresh_calibration_loader);

                if let Some(level) = desired_ddc_brightness {
                    match lg_monitor::ddc::set_brightness_all(level) {
                        Ok(n) => info!("DDC brightness set to {} on {} monitor(s)", level, n),
                        Err(e) => {
                            warn!("DDC brightness set failed: {} (non-fatal)", e);
                            app_state::append_diagnostic_event(
                                "service",
                                "WARN",
                                "ddc_warning",
                                &format!("ddc brightness write failed: {}", e),
                            );
                        }
                    }
                }

                if applied_count > 0 {
                    if let Some(profile_path) = &last_applied_profile {
                        if let Ok(snapshot) = app_state::create_profile_snapshot(
                            &effective_cfg,
                            "Auto Last Good (Service)",
                            "auto",
                            profile_path,
                            desired_ddc_brightness,
                            "Service successful reapply",
                        ) {
                            let _ = app_state::mark_snapshot_last_good(
                                &snapshot,
                                snapshot.ddc_brightness,
                                "Service successful reapply",
                            );
                        }
                    }
                    app_state::append_diagnostic_event(
                        "service",
                        "INFO",
                        "apply_success",
                        &format!(
                            "trigger={} source={} applied profiles to {} monitor(s)",
                            trigger, decision.source, applied_count
                        ),
                    );
                } else {
                    app_state::append_diagnostic_event(
                        "service",
                        "WARN",
                        "apply_skip",
                        "no monitors successfully reapplied",
                    );
                }

                lg_notify::show_reapply_toast(
                    effective_cfg.toast_enabled,
                    &effective_cfg.toast_title,
                    &effective_cfg.toast_body,
                    effective_cfg.verbose,
                );
                info!("Profile reapply complete for {} monitor(s)", applied_count);
                applied_count > 0
            }
            Err(e) => {
                error!("Monitor enumeration failed: {}", e);
                app_state::append_diagnostic_event(
                    "service",
                    "ERROR",
                    "apply_error",
                    &format!("monitor enumeration failed: {}", e),
                );
                false
            }
        }
    })();

    emit_apply_latency(
        "service",
        started,
        success,
        &format!(
            "trigger={} preset={} source={}",
            trigger, active_preset, decision.source
        ),
    );
}

// ============================================================================
// Service install/uninstall/start/stop/status
// ============================================================================

fn default_service_failure_actions() -> ServiceFailureActions {
    let actions = SERVICE_FAILURE_RESTART_DELAYS_SECS
        .iter()
        .map(|secs| ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(*secs),
        })
        .collect::<Vec<_>>();

    ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(
            SERVICE_FAILURE_RESET_SECS,
        )),
        reboot_msg: None,
        command: None,
        actions: Some(actions),
    }
}

fn configure_service_recovery(
    service: &windows_service::service::Service,
) -> Result<(), Box<dyn Error>> {
    service.update_failure_actions(default_service_failure_actions())?;
    service.set_failure_actions_on_non_crash_failures(true)?;
    info!(
        "Service recovery configured: restart delays = {:?}s, reset period = {}s",
        SERVICE_FAILURE_RESTART_DELAYS_SECS, SERVICE_FAILURE_RESET_SECS
    );
    Ok(())
}

pub fn install(monitor_match: &str) -> Result<(), Box<dyn Error>> {
    // If the service already exists, stop it first so we can overwrite the
    // binary.  Errors here are expected (service may not exist yet).
    stop_existing_service();

    // Copy the running binary to the install directory so the service
    // survives moves/deletes of the original file.
    let src_path = std::env::current_exe()?;
    let install_dir = config::config_dir();
    if !install_dir.exists() {
        std::fs::create_dir_all(&install_dir)?;
    }
    let dest_path = config::install_path();
    copy_with_retry(&src_path, &dest_path)?;
    info!("Binary copied to {}", dest_path.display());

    // Generate active/specialized ICC profiles in the Windows color store
    let cfg = Config::load();
    let color_dir = lg_profile::color_directory();
    let sdr_preset = effective_preset_for_mode(&cfg, false);
    let hdr_preset = effective_preset_for_mode(&cfg, true);
    let (sdr_profile_path, hdr_profile_path) = lg_profile::ensure_mode_profiles_installed_tuned(
        &color_dir,
        &sdr_preset,
        &hdr_preset,
        &cfg.profile_name,
        cfg.icc_gamma,
        cfg.icc_luminance_cd_m2,
        cfg.icc_generate_specialized_profiles,
        tuning_from_config(&cfg),
    )?;
    info!(
        "ICC mode profiles ensured at SDR={} HDR={}",
        sdr_profile_path.display(),
        hdr_profile_path.display()
    );

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    // If the service already exists in SCM, delete the old registration so
    // create_service succeeds.  The binary was already stopped above.
    if let Ok(existing) = manager.open_service(SERVICE_NAME, ServiceAccess::DELETE) {
        let _ = existing.delete();
        // Brief pause for SCM to finish the deletion.
        thread::sleep(Duration::from_millis(500));
        info!("Deleted previous service registration before reinstall");
    }

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
    configure_service_recovery(&service)?;

    // Store monitor match pattern in registry (informational)
    write_monitor_match(monitor_match)?;

    // Register the event log source so Event Viewer can resolve message strings.
    // The winlog crate embeds a message table resource (eventmsgs) into the
    // binary.  We point EventMessageFile at the *installed* copy so messages
    // render correctly regardless of where the installer was launched from.
    if let Err(e) = register_event_source(&config::install_path()) {
        // Event log registration is useful for richer diagnostics, but it
        // should not block service install/start for end users.
        warn!("Event log source registration failed (non-fatal): {}", e);
    }

    info!("Service installed successfully");
    Ok(())
}

/// Stop the existing service (if any) so we can safely overwrite the binary.
/// All errors are silently absorbed — the service may not exist yet.
fn stop_existing_service() {
    let Ok(manager) = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
    else {
        return;
    };
    let Ok(service) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    ) else {
        return; // service doesn't exist yet
    };

    let _ = service.stop();

    // Poll until stopped (up to ~10 s).
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(status) = service.query_status() {
            if status.current_state == ServiceState::Stopped {
                info!("Existing service stopped before reinstall");
                return;
            }
        }
        if Instant::now() >= deadline {
            warn!("Existing service did not stop within 10 s — proceeding anyway");
            return;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// Copy a file with retries on sharing violations (error 32).
/// Retries up to 5 times with escalating back-off (~3.2 s total).
fn copy_with_retry(src: &std::path::Path, dst: &std::path::Path) -> Result<u64, Box<dyn Error>> {
    let delays_ms: &[u64] = &[0, 200, 500, 1000, 1500];
    for (attempt, &ms) in delays_ms.iter().enumerate() {
        if ms > 0 {
            thread::sleep(Duration::from_millis(ms));
        }
        match std::fs::copy(src, dst) {
            Ok(bytes) => return Ok(bytes),
            Err(e) if e.raw_os_error() == Some(32) && attempt < delays_ms.len() - 1 => {
                info!(
                    "Binary copy attempt {} blocked (sharing violation) — retrying",
                    attempt + 1
                );
            }
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}

pub fn uninstall() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    // Open the service — if it doesn't exist, that's fine (already removed).
    match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(service) => {
            // Try to stop first, then poll until actually stopped (up to ~10 s).
            let _ = service.stop();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Ok(status) = service.query_status() {
                    if status.current_state == ServiceState::Stopped {
                        break;
                    }
                }
                if Instant::now() >= deadline {
                    warn!("Service did not stop within 10 s — proceeding with delete");
                    break;
                }
                thread::sleep(Duration::from_millis(250));
            }

            // Delete the service registration from SCM.
            if let Err(e) = service.delete() {
                warn!(
                    "service.delete() failed: {} (may already be marked for deletion)",
                    e
                );
            }
        }
        Err(e) => {
            // Service not installed / already deleted — not an error.
            info!("Service not found (already removed): {}", e);
        }
    }

    // Deregister the event log source (best-effort)
    deregister_event_source();

    // Remove the installed binary with retry + schedule-for-reboot fallback.
    let install_bin = config::install_path();
    if install_bin.exists() {
        force_remove_file(&install_bin);
    }

    info!("Service uninstalled");
    Ok(())
}

/// Attempt to delete a file with retries.  If still locked after all
/// attempts, schedule it for deletion on next reboot via
/// `MoveFileExW(MOVEFILE_DELAY_UNTIL_REBOOT)`.
fn force_remove_file(path: &std::path::Path) {
    // Retry up to 6 times with increasing back-off (total ~7 s).
    let delays = [200, 500, 1000, 1500, 2000, 2000];
    for (attempt, &ms) in delays.iter().enumerate() {
        thread::sleep(Duration::from_millis(ms));
        match std::fs::remove_file(path) {
            Ok(()) => {
                info!("Removed file: {} (attempt {})", path.display(), attempt + 1);
                return;
            }
            Err(e) => {
                info!(
                    "Remove attempt {} for {}: {}",
                    attempt + 1,
                    path.display(),
                    e
                );
            }
        }
    }

    schedule_reboot_delete_impl(path);
}

/// Public wrapper: retry file deletion then fall back to reboot-delete.
/// Used by the CLI for locked files outside the service crate.
pub fn force_remove_file_public(path: &std::path::Path) {
    force_remove_file(path);
}

/// Schedule a path (file or empty directory) for deletion on next reboot
/// via `MoveFileExW(MOVEFILE_DELAY_UNTIL_REBOOT)`.
pub fn schedule_reboot_delete(path: &std::path::Path) {
    schedule_reboot_delete_impl(path);
}

fn schedule_reboot_delete_impl(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe { MoveFileExW(PCWSTR(wide.as_ptr()), None, MOVEFILE_DELAY_UNTIL_REBOOT) };
    match ok {
        Ok(()) => info!("Scheduled for deletion on reboot: {}", path.display()),
        Err(e) => warn!(
            "Could not schedule {} for reboot deletion: {} (clean up manually)",
            path.display(),
            e
        ),
    }
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
    let cfg = Config::load();

    let manager = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
    {
        Ok(m) => m,
        Err(e) => {
            return Err(format!("Cannot connect to Service Control Manager: {}", e).into());
        }
    };

    let service = match manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
        Ok(s) => s,
        Err(_) => {
            println!("Service: {}  (NOT INSTALLED)", SERVICE_NAME);
            println!("Binary:  {}", config::install_path().display());
            println!("Config:  {}", config::config_path().display());
            println!(
                "Monitor: {} ({})",
                cfg.monitor_match,
                if cfg.monitor_match_regex {
                    "regex"
                } else {
                    "substring"
                }
            );
            println!("Profile: {}", cfg.profile_name);
            println!("Toast:   {}", if cfg.toast_enabled { "on" } else { "off" });
            return Ok(());
        }
    };

    let status = match service.query_status() {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("Cannot query service status: {}", e).into());
        }
    };

    println!("Service: {}", SERVICE_NAME);
    println!("State:   {:?}", status.current_state);
    println!("PID:     {:?}", status.process_id);
    println!("Binary:  {}", config::install_path().display());
    println!("Config:  {}", config::config_path().display());
    println!(
        "Monitor: {} ({})",
        cfg.monitor_match,
        if cfg.monitor_match_regex {
            "regex"
        } else {
            "substring"
        }
    );
    println!("Profile: {}", cfg.profile_name);
    println!("Toast:   {}", if cfg.toast_enabled { "on" } else { "off" });
    Ok(())
}

/// Query service installation and running state for display purposes.
/// Returns `(installed, running)`. Never panics.
pub fn query_service_info() -> (bool, bool) {
    (|| -> Option<(bool, bool)> {
        let manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).ok()?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
            .ok()?;
        let status = service.query_status().ok()?;
        Some((true, status.current_state == ServiceState::Running))
    })()
    .unwrap_or((false, false))
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

/// Register the Windows Event Log source so Event Viewer can find the
/// message-table resource embedded by the `winlog` crate.
///
/// Sets `EventMessageFile` to the installed binary path and
/// `TypesSupported` to allow Error, Warning, and Information events.
fn register_event_source(exe_path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm.create_subkey(EVENTLOG_REG_KEY)?;
    key.set_value("EventMessageFile", &exe_path.to_string_lossy().as_ref())?;
    // EVENTLOG_ERROR_TYPE | EVENTLOG_WARNING_TYPE | EVENTLOG_INFORMATION_TYPE
    key.set_value("TypesSupported", &7u32)?;
    info!("Event log source registered: {}", exe_path.display());
    Ok(())
}

/// Remove the Event Log source registry key (best-effort, non-fatal).
fn deregister_event_source() {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(parent) = hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Services\EventLog\Application",
        KEY_WRITE,
    ) {
        match parent.delete_subkey(SERVICE_NAME) {
            Ok(()) => info!("Event log source deregistered"),
            Err(e) => warn!("Could not deregister event log source: {}", e),
        }
    }
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
