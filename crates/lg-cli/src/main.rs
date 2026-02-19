//! LG UltraGear — display color profile CLI tool.
//!
//! A full-featured command-line tool for managing ICC color profiles on
//! LG UltraGear displays. Prevents dimming by reapplying a calibrated
//! profile on display connect, session unlock, and logon events.
//!
//! Can also run as a Windows service for always-on monitoring.

use clap::{Parser, Subcommand};
use lg_core::config::{self, Config};
use std::error::Error;
use std::io::IsTerminal;

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

        /// Path to a custom ICC/ICM profile (uses embedded profile by default)
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

    /// DDC/CI monitor control (brightness, color presets, display mode, resets)
    Ddc {
        #[command(subcommand)]
        action: DdcAction,
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
}

/// Parse a hex string (with or without 0x prefix) into a u8.
fn parse_hex_u8(s: &str) -> Result<u8, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u8::from_str_radix(s, 16).map_err(|e| format!("Invalid hex byte '{}': {}", s, e))
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
        Some(Commands::Ddc { action }) => cmd_ddc(action, cli.dry_run)?,
        Some(Commands::Probe { pattern, regex }) => cmd_probe(pattern, regex)?,
    }

    Ok(())
}

// ============================================================================
// Command implementations
// ============================================================================

fn cmd_detect(pattern: Option<String>, _regex: bool) -> Result<(), Box<dyn Error>> {
    let cfg = Config::load();
    let pattern = pattern.as_deref().unwrap_or(&cfg.monitor_match);

    println!("Scanning for monitors matching \"{}\"...\n", pattern);

    let devices = lg_monitor::find_matching_monitors(pattern)?;
    if devices.is_empty() {
        println!("No matching monitors found.");
    } else {
        println!("Found {} monitor(s):\n", devices.len());
        for (i, device) in devices.iter().enumerate() {
            println!("  {}. {}", i + 1, device.name);
            println!("     Device: {}", device.device_key);
        }
    }

    println!("\nProfile: {}", cfg.profile_path().display());
    // Auto-extract embedded ICC profile if not already present
    let _ = lg_profile::ensure_profile_installed(&cfg.profile_path());
    println!(
        "Installed: {}",
        if lg_profile::is_profile_installed(&cfg.profile_path()) {
            "yes"
        } else {
            "NO — extraction failed, check permissions"
        }
    );

    Ok(())
}

/// Options for apply command (avoids too-many-arguments lint).
struct ApplyOpts {
    pattern: Option<String>,
    #[allow(dead_code)]
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
    let profile = if let Some(ref custom) = opts.profile_path {
        std::path::PathBuf::from(custom)
    } else {
        cfg.profile_path()
    };

    println!("[INFO] Running one-shot profile reapply...");
    println!("[INFO] Config:  {}", config::config_path().display());
    println!("[INFO] Pattern: {}", cfg.monitor_match);
    println!("[INFO] Profile: {}", profile.display());
    println!(
        "[INFO] Toast:   {}",
        if cfg.toast_enabled { "on" } else { "off" }
    );
    println!();

    // Auto-extract embedded ICC profile if not already present
    lg_profile::ensure_profile_installed(&profile)?;

    if !lg_profile::is_profile_installed(&profile) {
        return Err(format!("ICC profile not found: {}", profile.display()).into());
    }

    if opts.dry_run {
        let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
        println!(
            "[DRY RUN] Would reapply profile for {} matching monitor(s)",
            devices.len()
        );
        return Ok(());
    }

    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    if devices.is_empty() {
        println!("[SKIP] No matching monitors found.");
    } else {
        for device in &devices {
            println!("[INFO] Found: {}", device.name);
            lg_profile::reapply_profile(&device.device_key, &profile, cfg.toggle_delay_ms, false)?;
            println!("[OK]   Profile reapplied for {}", device.name);
        }

        lg_profile::refresh_display(
            cfg.refresh_display_settings,
            cfg.refresh_broadcast_color,
            cfg.refresh_invalidate,
        );
        lg_profile::trigger_calibration_loader(cfg.refresh_calibration_loader);

        if cfg.toast_enabled {
            println!("[INFO] Sending toast notification...");
            lg_notify::show_reapply_toast(true, &cfg.toast_title, &cfg.toast_body, cfg.verbose);
        }

        println!("\n[DONE] All profiles reapplied.");
    }

