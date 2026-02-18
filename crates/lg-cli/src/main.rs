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

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
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

    /// Windows service management
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

    match &cli.command {
        Some(Commands::Service {
            action: ServiceAction::Run,
        }) => {
            // SCM mode — use Windows Event Log
            winlog::init("lg-ultragear-color-svc").ok();
            lg_service::run()?;
        }
        _ => {
            // CLI mode — use console logging
            env_logger::Builder::new()
                .filter_level(if cli.verbose {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Warn
                })
                .format_timestamp(None)
                .init();

            match cli.command {
                None => {
                    use clap::CommandFactory;
                    Cli::command().print_help()?;
                    println!();
                }
                Some(Commands::Detect { pattern }) => cmd_detect(pattern)?,
                Some(Commands::Apply { pattern }) => cmd_apply(pattern, cli.verbose)?,
                Some(Commands::Watch { pattern }) => cmd_watch(pattern)?,
                Some(Commands::Config { action }) => cmd_config(action)?,
                Some(Commands::Service { action }) => cmd_service(action)?,
            }
        }
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

fn cmd_apply(pattern: Option<String>, verbose: bool) -> Result<(), Box<dyn Error>> {
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
