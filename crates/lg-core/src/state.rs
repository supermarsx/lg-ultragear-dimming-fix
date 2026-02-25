//! App state helpers (snapshots, diagnostics, guardrails, recovery).

use crate::config::{self, Config};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const MAX_AUTO_SNAPSHOTS: usize = 24;

pub fn state_dir() -> PathBuf {
    config::config_dir().join("state")
}

pub fn snapshots_dir() -> PathBuf {
    state_dir().join("snapshots")
}

pub fn diagnostics_log_path() -> PathBuf {
    state_dir().join("diagnostics.log")
}

pub fn guardrails_path() -> PathBuf {
    state_dir().join("ddc_guardrails.toml")
}

pub fn recovery_state_path() -> PathBuf {
    state_dir().join("last_good.toml")
}

pub fn ab_compare_state_path() -> PathBuf {
    state_dir().join("ab_compare.toml")
}

pub fn automation_config_path() -> PathBuf {
    state_dir().join("automation.toml")
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn ensure_parent(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn sanitize_log_field(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

pub fn windows_color_directory() -> PathBuf {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    PathBuf::from(windir)
        .join("System32")
        .join("spool")
        .join("drivers")
        .join("color")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DdcGuardrails {
    pub enabled: bool,
    pub min_brightness: u32,
    pub max_brightness: u32,
    pub confirm_risky_writes: bool,
}

impl Default for DdcGuardrails {
    fn default() -> Self {
        Self {
            enabled: false,
            min_brightness: 15,
            max_brightness: 90,
            confirm_risky_writes: true,
        }
    }
}

impl DdcGuardrails {
    pub fn sanitized(self) -> Self {
        let min = self.min_brightness.min(100);
        let max = self.max_brightness.min(100).max(min);
        Self {
            enabled: self.enabled,
            min_brightness: min,
            max_brightness: max,
            confirm_risky_writes: self.confirm_risky_writes,
        }
    }
}

pub fn load_ddc_guardrails() -> DdcGuardrails {
    let path = guardrails_path();
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str::<DdcGuardrails>(&text)
            .map(|g| g.sanitized())
            .unwrap_or_default(),
        Err(_) => DdcGuardrails::default(),
    }
}

pub fn save_ddc_guardrails(guardrails: &DdcGuardrails) -> Result<(), Box<dyn std::error::Error>> {
    let path = guardrails_path();
    ensure_parent(&path)?;
    let text = toml::to_string_pretty(&guardrails.clone().sanitized())?;
    fs::write(path, text)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DiagnosticEvent {
    pub timestamp: String,
    pub source: String,
    pub level: String,
    pub event: String,
    pub details: String,
}

pub fn append_diagnostic_event(source: &str, level: &str, event: &str, details: &str) {
    let path = diagnostics_log_path();
    if ensure_parent(&path).is_err() {
        return;
    }
    let line = format!(
        "{}\t{}\t{}\t{}\t{}",
        now_iso(),
        sanitize_log_field(source),
        sanitize_log_field(level),
        sanitize_log_field(event),
        sanitize_log_field(details)
    );
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", line);
    }
}

fn parse_diagnostic_line(line: &str) -> Option<DiagnosticEvent> {
    let mut parts = line.splitn(5, '\t');
    let timestamp = parts.next()?.to_string();
    let source = parts.next()?.to_string();
    let level = parts.next()?.to_string();
    let event = parts.next()?.to_string();
    let details = parts.next()?.to_string();
    Some(DiagnosticEvent {
        timestamp,
        source,
        level,
        event,
        details,
    })
}

pub fn read_recent_diagnostic_events(
    limit: usize,
) -> Result<Vec<DiagnosticEvent>, Box<dyn std::error::Error>> {
    let path = diagnostics_log_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut parsed = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| parse_diagnostic_line(&line))
        .collect::<Vec<_>>();
    parsed.reverse();
    if parsed.len() > limit {
        parsed.truncate(limit);
    }
    Ok(parsed)
}

pub fn latest_success_timestamp() -> Option<String> {
    read_recent_diagnostic_events(200).ok().and_then(|entries| {
        entries
            .into_iter()
            .find(|e| e.event == "apply_success")
            .map(|e| e.timestamp)
    })
}

pub fn clear_diagnostics_log() -> Result<(), Box<dyn std::error::Error>> {
    let path = diagnostics_log_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileSnapshot {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub created_at: String,
    pub profile_name: String,
    pub active_preset: String,
    pub tuning_preset: String,
    pub gamma: f64,
    pub luminance_cd_m2: f64,
    pub per_monitor_profiles: bool,
    pub ddc_brightness: Option<u32>,
    pub profile_file: String,
    pub notes: String,
}

impl Default for ProfileSnapshot {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            kind: "manual".to_string(),
            created_at: String::new(),
            profile_name: String::new(),
            active_preset: String::new(),
            tuning_preset: String::new(),
            gamma: 2.2,
            luminance_cd_m2: 120.0,
            per_monitor_profiles: false,
            ddc_brightness: None,
            profile_file: String::new(),
            notes: String::new(),
        }
    }
}

fn snapshot_meta_path(id: &str) -> PathBuf {
    snapshots_dir().join(format!("{}.toml", id))
}

pub fn snapshot_profile_path(snapshot: &ProfileSnapshot) -> PathBuf {
    snapshots_dir().join(&snapshot.profile_file)
}

fn snapshot_id() -> String {
    Utc::now().format("%Y%m%d-%H%M%S-%3f").to_string()
}

fn trim_label(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        "Snapshot".to_string()
    } else {
        trimmed.to_string()
    }
}

