//! LG UltraGear — display color profile CLI tool.
//!
//! A full-featured command-line tool for managing ICC color profiles on
//! LG UltraGear displays. Prevents dimming by reapplying a calibrated
//! profile on display connect, session unlock, and logon events.
//!
//! Can also run as a Windows service for always-on monitoring.

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal;
use lg_core::{
    config::{self, Config},
    state as app_state,
};
use std::error::Error;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod elevation;
mod tui;

#[derive(Parser)]
#[command(
    name = "lg-ultragear-dimming-fix",
    version = env!("APP_VERSION"),
    about = "LG UltraGear display color profile manager",
    long_about = "Prevents display dimming by managing ICC color profiles.\n\n\
        Reapplies a calibrated color profile on display connect, session unlock,\n\
        and logon events using a toggle approach (disassociate → reassociate)\n\
        to force Windows to reload the profile."
)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Simulate operations without making changes
    #[arg(long, global = true)]
    dry_run: bool,

    /// Force non-interactive CLI mode (skip TUI)
    #[arg(long, global = true)]
    non_interactive: bool,

    /// Do not auto-elevate to administrator
    #[arg(long, global = true)]
    skip_elevation: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install color profile and/or service
    Install {
        /// Monitor name pattern (case-insensitive substring match)
        #[arg(short, long)]
        pattern: Option<String>,

        /// Use regex pattern matching instead of substring
        #[arg(long)]
        regex: bool,

        /// Install ICC profile only (no service)
        #[arg(long, conflicts_with = "service_only")]
        profile_only: bool,

        /// Install service only (skip explicit profile extraction)
        #[arg(long, conflicts_with = "profile_only")]
        service_only: bool,

        /// Path to a custom ICC/ICM profile (uses dynamic generated profile by default)
        #[arg(long)]
        profile_path: Option<String>,

        /// Also associate profile in per-user scope (default: system-wide only)
        #[arg(long)]
        per_user: bool,

        /// Skip HDR/advanced-color association
        #[arg(long)]
        skip_hdr: bool,

        /// Skip hash check — always overwrite profile in color store
        #[arg(long)]
        skip_hash_check: bool,

        /// Force overwrite even if profile and service already exist
        #[arg(long)]
        force: bool,

        /// Skip monitor detection during install
        #[arg(long)]
        skip_detect: bool,
    },

    /// Uninstall service and/or profile
    Uninstall {
        /// Remove everything (service + profile + config)
        #[arg(long)]
        full: bool,

        /// Also remove the ICC profile from color store
        #[arg(long)]
        profile: bool,
    },

    /// Clean reinstall (uninstall then install)
    Reinstall {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,

        /// Use regex pattern matching instead of substring
        #[arg(long)]
        regex: bool,
    },

    /// Detect connected monitors matching a pattern
    Detect {
        /// Monitor name pattern (case-insensitive substring match)
        #[arg(short, long)]
        pattern: Option<String>,

        /// Use regex pattern matching instead of substring
        #[arg(long)]
        regex: bool,
    },

    /// One-shot profile reapply for matching monitors
    Apply {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,

        /// Use regex pattern matching instead of substring
        #[arg(long)]
        regex: bool,

        /// Path to a custom ICC/ICM profile
        #[arg(long)]
        profile_path: Option<String>,

        /// Also associate profile in per-user scope
        #[arg(long)]
        per_user: bool,

        /// Skip HDR/advanced-color association
        #[arg(long)]
        skip_hdr: bool,

        /// Enable toast notification for this run
        #[arg(long, conflicts_with = "no_toast")]
        toast: bool,

        /// Disable toast notification for this run
        #[arg(long, conflicts_with = "toast")]
        no_toast: bool,
    },

    /// Run event watcher in foreground (Ctrl+C to stop)
    Watch {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,

        /// Use regex pattern matching instead of substring
        #[arg(long)]
        regex: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Windows service management (advanced)
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },

    /// Run diagnostic tests
    Test {
        #[command(subcommand)]
        action: TestAction,
    },

    /// ICC conversion, inspection, validation, and tag manipulation utilities
    Icc {
        #[command(subcommand)]
        action: IccAction,
    },

    /// DDC/CI monitor control (brightness, color presets, display mode, resets)
    Ddc {
        #[command(subcommand)]
        action: DdcAction,
    },

    /// Automation engine (ambient sensor + per-app rules + self-heal settings)
    Automation {
        #[command(subcommand)]
        action: AutomationAction,
    },

    /// Quick tray mode management
    Tray {
        #[command(subcommand)]
        action: TrayAction,
    },

    /// Export/import full app bundle (config + state + profiles)
    Bundle {
        #[command(subcommand)]
        action: BundleAction,
    },

    /// Probe monitors and profile status (alias for detect with extra info)
    Probe {
        /// Monitor name pattern
        #[arg(short, long)]
        pattern: Option<String>,

        /// Use regex pattern matching instead of substring
        #[arg(long)]
        regex: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Print config file path
    Path,
    /// Reset config to defaults
    Reset,
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install the Windows service
    Install {
        /// Monitor name pattern
        #[arg(short, long)]
        pattern: Option<String>,

        /// Custom service name (default: lg-ultragear-color-svc)
        #[arg(long)]
        service_name: Option<String>,
    },
    /// Uninstall the Windows service
    Uninstall,
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Show service status
    Status,
    /// Run as Windows service (SCM dispatch — do not call directly)
    Run,
}

#[derive(Subcommand)]
enum TestAction {
    /// Send a test toast notification
    Toast {
        /// Custom title for test notification
        #[arg(long, default_value = "LG UltraGear Test")]
        title: String,

        /// Custom body for test notification
        #[arg(long, default_value = "Toast notification is working ✓")]
        body: String,
    },
    /// Verify ICC profile integrity (hash check)
    Profile,
    /// Test monitor detection
    Monitors {
        /// Monitor name pattern
        #[arg(short, long)]
        pattern: Option<String>,

        /// Use regex pattern matching instead of substring
        #[arg(long)]
        regex: bool,
    },
}

#[derive(Subcommand)]
enum AutomationAction {
    /// Show current automation configuration
    Show,
    /// Print automation config file path
    Path,
    /// Reset automation config to defaults
    Reset,
    /// Evaluate automation rules once and apply profile/DDC as needed
    ApplyNow,
}

#[derive(Subcommand)]
enum TrayAction {
    /// Run a configurable quick tray mode (PowerShell NotifyIcon host)
    Run,
}

#[derive(Subcommand)]
enum BundleAction {
    /// Export config/state/profile artifacts into a folder bundle
    Export {
        /// Output directory for bundle files
        #[arg(short, long)]
        output: String,
    },
    /// Import a previously exported bundle folder
    Import {
        /// Input directory containing bundle files
        #[arg(short, long)]
        input: String,
    },
}

