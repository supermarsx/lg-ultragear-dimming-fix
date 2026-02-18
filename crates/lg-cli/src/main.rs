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

mod tui;

#[derive(Parser)]
#[command(
    name = "lg-ultragear-dimming-fix",
    version,
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

        /// Install ICC profile only (no service)
        #[arg(long, conflicts_with = "service_only")]
        profile_only: bool,

        /// Install service only (skip explicit profile extraction)
        #[arg(long, conflicts_with = "profile_only")]
        service_only: bool,
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
    },

    /// Detect connected monitors matching a pattern
    Detect {
        /// Monitor name pattern (case-insensitive substring match)
        #[arg(short, long)]
        pattern: Option<String>,
    },

    /// One-shot profile reapply for matching monitors
    Apply {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
    },

    /// Run event watcher in foreground (Ctrl+C to stop)
    Watch {
        /// Monitor name pattern override
        #[arg(short, long)]
        pattern: Option<String>,
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

fn main() -> Result<(), Box<dyn Error>> {
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
            return tui::run();
        }
        // Non-interactive or not a terminal → show help
        use clap::CommandFactory;
        Cli::command().print_help()?;
        println!();
        return Ok(());
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
            profile_only,
            service_only,
        }) => cmd_install(pattern, profile_only, service_only, cli.dry_run)?,
        Some(Commands::Uninstall { full, profile }) => {
            cmd_uninstall(full, profile, cli.dry_run)?
        }
        Some(Commands::Reinstall { pattern }) => cmd_reinstall(pattern, cli.dry_run)?,
        Some(Commands::Detect { pattern }) => cmd_detect(pattern)?,
        Some(Commands::Apply { pattern }) => cmd_apply(pattern, cli.verbose, cli.dry_run)?,
        Some(Commands::Watch { pattern }) => cmd_watch(pattern)?,
        Some(Commands::Config { action }) => cmd_config(action)?,
        Some(Commands::Service { action }) => cmd_service(action)?,
    }

    Ok(())
}

// ============================================================================
// Command implementations
// ============================================================================

fn cmd_detect(pattern: Option<String>) -> Result<(), Box<dyn Error>> {
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

fn cmd_apply(pattern: Option<String>, verbose: bool, dry_run: bool) -> Result<(), Box<dyn Error>> {
    let mut cfg = Config::load();
    if let Some(ref p) = pattern {
        cfg.monitor_match = p.clone();
    }
    if verbose {
        cfg.verbose = true;
    }
    let profile_path = cfg.profile_path();

    println!("[INFO] Running one-shot profile reapply...");
    println!("[INFO] Config:  {}", config::config_path().display());
    println!("[INFO] Pattern: {}", cfg.monitor_match);
    println!("[INFO] Profile: {}", profile_path.display());
    println!(
        "[INFO] Toast:   {}",
        if cfg.toast_enabled { "on" } else { "off" }
    );
    println!();

    // Auto-extract embedded ICC profile if not already present
    lg_profile::ensure_profile_installed(&profile_path)?;

    if !lg_profile::is_profile_installed(&profile_path) {
        return Err(format!("ICC profile not found: {}", profile_path.display()).into());
    }

    if dry_run {
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
            lg_profile::reapply_profile(&device.device_key, &profile_path, cfg.toggle_delay_ms)?;
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

fn cmd_watch(pattern: Option<String>) -> Result<(), Box<dyn Error>> {
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
        ServiceAction::Install { pattern } => {
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
            println!("     Binary removed from: {}", config::install_path().display());
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

fn cmd_install(
    pattern: Option<String>,
    profile_only: bool,
    service_only: bool,
    dry_run: bool,
) -> Result<(), Box<dyn Error>> {
    let mut cfg = Config::load();
    if let Some(ref p) = pattern {
        cfg.monitor_match = p.clone();
    }

    if profile_only {
        // Profile-only install
        if dry_run {
            println!("[DRY RUN] Would extract ICC profile to color store");
            return Ok(());
        }
        let profile_path = cfg.profile_path();
        match lg_profile::ensure_profile_installed(&profile_path)? {
            true => println!("[OK] ICC profile installed to {}", profile_path.display()),
            false => println!("[OK] ICC profile already present"),
        }
        println!("[DONE] Profile install complete.");
        return Ok(());
    }

    if dry_run {
        if !service_only {
            println!("[DRY RUN] Would extract ICC profile to color store");
        }
        println!("[DRY RUN] Would write default config");
        println!("[DRY RUN] Would install Windows service");
        println!("[DRY RUN] Would start service");
        return Ok(());
    }

    // Extract ICC profile (unless service-only)
    if !service_only {
        let profile_path = cfg.profile_path();
        match lg_profile::ensure_profile_installed(&profile_path)? {
            true => println!("[OK] ICC profile installed to {}", profile_path.display()),
            false => println!("[OK] ICC profile already present"),
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
    if pattern.is_some() {
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
                println!(
                    "     Binary removed from: {}",
                    config::install_path().display()
                );
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
    }

    // Remove config directory if full uninstall
    if full {
        let cfg_dir = config::config_dir();
        if cfg_dir.exists() {
            match std::fs::remove_dir_all(&cfg_dir) {
                Ok(()) => println!("[OK] Config directory removed: {}", cfg_dir.display()),
                Err(e) => println!(
                    "[WARN] Could not remove config dir: {} (clean up manually)",
                    e
                ),
            }
        }
    }

    if !full && !profile {
        println!(
            "     Config preserved at: {}",
            config::config_path().display()
        );
    }

    println!("[DONE] Uninstall complete.");
    Ok(())
}

fn cmd_reinstall(pattern: Option<String>, dry_run: bool) -> Result<(), Box<dyn Error>> {
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
    cmd_install(pattern, false, false, false)
}
