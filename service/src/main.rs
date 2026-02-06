//! LG UltraGear Color Profile Service
//!
//! A lightweight Windows service that listens for display connect/disconnect,
//! session unlock, and logon events, then reapplies the LG UltraGear ICC
//! color profile using a toggle approach (disassociate → reassociate) to
//! force Windows to reload it.

mod config;
mod monitor;
mod profile;
mod service;
mod toast;

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("install") => {
            let monitor_match = args.get(2).map(|s| s.as_str()).unwrap_or("LG ULTRAGEAR");

            // Write default config file (won't overwrite if exists)
            let cfg_path = config::config_path();
            if !cfg_path.exists() {
                config::Config::write_default()?;
                println!("[OK] Default config written to {}", cfg_path.display());
            } else {
                println!("[OK] Config already exists at {}", cfg_path.display());
            }

            // Update monitor_match in config if provided on CLI
            let mut cfg = config::Config::load();
            if monitor_match != "LG ULTRAGEAR" {
                cfg.monitor_match = monitor_match.to_string();
                config::Config::write_config(&cfg)?;
                println!("[OK] Config updated with monitor pattern: {monitor_match}");
            }

            service::install(&cfg.monitor_match)?;
            println!(
                "[OK] Service installed. Monitor pattern: {}",
                cfg.monitor_match
            );
            println!("     Config: {}", cfg_path.display());
            println!("     Start with: sc start lg-ultragear-color-svc");
        }
        Some("uninstall") => {
            service::uninstall()?;
            println!("[OK] Service uninstalled.");
            println!(
                "     Config preserved at: {}",
                config::config_path().display()
            );
        }
        Some("start") => {
            service::start_service()?;
            println!("[OK] Service started.");
        }
        Some("stop") => {
            service::stop_service()?;
            println!("[OK] Service stopped.");
        }
        Some("status") => {
            service::print_status()?;
        }
        Some("config") => match args.get(2).map(|s| s.as_str()) {
            Some("reset") => {
                config::Config::write_default()?;
                println!(
                    "[OK] Config reset to defaults at {}",
                    config::config_path().display()
                );
            }
            Some("path") => {
                println!("{}", config::config_path().display());
            }
            _ => {
                let cfg = config::Config::load();
                println!("Config file: {}", config::config_path().display());
                println!();
                println!("{:#?}", cfg);
            }
        },
        Some("run-once") => {
            let cfg = config::Config::load();
            let pattern = args
                .get(2)
                .map(|s| s.as_str())
                .unwrap_or(&cfg.monitor_match);
            println!("[INFO] Running one-shot profile reapply...");
            println!("[INFO] Config: {}", config::config_path().display());
            println!("[INFO] Pattern: {pattern}");
            println!(
                "[INFO] Toast: {}",
                if cfg.toast_enabled { "on" } else { "off" }
            );

            let devices = monitor::find_matching_monitors(pattern)?;
            if devices.is_empty() {
                println!("[SKIP] No matching monitors found.");
            } else {
                for device in &devices {
                    println!("[INFO] Found: {}", device.name);
                    profile::reapply_profile(&device.device_key, &cfg)?;
                    println!("[OK]   Profile reapplied for {}", device.name);
                }
                profile::refresh_display(&cfg);
                profile::trigger_calibration_loader(&cfg);

                if cfg.toast_enabled {
                    println!("[INFO] Sending toast notification...");
                    toast::show_reapply_toast(&cfg);
                }

                println!("[DONE] All profiles reapplied.");
            }
        }
        None => {
            service::run()?;
        }
        Some(other) => {
            eprintln!("Unknown command: {other}");
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_usage() {
    eprintln!(
        r#"
LG UltraGear Color Profile Service

Usage:
  lg-ultragear-color-svc                     Run as Windows service (SCM only)
  lg-ultragear-color-svc install [PATTERN]   Install service (default pattern: "LG ULTRAGEAR")
  lg-ultragear-color-svc uninstall           Uninstall service
  lg-ultragear-color-svc start               Start the service
  lg-ultragear-color-svc stop                Stop the service
  lg-ultragear-color-svc status              Show service status
  lg-ultragear-color-svc config              Show current configuration
  lg-ultragear-color-svc config reset        Reset config to defaults
  lg-ultragear-color-svc config path         Print config file path
  lg-ultragear-color-svc run-once [PATTERN]  One-shot reapply (for testing)

Config: %ProgramData%\LG-UltraGear-Monitor\config.toml
"#
    );
}