#[derive(Subcommand)]
enum DdcAction {
    /// Set brightness on all monitors (or use --pattern to target one)
    Brightness {
        /// Brightness value (0–100)
        value: u32,

        /// Monitor name pattern (targets specific monitor)
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Read color preset (VCP 0x14) from the target monitor
    ColorPreset {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Set color preset (VCP 0x14)
    SetColorPreset {
        /// Preset value (1=sRGB, 2=Native, 4=4000K, 5=5000K, 6=6500K, 8=7500K, 10=9300K, 11=User1)
        value: u32,

        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Read display mode / picture mode (VCP 0xDC)
    DisplayMode {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Set display mode / picture mode (VCP 0xDC)
    SetDisplayMode {
        /// Mode value
        value: u32,

        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Reset brightness and contrast to factory defaults (VCP 0x06)
    ResetBrightnessContrast {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Reset color to factory defaults (VCP 0x0A)
    ResetColor {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Read VCP version from the target monitor (VCP 0xDF)
    Version {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Read any VCP code (advanced)
    GetVcp {
        /// VCP code in hex (e.g. 10, 14, DC)
        #[arg(value_parser = parse_hex_u8)]
        code: u8,

        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Write any VCP code (advanced — use with caution)
    SetVcp {
        /// VCP code in hex (e.g. 10, 14, DC)
        #[arg(value_parser = parse_hex_u8)]
        code: u8,

        /// Value to write
        value: u32,

        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// List all physical monitors visible via DDC/CI
    List,
    /// Build a DDC capability map by probing known VCP codes
    Map,
}

#[derive(Subcommand)]
enum IccAction {
    /// Generate a binary ICC profile from a TOML config file
    FromToml {
        /// TOML config path (defaults to app config.toml)
        #[arg(short, long)]
        input: Option<String>,
        /// Output ICC/ICM file path
        #[arg(short, long)]
        output: String,
        /// Optional preset override (gamma22|gamma24|reader|custom)
        #[arg(long)]
        preset: Option<String>,
        /// Optional gamma override
        #[arg(long)]
        gamma: Option<f64>,
        /// Optional luminance override (cd/m^2)
        #[arg(long)]
        luminance: Option<f64>,
        /// Optional monitor model/name to embed
        #[arg(long)]
        monitor_name: Option<String>,
        /// Optional monitor serial to embed
        #[arg(long)]
        serial: Option<String>,
        /// Optional manufacturer id to embed
        #[arg(long)]
        manufacturer_id: Option<String>,
        /// Optional product code to embed
        #[arg(long)]
        product_code: Option<String>,
        /// Optional device key to embed
        #[arg(long)]
        device_key: Option<String>,
    },
    /// Validate an ICC profile and print warnings/errors
    Validate {
        /// ICC/ICM file to validate
        #[arg(short, long)]
        input: String,
        /// Print per-tag detailed validation context
        #[arg(long)]
        detailed: bool,
    },
    /// Inspect ICC profile metadata and tags
    Inspect {
        /// ICC/ICM file to inspect
        #[arg(short, long)]
        input: String,
        /// Print per-tag details (type signature, size, known/unknown)
        #[arg(long)]
        detailed: bool,
    },
    /// Normalize/rewrite ICC bytes via parser round-trip
    Normalize {
        /// Input ICC/ICM file
        #[arg(short, long)]
        input: String,
        /// Output ICC/ICM file
        #[arg(short, long)]
        output: String,
    },
    /// Set or replace a raw ICC tag payload
    SetTag {
        /// Input ICC/ICM file
        #[arg(short, long)]
        input: String,
        /// Output ICC/ICM file
        #[arg(short, long)]
        output: String,
        /// Tag signature (4-char ICC signature)
        #[arg(long)]
        signature: String,
        /// Optional type signature wrapper (e.g. data/text/meta). Defaults to raw payload as-is.
        #[arg(long)]
        type_signature: Option<String>,
        /// Hex payload bytes (without wrapper unless --type-signature is used)
        #[arg(long, conflicts_with = "payload_text")]
        payload_hex: Option<String>,
        /// UTF-8 payload text (without wrapper unless --type-signature is used)
        #[arg(long, conflicts_with = "payload_hex")]
        payload_text: Option<String>,
    },
    /// Remove an ICC tag by signature
    RemoveTag {
        /// Input ICC/ICM file
        #[arg(short, long)]
        input: String,
        /// Output ICC/ICM file
        #[arg(short, long)]
        output: String,
        /// Tag signature (4-char ICC signature)
        #[arg(long)]
        signature: String,
    },
    /// Import an X-Rite i1Profiler-exported ICC profile (validate + normalize)
    ImportI1 {
        /// Input ICC/ICM file exported by i1Profiler
        #[arg(short, long)]
        input: String,
        /// Output ICC/ICM file path (defaults next to input with .normalized.icc suffix)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Generate optimized ICC from presets/tuning and optionally apply immediately
    Optimize {
        /// Optional TOML config path (defaults to app config.toml)
        #[arg(short, long)]
        input: Option<String>,
        /// Override tuning preset (manual|anti_dim_soft|anti_dim_balanced|anti_dim_aggressive|anti_dim_night|reader_balanced|color_rgb_full|color_rgb_limited|color_ycbcr444|color_ycbcr422|color_ycbcr420|color_bt2020_pq|unyellow_soft|unyellow_balanced|unyellow_aggressive|black_depth|white_clarity|anti_fade_punch|anti_fade_cinematic)
        #[arg(long)]
        tuning_preset: Option<String>,
        /// Optional gamma override
        #[arg(long)]
        gamma: Option<f64>,
        /// Optional luminance override (cd/m^2)
        #[arg(long)]
        luminance: Option<f64>,
        /// Apply generated profile to matching monitors immediately
        #[arg(long)]
        apply: bool,
        /// Override monitor pattern used when --apply is set
        #[arg(short, long)]
        pattern: Option<String>,
        /// Use regex pattern matching when --apply is set
        #[arg(long)]
        regex: bool,
        /// Persist overrides back to config.toml (only when using default config path)
        #[arg(long)]
        save_config: bool,
        /// Optional output file to export generated ICC bytes
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// Parse a hex string (with or without 0x prefix) into a u8.
fn parse_hex_u8(s: &str) -> Result<u8, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u8::from_str_radix(s, 16).map_err(|e| format!("Invalid hex byte '{}': {}", s, e))
}

fn parse_hex_bytes(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let compact = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect::<String>();
    if compact.is_empty() {
        return Ok(Vec::new());
    }
    if compact.len() % 2 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "hex payload must have an even number of digits",
        )
        .into());
    }
    let mut out = Vec::with_capacity(compact.len() / 2);
    let bytes = compact.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let pair = std::str::from_utf8(&bytes[i..i + 2]).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid UTF-8 in hex payload: {}", e),
            )
        })?;
        let b = u8::from_str_radix(pair, 16).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid hex byte '{}': {}", pair, e),
            )
        })?;
        out.push(b);
    }
    Ok(out)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Set console to UTF-8 early — before any output or elevation relaunch.
    // This ensures box-drawing characters render correctly even in cmd.exe
    // or legacy PowerShell that default to OEM code pages (437/850).
    tui::enable_utf8_console();

    let cli = Cli::parse();

    // SCM dispatch — must happen before any logger initialization
    if matches!(
        &cli.command,
        Some(Commands::Service {
            action: ServiceAction::Run
        })
    ) {
        winlog::init("lg-ultragear-color-svc").ok();
        return lg_service::run();
    }

    // No subcommand → interactive TUI (unless --non-interactive or not a terminal)
    if cli.command.is_none() {
        if !cli.non_interactive && std::io::stdout().is_terminal() {
            // Auto-elevate for TUI mode (profile + service install needs admin)
            if !cli.skip_elevation && !elevation::is_elevated() {
                println!("[INFO] Requesting administrator privileges...");
                elevation::relaunch_elevated()?;
            }
            return tui::run();
        }
        // Non-interactive or not a terminal → show help
        use clap::CommandFactory;
        Cli::command().print_help()?;
        println!();
        return Ok(());
    }

    // Auto-elevate for commands that need admin privileges
    if !cli.skip_elevation && !cli.dry_run {
        let needs_admin = matches!(
            &cli.command,
            Some(Commands::Install { .. })
                | Some(Commands::Uninstall { .. })
                | Some(Commands::Reinstall { .. })
                | Some(Commands::Apply { .. })
                | Some(Commands::Watch { .. })
                | Some(Commands::Service { .. })
        );
        if needs_admin && !elevation::is_elevated() {
            println!("[INFO] Requesting administrator privileges...");
            elevation::relaunch_elevated()?;
        }
    }

    // CLI mode — console logger
    env_logger::Builder::new()
        .filter_level(if cli.verbose {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Warn
        })
        .format_timestamp(None)
        .init();

    match cli.command {
        None => unreachable!(), // handled above
        Some(Commands::Install {
            pattern,
            regex,
            profile_only,
            service_only,
            profile_path,
            per_user,
            skip_hdr,
            skip_hash_check,
            force,
            skip_detect,
        }) => cmd_install(InstallOpts {
            pattern,
            regex,
            profile_only,
            service_only,
            custom_profile: profile_path,
            per_user,
            skip_hdr,
            skip_hash_check,
            force,
            skip_detect,
            dry_run: cli.dry_run,
        })?,
        Some(Commands::Uninstall { full, profile }) => cmd_uninstall(full, profile, cli.dry_run)?,
        Some(Commands::Reinstall { pattern, regex }) => cmd_reinstall(pattern, regex, cli.dry_run)?,
        Some(Commands::Detect { pattern, regex }) => cmd_detect(pattern, regex)?,
        Some(Commands::Apply {
            pattern,
            regex,
            profile_path,
            per_user,
            skip_hdr,
            toast,
            no_toast,
        }) => cmd_apply(ApplyOpts {
            pattern,
            regex,
            profile_path,
            per_user,
            skip_hdr,
            toast,
            no_toast,
            verbose: cli.verbose,
            dry_run: cli.dry_run,
        })?,
        Some(Commands::Watch { pattern, regex }) => cmd_watch(pattern, regex)?,
        Some(Commands::Config { action }) => cmd_config(action)?,
        Some(Commands::Service { action }) => cmd_service(action)?,
        Some(Commands::Test { action }) => cmd_test(action)?,
        Some(Commands::Icc { action }) => cmd_icc(action, cli.dry_run)?,
        Some(Commands::Ddc { action }) => cmd_ddc(action, cli.dry_run)?,
        Some(Commands::Automation { action }) => cmd_automation(action, cli.dry_run)?,
        Some(Commands::Tray { action }) => cmd_tray(action, cli.dry_run)?,
        Some(Commands::Bundle { action }) => cmd_bundle(action, cli.dry_run)?,
        Some(Commands::Probe { pattern, regex }) => cmd_probe(pattern, regex)?,
    }

    Ok(())
}

// ============================================================================
// Command implementations
// ============================================================================

fn resolve_active_profile_path(cfg: &Config) -> std::path::PathBuf {
    resolve_active_profile_path_for_mode(cfg, false)
}

fn tuning_from_config(cfg: &Config) -> lg_profile::DynamicIccTuning {
    let manual = lg_profile::DynamicIccTuning {
        black_lift: cfg.icc_black_lift,
        midtone_boost: cfg.icc_midtone_boost,
        white_compression: cfg.icc_white_compression,
        gamma_r: cfg.icc_gamma_r,
        gamma_g: cfg.icc_gamma_g,
        gamma_b: cfg.icc_gamma_b,
        vcgt_enabled: cfg.icc_vcgt_enabled,
        vcgt_strength: cfg.icc_vcgt_strength,
        target_black_cd_m2: cfg.icc_target_black_cd_m2,
        include_media_black_point: cfg.icc_include_media_black_point,
        include_device_descriptions: cfg.icc_include_device_descriptions,
        include_characterization_target: cfg.icc_include_characterization_target,
        include_viewing_cond_desc: cfg.icc_include_viewing_cond_desc,
        technology_signature: lg_profile::parse_icc_signature_or_zero(
            &cfg.icc_technology_signature,
        ),
        ciis_signature: lg_profile::parse_icc_signature_or_zero(&cfg.icc_ciis_signature),
        cicp_enabled: cfg.icc_cicp_enabled,
        cicp_color_primaries: cfg.icc_cicp_primaries,
        cicp_transfer_characteristics: cfg.icc_cicp_transfer,
        cicp_matrix_coefficients: cfg.icc_cicp_matrix,
        cicp_full_range: cfg.icc_cicp_full_range,
        metadata_enabled: cfg.icc_metadata_enabled,
        include_calibration_datetime: cfg.icc_include_calibration_datetime,
        include_chromatic_adaptation: cfg.icc_include_chromatic_adaptation,
        include_chromaticity: cfg.icc_include_chromaticity,
        include_measurement: cfg.icc_include_measurement,
        include_viewing_conditions: cfg.icc_include_viewing_conditions,
        include_spectral_scaffold: cfg.icc_include_spectral_scaffold,
    };
    lg_profile::resolve_dynamic_icc_tuning(
        manual,
        &cfg.icc_tuning_preset,
        cfg.icc_tuning_overlay_manual,
    )
}

fn tuning_for_active_preset(cfg: &Config, active_preset: &str) -> lg_profile::DynamicIccTuning {
    let _ = active_preset;
    tuning_from_config(cfg)
}

fn sync_mode_presets_to_active(cfg: &mut Config) {
    cfg.icc_sdr_preset = cfg.icc_active_preset.clone();
    cfg.icc_hdr_preset = cfg.icc_active_preset.clone();
}

fn effective_preset_for_mode(cfg: &Config, hdr_mode: bool) -> String {
    lg_profile::select_effective_preset(
        &cfg.icc_active_preset,
        &cfg.icc_sdr_preset,
        &cfg.icc_hdr_preset,
        &cfg.icc_schedule_day_preset,
        &cfg.icc_schedule_night_preset,
        hdr_mode,
    )
}

fn resolve_active_profile_path_for_mode(cfg: &Config, hdr_mode: bool) -> std::path::PathBuf {
    let active_preset = effective_preset_for_mode(cfg, hdr_mode);
    lg_profile::resolve_active_profile_path(
        &lg_profile::color_directory(),
        &active_preset,
        &cfg.profile_name,
    )
}

fn ensure_active_profile(cfg: &Config) -> Result<std::path::PathBuf, Box<dyn Error>> {
    ensure_active_profile_for_mode(cfg, false)
}

fn ensure_active_profile_for_mode(
    cfg: &Config,
    hdr_mode: bool,
) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let active_preset = effective_preset_for_mode(cfg, hdr_mode);
    lg_profile::ensure_active_profile_installed_tuned(
        &lg_profile::color_directory(),
        &active_preset,
        &cfg.profile_name,
        cfg.icc_gamma,
        cfg.icc_luminance_cd_m2,
        cfg.icc_generate_specialized_profiles,
        tuning_for_active_preset(cfg, &active_preset),
    )
}

fn effective_regex(cli_regex: bool, cfg: &Config) -> bool {
    cli_regex || cfg.monitor_match_regex
}

fn monitor_match_mode(use_regex: bool) -> lg_monitor::MonitorMatchMode {
    if use_regex {
        lg_monitor::MonitorMatchMode::Regex
    } else {
        lg_monitor::MonitorMatchMode::Substring
    }
}

fn find_matching_monitors(
    pattern: &str,
    use_regex: bool,
) -> Result<Vec<lg_monitor::MatchedMonitor>, Box<dyn Error>> {
    lg_monitor::find_matching_monitors_with_mode(pattern, monitor_match_mode(use_regex))
}

fn identity_from_monitor(mon: &lg_monitor::MatchedMonitor) -> lg_profile::DynamicMonitorIdentity {
    lg_profile::DynamicMonitorIdentity {
        monitor_name: mon.name.clone(),
        device_key: mon.device_key.clone(),
        serial_number: mon.serial.clone(),
        manufacturer_id: mon.manufacturer_id.clone(),
        product_code: mon.product_code.clone(),
    }
}

fn maybe_capture_last_good_cli(
    cfg: &Config,
    profile_path: &std::path::Path,
    note: &str,
    ddc_brightness: Option<u32>,
) {
    if let Ok(snapshot) = app_state::create_profile_snapshot(
        cfg,
        "Auto Last Good (CLI)",
        "auto",
        profile_path,
        ddc_brightness,
        note,
    ) {
        let _ = app_state::mark_snapshot_last_good(&snapshot, ddc_brightness, note);
    }
}

fn ddc_guardrail_error(vcp_code: u8, value: u32) -> Option<String> {
    let guardrails = app_state::load_ddc_guardrails();
    if guardrails.enabled && vcp_code == lg_monitor::ddc::VCP_BRIGHTNESS {
        let min = guardrails.min_brightness.min(100);
        let max = guardrails.max_brightness.min(100).max(min);
        if value < min || value > max {
            return Some(format!(
                "DDC guardrails blocked brightness {} (allowed {}..{}).",
                value, min, max
            ));
        }
    }
    if guardrails.confirm_risky_writes
        && matches!(
            vcp_code,
            lg_monitor::ddc::VCP_FACTORY_RESET
                | lg_monitor::ddc::VCP_RESET_BRIGHTNESS_CONTRAST
                | lg_monitor::ddc::VCP_RESET_COLOR
                | lg_monitor::ddc::VCP_INPUT_SOURCE
                | lg_monitor::ddc::VCP_POWER_MODE
        )
    {
        let allow_risky = std::env::var("LG_DDC_ALLOW_RISKY")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !allow_risky {
            return Some(format!(
                "DDC guardrails blocked risky VCP 0x{:02X}. Set LG_DDC_ALLOW_RISKY=1 to override.",
                vcp_code
            ));
        }
    }
    None
}

fn emit_apply_latency_cli(started: Instant, success: bool, details: &str) {
    let metrics_cfg = app_state::load_automation_config().metrics;
    if !metrics_cfg.enabled || !metrics_cfg.collect_latency {
        return;
    }
    let ms = started.elapsed().as_millis() as u64;
    app_state::append_diagnostic_event(
        "cli",
        "INFO",
        "apply_latency",
        &format!(
            "ms={} success={} {}",
            ms,
            if success { 1 } else { 0 },
            details.trim()
        ),
    );
}

fn is_risky_vcp_write(vcp_code: u8, automation_cfg: &app_state::AutomationConfig) -> bool {
    app_state::risky_vcp_codes_from_csv(&automation_cfg.ddc_safety.risky_vcp_codes)
        .into_iter()
        .any(|code| code == vcp_code)
}

fn prompt_confirm_risky_write(vcp_code: u8, value: u32) -> io::Result<bool> {
    println!(
        "[WARN] Risky VCP write detected: 0x{:02X}={}. Type YES to continue.",
        vcp_code, value
    );
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim(), "YES" | "yes"))
}