fn prune_auto_snapshots() -> Result<(), Box<dyn std::error::Error>> {
    let mut autos = list_profile_snapshots()?
        .into_iter()
        .filter(|s| s.kind == "auto")
        .collect::<Vec<_>>();
    if autos.len() <= MAX_AUTO_SNAPSHOTS {
        return Ok(());
    }
    autos.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    for snapshot in autos.into_iter().skip(MAX_AUTO_SNAPSHOTS) {
        let _ = fs::remove_file(snapshot_meta_path(&snapshot.id));
        let _ = fs::remove_file(snapshot_profile_path(&snapshot));
    }
    Ok(())
}

pub fn create_profile_snapshot(
    cfg: &Config,
    label: &str,
    kind: &str,
    profile_path: &Path,
    ddc_brightness: Option<u32>,
    notes: &str,
) -> Result<ProfileSnapshot, Box<dyn std::error::Error>> {
    if !profile_path.exists() {
        return Err(format!("profile file not found: {}", profile_path.display()).into());
    }
    let snapshot_dir = snapshots_dir();
    if !snapshot_dir.exists() {
        fs::create_dir_all(&snapshot_dir)?;
    }
    let id = snapshot_id();
    let profile_file = format!("{}.icm", id);
    let profile_copy = snapshot_dir.join(&profile_file);
    fs::copy(profile_path, &profile_copy)?;

    let resolved_profile_name = profile_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| cfg.profile_name.clone());

    let snapshot = ProfileSnapshot {
        id: id.clone(),
        label: trim_label(label),
        kind: if kind.trim().is_empty() {
            "manual".to_string()
        } else {
            kind.trim().to_string()
        },
        created_at: now_iso(),
        profile_name: resolved_profile_name,
        active_preset: cfg.icc_active_preset.clone(),
        tuning_preset: cfg.icc_tuning_preset.clone(),
        gamma: cfg.icc_gamma,
        luminance_cd_m2: cfg.icc_luminance_cd_m2,
        per_monitor_profiles: cfg.icc_per_monitor_profiles,
        ddc_brightness,
        profile_file,
        notes: notes.trim().to_string(),
    };

    let meta_text = toml::to_string_pretty(&snapshot)?;
    fs::write(snapshot_meta_path(&id), meta_text)?;

    if snapshot.kind == "auto" {
        let _ = prune_auto_snapshots();
    }
    Ok(snapshot)
}