    Ok(())
}

fn cmd_watch(pattern: Option<String>, _regex: bool) -> Result<(), Box<dyn Error>> {
    let mut cfg = Config::load();
    if let Some(p) = pattern {
        cfg.monitor_match = p;
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
            println!("  profile_name             = \"{}\"", cfg.profile_name);
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

            lg_service::install(&cfg.monitor_match)?;
            println!(
                "[OK] Service installed. Monitor pattern: {}",
                cfg.monitor_match
            );
            println!("     Binary: {}", config::install_path().display());
            println!("     Config: {}", cfg_path.display());
            println!("     Start with: lg-ultragear-dimming-fix service start");
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

// ============================================================================
// New top-level commands (parity with PowerShell installer)
// ============================================================================

/// Options for install command (avoids too-many-arguments lint).
struct InstallOpts {
    pattern: Option<String>,
    #[allow(dead_code)]
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

    if opts.profile_only {
        // Profile-only install
        if opts.dry_run {
            println!("[DRY RUN] Would extract ICC profile to color store");
            return Ok(());
        }
        let profile_path = if let Some(ref custom) = opts.custom_profile {
            std::path::PathBuf::from(custom)
        } else {
            cfg.profile_path()
        };
        match lg_profile::ensure_profile_installed(&profile_path)? {
            true => println!("[OK] ICC profile installed to {}", profile_path.display()),
            false => {
                if opts.force {
                    // Force overwrite: remove and re-extract
                    let _ = lg_profile::remove_profile(&profile_path);
                    lg_profile::ensure_profile_installed(&profile_path)?;
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
        let stale = lg_profile::cleanup_stale_profiles(&cfg.profile_name);
        for p in &stale {
            println!("[OK] Removed stale profile: {}", p.display());
        }

        println!("[DONE] Profile install complete.");
        return Ok(());
    }

    if opts.dry_run {
        if !opts.service_only {
            println!("[DRY RUN] Would extract ICC profile to color store");
        }
        if !opts.skip_detect {
            println!("[DRY RUN] Would detect matching monitors");
        }
        println!("[DRY RUN] Would write default config");
        println!("[DRY RUN] Would install Windows service");
        println!("[DRY RUN] Would start service");
        return Ok(());
    }

    // Extract ICC profile (unless service-only)
    if !opts.service_only {
        let profile_path = if let Some(ref custom) = opts.custom_profile {
            std::path::PathBuf::from(custom)
        } else {
            cfg.profile_path()
        };
        match lg_profile::ensure_profile_installed(&profile_path)? {
            true => println!("[OK] ICC profile installed to {}", profile_path.display()),
            false => {
                if opts.force {
                    let _ = lg_profile::remove_profile(&profile_path);
                    lg_profile::ensure_profile_installed(&profile_path)?;
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
        let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
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
    if opts.pattern.is_some() {
        Config::write_config(&cfg)?;
        println!(
            "[OK] Config updated with monitor pattern: {}",
            cfg.monitor_match
        );
    }

    // Install service
    lg_service::install(&cfg.monitor_match)?;
    println!("[OK] Service installed");
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
        let profile_path = cfg.profile_path();
        match lg_profile::remove_profile(&profile_path)? {
            true => println!("[OK] ICC profile removed from {}", profile_path.display()),
            false => println!("[NOTE] ICC profile not found (already removed)"),
        }

        // Clean up any stale/leftover ICM files (from test runs, etc.)
        let stale = lg_profile::cleanup_stale_profiles(&cfg.profile_name);
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
            let profile_path = cfg.profile_path();
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
                "[INFO] Embedded size: {} bytes",
                lg_profile::EMBEDDED_ICM_SIZE
            );

            // Verify profile on disk matches embedded
            if lg_profile::is_profile_installed(&profile_path) {
                let on_disk = std::fs::read(&profile_path)?;
                if on_disk.len() == lg_profile::EMBEDDED_ICM_SIZE {
                    println!("[OK] Profile on disk matches embedded size");
                } else {
                    println!(
                        "[WARN] Profile on disk ({} bytes) differs from embedded ({} bytes)",
                        on_disk.len(),
                        lg_profile::EMBEDDED_ICM_SIZE
                    );
                }
            } else {
                println!("[NOTE] Profile not installed — run 'install' to extract");
            }
        }
        TestAction::Monitors {
            pattern,
            regex: _regex,
        } => {
            let cfg = Config::load();
            let pattern = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            println!("[INFO] Testing monitor detection...");
            println!("[INFO] Pattern: \"{}\"", pattern);
            println!();

            let devices = lg_monitor::find_matching_monitors(pattern)?;
            if devices.is_empty() {
                println!("[WARN] No monitors matching \"{}\"", pattern);
            } else {
                println!("[OK] Found {} monitor(s):\n", devices.len());
                for (i, device) in devices.iter().enumerate() {
                    println!("  {}. {}", i + 1, device.name);
                    println!("     Device key: {}", device.device_key);
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
            if dry_run {
                println!("[DRY RUN] Would set DDC brightness to {}", value);
                return Ok(());
            }
            if let Some(ref pat) = pattern {
                println!(
                    "[INFO] Setting DDC brightness to {} for monitors matching \"{}\"...",
                    value, pat
                );
                lg_monitor::ddc::set_vcp_by_pattern(pat, lg_monitor::ddc::VCP_BRIGHTNESS, value)?;
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
            lg_monitor::ddc::set_vcp_by_pattern(pat, lg_monitor::ddc::VCP_COLOR_PRESET, value)?;
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
            lg_monitor::ddc::set_vcp_by_pattern(pat, lg_monitor::ddc::VCP_DISPLAY_MODE, value)?;
            println!("[OK] Display mode set to {}", value);
        }

        DdcAction::ResetBrightnessContrast { pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            if dry_run {
                println!("[DRY RUN] Would reset brightness/contrast for \"{}\"", pat);
                return Ok(());
            }
            println!("[INFO] Resetting brightness + contrast for \"{}\"...", pat);
            lg_monitor::ddc::set_vcp_by_pattern(
                pat,
                lg_monitor::ddc::VCP_RESET_BRIGHTNESS_CONTRAST,
                1,
            )?;
            println!("[OK] Brightness + contrast reset sent");
        }

        DdcAction::ResetColor { pattern } => {
            let pat = pattern.as_deref().unwrap_or(&cfg.monitor_match);
            if dry_run {
                println!("[DRY RUN] Would reset color for \"{}\"", pat);
                return Ok(());
            }
            println!("[INFO] Resetting color for \"{}\"...", pat);
            lg_monitor::ddc::set_vcp_by_pattern(pat, lg_monitor::ddc::VCP_RESET_COLOR, 1)?;
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
            lg_monitor::ddc::set_vcp_by_pattern(pat, code, value)?;
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

fn cmd_probe(pattern: Option<String>, _regex: bool) -> Result<(), Box<dyn Error>> {
    let cfg = Config::load();
    let pattern_str = pattern.as_deref().unwrap_or(&cfg.monitor_match);

    println!("═══ LG UltraGear Probe ═══\n");

    // Profile status
    let profile_path = cfg.profile_path();
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
    println!("  Embedded:  {} bytes", lg_profile::EMBEDDED_ICM_SIZE);

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
        "  Toast:   {}",
        if cfg.toast_enabled { "on" } else { "off" }
    );
    println!("  Verbose: {}", cfg.verbose);

    // Monitor detection
    println!("\n── Monitors (matching \"{}\") ──", pattern_str);
    let devices = lg_monitor::find_matching_monitors(pattern_str)?;
    if devices.is_empty() {
        println!("  (none found)");
    } else {
        for (i, device) in devices.iter().enumerate() {
            println!("  {}. {}", i + 1, device.name);
            println!("     Device: {}", device.device_key);
        }
    }

    println!("\n═══ Probe complete ═══");
    Ok(())
}