fn wait_for_keep_key(timeout_ms: u64, keep_key: &str) -> io::Result<bool> {
    if keep_key.trim().is_empty() {
        return Ok(false);
    }
    let expected = keep_key
        .trim()
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
        .unwrap_or('k');
    terminal::enable_raw_mode()?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(timeout_ms) {
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char(c) if c.to_ascii_lowercase() == expected => {
                        terminal::disable_raw_mode()?;
                        return Ok(true);
                    }
                    KeyCode::Esc => {
                        terminal::disable_raw_mode()?;
                        return Ok(false);
                    }
                    _ => {}
                }
            }
        }
    }
    terminal::disable_raw_mode()?;
    Ok(false)
}

fn set_vcp_with_safety(pattern: &str, vcp_code: u8, value: u32) -> Result<(), Box<dyn Error>> {
    let automation_cfg = app_state::load_automation_config();
    let risky = is_risky_vcp_write(vcp_code, &automation_cfg);

    if risky
        && automation_cfg.ddc_safety.require_confirm_before_risky
        && std::io::stdout().is_terminal()
        && !prompt_confirm_risky_write(vcp_code, value)?
    {
        return Err("risky write cancelled by user".into());
    }

    let previous_value = if risky && automation_cfg.ddc_safety.rollback_timer_enabled {
        lg_monitor::ddc::get_vcp_by_pattern(pattern, vcp_code)
            .ok()
            .map(|v| v.current)
    } else {
        None
    };

    lg_monitor::ddc::set_vcp_by_pattern(pattern, vcp_code, value)?;

    if risky
        && automation_cfg.ddc_safety.rollback_timer_enabled
        && std::io::stdout().is_terminal()
        && previous_value.is_some()
    {
        let timeout_ms = automation_cfg.ddc_safety.rollback_timeout_ms;
        let keep_key = automation_cfg.ddc_safety.keep_key.trim().to_string();
        println!(
            "[SAFE] Press {} within {} ms to keep this VCP write. Otherwise it will roll back.",
            if keep_key.is_empty() { "K" } else { &keep_key },
            timeout_ms
        );
        let keep = wait_for_keep_key(
            timeout_ms,
            if keep_key.is_empty() { "K" } else { &keep_key },
        )
        .unwrap_or(false);
        if !keep {
            if let Some(prev) = previous_value {
                lg_monitor::ddc::set_vcp_by_pattern(pattern, vcp_code, prev)?;
                app_state::append_diagnostic_event(
                    "cli",
                    "WARN",
                    "ddc_rollback",
                    &format!("rolled back 0x{:02X} from {} to {}", vcp_code, value, prev),
                );
                println!(
                    "[SAFE] Rollback applied for VCP 0x{:02X} -> {}",
                    vcp_code, prev
                );
            }
        } else {
            app_state::append_diagnostic_event(
                "cli",
                "INFO",
                "ddc_rollback_keep",
                &format!("kept risky write 0x{:02X}={}", vcp_code, value),
            );
        }
    }

    Ok(())
}

fn cmd_detect(pattern: Option<String>, regex: bool) -> Result<(), Box<dyn Error>> {
    let cfg = Config::load();
    let pattern = pattern.as_deref().unwrap_or(&cfg.monitor_match);
    let use_regex = effective_regex(regex, &cfg);

    println!(
        "Scanning for monitors matching \"{}\" (mode: {})...\n",
        pattern,
        if use_regex { "regex" } else { "substring" }
    );

    let devices = find_matching_monitors(pattern, use_regex)?;
    if devices.is_empty() {
        println!("No matching monitors found.");
    } else {
        println!("Found {} monitor(s):\n", devices.len());
        for (i, device) in devices.iter().enumerate() {
            println!("  {}. {}", i + 1, device.name);
            println!("     Device: {}", device.device_key);
            println!(
                "     Serial: {}",
                if device.serial.is_empty() {
                    "(unknown)"
                } else {
                    &device.serial
                }
            );
        }
    }

    let active_profile_path = resolve_active_profile_path(&cfg);
    println!("\nProfile: {}", active_profile_path.display());
    let _ = ensure_active_profile(&cfg);
    println!(
        "Installed: {}",
        if lg_profile::is_profile_installed(&active_profile_path) {
            "yes"
        } else {
            "NO — generation failed, check permissions"
        }
    );

    Ok(())
}

/// Options for apply command (avoids too-many-arguments lint).
struct ApplyOpts {
    pattern: Option<String>,
    regex: bool,
    profile_path: Option<String>,
    #[allow(dead_code)]
    per_user: bool,
    #[allow(dead_code)]
    skip_hdr: bool,
    toast: bool,
    no_toast: bool,
    verbose: bool,
    dry_run: bool,
}

fn cmd_apply(opts: ApplyOpts) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let mut cfg = Config::load();
    if let Some(ref p) = opts.pattern {
        cfg.monitor_match = p.clone();
    }
    if opts.verbose {
        cfg.verbose = true;
    }
    // Override toast from CLI flags
    if opts.toast {
        cfg.toast_enabled = true;
    } else if opts.no_toast {
        cfg.toast_enabled = false;
    }
    let using_custom_profile = opts.profile_path.is_some();
    let include_hdr_association = !opts.skip_hdr;
    let active_hdr_mode = lg_monitor::is_any_display_hdr_enabled().unwrap_or(false);
    let use_regex = effective_regex(opts.regex, &cfg);
    let sdr_preset = effective_preset_for_mode(&cfg, false);
    let hdr_preset = effective_preset_for_mode(&cfg, true);
    let mut sdr_shared_profile = if using_custom_profile {
        std::path::PathBuf::from(opts.profile_path.as_deref().unwrap_or_default())
    } else {
        lg_profile::resolve_active_profile_path(
            &lg_profile::color_directory(),
            &sdr_preset,
            &cfg.profile_name,
        )
    };
    let mut hdr_shared_profile = if using_custom_profile {
        sdr_shared_profile.clone()
    } else {
        lg_profile::resolve_active_profile_path(
            &lg_profile::color_directory(),
            &hdr_preset,
            &cfg.profile_name,
        )
    };
    if !include_hdr_association {
        hdr_shared_profile = sdr_shared_profile.clone();
    }
    let active_profile = if active_hdr_mode {
        hdr_shared_profile.clone()
    } else {
        sdr_shared_profile.clone()
    };

    app_state::append_diagnostic_event(
        "cli",
        "INFO",
        "apply_begin",
        &format!(
            "pattern=\"{}\" mode={} profile={} sdr={} hdr={} hdr_assoc={}",
            cfg.monitor_match,
            if use_regex { "regex" } else { "substring" },
            active_profile.display(),
            sdr_shared_profile.display(),
            hdr_shared_profile.display(),
            include_hdr_association
        ),
    );

    println!("[INFO] Running one-shot profile reapply...");
    println!("[INFO] Config:  {}", config::config_path().display());
    println!("[INFO] Pattern: {}", cfg.monitor_match);
    println!(
        "[INFO] Match:   {}",
        if use_regex { "regex" } else { "substring" }
    );
    println!("[INFO] Active Profile: {}", active_profile.display());
    println!("[INFO] SDR Profile:    {}", sdr_shared_profile.display());
    println!("[INFO] HDR Profile:    {}", hdr_shared_profile.display());
    println!("[INFO] HDR Assoc:      {}", include_hdr_association);
    println!(
        "[INFO] Toast:   {}",
        if cfg.toast_enabled { "on" } else { "off" }
    );
    println!();

    if opts.dry_run {
        let devices = find_matching_monitors(&cfg.monitor_match, use_regex)?;
        println!(
            "[DRY RUN] Would reapply mode-aware profiles for {} matching monitor(s)",
            devices.len()
        );
        app_state::append_diagnostic_event(
            "cli",
            "INFO",
            "apply_dry_run",
            &format!("matching_monitors={}", devices.len()),
        );
        emit_apply_latency_cli(started, true, "mode=dry_run");
        return Ok(());
    }

    if using_custom_profile {
        lg_profile::ensure_profile_installed_with_gamma_luminance_and_tuning(
            &sdr_shared_profile,
            cfg.icc_gamma,
            cfg.icc_luminance_cd_m2,
            tuning_from_config(&cfg),
        )?;
    } else if !cfg.icc_per_monitor_profiles {
        let (sdr_path, hdr_path) = lg_profile::ensure_mode_profiles_installed_tuned(
            &lg_profile::color_directory(),
            &sdr_preset,
            &hdr_preset,
            &cfg.profile_name,
            cfg.icc_gamma,
            cfg.icc_luminance_cd_m2,
            cfg.icc_generate_specialized_profiles,
            tuning_from_config(&cfg),
        )?;
        sdr_shared_profile = sdr_path;
        hdr_shared_profile = if include_hdr_association {
            hdr_path
        } else {
            sdr_shared_profile.clone()
        };
    }

    if !cfg.icc_per_monitor_profiles
        && (!lg_profile::is_profile_installed(&sdr_shared_profile)
            || !lg_profile::is_profile_installed(&hdr_shared_profile))
    {
        return Err(format!(
            "ICC mode profile not found (sdr={}, hdr={})",
            sdr_shared_profile.display(),
            hdr_shared_profile.display()
        )
        .into());
    }

    let devices = find_matching_monitors(&cfg.monitor_match, use_regex)?;
    let success = if devices.is_empty() {
        println!("[SKIP] No matching monitors found.");
        app_state::append_diagnostic_event("cli", "WARN", "apply_skip", "no matching monitors");
        false
    } else {
        let mut last_applied_profile: Option<std::path::PathBuf> = None;
        for device in &devices {
            println!("[INFO] Found: {}", device.name);
            let (sdr_profile_for_device, hdr_profile_for_device) = if using_custom_profile {
                (sdr_shared_profile.clone(), hdr_shared_profile.clone())
            } else if cfg.icc_per_monitor_profiles {
                let identity = identity_from_monitor(device);
                let (sdr_path, hdr_path) =
                    lg_profile::ensure_mode_profiles_installed_tuned_for_monitor(
                        &lg_profile::color_directory(),
                        &sdr_preset,
                        &hdr_preset,
                        &cfg.profile_name,
                        cfg.icc_gamma,
                        cfg.icc_luminance_cd_m2,
                        cfg.icc_generate_specialized_profiles,
                        tuning_from_config(&cfg),
                        &identity,
                    )?;
                if include_hdr_association {
                    (sdr_path, hdr_path)
                } else {
                    (sdr_path.clone(), sdr_path)
                }
            } else {
                (sdr_shared_profile.clone(), hdr_shared_profile.clone())
            };
            let active_profile_for_device = if active_hdr_mode {
                &hdr_profile_for_device
            } else {
                &sdr_profile_for_device
            };
            lg_profile::reapply_profile_with_mode_associations(
                &device.device_key,
                active_profile_for_device,
                &sdr_profile_for_device,
                &hdr_profile_for_device,
                cfg.toggle_delay_ms,
                opts.per_user,
            )?;
            last_applied_profile = Some(active_profile_for_device.clone());
            println!("[OK]   SDR/HDR profiles associated for {}", device.name);
        }

        // Keep post-apply refresh non-disruptive; hard refresh is handled as
        // an internal fallback inside lg_profile when verification fails.
        lg_profile::refresh_display(false, cfg.refresh_broadcast_color, cfg.refresh_invalidate);
        lg_profile::trigger_calibration_loader(cfg.refresh_calibration_loader);

        if cfg.toast_enabled {
            println!("[INFO] Sending toast notification...");
            lg_notify::show_reapply_toast(true, &cfg.toast_title, &cfg.toast_body, cfg.verbose);
        }

        if let Some(profile_path) = last_applied_profile.as_ref() {
            maybe_capture_last_good_cli(
                &cfg,
                profile_path,
                "CLI apply success",
                if cfg.ddc_brightness_on_reapply {
                    Some(cfg.ddc_brightness_value)
                } else {
                    None
                },
            );
        }
        app_state::append_diagnostic_event(
            "cli",
            "INFO",
            "apply_success",
            &format!("reapplied for {} monitor(s)", devices.len()),
        );

        println!("\n[DONE] All profiles reapplied.");
        true
    };

    emit_apply_latency_cli(
        started,
        success,
        &format!(
            "pattern=\"{}\" mode={} monitors={}",
            cfg.monitor_match,
            if use_regex { "regex" } else { "substring" },
            if success { "applied" } else { "none" }
        ),
    );

    Ok(())
}

fn cmd_watch(pattern: Option<String>, regex: bool) -> Result<(), Box<dyn Error>> {
    let mut cfg = Config::load();
    if let Some(p) = pattern {
        cfg.monitor_match = p;
    }
    if regex {
        cfg.monitor_match_regex = true;
    }
    lg_service::watch(&cfg)?;
    Ok(())
}