pub fn list_profile_snapshots() -> Result<Vec<ProfileSnapshot>, Box<dyn std::error::Error>> {
    let dir = snapshots_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(snapshot) = toml::from_str::<ProfileSnapshot>(&text) {
                out.push(snapshot);
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

pub fn load_profile_snapshot(id: &str) -> Result<ProfileSnapshot, Box<dyn std::error::Error>> {
    let path = snapshot_meta_path(id);
    let text = fs::read_to_string(path)?;
    Ok(toml::from_str::<ProfileSnapshot>(&text)?)
}

pub fn restore_profile_snapshot(
    snapshot: &ProfileSnapshot,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let src = snapshot_profile_path(snapshot);
    if !src.exists() {
        return Err(format!("snapshot profile missing: {}", src.display()).into());
    }
    let color_dir = windows_color_directory();
    if !color_dir.exists() {
        fs::create_dir_all(&color_dir)?;
    }
    let dst = color_dir.join(&snapshot.profile_name);
    fs::copy(src, &dst)?;
    Ok(dst)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RecoveryState {
    pub created_at: String,
    pub snapshot_id: String,
    pub profile_name: String,
    pub ddc_brightness: Option<u32>,
    pub notes: String,
}

pub fn save_recovery_state(state: &RecoveryState) -> Result<(), Box<dyn std::error::Error>> {
    let path = recovery_state_path();
    ensure_parent(&path)?;
    let text = toml::to_string_pretty(state)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn load_recovery_state() -> Option<RecoveryState> {
    let path = recovery_state_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<RecoveryState>(&text).ok())
}

pub fn mark_snapshot_last_good(
    snapshot: &ProfileSnapshot,
    ddc_brightness: Option<u32>,
    notes: &str,
) -> Result<RecoveryState, Box<dyn std::error::Error>> {
    let state = RecoveryState {
        created_at: now_iso(),
        snapshot_id: snapshot.id.clone(),
        profile_name: snapshot.profile_name.clone(),
        ddc_brightness,
        notes: notes.trim().to_string(),
    };
    save_recovery_state(&state)?;
    Ok(state)
}

pub fn load_last_good_snapshot() -> Result<Option<ProfileSnapshot>, Box<dyn std::error::Error>> {
    let Some(state) = load_recovery_state() else {
        return Ok(None);
    };
    if state.snapshot_id.is_empty() {
        return Ok(None);
    }
    let snapshot = load_profile_snapshot(&state.snapshot_id)?;
    Ok(Some(snapshot))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AbCompareState {
    pub enabled: bool,
    pub current_side: String,
    pub profile_a_path: String,
    pub profile_b_path: String,
    pub profile_a_label: String,
    pub profile_b_label: String,
    pub last_switch_at: String,
}

impl Default for AbCompareState {
    fn default() -> Self {
        Self {
            enabled: false,
            current_side: "A".to_string(),
            profile_a_path: String::new(),
            profile_b_path: String::new(),
            profile_a_label: "Current".to_string(),
            profile_b_label: "Baseline".to_string(),
            last_switch_at: String::new(),
        }
    }
}

pub fn load_ab_compare_state() -> AbCompareState {
    let path = ab_compare_state_path();
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str::<AbCompareState>(&text).unwrap_or_default(),
        Err(_) => AbCompareState::default(),
    }
}

pub fn save_ab_compare_state(state: &AbCompareState) -> Result<(), Box<dyn std::error::Error>> {
    let path = ab_compare_state_path();
    ensure_parent(&path)?;
    let text = toml::to_string_pretty(state)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn clear_ab_compare_state() -> Result<(), Box<dyn std::error::Error>> {
    let path = ab_compare_state_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppProfileRule {
    pub enabled: bool,
    pub name: String,
    pub process_pattern: String,
    pub preset: String,
    pub tuning_preset: String,
    pub luminance_override: Option<f64>,
    pub ddc_brightness: Option<u32>,
    pub priority: i32,
}

impl Default for AppProfileRule {
    fn default() -> Self {
        Self {
            enabled: true,
            name: "Example Rule".to_string(),
            process_pattern: "examplegame".to_string(),
            preset: "reader".to_string(),
            tuning_preset: "reader_balanced".to_string(),
            luminance_override: None,
            ddc_brightness: None,
            priority: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientAutomationConfig {
    pub enabled: bool,
    pub sensor_enabled: bool,
    pub sensor_method: String,
    pub sensor_command: String,
    pub sensor_poll_interval_ms: u64,
    pub sensor_timeout_ms: u64,
    pub sensor_scale: f64,
    pub sensor_offset: f64,
    pub sensor_smoothing_alpha: f64,
    pub lux_day_threshold: f64,
    pub lux_night_threshold: f64,
    pub lux_hysteresis: f64,
    pub day_preset: String,
    pub evening_preset: String,
    pub night_preset: String,
    pub unknown_sensor_preset: String,
    pub day_start: String,
    pub evening_start: String,
    pub night_start: String,
    pub allow_time_fallback: bool,
    pub day_luminance_cd_m2: Option<f64>,
    pub evening_luminance_cd_m2: Option<f64>,
    pub night_luminance_cd_m2: Option<f64>,
}

impl Default for AmbientAutomationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sensor_enabled: true,
            sensor_method: "ddc_brightness".to_string(),
            sensor_command: String::new(),
            sensor_poll_interval_ms: 4000,
            sensor_timeout_ms: 1500,
            sensor_scale: 1.0,
            sensor_offset: 0.0,
            sensor_smoothing_alpha: 0.35,
            lux_day_threshold: 220.0,
            lux_night_threshold: 80.0,
            lux_hysteresis: 10.0,
            day_preset: "gamma22".to_string(),
            evening_preset: "reader".to_string(),
            night_preset: "reader".to_string(),
            unknown_sensor_preset: "reader".to_string(),
            day_start: "08:00".to_string(),
            evening_start: "18:00".to_string(),
            night_start: "22:30".to_string(),
            allow_time_fallback: true,
            day_luminance_cd_m2: Some(180.0),
            evening_luminance_cd_m2: Some(140.0),
            night_luminance_cd_m2: Some(110.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppRulesConfig {
    pub enabled: bool,
    pub poll_interval_ms: u64,
    pub match_mode: String,
    pub case_sensitive: bool,
    pub override_ambient: bool,
    pub rules: Vec<AppProfileRule>,
}

impl Default for AppRulesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_ms: 3000,
            match_mode: "contains".to_string(),
            case_sensitive: false,
            override_ambient: true,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayQuickConfig {
    pub enabled: bool,
    pub auto_start_with_watch: bool,
    pub tooltip: String,
    pub menu_refresh_ms: u64,
    pub brightness_step: u32,
    pub show_apply_action: bool,
    pub show_reader_toggle: bool,
    pub show_ab_toggle: bool,
    pub show_brightness_controls: bool,
    pub show_exit_action: bool,
}

impl Default for TrayQuickConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_start_with_watch: false,
            tooltip: "LG UltraGear Quick Mode".to_string(),
            menu_refresh_ms: 5000,
            brightness_step: 10,
            show_apply_action: true,
            show_reader_toggle: true,
            show_ab_toggle: true,
            show_brightness_controls: true,
            show_exit_action: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthSelfHealConfig {
    pub enabled: bool,
    pub startup_self_heal: bool,
    pub wake_self_heal: bool,
    pub run_every_event: bool,
    pub verify_profile_presence: bool,
    pub regenerate_if_missing: bool,
    pub cleanup_stale_profiles: bool,
}

impl Default for HealthSelfHealConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            startup_self_heal: true,
            wake_self_heal: true,
            run_every_event: true,
            verify_profile_presence: true,
            regenerate_if_missing: true,
            cleanup_stale_profiles: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DdcSafetyConfig {
    pub rollback_timer_enabled: bool,
    pub rollback_timeout_ms: u64,
    pub keep_key: String,
    pub risky_vcp_codes: String,
    pub require_confirm_before_risky: bool,
}

impl Default for DdcSafetyConfig {
    fn default() -> Self {
        Self {
            rollback_timer_enabled: true,
            rollback_timeout_ms: 15000,
            keep_key: "K".to_string(),
            risky_vcp_codes: "04,06,0A,60,D6,DC,14".to_string(),
            require_confirm_before_risky: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub collect_latency: bool,
    pub collect_success_rate: bool,
    pub rolling_window: usize,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collect_latency: true,
            collect_success_rate: true,
            rolling_window: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AutomationConfig {
    pub ambient: AmbientAutomationConfig,
    pub app_rules: AppRulesConfig,
    pub tray: TrayQuickConfig,
    pub health: HealthSelfHealConfig,
    pub ddc_safety: DdcSafetyConfig,
    pub metrics: MetricsConfig,
}

impl AutomationConfig {
    pub fn sanitized(mut self) -> Self {
        self.ambient.sensor_poll_interval_ms = self.ambient.sensor_poll_interval_ms.max(500);
        self.ambient.sensor_timeout_ms = self.ambient.sensor_timeout_ms.max(100);
        self.ambient.sensor_smoothing_alpha = self.ambient.sensor_smoothing_alpha.clamp(0.0, 1.0);
        self.ambient.lux_hysteresis = self.ambient.lux_hysteresis.max(0.0);
        self.app_rules.poll_interval_ms = self.app_rules.poll_interval_ms.max(500);
        self.tray.brightness_step = self.tray.brightness_step.clamp(1, 100);
        self.ddc_safety.rollback_timeout_ms = self.ddc_safety.rollback_timeout_ms.max(1000);
        self.metrics.rolling_window = self.metrics.rolling_window.max(1);
        self
    }
}

pub fn load_automation_config() -> AutomationConfig {
    let path = automation_config_path();
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str::<AutomationConfig>(&text)
            .map(|cfg| cfg.sanitized())
            .unwrap_or_default(),
        Err(_) => AutomationConfig::default(),
    }
}

pub fn save_automation_config(cfg: &AutomationConfig) -> Result<(), Box<dyn std::error::Error>> {
    let path = automation_config_path();
    ensure_parent(&path)?;
    fs::write(path, toml::to_string_pretty(&cfg.clone().sanitized())?)?;
    Ok(())
}

pub fn risky_vcp_codes_from_csv(csv: &str) -> Vec<u8> {
    csv.split(',')
        .filter_map(|part| {
            let t = part.trim();
            if t.is_empty() {
                return None;
            }
            let normalized = t
                .strip_prefix("0x")
                .or_else(|| t.strip_prefix("0X"))
                .unwrap_or(t);
            u8::from_str_radix(normalized, 16)
                .ok()
                .or_else(|| normalized.parse::<u8>().ok())
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct ApplyLatencyMetrics {
    pub samples: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub avg_ms: f64,
    pub p95_ms: u64,
    pub last_ms: u64,
}

pub fn compute_apply_latency_metrics(window: usize) -> ApplyLatencyMetrics {
    let mut values: Vec<(u64, bool)> = read_recent_diagnostic_events(window.saturating_mul(4))
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.event == "apply_latency")
        .filter_map(|e| {
            let mut ms: Option<u64> = None;
            let mut success = true;
            for token in e.details.split_whitespace() {
                if let Some(v) = token.strip_prefix("ms=") {
                    ms = v.parse::<u64>().ok();
                } else if let Some(v) = token.strip_prefix("success=") {
                    success = matches!(v, "1" | "true" | "yes");
                }
            }
            ms.map(|v| (v, success))
        })
        .collect();

    if values.is_empty() {
        return ApplyLatencyMetrics::default();
    }

    values.truncate(window.max(1));
    let samples = values.len();
    let success_count = values.iter().filter(|(_, ok)| *ok).count();
    let failure_count = samples.saturating_sub(success_count);
    let sum = values.iter().map(|(ms, _)| *ms as f64).sum::<f64>();
    let avg_ms = sum / samples as f64;
    let last_ms = values.first().map(|(ms, _)| *ms).unwrap_or(0);
    let mut sorted = values.iter().map(|(ms, _)| *ms).collect::<Vec<_>>();
    sorted.sort_unstable();
    let p95_idx = ((sorted.len() as f64 - 1.0) * 0.95).round() as usize;
    let p95_ms = sorted
        .get(p95_idx.min(sorted.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0);

    ApplyLatencyMetrics {
        samples,
        success_count,
        failure_count,
        avg_ms,
        p95_ms,
        last_ms,
    }
}

#[cfg(test)]
#[path = "tests/state_tests.rs"]
mod tests;