fn cmd_config(action: Option<ConfigAction>) -> Result<(), Box<dyn Error>> {
    match action {
        None | Some(ConfigAction::Show) => {
            let cfg = Config::load();
            let path = config::config_path();
            println!("Config file: {}\n", path.display());
            println!("── Monitor Detection ──");
            println!("  monitor_match            = \"{}\"", cfg.monitor_match);
            println!("  monitor_match_regex      = {}", cfg.monitor_match_regex);
            println!("  profile_name             = \"{}\"", cfg.profile_name);
            println!("  icc_gamma                = {:.3}", cfg.icc_gamma);
            println!("  icc_active_preset        = \"{}\"", cfg.icc_active_preset);
            println!("  icc_sdr_preset           = \"{}\"", cfg.icc_sdr_preset);
            println!("  icc_hdr_preset           = \"{}\"", cfg.icc_hdr_preset);
            println!(
                "  icc_schedule_day_preset  = \"{}\"",
                cfg.icc_schedule_day_preset
            );
            println!(
                "  icc_schedule_night_preset = \"{}\"",
                cfg.icc_schedule_night_preset
            );
            println!(
                "  icc_generate_specialized_profiles = {}",
                cfg.icc_generate_specialized_profiles
            );
            println!(
                "  icc_luminance_cd_m2      = {:.1}",
                cfg.icc_luminance_cd_m2
            );
            println!("  icc_tuning_preset        = \"{}\"", cfg.icc_tuning_preset);
            println!(
                "  icc_tuning_overlay_manual = {}",
                cfg.icc_tuning_overlay_manual
            );
            println!("  icc_black_lift           = {:.3}", cfg.icc_black_lift);
            println!("  icc_midtone_boost        = {:.3}", cfg.icc_midtone_boost);
            println!(
                "  icc_white_compression    = {:.3}",
                cfg.icc_white_compression
            );
            println!("  icc_gamma_r              = {:.3}", cfg.icc_gamma_r);
            println!("  icc_gamma_g              = {:.3}", cfg.icc_gamma_g);
            println!("  icc_gamma_b              = {:.3}", cfg.icc_gamma_b);
            println!("  icc_vcgt_enabled         = {}", cfg.icc_vcgt_enabled);
            println!("  icc_vcgt_strength        = {:.3}", cfg.icc_vcgt_strength);
            println!(
                "  icc_target_black_cd_m2   = {:.3}",
                cfg.icc_target_black_cd_m2
            );
            println!(
                "  icc_include_media_black_point = {}",
                cfg.icc_include_media_black_point
            );
            println!(
                "  icc_include_device_descriptions = {}",
                cfg.icc_include_device_descriptions
            );
            println!(
                "  icc_include_characterization_target = {}",
                cfg.icc_include_characterization_target
            );
            println!(
                "  icc_include_viewing_cond_desc = {}",
                cfg.icc_include_viewing_cond_desc
            );
            println!(
                "  icc_technology_signature = \"{}\"",
                cfg.icc_technology_signature
            );
            println!(
                "  icc_ciis_signature       = \"{}\"",
                cfg.icc_ciis_signature
            );
            println!("  icc_cicp_enabled         = {}", cfg.icc_cicp_enabled);
            println!("  icc_cicp_primaries       = {}", cfg.icc_cicp_primaries);
            println!("  icc_cicp_transfer        = {}", cfg.icc_cicp_transfer);
            println!("  icc_cicp_matrix          = {}", cfg.icc_cicp_matrix);
            println!("  icc_cicp_full_range      = {}", cfg.icc_cicp_full_range);
            println!("  icc_metadata_enabled     = {}", cfg.icc_metadata_enabled);
            println!(
                "  icc_include_calibration_datetime = {}",
                cfg.icc_include_calibration_datetime
            );
            println!(
                "  icc_include_chromatic_adaptation = {}",
                cfg.icc_include_chromatic_adaptation
            );
            println!(
                "  icc_include_chromaticity = {}",
                cfg.icc_include_chromaticity
            );
            println!(
                "  icc_include_measurement  = {}",
                cfg.icc_include_measurement
            );
            println!(
                "  icc_include_viewing_conditions = {}",
                cfg.icc_include_viewing_conditions
            );
            println!(
                "  icc_include_spectral_scaffold = {}",
                cfg.icc_include_spectral_scaffold
            );
            println!(
                "  icc_per_monitor_profiles = {}",
                cfg.icc_per_monitor_profiles
            );
            println!(
                "  icc_auto_apply_on_change = {}",
                cfg.icc_auto_apply_on_change
            );
            println!(
                "  active_profile_path      = \"{}\"",
                resolve_active_profile_path(&cfg).display()
            );
            println!("\n── Toast Notifications ──");
            println!("  toast_enabled            = {}", cfg.toast_enabled);
            println!("  toast_title              = \"{}\"", cfg.toast_title);
            println!("  toast_body               = \"{}\"", cfg.toast_body);
            println!("\n── Timing ──");
            println!("  stabilize_delay_ms       = {}", cfg.stabilize_delay_ms);
            println!("  toggle_delay_ms          = {}", cfg.toggle_delay_ms);
            println!("  reapply_delay_ms         = {}", cfg.reapply_delay_ms);
            println!("\n── Refresh Methods ──");
            println!(
                "  refresh_display_settings = {}",
                cfg.refresh_display_settings
            );
            println!(
                "  refresh_broadcast_color  = {}",
                cfg.refresh_broadcast_color
            );
            println!("  refresh_invalidate       = {}", cfg.refresh_invalidate);
            println!(
                "  refresh_calibration_loader = {}",
                cfg.refresh_calibration_loader
            );
            println!("\n── DDC/CI Brightness ──");
            println!(
                "  ddc_brightness_on_reapply = {}",
                cfg.ddc_brightness_on_reapply
            );
            println!("  ddc_brightness_value      = {}", cfg.ddc_brightness_value);
            println!("\n── Debug ──");
            println!("  verbose                  = {}", cfg.verbose);
        }
        Some(ConfigAction::Path) => {
            println!("{}", config::config_path().display());
        }
        Some(ConfigAction::Reset) => {
            Config::write_default()?;
            println!(
                "[OK] Config reset to defaults at {}",
                config::config_path().display()
            );
        }
    }
    Ok(())
}

fn cmd_automation(action: AutomationAction, dry_run: bool) -> Result<(), Box<dyn Error>> {
    match action {
        AutomationAction::Show => {
            let cfg = app_state::load_automation_config();
            println!(
                "Automation config: {}\n",
                app_state::automation_config_path().display()
            );
            println!("{}", toml::to_string_pretty(&cfg)?);
            println!("\nHints:");
            println!("  - ambient.sensor_method: ddc_brightness | powershell | command | env | simulated");
            println!("  - app_rules.match_mode: contains | exact | regex");
            println!("  - ddc_safety.risky_vcp_codes accepts hex CSV, e.g. 04,06,0A,60,D6,DC");
        }
        AutomationAction::Path => {
            println!("{}", app_state::automation_config_path().display());
        }
        AutomationAction::Reset => {
            if dry_run {
                println!(
                    "[DRY RUN] Would reset automation config at {}",
                    app_state::automation_config_path().display()
                );
            } else {
                app_state::save_automation_config(&app_state::AutomationConfig::default())?;
                println!(
                    "[OK] Automation config reset to defaults at {}",
                    app_state::automation_config_path().display()
                );
            }
        }
        AutomationAction::ApplyNow => {
            println!("[INFO] Running one-shot apply with current config...");
            println!("[INFO] Full ambient/app automation runs continuously in service/watch mode.");
            cmd_apply(ApplyOpts {
                pattern: None,
                regex: false,
                profile_path: None,
                per_user: false,
                skip_hdr: false,
                toast: false,
                no_toast: false,
                verbose: false,
                dry_run,
            })?;
        }
    }
    Ok(())
}

fn cmd_tray(action: TrayAction, dry_run: bool) -> Result<(), Box<dyn Error>> {
    match action {
        TrayAction::Run => {
            let cfg = app_state::load_automation_config();
            let tray = cfg.tray;
            if !tray.enabled {
                println!(
                    "[WARN] Tray mode is disabled in automation config. Enable tray.enabled=true first."
                );
            }
            let exe = std::env::current_exe()?;
            let script_path = std::env::temp_dir().join("lg-ultragear-tray-mode.ps1");
            let script = r#"
param(
  [string]$Exe,
  [string]$Tooltip,
  [int]$Step,
  [bool]$ShowApply,
  [bool]$ShowReader,
  [bool]$ShowAB,
  [bool]$ShowBrightness,
  [bool]$ShowExit
)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$notify = New-Object System.Windows.Forms.NotifyIcon
$notify.Icon = [System.Drawing.SystemIcons]::Information
$notify.Text = $Tooltip
$notify.Visible = $true

$menu = New-Object System.Windows.Forms.ContextMenuStrip
function Add-Menu([string]$text, [scriptblock]$onClick) {
  $item = New-Object System.Windows.Forms.ToolStripMenuItem($text)
  $item.add_Click($onClick)
  [void]$menu.Items.Add($item)
}

function Get-Brightness {
  $out = & $Exe ddc get-vcp 10 2>$null
  $m = [regex]::Match(($out | Out-String), 'current=(\d+)')
  if ($m.Success) { return [int]$m.Groups[1].Value }
  return $null
}

function Set-BrightnessStep([int]$delta) {
  $cur = Get-Brightness
  if ($null -eq $cur) { return }
  $next = [Math]::Max(0, [Math]::Min(100, $cur + $delta))
  & $Exe ddc brightness $next | Out-Null
}

if ($ShowApply) {
  Add-Menu "Apply Now" { & $Exe apply --skip-elevation | Out-Null }
}
if ($ShowReader) {
  Add-Menu "Reader Preset" { & $Exe icc optimize --tuning-preset reader_balanced --apply --skip-elevation | Out-Null }
}
if ($ShowAB) {
  Add-Menu "Toggle A/B (TUI)" { & $Exe --skip-elevation | Out-Null }
}
if ($ShowBrightness) {
  Add-Menu ("Brightness +" + $Step) { Set-BrightnessStep $Step }
  Add-Menu ("Brightness -" + $Step) { Set-BrightnessStep (-1 * $Step) }
}
if ($ShowExit) {
  Add-Menu "Exit Tray Mode" {
    $notify.Visible = $false
    $notify.Dispose()
    [System.Windows.Forms.Application]::Exit()
  }
}

$notify.ContextMenuStrip = $menu
[System.Windows.Forms.Application]::Run()
"#;

            if dry_run {
                println!(
                    "[DRY RUN] Would start tray host script at {}",
                    script_path.display()
                );
                return Ok(());
            }

            std::fs::write(&script_path, script)?;
            println!("[INFO] Launching tray mode...");
            let status = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &script_path.to_string_lossy(),
                    "-Exe",
                    &exe.to_string_lossy(),
                    "-Tooltip",
                    &tray.tooltip,
                    "-Step",
                    &tray.brightness_step.to_string(),
                    "-ShowApply",
                    &tray.show_apply_action.to_string(),
                    "-ShowReader",
                    &tray.show_reader_toggle.to_string(),
                    "-ShowAB",
                    &tray.show_ab_toggle.to_string(),
                    "-ShowBrightness",
                    &tray.show_brightness_controls.to_string(),
                    "-ShowExit",
                    &tray.show_exit_action.to_string(),
                ])
                .status()?;
            if !status.success() {
                return Err("tray host exited with failure".into());
            }
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn Error>> {
    if !src.exists() {
        return Ok(());
    }
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn cmd_bundle(action: BundleAction, dry_run: bool) -> Result<(), Box<dyn Error>> {
    match action {
        BundleAction::Export { output } => {
            let out_dir = PathBuf::from(output);
            if dry_run {
                println!("[DRY RUN] Would export bundle to {}", out_dir.display());
                return Ok(());
            }
            if !out_dir.exists() {
                std::fs::create_dir_all(&out_dir)?;
            }

            let config_src = config::config_path();
            let automation_src = app_state::automation_config_path();
            let state_src = app_state::state_dir();
            let color_src = app_state::windows_color_directory();

            if config_src.exists() {
                std::fs::copy(&config_src, out_dir.join("config.toml"))?;
            }
            if automation_src.exists() {
                std::fs::copy(&automation_src, out_dir.join("automation.toml"))?;
            }
            copy_dir_recursive(&state_src, &out_dir.join("state"))?;

            let color_out = out_dir.join("color");
            std::fs::create_dir_all(&color_out)?;
            if color_src.exists() {
                for entry in std::fs::read_dir(&color_src)? {
                    let entry = entry?;
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let lower = name.to_ascii_lowercase();
                    if lower.ends_with(".icm") && lower.contains("lg-ultragear") {
                        std::fs::copy(&path, color_out.join(name))?;
                    }
                }
            }
            let manifest = format!(
                "bundle_version = 1\nexported_at = \"{}\"\n",
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            );
            std::fs::write(out_dir.join("manifest.toml"), manifest)?;
            println!("[OK] Bundle exported to {}", out_dir.display());
        }
        BundleAction::Import { input } => {
            let in_dir = PathBuf::from(input);
            if !in_dir.exists() {
                return Err(format!("bundle path does not exist: {}", in_dir.display()).into());
            }
            if dry_run {
                println!("[DRY RUN] Would import bundle from {}", in_dir.display());
                return Ok(());
            }

            let cfg_dst = config::config_path();
            if let Some(parent) = cfg_dst.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let cfg_src = in_dir.join("config.toml");
            if cfg_src.exists() {
                std::fs::copy(&cfg_src, &cfg_dst)?;
            }

            let automation_dst = app_state::automation_config_path();
            let automation_src = in_dir.join("automation.toml");
            if automation_src.exists() {
                if let Some(parent) = automation_dst.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
                std::fs::copy(&automation_src, &automation_dst)?;
            }

            let state_src = in_dir.join("state");
            let state_dst = app_state::state_dir();
            copy_dir_recursive(&state_src, &state_dst)?;

            let color_src = in_dir.join("color");
            let color_dst = app_state::windows_color_directory();
            copy_dir_recursive(&color_src, &color_dst)?;
            println!("[OK] Bundle imported from {}", in_dir.display());
        }
    }
    Ok(())
}

fn cmd_service(action: ServiceAction) -> Result<(), Box<dyn Error>> {
    match action {
        ServiceAction::Install {
            pattern,
            service_name: _service_name,
        } => {
            let monitor_match = pattern.as_deref().unwrap_or("LG ULTRAGEAR");

            // Write default config file (won't overwrite if exists)
            let cfg_path = config::config_path();
            if !cfg_path.exists() {
                Config::write_default()?;
                println!("[OK] Default config written to {}", cfg_path.display());
            } else {
                println!("[OK] Config already exists at {}", cfg_path.display());
            }

            // Update monitor_match in config if provided on CLI
            let mut cfg = Config::load();
            if monitor_match != "LG ULTRAGEAR" {
                cfg.monitor_match = monitor_match.to_string();
                Config::write_config(&cfg)?;
                println!(
                    "[OK] Config updated with monitor pattern: {}",
                    monitor_match
                );
            }

            if let Err(e) = lg_service::install(&cfg.monitor_match) {
                print_service_binary_placement(true);
                return Err(e);
            }
            println!(
                "[OK] Service installed. Monitor pattern: {}",
                cfg.monitor_match
            );
            print_service_binary_placement(false);
            lg_service::start_service()?;
            println!("[OK] Service started.");
            println!("     Binary: {}", config::install_path().display());
            println!("     Config: {}", cfg_path.display());
        }
        ServiceAction::Uninstall => {
            lg_service::uninstall()?;
            println!("[OK] Service uninstalled.");
            println!(
                "     Config preserved at: {}",
                config::config_path().display()
            );
            println!(
                "     Binary removed from: {}",
                config::install_path().display()
            );
        }
        ServiceAction::Start => {
            lg_service::start_service()?;
            println!("[OK] Service started.");
        }
        ServiceAction::Stop => {
            lg_service::stop_service()?;
            println!("[OK] Service stopped.");
        }
        ServiceAction::Status => {
            lg_service::print_status()?;
        }
        ServiceAction::Run => {
            // Handled in main() — should never reach here
            unreachable!("SCM mode handled in main()");
        }
    }
    Ok(())
}

fn print_service_binary_placement(after_failed_install: bool) {
    let path = config::install_path();
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_file() => {
            if after_failed_install {
                println!(
                    "[NOTE] Service binary is present at {} ({} bytes); install failed in a later step.",
                    path.display(),
                    meta.len()
                );
            } else {
                println!(
                    "[OK] Service binary placed at {} ({} bytes)",
                    path.display(),
                    meta.len()
                );
            }
        }
        Ok(_) => {
            println!(
                "[WARN] Service install path exists but is not a file: {}",
                path.display()
            );
        }
        Err(e) => {
            println!(
                "[WARN] Service binary not found at {} ({})",
                path.display(),
                e
            );
        }
    }
}

// ============================================================================
// New top-level commands (parity with PowerShell installer)
// ============================================================================

/// Options for install command (avoids too-many-arguments lint).
struct InstallOpts {
    pattern: Option<String>,
    regex: bool,
    profile_only: bool,
    service_only: bool,
    custom_profile: Option<String>,
    #[allow(dead_code)]
    per_user: bool,
    #[allow(dead_code)]
    skip_hdr: bool,
    #[allow(dead_code)]
    skip_hash_check: bool,
    force: bool,
    skip_detect: bool,
    dry_run: bool,
}

fn cmd_install(opts: InstallOpts) -> Result<(), Box<dyn Error>> {
    let mut cfg = Config::load();
    if let Some(ref p) = opts.pattern {
        cfg.monitor_match = p.clone();
    }
    if opts.regex {
        cfg.monitor_match_regex = true;
    }
    let use_regex = effective_regex(opts.regex, &cfg);

    if opts.profile_only {
        // Profile-only install
        if opts.dry_run {
            println!("[DRY RUN] Would extract SDR/HDR ICC profile(s) to color store");
            return Ok(());
        }
        let custom_profile = opts.custom_profile.is_some();
        let sdr_preset = effective_preset_for_mode(&cfg, false);
        let hdr_preset = effective_preset_for_mode(&cfg, true);
        let profile_path = if let Some(ref custom) = opts.custom_profile {
            std::path::PathBuf::from(custom)
        } else {
            lg_profile::resolve_active_profile_path(
                &lg_profile::color_directory(),
                &sdr_preset,
                &cfg.profile_name,
            )
        };
        let wrote = if custom_profile {
            lg_profile::ensure_profile_installed_with_gamma_luminance_and_tuning(
                &profile_path,
                cfg.icc_gamma,
                cfg.icc_luminance_cd_m2,
                tuning_from_config(&cfg),
            )?
        } else {
            let _ = lg_profile::ensure_mode_profiles_installed_tuned(
                &lg_profile::color_directory(),
                &sdr_preset,
                &hdr_preset,
                &cfg.profile_name,
                cfg.icc_gamma,
                cfg.icc_luminance_cd_m2,
                cfg.icc_generate_specialized_profiles,
                tuning_from_config(&cfg),
            )?;
            true
        };
        match wrote {
            true => println!("[OK] ICC profile installed to {}", profile_path.display()),
            false => {
                if opts.force {
                    // Force overwrite: remove and re-extract
                    let _ = lg_profile::remove_profile(&profile_path);
                    if custom_profile {
                        lg_profile::ensure_profile_installed_with_gamma_luminance_and_tuning(
                            &profile_path,
                            cfg.icc_gamma,
                            cfg.icc_luminance_cd_m2,
                            tuning_from_config(&cfg),
                        )?;
                    } else {
                        let _ = lg_profile::ensure_mode_profiles_installed_tuned(
                            &lg_profile::color_directory(),
                            &sdr_preset,
                            &hdr_preset,
                            &cfg.profile_name,
                            cfg.icc_gamma,
                            cfg.icc_luminance_cd_m2,
                            cfg.icc_generate_specialized_profiles,
                            tuning_from_config(&cfg),
                        )?;
                    }
                    println!(
                        "[OK] ICC profile force-installed to {}",
                        profile_path.display()
                    );
                } else {
                    println!("[OK] ICC profile already present");
                }
            }
        }

        // Clean up any stale/leftover ICM files (from test runs, etc.)
        let expected_name = profile_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cfg.profile_name.clone());
        let stale = lg_profile::cleanup_stale_profiles(&expected_name);
        for p in &stale {
            println!("[OK] Removed stale profile: {}", p.display());
        }

        println!("[DONE] Profile install complete.");
        return Ok(());
    }

    if opts.dry_run {
        if !opts.service_only {
            println!("[DRY RUN] Would extract SDR/HDR ICC profile(s) to color store");
        }
        if !opts.skip_detect {
            println!(
                "[DRY RUN] Would detect matching monitors ({})",
                if use_regex { "regex" } else { "substring" }
            );
        }
        println!("[DRY RUN] Would write default config");
        println!("[DRY RUN] Would install Windows service");
        println!("[DRY RUN] Would start service");
        return Ok(());
    }

    // Extract ICC profile (unless service-only)
    if !opts.service_only {
        let custom_profile = opts.custom_profile.is_some();
        let sdr_preset = effective_preset_for_mode(&cfg, false);
        let hdr_preset = effective_preset_for_mode(&cfg, true);
        let profile_path = if let Some(ref custom) = opts.custom_profile {
            std::path::PathBuf::from(custom)
        } else {
            lg_profile::resolve_active_profile_path(
                &lg_profile::color_directory(),
                &sdr_preset,
                &cfg.profile_name,
            )
        };
        let wrote = if custom_profile {
            lg_profile::ensure_profile_installed_with_gamma_luminance_and_tuning(
                &profile_path,
                cfg.icc_gamma,
                cfg.icc_luminance_cd_m2,
                tuning_from_config(&cfg),
            )?
        } else {
            let _ = lg_profile::ensure_mode_profiles_installed_tuned(
                &lg_profile::color_directory(),
                &sdr_preset,
                &hdr_preset,
                &cfg.profile_name,
                cfg.icc_gamma,
                cfg.icc_luminance_cd_m2,
                cfg.icc_generate_specialized_profiles,
                tuning_from_config(&cfg),
            )?;
            true
        };
        match wrote {
            true => println!("[OK] ICC profile installed to {}", profile_path.display()),
            false => {
                if opts.force {
                    let _ = lg_profile::remove_profile(&profile_path);
                    if custom_profile {
                        lg_profile::ensure_profile_installed_with_gamma_luminance_and_tuning(
                            &profile_path,
                            cfg.icc_gamma,
                            cfg.icc_luminance_cd_m2,
                            tuning_from_config(&cfg),
                        )?;
                    } else {
                        let _ = lg_profile::ensure_mode_profiles_installed_tuned(
                            &lg_profile::color_directory(),
                            &sdr_preset,
                            &hdr_preset,
                            &cfg.profile_name,
                            cfg.icc_gamma,
                            cfg.icc_luminance_cd_m2,
                            cfg.icc_generate_specialized_profiles,
                            tuning_from_config(&cfg),
                        )?;
                    }
                    println!(
                        "[OK] ICC profile force-installed to {}",
                        profile_path.display()
                    );
                } else {
                    println!("[OK] ICC profile already present");
                }
            }
        }
    }

    // Detect monitors (unless skipped)
    if !opts.skip_detect {
        let devices = find_matching_monitors(&cfg.monitor_match, use_regex)?;
        if devices.is_empty() {
            println!(
                "[NOTE] No monitors matching \"{}\" found",
                cfg.monitor_match
            );
        } else {
            println!(
                "[OK] Found {} monitor(s) matching \"{}\"",
                devices.len(),
                cfg.monitor_match
            );
            if cfg.icc_per_monitor_profiles && !opts.service_only {
                let sdr_preset = effective_preset_for_mode(&cfg, false);
                let hdr_preset = effective_preset_for_mode(&cfg, true);
                for device in &devices {
                    let identity = identity_from_monitor(device);
                    match lg_profile::ensure_mode_profiles_installed_tuned_for_monitor(
                        &lg_profile::color_directory(),
                        &sdr_preset,
                        &hdr_preset,
                        &cfg.profile_name,
                        cfg.icc_gamma,
                        cfg.icc_luminance_cd_m2,
                        cfg.icc_generate_specialized_profiles,
                        tuning_from_config(&cfg),
                        &identity,
                    ) {
                        Ok((sdr_path, hdr_path)) => println!(
                            "[OK] Monitor-scoped profiles ready for {}: SDR={} HDR={}",
                            device.name,
                            sdr_path.display(),
                            hdr_path.display()
                        ),
                        Err(e) => println!(
                            "[WARN] Failed to generate monitor-scoped profile for {}: {}",
                            device.name, e
                        ),
                    }
                }
            }
        }
    }

    // Write default config
    let cfg_path = config::config_path();
    if !cfg_path.exists() {
        Config::write_default()?;
        println!("[OK] Default config written to {}", cfg_path.display());
    } else {
        println!("[OK] Config already exists at {}", cfg_path.display());
    }

    // Update monitor_match in config if provided on CLI
    if opts.pattern.is_some() || opts.regex {
        Config::write_config(&cfg)?;
        println!(
            "[OK] Config updated with monitor pattern: {} (mode: {})",
            cfg.monitor_match,
            if cfg.monitor_match_regex {
                "regex"
            } else {
                "substring"
            }
        );
    }

    // Install service
    if let Err(e) = lg_service::install(&cfg.monitor_match) {
        print_service_binary_placement(true);
        return Err(e);
    }
    println!("[OK] Service installed");
    print_service_binary_placement(false);
    println!("     Binary: {}", config::install_path().display());
    println!("     Config: {}", cfg_path.display());

    // Start service
    lg_service::start_service()?;
    println!("[OK] Service started");
    println!("\n[DONE] Install complete!");
    Ok(())
}

fn cmd_uninstall(full: bool, profile: bool, dry_run: bool) -> Result<(), Box<dyn Error>> {
    if dry_run {
        if full {
            println!("[DRY RUN] Would uninstall service");
            println!("[DRY RUN] Would remove ICC profile");
            println!("[DRY RUN] Would remove config directory");
        } else if profile {
            println!("[DRY RUN] Would uninstall service");
            println!("[DRY RUN] Would remove ICC profile");
        } else {
            println!("[DRY RUN] Would uninstall service");
        }
        return Ok(());
    }

    // Always remove service (unless profile-only removal requested without --full)
    if full || !profile {
        match lg_service::uninstall() {
            Ok(()) => {
                println!("[OK] Service uninstalled.");
            }
            Err(e) => {
                if full {
                    println!("[NOTE] Service removal: {} (continuing)", e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Remove profile if requested
    if full || profile {
        let cfg = Config::load();
        let color_dir = lg_profile::color_directory();
        let mut targets = vec![
            resolve_active_profile_path(&cfg),
            color_dir.join(lg_profile::GAMMA22_PROFILE_NAME),
            color_dir.join(lg_profile::GAMMA24_PROFILE_NAME),
        ];
        targets.sort();
        targets.dedup();

        let mut removed_any = false;
        for profile_path in targets {
            if lg_profile::remove_profile(&profile_path)? {
                println!("[OK] ICC profile removed from {}", profile_path.display());
                removed_any = true;
            }
        }
        if !removed_any {
            println!("[NOTE] ICC profile not found (already removed)");
        }

        // Clean up any stale/leftover ICM files (from test runs, etc.)
        let expected_name = resolve_active_profile_path(&cfg)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cfg.profile_name.clone());
        let stale = lg_profile::cleanup_stale_profiles(&expected_name);
        for p in &stale {
            println!("[OK] Removed stale profile: {}", p.display());
        }
    }

    // Remove config directory if full uninstall
    if full {
        let cfg_dir = config::config_dir();
        if cfg_dir.exists() {
            // Force-remove any known files that may be locked (e.g. the
            // installed binary that the service was running from).
            let install_bin = config::install_path();
            if install_bin.exists() {
                lg_service::force_remove_file_public(&install_bin);
            }

            // Now try to remove the whole directory tree.
            let mut removed = false;
            for attempt in 0..5 {
                if attempt > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
                match std::fs::remove_dir_all(&cfg_dir) {
                    Ok(()) => {
                        println!("[OK] Config directory removed: {}", cfg_dir.display());
                        removed = true;
                        break;
                    }
                    Err(_) if attempt < 4 => continue,
                    Err(e) => {
                        println!("[WARN] Could not remove config dir: {}", e);
                    }
                }
            }
            if !removed {
                // Schedule the directory itself for reboot-deletion.
                lg_service::schedule_reboot_delete(&cfg_dir);
                println!(
                    "[NOTE] Config directory scheduled for removal on next reboot: {}",
                    cfg_dir.display()
                );
            }
        }
    }

    if !full && !profile {
        println!(
            "     Config preserved at: {}",
            config::config_path().display()
        );
    }

    if full {
        println!("\n[DONE] Full uninstall complete.");
    } else {
        println!("\n[DONE] Uninstall complete.");
    }
    Ok(())
}

fn cmd_reinstall(
    pattern: Option<String>,
    regex: bool,
    dry_run: bool,
) -> Result<(), Box<dyn Error>> {
    if dry_run {
        println!("[DRY RUN] Would uninstall existing service");
        println!("[DRY RUN] Would reinstall profile + service");
        println!("[DRY RUN] Would start service");
        return Ok(());
    }

    println!("[INFO] Removing existing installation...");
    match lg_service::uninstall() {
        Ok(()) => println!("[OK] Service uninstalled"),
        Err(e) => println!("[NOTE] Service removal: {} (continuing)", e),
    }

    println!("\n[INFO] Installing fresh...");
    cmd_install(InstallOpts {
        pattern,
        regex,
        profile_only: false,
        service_only: false,
        custom_profile: None,
        per_user: false,
        skip_hdr: false,
        skip_hash_check: false,
        force: false,
        skip_detect: false,
        dry_run: false,
    })
}

// ============================================================================
// Test / probe commands
// ============================================================================

fn cmd_test(action: TestAction) -> Result<(), Box<dyn Error>> {
    match action {
        TestAction::Toast { title, body } => {
            println!("[INFO] Sending test toast notification...");
            println!("[INFO] Title: {}", title);
            println!("[INFO] Body:  {}", body);
            lg_notify::show_reapply_toast(true, &title, &body, true);
            println!("[DONE] Toast notification sent (check your notification center).");
        }
        TestAction::Profile => {
            let cfg = Config::load();
            let profile_path = resolve_active_profile_path(&cfg);
            let selected_preset = effective_preset_for_mode(&cfg, false);
            let preset = lg_profile::parse_dynamic_icc_preset(&selected_preset);
            let active_gamma = preset.gamma(cfg.icc_gamma);
            let generated = lg_profile::generate_dynamic_profile_bytes_with_luminance_and_tuning(
                active_gamma,
                cfg.icc_luminance_cd_m2,
                tuning_for_active_preset(&cfg, &selected_preset),
            )?;
            println!("[INFO] Profile: {}", profile_path.display());
            println!(
                "[INFO] Installed: {}",
                if lg_profile::is_profile_installed(&profile_path) {
                    "yes"
                } else {
                    "no"
                }
            );
            println!(
                "[INFO] Dynamic size (gamma {:.3}, luminance {:.1}): {} bytes",
                active_gamma,
                cfg.icc_luminance_cd_m2,
                generated.len()
            );
            let generated_report = lg_profile::validate_icc_profile_bytes(&generated);
            println!(
                "[INFO] Generated ICC validation: {}",
                if generated_report.is_valid() {
                    "valid"
                } else {
                    "INVALID"
                }
            );
            if !generated_report.warnings.is_empty() {
                println!(
                    "[NOTE] Generated ICC warnings: {}",
                    generated_report.warnings.join(" | ")
                );
            }
            if !generated_report.errors.is_empty() {
                println!(
                    "[WARN] Generated ICC errors: {}",
                    generated_report.errors.join(" | ")
                );
            }

            // Verify profile on disk matches generated content
            if lg_profile::is_profile_installed(&profile_path) {
                let on_disk = std::fs::read(&profile_path)?;
                if on_disk == generated {
                    println!("[OK] Profile on disk matches generated dynamic ICC");
                } else {
                    println!(
                        "[WARN] Profile on disk ({} bytes) differs from generated ({} bytes)",
                        on_disk.len(),
                        generated.len()
                    );
                }

                let on_disk_report = lg_profile::validate_icc_profile_bytes(&on_disk);
                println!(
                    "[INFO] On-disk ICC validation: {}",
                    if on_disk_report.is_valid() {
                        "valid"
                    } else {
                        "INVALID"
                    }
                );
                if !on_disk_report.warnings.is_empty() {
                    println!(
                        "[NOTE] On-disk ICC warnings: {}",
                        on_disk_report.warnings.join(" | ")
                    );
                }
                if !on_disk_report.errors.is_empty() {
                    println!(
                        "[WARN] On-disk ICC errors: {}",
                        on_disk_report.errors.join(" | ")
                    );
                }
            } else {
                println!("[NOTE] Profile not installed — run 'install' to generate it");
            }
        }
        TestAction::Monitors { pattern, regex } => {
            let cfg = Config::load();
            let pattern = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            let use_regex = effective_regex(regex, &cfg);
            println!("[INFO] Testing monitor detection...");
            println!(
                "[INFO] Pattern: \"{}\" ({})",
                pattern,
                if use_regex { "regex" } else { "substring" }
            );
            println!();

            let devices = find_matching_monitors(pattern, use_regex)?;
            if devices.is_empty() {
                println!("[WARN] No monitors matching \"{}\"", pattern);
            } else {
                println!("[OK] Found {} monitor(s):\n", devices.len());
                for (i, device) in devices.iter().enumerate() {
                    println!("  {}. {}", i + 1, device.name);
                    println!("     Device key: {}", device.device_key);
                    println!(
                        "     Serial: {}",
                        if device.serial.is_empty() {
                            "(unknown)"
                        } else {
                            &device.serial
                        }
                    );
                }
            }
        }
    }
    Ok(())
}

fn parse_signature_or_err(value: &str) -> Result<u32, Box<dyn Error>> {
    let signature = lg_profile::parse_icc_signature_or_zero(value);
    if signature == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "invalid ICC signature \"{}\" (expected 1-4 ASCII chars)",
                value
            ),
        )
        .into());
    }
    Ok(signature)
}

fn wrap_payload_with_type_signature(type_signature: u32, payload: &[u8]) -> Vec<u8> {
    if type_signature == lg_profile::parse_icc_signature_or_zero("data") {
        return lg_profile::build_icc_data_type_payload(payload);
    }
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&type_signature.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn cmd_icc(action: IccAction, dry_run: bool) -> Result<(), Box<dyn Error>> {
    match action {
        IccAction::FromToml {
            input,
            output,
            preset,
            gamma,
            luminance,
            monitor_name,
            serial,
            manufacturer_id,
            product_code,
            device_key,
        } => {
            let cfg = if let Some(path) = input.as_deref() {
                let contents = std::fs::read_to_string(path)?;
                toml::from_str::<Config>(&contents)?
            } else {
                Config::load()
            };
            let selected_preset = preset.unwrap_or_else(|| effective_preset_for_mode(&cfg, false));
            let preset = lg_profile::parse_dynamic_icc_preset(&selected_preset);
            let active_gamma = gamma.unwrap_or_else(|| preset.gamma(cfg.icc_gamma));
            let active_luminance = luminance.unwrap_or(cfg.icc_luminance_cd_m2);
            let tuning = tuning_for_active_preset(&cfg, &selected_preset);
            let identity = if monitor_name.is_some()
                || serial.is_some()
                || manufacturer_id.is_some()
                || product_code.is_some()
                || device_key.is_some()
            {
                Some(lg_profile::DynamicMonitorIdentity {
                    monitor_name: monitor_name.unwrap_or_default(),
                    serial_number: serial.unwrap_or_default(),
                    manufacturer_id: manufacturer_id.unwrap_or_default(),
                    product_code: product_code.unwrap_or_default(),
                    device_key: device_key.unwrap_or_default(),
                })
            } else {
                None
            };
            let bytes =
                lg_profile::generate_dynamic_profile_bytes_with_luminance_tuning_identity_and_extra_tags(
                    active_gamma,
                    active_luminance,
                    tuning,
                    identity.as_ref(),
                    &[],
                )?;
            if dry_run {
                println!("[DRY RUN] Would write ICC to {}", output);
            } else {
                std::fs::write(&output, &bytes)?;
                println!("[OK] ICC generated: {}", output);
            }
            let report = lg_profile::validate_icc_profile_bytes(&bytes);
            println!("[INFO] Size: {} bytes", bytes.len());
            println!(
                "[INFO] Validation: {}",
                if report.is_valid() {
                    "valid"
                } else {
                    "INVALID"
                }
            );
            if !report.warnings.is_empty() {
                println!("[NOTE] Warnings: {}", report.warnings.join(" | "));
            }
            if !report.errors.is_empty() {
                println!("[WARN] Errors: {}", report.errors.join(" | "));
            }
        }
        IccAction::Validate { input, detailed } => {
            let bytes = std::fs::read(&input)?;
            let report = lg_profile::validate_icc_profile_bytes(&bytes);
            println!(
                "[INFO] ICC validation for {}: {}",
                input,
                if report.is_valid() {
                    "valid"
                } else {
                    "INVALID"
                }
            );
            println!(
                "[INFO] Tags: {:?}, size: {}, declared: {:?}, known: {}, unknown: {}",
                report.tag_count,
                report.actual_size,
                report.declared_size,
                report.known_tag_count,
                report.unknown_tag_count
            );
            if detailed {
                println!("[INFO] Tag details ({}):", report.tag_details.len());
                for detail in &report.tag_details {
                    let type_sig = detail.type_signature.as_deref().unwrap_or("----");
                    let reserved = match detail.reserved_bytes_zero {
                        Some(true) => "reserved=ok",
                        Some(false) => "reserved=nonzero",
                        None => "reserved=n/a",
                    };
                    println!(
                        "  - {} type={} size={} known_sig={} known_type={} {}",
                        detail.signature,
                        type_sig,
                        detail.payload_size,
                        detail.known_signature,
                        detail.known_type_signature,
                        reserved
                    );
                }
            }
            if !report.warnings.is_empty() {
                println!("[NOTE] Warnings: {}", report.warnings.join(" | "));
            }
            if !report.errors.is_empty() {
                println!("[WARN] Errors: {}", report.errors.join(" | "));
                return Err("ICC validation failed".into());
            }
        }
        IccAction::Inspect { input, detailed } => {
            let bytes = std::fs::read(&input)?;
            let report = lg_profile::inspect_icc_profile_bytes(&bytes)?;
            println!("[INFO] ICC: {}", input);
            println!("[INFO] Size: {}", report.profile_size);
            println!("[INFO] Class: {}", report.device_class);
            println!("[INFO] Color space: {}", report.data_color_space);
            println!(
                "[INFO] Known tags: {}, unknown tags: {}",
                report.known_tag_count, report.unknown_tag_count
            );
            println!("[INFO] Tags ({}):", report.tag_signatures.len());
            for sig in &report.tag_signatures {
                println!("  - {}", sig);
            }
            if detailed {
                println!("[INFO] Detailed tags:");
                for detail in &report.tag_details {
                    let type_sig = detail.type_signature.as_deref().unwrap_or("----");
                    let reserved = match detail.reserved_bytes_zero {
                        Some(true) => "reserved=ok",
                        Some(false) => "reserved=nonzero",
                        None => "reserved=n/a",
                    };
                    println!(
                        "  - {} type={} size={} known_sig={} known_type={} {}",
                        detail.signature,
                        type_sig,
                        detail.payload_size,
                        detail.known_signature,
                        detail.known_type_signature,
                        reserved
                    );
                }
            }
        }
        IccAction::Normalize { input, output } => {
            let bytes = std::fs::read(&input)?;
            let normalized = lg_profile::normalize_icc_profile_bytes(&bytes)?;
            if dry_run {
                println!("[DRY RUN] Would write normalized ICC to {}", output);
            } else {
                std::fs::write(&output, &normalized)?;
                println!("[OK] Normalized ICC written to {}", output);
            }
            println!("[INFO] Size: {} -> {} bytes", bytes.len(), normalized.len());
        }
        IccAction::SetTag {
            input,
            output,
            signature,
            type_signature,
            payload_hex,
            payload_text,
        } => {
            let tag_signature = parse_signature_or_err(&signature)?;
            let payload = if let Some(hex) = payload_hex.as_deref() {
                parse_hex_bytes(hex)?
            } else if let Some(text) = payload_text.as_deref() {
                text.as_bytes().to_vec()
            } else {
                Vec::new()
            };
            let payload = if let Some(ts) = type_signature.as_deref() {
                let type_sig = parse_signature_or_err(ts)?;
                wrap_payload_with_type_signature(type_sig, &payload)
            } else {
                payload
            };
            let source = std::fs::read(&input)?;
            let patched = lg_profile::patch_icc_profile_bytes(
                &source,
                &[lg_profile::ExtraRawTag {
                    signature: tag_signature,
                    payload,
                }],
                &[],
            )?;
            if dry_run {
                println!(
                    "[DRY RUN] Would write ICC with tag {} to {}",
                    signature, output
                );
            } else {
                std::fs::write(&output, &patched)?;
                println!("[OK] Wrote ICC with tag {} to {}", signature, output);
            }
        }
        IccAction::RemoveTag {
            input,
            output,
            signature,
        } => {
            let tag_signature = parse_signature_or_err(&signature)?;
            let source = std::fs::read(&input)?;
            let patched = lg_profile::patch_icc_profile_bytes(&source, &[], &[tag_signature])?;
            if dry_run {
                println!(
                    "[DRY RUN] Would remove tag {} and write {}",
                    signature, output
                );
            } else {
                std::fs::write(&output, &patched)?;
                println!("[OK] Removed tag {} and wrote {}", signature, output);
            }
        }
        IccAction::ImportI1 { input, output } => {
            let source = std::fs::read(&input)?;
            let inspection = lg_profile::inspect_icc_profile_bytes(&source)?;
            let is_likely_i1 = source.windows(10).any(|w| {
                let text = String::from_utf8_lossy(w).to_ascii_lowercase();
                text.contains("i1profiler") || text.contains("x-rite")
            });
            let normalized = lg_profile::normalize_icc_profile_bytes(&source)?;
            let report = lg_profile::validate_icc_profile_bytes(&normalized);
            let output_path = output.unwrap_or_else(|| {
                let in_path = std::path::PathBuf::from(&input);
                let stem = in_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("profile");
                let mut out = in_path.clone();
                out.set_file_name(format!("{}.normalized.icc", stem));
                out.to_string_lossy().to_string()
            });
            if dry_run {
                println!(
                    "[DRY RUN] Would import i1 profile to {} (likely_i1={})",
                    output_path, is_likely_i1
                );
            } else {
                std::fs::write(&output_path, &normalized)?;
                println!(
                    "[OK] Imported i1 profile to {} (likely_i1={})",
                    output_path, is_likely_i1
                );
            }
            println!(
                "[INFO] Source tags: {}, class: {}, color space: {}",
                inspection.tag_signatures.len(),
                inspection.device_class,
                inspection.data_color_space
            );
            println!(
                "[INFO] Validation: {}",
                if report.is_valid() {
                    "valid"
                } else {
                    "INVALID"
                }
            );
            if !report.warnings.is_empty() {
                println!("[NOTE] Warnings: {}", report.warnings.join(" | "));
            }
            if !report.errors.is_empty() {
                println!("[WARN] Errors: {}", report.errors.join(" | "));
                return Err("imported ICC failed validation".into());
            }
        }
        IccAction::Optimize {
            input,
            tuning_preset,
            gamma,
            luminance,
            apply,
            pattern,
            regex,
            save_config,
            output,
        } => {
            let mut cfg = if let Some(path) = input.as_deref() {
                let contents = std::fs::read_to_string(path)?;
                toml::from_str::<Config>(&contents)?
            } else {
                Config::load()
            };

            if let Some(preset) = tuning_preset {
                cfg.icc_tuning_preset = preset;
                // Choosing a tuning preset should be deterministic.
                // Manual overlay can be re-enabled explicitly if needed.
                cfg.icc_tuning_overlay_manual = false;
            }
            if let Some(g) = gamma {
                cfg.icc_gamma = lg_profile::sanitize_dynamic_gamma(g);
                cfg.icc_active_preset = "custom".to_string();
                sync_mode_presets_to_active(&mut cfg);
            }
            if let Some(l) = luminance {
                cfg.icc_luminance_cd_m2 = lg_profile::sanitize_dynamic_luminance_cd_m2(l);
                cfg.icc_active_preset = "custom".to_string();
                sync_mode_presets_to_active(&mut cfg);
            }
            if let Some(pat) = pattern {
                cfg.monitor_match = pat;
            }
            if regex {
                cfg.monitor_match_regex = true;
            }

            let active_preset = effective_preset_for_mode(&cfg, false);
            let active_gamma =
                lg_profile::parse_dynamic_icc_preset(&active_preset).gamma(cfg.icc_gamma);
            let tuning = tuning_for_active_preset(&cfg, &active_preset);
            let color_dir = lg_profile::color_directory();
            let (profile_path, bytes) = if dry_run {
                (
                    lg_profile::resolve_active_profile_path(
                        &color_dir,
                        &active_preset,
                        &cfg.profile_name,
                    ),
                    lg_profile::generate_dynamic_profile_bytes_with_luminance_and_tuning(
                        active_gamma,
                        cfg.icc_luminance_cd_m2,
                        tuning,
                    )?,
                )
            } else {
                let profile_path = lg_profile::ensure_active_profile_installed_tuned(
                    &color_dir,
                    &active_preset,
                    &cfg.profile_name,
                    active_gamma,
                    cfg.icc_luminance_cd_m2,
                    cfg.icc_generate_specialized_profiles,
                    tuning,
                )?;
                let bytes = std::fs::read(&profile_path)?;
                (profile_path, bytes)
            };
            let report = lg_profile::validate_icc_profile_bytes(&bytes);

            if dry_run {
                println!(
                    "[DRY RUN] Would generate optimized ICC at {}",
                    profile_path.display()
                );
            } else {
                println!("[OK] Optimized ICC generated: {}", profile_path.display());
            }
            println!(
                "[INFO] Preset='{}' active_preset='{}' gamma={:.3} luminance={:.1}",
                cfg.icc_tuning_preset, active_preset, active_gamma, cfg.icc_luminance_cd_m2
            );
            println!(
                "[INFO] Resolved tuning: lift={:.3} mid={:.3} comp={:.3} vcgt={} strength={:.3} target_black={:.3}",
                tuning.black_lift,
                tuning.midtone_boost,
                tuning.white_compression,
                tuning.vcgt_enabled,
                tuning.vcgt_strength,
                tuning.target_black_cd_m2
            );
            println!(
                "[INFO] Validation: {}",
                if report.is_valid() {
                    "valid"
                } else {
                    "INVALID"
                }
            );
            if !report.warnings.is_empty() {
                println!("[NOTE] Warnings: {}", report.warnings.join(" | "));
            }
            if !report.errors.is_empty() {
                println!("[WARN] Errors: {}", report.errors.join(" | "));
                return Err("optimized ICC validation failed".into());
            }

            if let Some(path) = output {
                if dry_run {
                    println!("[DRY RUN] Would export ICC to {}", path);
                } else {
                    std::fs::write(&path, &bytes)?;
                    println!("[OK] Exported ICC to {}", path);
                }
            }

            if save_config {
                if input.is_some() {
                    return Err(
                        "--save-config is only supported when using default app config.toml".into(),
                    );
                }
                if dry_run {
                    println!(
                        "[DRY RUN] Would save optimized ICC settings to {}",
                        config::config_path().display()
                    );
                } else {
                    Config::write_config(&cfg)?;
                    println!(
                        "[OK] Saved optimized ICC settings to {}",
                        config::config_path().display()
                    );
                }
            }

            if apply {
                let use_regex = cfg.monitor_match_regex;
                let devices = find_matching_monitors(&cfg.monitor_match, use_regex)?;
                if devices.is_empty() {
                    println!("[WARN] No matching monitors found for apply.");
                } else {
                    let active_hdr_mode = lg_monitor::is_any_display_hdr_enabled().unwrap_or(false);
                    let sdr_preset = effective_preset_for_mode(&cfg, false);
                    let hdr_preset = effective_preset_for_mode(&cfg, true);
                    if dry_run {
                        for device in &devices {
                            let (sdr_profile_for_device, hdr_profile_for_device) =
                                if cfg.icc_per_monitor_profiles {
                                    let identity = identity_from_monitor(device);
                                    let sdr_base_name =
                                        lg_profile::parse_dynamic_icc_preset(&sdr_preset)
                                            .profile_name(&cfg.profile_name);
                                    let hdr_base_name =
                                        lg_profile::parse_dynamic_icc_preset(&hdr_preset)
                                            .profile_name(&cfg.profile_name);
                                    (
                                        lg_profile::resolve_monitor_scoped_profile_path(
                                            &color_dir,
                                            &sdr_base_name,
                                            &identity,
                                        ),
                                        lg_profile::resolve_monitor_scoped_profile_path(
                                            &color_dir,
                                            &hdr_base_name,
                                            &identity,
                                        ),
                                    )
                                } else {
                                    (
                                        lg_profile::resolve_active_profile_path(
                                            &color_dir,
                                            &sdr_preset,
                                            &cfg.profile_name,
                                        ),
                                        lg_profile::resolve_active_profile_path(
                                            &color_dir,
                                            &hdr_preset,
                                            &cfg.profile_name,
                                        ),
                                    )
                                };
                            let active_profile_for_device = if active_hdr_mode {
                                &hdr_profile_for_device
                            } else {
                                &sdr_profile_for_device
                            };
                            println!(
                                "[DRY RUN] Would apply optimized ICC active={} (sdr={}, hdr={}) to {}",
                                active_profile_for_device.display(),
                                sdr_profile_for_device.display(),
                                hdr_profile_for_device.display(),
                                device.name
                            );
                        }
                        println!("[DRY RUN] Would refresh display and trigger calibration loader");
                        println!(
                            "[DRY RUN] Would apply optimized ICC to {} monitor(s).",
                            devices.len()
                        );
                    } else {
                        let shared_mode_profiles = if cfg.icc_per_monitor_profiles {
                            None
                        } else {
                            Some(lg_profile::ensure_mode_profiles_installed_tuned(
                                &color_dir,
                                &sdr_preset,
                                &hdr_preset,
                                &cfg.profile_name,
                                cfg.icc_gamma,
                                cfg.icc_luminance_cd_m2,
                                cfg.icc_generate_specialized_profiles,
                                tuning,
                            )?)
                        };
                        for device in &devices {
                            let (sdr_profile_for_device, hdr_profile_for_device) =
                                if let Some((sdr, hdr)) = &shared_mode_profiles {
                                    (sdr.clone(), hdr.clone())
                                } else {
                                    let identity = identity_from_monitor(device);
                                    lg_profile::ensure_mode_profiles_installed_tuned_for_monitor(
                                        &color_dir,
                                        &sdr_preset,
                                        &hdr_preset,
                                        &cfg.profile_name,
                                        cfg.icc_gamma,
                                        cfg.icc_luminance_cd_m2,
                                        cfg.icc_generate_specialized_profiles,
                                        tuning,
                                        &identity,
                                    )?
                                };
                            let active_profile_for_device = if active_hdr_mode {
                                &hdr_profile_for_device
                            } else {
                                &sdr_profile_for_device
                            };
                            lg_profile::reapply_profile_with_mode_associations(
                                &device.device_key,
                                active_profile_for_device,
                                &sdr_profile_for_device,
                                &hdr_profile_for_device,
                                cfg.toggle_delay_ms,
                                false,
                            )?;
                            println!("[OK] Applied optimized ICC to {}", device.name);
                        }
                        // Use a non-disruptive refresh first to avoid monitor
                        // mode flashes/flicker during successful apply.
                        lg_profile::refresh_display(false, true, false);
                        lg_profile::trigger_calibration_loader(true);
                        println!(
                            "[DONE] Optimized ICC applied to {} monitor(s).",
                            devices.len()
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

// ============================================================================
// DDC/CI commands
// ============================================================================

fn cmd_ddc(action: DdcAction, dry_run: bool) -> Result<(), Box<dyn Error>> {
    let cfg = Config::load();

    match action {
        DdcAction::Brightness { value, pattern } => {
            if value > 100 {
                return Err("Brightness value must be 0–100".into());
            }
            if let Some(message) = ddc_guardrail_error(lg_monitor::ddc::VCP_BRIGHTNESS, value) {
                return Err(message.into());
            }
            if dry_run {
                println!("[DRY RUN] Would set DDC brightness to {}", value);
                return Ok(());
            }
            if let Some(ref pat) = pattern {
                println!(
                    "[INFO] Setting DDC brightness to {} for monitors matching \"{}\"...",
                    value, pat
                );
                set_vcp_with_safety(pat, lg_monitor::ddc::VCP_BRIGHTNESS, value)?;
                println!("[OK] Brightness set to {}", value);
            } else {
                println!(
                    "[INFO] Setting DDC brightness to {} on all monitors...",
                    value
                );
                let count = lg_monitor::ddc::set_brightness_all(value)?;
                println!("[OK] Brightness set to {} on {} monitor(s)", value, count);
            }
        }

        DdcAction::ColorPreset { pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            println!("[INFO] Reading color preset from \"{}\"...", pat);
            let val = lg_monitor::ddc::get_vcp_by_pattern(pat, lg_monitor::ddc::VCP_COLOR_PRESET)?;
            let name = color_preset_name(val.current);
            println!(
                "[OK] Color Preset: {} (value={}, max={})",
                name, val.current, val.max
            );
        }

        DdcAction::SetColorPreset { value, pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            if let Some(message) = ddc_guardrail_error(lg_monitor::ddc::VCP_COLOR_PRESET, value) {
                return Err(message.into());
            }
            if dry_run {
                println!(
                    "[DRY RUN] Would set color preset to {} for \"{}\"",
                    value, pat
                );
                return Ok(());
            }
            let name = color_preset_name(value);
            println!(
                "[INFO] Setting color preset to {} ({}) for \"{}\"...",
                name, value, pat
            );
            set_vcp_with_safety(pat, lg_monitor::ddc::VCP_COLOR_PRESET, value)?;
            println!("[OK] Color preset set to {} ({})", name, value);
        }

        DdcAction::DisplayMode { pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            println!("[INFO] Reading display mode from \"{}\"...", pat);
            let val = lg_monitor::ddc::get_vcp_by_pattern(pat, lg_monitor::ddc::VCP_DISPLAY_MODE)?;
            println!(
                "[OK] Display Mode: current={}, max={} (type={})",
                val.current, val.max, val.vcp_type
            );
        }

        DdcAction::SetDisplayMode { value, pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            if let Some(message) = ddc_guardrail_error(lg_monitor::ddc::VCP_DISPLAY_MODE, value) {
                return Err(message.into());
            }
            if dry_run {
                println!(
                    "[DRY RUN] Would set display mode to {} for \"{}\"",
                    value, pat
                );
                return Ok(());
            }
            println!(
                "[INFO] Setting display mode to {} for \"{}\"...",
                value, pat
            );
            set_vcp_with_safety(pat, lg_monitor::ddc::VCP_DISPLAY_MODE, value)?;
            println!("[OK] Display mode set to {}", value);
        }

        DdcAction::ResetBrightnessContrast { pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            if let Some(message) =
                ddc_guardrail_error(lg_monitor::ddc::VCP_RESET_BRIGHTNESS_CONTRAST, 1)
            {
                return Err(message.into());
            }
            if dry_run {
                println!("[DRY RUN] Would reset brightness/contrast for \"{}\"", pat);
                return Ok(());
            }
            println!("[INFO] Resetting brightness + contrast for \"{}\"...", pat);
            set_vcp_with_safety(pat, lg_monitor::ddc::VCP_RESET_BRIGHTNESS_CONTRAST, 1)?;
            println!("[OK] Brightness + contrast reset sent");
        }

        DdcAction::ResetColor { pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            if let Some(message) = ddc_guardrail_error(lg_monitor::ddc::VCP_RESET_COLOR, 1) {
                return Err(message.into());
            }
            if dry_run {
                println!("[DRY RUN] Would reset color for \"{}\"", pat);
                return Ok(());
            }
            println!("[INFO] Resetting color for \"{}\"...", pat);
            set_vcp_with_safety(pat, lg_monitor::ddc::VCP_RESET_COLOR, 1)?;
            println!("[OK] Color reset sent");
        }

        DdcAction::Version { pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            println!("[INFO] Reading VCP version from \"{}\"...", pat);
            let val = lg_monitor::ddc::get_vcp_by_pattern(pat, lg_monitor::ddc::VCP_VERSION)?;
            let major = (val.current >> 8) & 0xFF;
            let minor = val.current & 0xFF;
            println!(
                "[OK] VCP Version: {}.{} (raw={})",
                major, minor, val.current
            );
        }

        DdcAction::GetVcp { code, pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            println!("[INFO] Reading VCP 0x{:02X} from \"{}\"...", code, pat);
            let val = lg_monitor::ddc::get_vcp_by_pattern(pat, code)?;
            println!(
                "[OK] VCP 0x{:02X}: current={}, max={}, type={}",
                code, val.current, val.max, val.vcp_type
            );
        }

        DdcAction::SetVcp {
            code,
            value,
            pattern,
        } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            if let Some(message) = ddc_guardrail_error(code, value) {
                return Err(message.into());
            }
            if dry_run {
                println!(
                    "[DRY RUN] Would set VCP 0x{:02X} = {} for \"{}\"",
                    code, value, pat
                );
                return Ok(());
            }
            println!(
                "[INFO] Setting VCP 0x{:02X} = {} for \"{}\"...",
                code, value, pat
            );
            set_vcp_with_safety(pat, code, value)?;
            println!("[OK] VCP 0x{:02X} set to {}", code, value);
        }

        DdcAction::List => {
            println!("[INFO] Listing physical monitors via DDC/CI...\n");
            let monitors = lg_monitor::ddc::list_physical_monitors()?;
            if monitors.is_empty() {
                println!("  (no physical monitors found)");
            } else {
                for (idx, desc) in &monitors {
                    let label = if desc.is_empty() {
                        "(no description)"
                    } else {
                        desc.as_str()
                    };
                    println!("  [{}] {}", idx, label);
                }
                println!("\n[OK] {} physical monitor(s) found", monitors.len());
            }
        }
        DdcAction::Map => {
            println!("[INFO] Probing monitor DDC capability map...\n");
            let maps = lg_monitor::ddc::probe_monitor_capabilities()?;
            if maps.is_empty() {
                println!("  (no physical monitors found)");
                return Ok(());
            }
            for map in maps {
                println!(
                    "── Monitor #{}: {} ──",
                    map.index,
                    if map.name.trim().is_empty() {
                        "(unknown)"
                    } else {
                        map.name.as_str()
                    }
                );
                for cap in map.capabilities {
                    if cap.supported {
                        println!(
                            "  0x{:02X} {:28} supported current={:?} max={:?} type={:?}{}",
                            cap.code,
                            cap.label,
                            cap.current,
                            cap.max,
                            cap.vcp_type,
                            if cap.risky { " [risky]" } else { "" }
                        );
                    } else {
                        println!(
                            "  0x{:02X} {:28} not-supported{}",
                            cap.code,
                            cap.label,
                            if cap.risky { " [risky]" } else { "" }
                        );
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}

/// Human-readable color preset name from VCP 0x14 value.
fn color_preset_name(value: u32) -> &'static str {
    match value {
        1 => "sRGB",
        2 => "Native",
        4 => "4000K",
        5 => "5000K",
        6 => "6500K",
        8 => "7500K",
        9 => "8200K",
        10 => "9300K",
        11 => "User 1",
        12 => "User 2",
        13 => "User 3",
        _ => "Unknown",
    }
}

fn cmd_probe(pattern: Option<String>, regex: bool) -> Result<(), Box<dyn Error>> {
    let cfg = Config::load();
    let pattern_str = pattern.as_deref().unwrap_or(&cfg.monitor_match);
    let use_regex = effective_regex(regex, &cfg);
    let selected_preset = effective_preset_for_mode(&cfg, false);
    let preset = lg_profile::parse_dynamic_icc_preset(&selected_preset);
    let active_gamma = preset.gamma(cfg.icc_gamma);

    println!("═══ LG UltraGear Probe ═══\n");

    // Profile status
    let profile_path = resolve_active_profile_path(&cfg);
    println!("── Profile ──");
    println!("  Path:      {}", profile_path.display());
    println!(
        "  Installed: {}",
        if lg_profile::is_profile_installed(&profile_path) {
            "yes ✓"
        } else {
            "no ✗"
        }
    );
    println!(
        "  Dynamic:   {} bytes (gamma {:.3}, luminance {:.1})",
        lg_profile::generate_dynamic_profile_bytes_with_luminance_and_tuning(
            active_gamma,
            cfg.icc_luminance_cd_m2,
            tuning_for_active_preset(&cfg, &selected_preset),
        )?
        .len(),
        active_gamma,
        cfg.icc_luminance_cd_m2
    );

    // Service status
    println!("\n── Service ──");
    let (installed, running) = lg_service::query_service_info();
    println!("  Installed: {}", if installed { "yes ✓" } else { "no ✗" });
    println!("  Running:   {}", if running { "yes ✓" } else { "no ✗" });

    // Config summary
    println!("\n── Config ──");
    println!("  File:    {}", config::config_path().display());
    println!("  Pattern: \"{}\"", cfg.monitor_match);
    println!(
        "  Match:   {}",
        if use_regex { "regex" } else { "substring" }
    );
    println!("  Preset:  \"{}\"", selected_preset);
    println!("  Gamma:   {:.3}", active_gamma);
    println!("  Lumi:    {:.1} cd/m^2", cfg.icc_luminance_cd_m2);
    println!(
        "  Toast:   {}",
        if cfg.toast_enabled { "on" } else { "off" }
    );
    println!("  Verbose: {}", cfg.verbose);

    // Monitor detection
    println!("\n── Monitors (matching \"{}\") ──", pattern_str);
    let devices = find_matching_monitors(pattern_str, use_regex)?;
    if devices.is_empty() {
        println!("  (none found)");
    } else {
        for (i, device) in devices.iter().enumerate() {
            println!("  {}. {}", i + 1, device.name);
            println!("     Device: {}", device.device_key);
            println!(
                "     Serial: {}",
                if device.serial.is_empty() {
                    "(unknown)"
                } else {
                    &device.serial
                }
            );
        }
    }

    println!("\n═══ Probe complete ═══");
    Ok(())
}

#[cfg(test)]
#[path = "tests/main_tests.rs"]
mod tests;
