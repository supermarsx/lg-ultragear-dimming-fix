//! Interactive TUI for the LG UltraGear dimming fix tool.
//!
//! Provides a box-drawing terminal menu replicating the PowerShell
//! installer's interactive experience: live status display, numbered
//! actions, and toggle-based advanced settings.

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{Color, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use lg_core::config::{self, Config};
use std::io::{self, Write};

// ── Layout constants ─────────────────────────────────────────────────────

pub(crate) const W: usize = 76;
pub(crate) const INNER: usize = W - 4; // Content width between "║ " and " ║"
pub(crate) const BAR: usize = W - 2; // Fill width between ╔/╗, ╟/╢, ╚/╝
pub(crate) const TITLE: &str = "LG UltraGear Auto-Dimming Fix";
pub(crate) const REPO: &str = "github.com/supermarsx/lg-ultragear-dimming-fix";

// ── Types ────────────────────────────────────────────────────────────────

/// Advanced option toggles persisted within a TUI session.
pub(crate) struct Options {
    pub(crate) toast: bool,
    pub(crate) dry_run: bool,
    pub(crate) verbose: bool,
}

impl Default for Options {
    fn default() -> Self {
        let cfg = Config::load();
        Self {
            toast: cfg.toast_enabled,
            dry_run: false,
            verbose: cfg.verbose,
        }
    }
}

pub(crate) struct Status {
    pub(crate) profile_installed: bool,
    pub(crate) service_installed: bool,
    pub(crate) service_running: bool,
    pub(crate) monitor_count: usize,
}

pub(crate) enum Page {
    Main,
    Advanced,
}

// ── Entry point ──────────────────────────────────────────────────────────

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::stdout();
    let mut page = Page::Main;
    let mut opts = Options::default();

    loop {
        let status = gather_status();

        match page {
            Page::Main => draw_main(&mut out, &status, &opts)?,
            Page::Advanced => draw_advanced(&mut out, &status, &opts)?,
        }
        out.flush()?;

        let ch = read_key()?;

        match (&page, ch) {
            // ── Main menu ──────────────────────────────────
            (Page::Main, '1') => run_action(
                &mut out,
                "Installing profile + service...",
                || action_default_install(&opts),
            )?,
            (Page::Main, '2') => run_action(
                &mut out,
                "Installing profile only...",
                || action_profile_only(&opts),
            )?,
            (Page::Main, '3') => run_action(
                &mut out,
                "Installing service only...",
                || action_service_only(&opts),
            )?,
            (Page::Main, '4') => run_action(
                &mut out,
                "Refreshing profile...",
                || action_refresh(&opts),
            )?,
            (Page::Main, '5') => run_action(
                &mut out,
                "Reinstalling everything...",
                || action_reinstall(&opts),
            )?,
            (Page::Main, '6') => {
                run_action(&mut out, "Detecting monitors...", action_detect)?
            }
            (Page::Main, '7') => run_action(
                &mut out,
                "Removing service...",
                || action_remove_service(&opts),
            )?,
            (Page::Main, '8') => run_action(
                &mut out,
                "Removing profile...",
                || action_remove_profile(&opts),
            )?,
            (Page::Main, '9') => run_action(
                &mut out,
                "Full uninstall...",
                || action_full_uninstall(&opts),
            )?,
            (Page::Main, 'a') => page = Page::Advanced,
            (Page::Main, 'q') => break,

            // ── Advanced menu ──────────────────────────────
            (Page::Advanced, '1') => opts.toast = !opts.toast,
            (Page::Advanced, '2') => opts.dry_run = !opts.dry_run,
            (Page::Advanced, '3') => opts.verbose = !opts.verbose,
            (Page::Advanced, 'b') => page = Page::Main,
            (Page::Advanced, 'q') => break,

            _ => {} // ignore unknown keys
        }
    }

    draw_goodbye(&mut out)?;
    Ok(())
}

// ── Key reading (brief raw mode) ─────────────────────────────────────────

fn read_key() -> io::Result<char> {
    terminal::enable_raw_mode()?;
    let ch = loop {
        match event::read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => break 'q',
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) => break c.to_ascii_lowercase(),
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => break 'q',
            _ => continue,
        }
    };
    terminal::disable_raw_mode()?;
    Ok(ch)
}

// ── Status gathering ─────────────────────────────────────────────────────

pub(crate) fn gather_status() -> Status {
    let cfg = Config::load();
    let profile_installed = lg_profile::is_profile_installed(&cfg.profile_path());
    let (service_installed, service_running) = lg_service::query_service_info();
    let monitor_count = lg_monitor::find_matching_monitors(&cfg.monitor_match)
        .map(|v| v.len())
        .unwrap_or(0);
    Status {
        profile_installed,
        service_installed,
        service_running,
        monitor_count,
    }
}

// ============================================================================
// Drawing — Main menu
// ============================================================================

pub(crate) fn draw_main(out: &mut impl Write, status: &Status, opts: &Options) -> io::Result<()> {
    queue!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    draw_header(out, status)?;
    draw_sep(out, " MAIN MENU ")?;

    draw_empty(out)?;
    draw_section(out, "INSTALL OPTIONS")?;
    draw_item(out, "1", "Default Install (Profile + Service)")?;
    draw_item(out, "2", "Profile Only (Install ICC without service)")?;
    draw_item(out, "3", "Service Only (Install service only)")?;
    draw_empty(out)?;

    draw_section(out, "MAINTENANCE")?;
    draw_item(out, "4", "Refresh (Re-apply profile now)")?;
    draw_item(out, "5", "Reinstall (Clean reinstall everything)")?;
    draw_item(out, "6", "Detect Monitors")?;
    draw_empty(out)?;

    draw_section(out, "UNINSTALL")?;
    draw_item(out, "7", "Remove Service (Keep profile)")?;
    draw_item(out, "8", "Remove Profile Only")?;
    draw_item(out, "9", "Full Uninstall (Remove everything)")?;
    draw_empty(out)?;

    draw_section(out, "ADVANCED")?;

    // Active toggles summary
    let mut active: Vec<&str> = Vec::new();
    if !opts.toast {
        active.push("NoToast");
    }
    if opts.dry_run {
        active.push("DryRun");
    }
    if opts.verbose {
        active.push("Verbose");
    }

    if active.is_empty() {
        draw_item(out, "A", "Advanced Options (None active)")?;
    } else {
        let label = format!("Advanced Options ({})", active.join(", "));
        draw_item_colored(out, "A", &label, Color::Green)?;
    }

    draw_empty(out)?;
    draw_item_quit(out)?;
    draw_empty(out)?;
    draw_bottom(out)?;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "  Select option: ")?;
    queue!(out, ResetColor)?;
    Ok(())
}

// ============================================================================
// Drawing — Advanced menu
// ============================================================================

pub(crate) fn draw_advanced(out: &mut impl Write, status: &Status, opts: &Options) -> io::Result<()> {
    queue!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    draw_header(out, status)?;
    draw_sep(out, " ADVANCED OPTIONS (Toggles) ")?;

    draw_empty(out)?;
    draw_section(out, "NOTIFICATIONS")?;
    draw_toggle(
        out,
        "1",
        "Toast Notifications (Show reapply alerts)",
        opts.toast,
    )?;
    draw_empty(out)?;

    draw_section(out, "TESTING")?;
    draw_toggle(out, "2", "Dry Run (Simulate without changes)", opts.dry_run)?;
    draw_toggle(out, "3", "Verbose Logging (Detailed output)", opts.verbose)?;
    draw_empty(out)?;
    draw_line(
        out,
        "  These toggles affect main menu install options",
        Color::DarkGrey,
    )?;
    draw_empty(out)?;

    draw_section(out, "NAVIGATION")?;
    draw_item(out, "B", "Back to Main Menu")?;
    draw_item_quit(out)?;
    draw_empty(out)?;
    draw_bottom(out)?;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "  Select option: ")?;
    queue!(out, ResetColor)?;
    Ok(())
}

// ============================================================================
// Drawing — Header with status
// ============================================================================

pub(crate) fn draw_header(out: &mut impl Write, status: &Status) -> io::Result<()> {
    draw_top(out, TITLE)?;

    let version_line = format!(
        "Version {}  \u{2502}  {}",
        env!("CARGO_PKG_VERSION"),
        REPO
    );
    draw_line_center(out, &version_line, Color::DarkGrey)?;

    draw_sep(out, "")?;
    draw_empty(out)?;

    // Status sub-box top
    let status_label = "\u{2500} CURRENT STATUS ";
    let status_dashes = INNER - 2 - status_label.len();
    let status_top = format!(
        "\u{250C}{}{}{}",
        status_label,
        "\u{2500}".repeat(status_dashes),
        "\u{2510}"
    );
    draw_line(out, &status_top, Color::DarkCyan)?;

    // Profile status
    let (profile_text, profile_color) = if status.profile_installed {
        ("\u{25CF} Installed", Color::Green)
    } else {
        ("\u{25CB} Not Installed", Color::Red)
    };
    draw_status(out, "Color Profile:", profile_text, profile_color)?;

    // Service status
    let (service_text, service_color) = if status.service_installed {
        if status.service_running {
            ("\u{25CF} Running", Color::Green)
        } else {
            ("\u{25CB} Stopped", Color::Yellow)
        }
    } else {
        ("\u{25CB} Not Installed", Color::DarkGrey)
    };
    draw_status(out, "Service:      ", service_text, service_color)?;

    // Monitor status
    let (monitor_text, monitor_color) = if status.monitor_count > 0 {
        (
            format!(
                "\u{25CF} {} monitor(s) detected",
                status.monitor_count
            ),
            Color::Green,
        )
    } else {
        ("\u{25CB} None detected".to_string(), Color::DarkGrey)
    };
    draw_status(out, "LG UltraGear: ", &monitor_text, monitor_color)?;

    // Status sub-box bottom
    let status_bottom = format!(
        "\u{2514}{}\u{2518}",
        "\u{2500}".repeat(INNER - 2)
    );
    draw_line(out, &status_bottom, Color::DarkCyan)?;

    draw_empty(out)?;
    Ok(())
}

// ============================================================================
// Drawing — Goodbye screen
// ============================================================================

pub(crate) fn draw_goodbye(out: &mut impl Write) -> io::Result<()> {
    queue!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    let thank = "Thank you for using LG UltraGear Auto-Dimming Fix!";
    let n = 68usize;
    let bar = "\u{2550}".repeat(n);
    let empty = " ".repeat(n);
    let pad = n - 2;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, "  \u{2554}{}\u{2557}", bar)?;
    writeln!(out, "  \u{2551}{}\u{2551}", empty)?;

    write!(out, "  \u{2551} ")?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "{:<width$}", thank, width = pad)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;

    writeln!(out, "  \u{2551}{}\u{2551}", empty)?;

    write!(out, "  \u{2551} ")?;
    queue!(out, SetForegroundColor(Color::DarkGrey))?;
    write!(out, "{:<width$}", REPO, width = pad)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;

    writeln!(out, "  \u{2551}{}\u{2551}", empty)?;
    writeln!(out, "  \u{255A}{}\u{255D}", bar)?;
    queue!(out, ResetColor)?;
    writeln!(out)?;
    out.flush()?;
    Ok(())
}

// ============================================================================
// Box drawing primitives
// ============================================================================

fn draw_top(out: &mut impl Write, title: &str) -> io::Result<()> {
    queue!(out, SetForegroundColor(Color::Cyan))?;
    if title.is_empty() {
        writeln!(out, "\u{2554}{}\u{2557}", "\u{2550}".repeat(BAR))?;
    } else {
        let label = format!(" {} ", title);
        let pad = BAR.saturating_sub(label.len());
        let left = pad / 2;
        let right = pad - left;
        writeln!(
            out,
            "\u{2554}{}{}{}\u{2557}",
            "\u{2550}".repeat(left),
            label,
            "\u{2550}".repeat(right)
        )?;
    }
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_bottom(out: &mut impl Write) -> io::Result<()> {
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, "\u{255A}{}\u{255D}", "\u{2550}".repeat(BAR))?;
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_sep(out: &mut impl Write, title: &str) -> io::Result<()> {
    queue!(out, SetForegroundColor(Color::DarkCyan))?;
    if title.is_empty() {
        writeln!(out, "\u{255F}{}\u{2562}", "\u{2500}".repeat(BAR))?;
    } else {
        let pad = BAR.saturating_sub(title.len());
        let left = pad / 2;
        let right = pad - left;
        writeln!(
            out,
            "\u{255F}{}{}{}\u{2562}",
            "\u{2500}".repeat(left),
            title,
            "\u{2500}".repeat(right)
        )?;
    }
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_line(out: &mut impl Write, text: &str, color: Color) -> io::Result<()> {
    queue!(out, SetForegroundColor(Color::Cyan))?;
    write!(out, "\u{2551} ")?;
    queue!(out, SetForegroundColor(color))?;
    write!(out, "{:<width$}", text, width = INNER)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_line_center(out: &mut impl Write, text: &str, color: Color) -> io::Result<()> {
    queue!(out, SetForegroundColor(Color::Cyan))?;
    write!(out, "\u{2551} ")?;
    queue!(out, SetForegroundColor(color))?;
    write!(out, "{:^width$}", text, width = INNER)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_empty(out: &mut impl Write) -> io::Result<()> {
    draw_line(out, "", Color::White)
}

fn draw_section(out: &mut impl Write, title: &str) -> io::Result<()> {
    let text = format!("  {}", title);
    draw_line(out, &text, Color::Cyan)
}

fn draw_item(out: &mut impl Write, key: &str, text: &str) -> io::Result<()> {
    let key_display = format!("[{}]", key);
    let prefix_len = 2 + key_display.len() + 1; // indent + key + space
    let text_width = INNER.saturating_sub(prefix_len);

    queue!(out, SetForegroundColor(Color::Cyan))?;
    write!(out, "\u{2551} ")?;
    write!(out, "  ")?;
    queue!(out, SetForegroundColor(Color::Yellow))?;
    write!(out, "{}", key_display)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, " {:<width$}", text, width = text_width)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_item_colored(
    out: &mut impl Write,
    key: &str,
    text: &str,
    text_color: Color,
) -> io::Result<()> {
    let key_display = format!("[{}]", key);
    let prefix_len = 2 + key_display.len() + 1;
    let text_width = INNER.saturating_sub(prefix_len);

    queue!(out, SetForegroundColor(Color::Cyan))?;
    write!(out, "\u{2551} ")?;
    write!(out, "  ")?;
    queue!(out, SetForegroundColor(Color::Yellow))?;
    write!(out, "{}", key_display)?;
    queue!(out, SetForegroundColor(text_color))?;
    write!(out, " {:<width$}", text, width = text_width)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_item_quit(out: &mut impl Write) -> io::Result<()> {
    let key_display = "[Q]";
    let text = "Quit";
    let prefix_len = 2 + key_display.len() + 1;
    let text_width = INNER.saturating_sub(prefix_len);

    queue!(out, SetForegroundColor(Color::Cyan))?;
    write!(out, "\u{2551} ")?;
    write!(out, "  ")?;
    queue!(out, SetForegroundColor(Color::Red))?;
    write!(out, "{}", key_display)?;
    queue!(out, SetForegroundColor(Color::DarkGrey))?;
    write!(out, " {:<width$}", text, width = text_width)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_toggle(
    out: &mut impl Write,
    key: &str,
    text: &str,
    enabled: bool,
) -> io::Result<()> {
    let key_display = format!("[{}]", key);
    let toggle = if enabled { "[ON ]" } else { "[OFF]" };
    let toggle_color = if enabled {
        Color::Green
    } else {
        Color::DarkGrey
    };
    let prefix_len = 2 + key_display.len() + 1 + 5 + 1; // indent + key + sp + toggle + sp
    let text_width = INNER.saturating_sub(prefix_len);

    queue!(out, SetForegroundColor(Color::Cyan))?;
    write!(out, "\u{2551} ")?;
    write!(out, "  ")?;
    queue!(out, SetForegroundColor(Color::Yellow))?;
    write!(out, "{}", key_display)?;
    write!(out, " ")?;
    queue!(out, SetForegroundColor(toggle_color))?;
    write!(out, "{}", toggle)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, " {:<width$}", text, width = text_width)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;
    queue!(out, ResetColor)?;
    Ok(())
}

fn draw_status(
    out: &mut impl Write,
    label: &str,
    value: &str,
    color: Color,
) -> io::Result<()> {
    let prefix = format!("  {} ", label);
    let value_width = INNER.saturating_sub(prefix.len());

    queue!(out, SetForegroundColor(Color::Cyan))?;
    write!(out, "\u{2551} ")?;
    queue!(out, SetForegroundColor(Color::Grey))?;
    write!(out, "{}", prefix)?;
    queue!(out, SetForegroundColor(color))?;
    write!(out, "{:<width$}", value, width = value_width)?;
    queue!(out, SetForegroundColor(Color::Cyan))?;
    writeln!(out, " \u{2551}")?;
    queue!(out, ResetColor)?;
    Ok(())
}

// ============================================================================
// Action runner — wraps each operation with a processing screen
// ============================================================================

fn run_action<F>(out: &mut impl Write, banner: &str, action: F) -> io::Result<()>
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error>>,
{
    queue!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    draw_top(out, " PROCESSING ")?;
    draw_empty(out)?;
    draw_line(out, banner, Color::Yellow)?;
    draw_empty(out)?;
    draw_bottom(out)?;
    writeln!(out)?;
    out.flush()?;

    match action() {
        Ok(()) => {}
        Err(e) => {
            queue!(out, SetForegroundColor(Color::Red))?;
            writeln!(out, "  [ERR ] {}", e)?;
            queue!(out, ResetColor)?;
        }
    }

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::DarkGrey))?;
    write!(out, "  Press any key to continue...")?;
    queue!(out, ResetColor)?;
    out.flush()?;
    let _ = read_key();
    Ok(())
}

// ============================================================================
// Actions — called from TUI menu selections
// ============================================================================

fn action_default_install(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would extract ICC profile to color store");
        println!("  [DRY RUN] Would write default config");
        println!("  [DRY RUN] Would install Windows service");
        println!("  [DRY RUN] Would start service");
        return Ok(());
    }

    let cfg = Config::load();

    // Extract ICC profile
    let profile_path = cfg.profile_path();
    match lg_profile::ensure_profile_installed(&profile_path)? {
        true => println!(
            "  [ OK ] ICC profile installed to {}",
            profile_path.display()
        ),
        false => println!("  [ OK ] ICC profile already present"),
    }

    // Write default config
    let cfg_path = config::config_path();
    if !cfg_path.exists() {
        Config::write_default()?;
        println!("  [ OK ] Default config written to {}", cfg_path.display());
    } else {
        println!("  [ OK ] Config already exists at {}", cfg_path.display());
    }

    // Install service
    lg_service::install(&cfg.monitor_match)?;
    println!("  [ OK ] Service installed");

    // Start service
    lg_service::start_service()?;
    println!("  [ OK ] Service started");

    println!("\n  [DONE] Default install complete!");
    Ok(())
}

fn action_profile_only(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would extract ICC profile to color store");
        return Ok(());
    }

    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    match lg_profile::ensure_profile_installed(&profile_path)? {
        true => println!(
            "  [ OK ] ICC profile installed to {}",
            profile_path.display()
        ),
        false => println!("  [ OK ] ICC profile already present"),
    }

    println!("\n  [DONE] Profile install complete!");
    Ok(())
}

fn action_service_only(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would write default config");
        println!("  [DRY RUN] Would install Windows service");
        println!("  [DRY RUN] Would start service");
        return Ok(());
    }

    let cfg = Config::load();
    let cfg_path = config::config_path();
    if !cfg_path.exists() {
        Config::write_default()?;
        println!("  [ OK ] Default config written");
    }

    lg_service::install(&cfg.monitor_match)?;
    println!("  [ OK ] Service installed");

    lg_service::start_service()?;
    println!("  [ OK ] Service started");

    println!("\n  [DONE] Service install complete!");
    Ok(())
}

fn action_refresh(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would re-apply profile to matching monitors");
        return Ok(());
    }

    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    lg_profile::ensure_profile_installed(&profile_path)?;

    if !lg_profile::is_profile_installed(&profile_path) {
        return Err("ICC profile not found after extraction attempt".into());
    }

    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    if devices.is_empty() {
        println!("  [SKIP] No matching monitors found.");
    } else {
        for device in &devices {
            println!("  [INFO] Found: {}", device.name);
            lg_profile::reapply_profile(&device.device_key, &profile_path, cfg.toggle_delay_ms)?;
            println!("  [ OK ] Profile reapplied for {}", device.name);
        }
        lg_profile::refresh_display(
            cfg.refresh_display_settings,
            cfg.refresh_broadcast_color,
            cfg.refresh_invalidate,
        );
        lg_profile::trigger_calibration_loader(cfg.refresh_calibration_loader);

        if opts.toast && cfg.toast_enabled {
            lg_notify::show_reapply_toast(true, &cfg.toast_title, &cfg.toast_body, opts.verbose);
        }

        println!(
            "\n  [DONE] Profile refreshed for {} monitor(s).",
            devices.len()
        );
    }

    Ok(())
}

fn action_reinstall(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would uninstall service");
        println!("  [DRY RUN] Would reinstall profile + service");
        return Ok(());
    }

    // Best-effort uninstall first
    match lg_service::uninstall() {
        Ok(()) => println!("  [ OK ] Service uninstalled"),
        Err(e) => println!("  [NOTE] Service removal: {} (continuing)", e),
    }

    // Fresh install
    action_default_install(opts)
}

fn action_detect() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load();
    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;

    if devices.is_empty() {
        println!(
            "  No matching monitors found for pattern \"{}\".",
            cfg.monitor_match
        );
    } else {
        println!(
            "  Found {} monitor(s) matching \"{}\":\n",
            devices.len(),
            cfg.monitor_match
        );
        for (i, device) in devices.iter().enumerate() {
            println!("    {}. {}", i + 1, device.name);
            println!("       Device: {}", device.device_key);
        }
    }

    let profile_path = cfg.profile_path();
    println!("\n  Profile: {}", profile_path.display());
    println!(
        "  Installed: {}",
        if lg_profile::is_profile_installed(&profile_path) {
            "yes"
        } else {
            "no"
        }
    );

    Ok(())
}

fn action_remove_service(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would uninstall Windows service");
        return Ok(());
    }

    lg_service::uninstall()?;
    println!("  [ OK ] Service uninstalled");
    println!("  [NOTE] ICC profile preserved in color store");
    Ok(())
}

fn action_remove_profile(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would remove ICC profile from color store");
        return Ok(());
    }

    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    match lg_profile::remove_profile(&profile_path)? {
        true => println!(
            "  [ OK ] ICC profile removed from {}",
            profile_path.display()
        ),
        false => println!("  [NOTE] ICC profile not found (already removed)"),
    }
    Ok(())
}

fn action_full_uninstall(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("  [DRY RUN] Would uninstall service");
        println!("  [DRY RUN] Would remove ICC profile");
        println!("  [DRY RUN] Would remove config directory");
        return Ok(());
    }

    // Remove service (best-effort)
    match lg_service::uninstall() {
        Ok(()) => println!("  [ OK ] Service uninstalled"),
        Err(e) => println!("  [NOTE] Service removal: {} (continuing)", e),
    }

    // Remove profile
    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    match lg_profile::remove_profile(&profile_path)? {
        true => println!("  [ OK ] ICC profile removed"),
        false => println!("  [NOTE] ICC profile not found (already removed)"),
    }

    // Remove config directory
    let cfg_dir = config::config_dir();
    if cfg_dir.exists() {
        match std::fs::remove_dir_all(&cfg_dir) {
            Ok(()) => println!("  [ OK ] Config directory removed: {}", cfg_dir.display()),
            Err(e) => println!(
                "  [WARN] Could not remove config dir: {} (clean up manually)",
                e
            ),
        }
    }

    println!("\n  [DONE] Full uninstall complete!");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: create a test Status ─────────────────────────────

    fn test_status(
        profile_installed: bool,
        service_installed: bool,
        service_running: bool,
        monitor_count: usize,
    ) -> Status {
        Status {
            profile_installed,
            service_installed,
            service_running,
            monitor_count,
        }
    }

    fn default_status() -> Status {
        test_status(false, false, false, 0)
    }

    fn all_good_status() -> Status {
        test_status(true, true, true, 1)
    }

    fn default_opts() -> Options {
        Options {
            toast: true,
            dry_run: false,
            verbose: false,
        }
    }

    fn render_to_string<F>(f: F) -> String
    where
        F: FnOnce(&mut Vec<u8>) -> io::Result<()>,
    {
        let mut buf = Vec::new();
        f(&mut buf).expect("draw should not fail");
        String::from_utf8_lossy(&buf).to_string()
    }

    // ── Constants ────────────────────────────────────────────────

    #[test]
    fn layout_constants_are_consistent() {
        assert_eq!(INNER, W - 4);
        assert_eq!(BAR, W - 2);
        const { assert!(W > 40, "width should be wide enough for content") };
    }

    #[test]
    fn title_is_not_empty() {
        assert!(!TITLE.is_empty());
        assert!(TITLE.contains("UltraGear"));
    }

    #[test]
    fn repo_is_not_empty() {
        assert!(!REPO.is_empty());
        assert!(REPO.contains("github.com"));
    }

    // ── Options defaults ─────────────────────────────────────────

    #[test]
    fn options_default_toast_matches_config() {
        let opts = Options::default();
        let cfg = Config::load();
        assert_eq!(opts.toast, cfg.toast_enabled);
    }

    #[test]
    fn options_default_dry_run_is_false() {
        let opts = Options::default();
        assert!(!opts.dry_run);
    }

    #[test]
    fn options_default_verbose_matches_config() {
        let opts = Options::default();
        let cfg = Config::load();
        assert_eq!(opts.verbose, cfg.verbose);
    }

    // ── Status struct ────────────────────────────────────────────

    #[test]
    fn status_default_all_false() {
        let s = default_status();
        assert!(!s.profile_installed);
        assert!(!s.service_installed);
        assert!(!s.service_running);
        assert_eq!(s.monitor_count, 0);
    }

    #[test]
    fn status_all_good() {
        let s = all_good_status();
        assert!(s.profile_installed);
        assert!(s.service_installed);
        assert!(s.service_running);
        assert_eq!(s.monitor_count, 1);
    }

    // ── gather_status does not panic ─────────────────────────────

    #[test]
    fn gather_status_does_not_panic() {
        let _s = gather_status();
    }

    #[test]
    fn gather_status_returns_valid_data() {
        let s = gather_status();
        // If service not installed, it can't be running
        if !s.service_installed {
            assert!(!s.service_running);
        }
    }

    // ── Page enum ────────────────────────────────────────────────

    #[test]
    fn page_variants_exist() {
        let _main = Page::Main;
        let _adv = Page::Advanced;
    }

    // ── Main menu drawing ────────────────────────────────────────

    #[test]
    fn draw_main_contains_all_9_menu_items() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        // Verify all 9 numbered items
        assert!(output.contains("[1]"), "should contain item 1");
        assert!(output.contains("[2]"), "should contain item 2");
        assert!(output.contains("[3]"), "should contain item 3");
        assert!(output.contains("[4]"), "should contain item 4");
        assert!(output.contains("[5]"), "should contain item 5");
        assert!(output.contains("[6]"), "should contain item 6");
        assert!(output.contains("[7]"), "should contain item 7");
        assert!(output.contains("[8]"), "should contain item 8");
        assert!(output.contains("[9]"), "should contain item 9");
    }

    #[test]
    fn draw_main_contains_install_section() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("INSTALL OPTIONS"));
    }

    #[test]
    fn draw_main_contains_maintenance_section() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("MAINTENANCE"));
    }

    #[test]
    fn draw_main_contains_uninstall_section() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("UNINSTALL"));
    }

    #[test]
    fn draw_main_contains_advanced_section() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("ADVANCED"));
    }

    #[test]
    fn draw_main_contains_quit_option() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("[Q]"));
        assert!(output.contains("Quit"));
    }

    #[test]
    fn draw_main_contains_advanced_key() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("[A]"));
        assert!(output.contains("Advanced Options"));
    }

    #[test]
    fn draw_main_install_labels() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("Default Install"));
        assert!(output.contains("Profile Only"));
        assert!(output.contains("Service Only"));
    }

    #[test]
    fn draw_main_maintenance_labels() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("Refresh"));
        assert!(output.contains("Reinstall"));
        assert!(output.contains("Detect Monitors"));
    }

    #[test]
    fn draw_main_uninstall_labels() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("Remove Service"));
        assert!(output.contains("Remove Profile Only"));
        assert!(output.contains("Full Uninstall"));
    }

    #[test]
    fn draw_main_shows_no_active_toggles_by_default() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(
            output.contains("None active"),
            "Default opts should show 'None active'"
        );
    }

    #[test]
    fn draw_main_shows_active_toggles_when_set() {
        let opts = Options {
            toast: false, // toggled off → shows NoToast
            dry_run: true,
            verbose: true,
        };
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &opts)
        });
        assert!(output.contains("NoToast"), "should show NoToast");
        assert!(output.contains("DryRun"), "should show DryRun");
        assert!(output.contains("Verbose"), "should show Verbose");
    }

    #[test]
    fn draw_main_select_option_prompt() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("Select option"));
    }

    // ── Box drawing characters in main menu ──────────────────────

    #[test]
    fn draw_main_contains_box_drawing_chars() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(output.contains('\u{2554}'), "top-left corner \u{2554}");
        assert!(output.contains('\u{2557}'), "top-right corner \u{2557}");
        assert!(output.contains('\u{255A}'), "bottom-left corner \u{255a}");
        assert!(output.contains('\u{255D}'), "bottom-right corner \u{255d}");
        assert!(output.contains('\u{2551}'), "vertical line \u{2551}");
    }

    // ── Advanced menu drawing ────────────────────────────────────

    #[test]
    fn draw_advanced_contains_3_toggles() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("[1]"), "toggle 1");
        assert!(output.contains("[2]"), "toggle 2");
        assert!(output.contains("[3]"), "toggle 3");
    }

    #[test]
    fn draw_advanced_contains_toggle_labels() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("Toast Notifications"));
        assert!(output.contains("Dry Run"));
        assert!(output.contains("Verbose Logging"));
    }

    #[test]
    fn draw_advanced_contains_back_option() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("[B]"));
        assert!(output.contains("Back to Main Menu"));
    }

    #[test]
    fn draw_advanced_contains_quit_option() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("[Q]"));
        assert!(output.contains("Quit"));
    }

    #[test]
    fn draw_advanced_toast_on_by_default() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        // Toast should be ON by default (assuming config has toast_enabled=true)
        assert!(output.contains("[ON ]"), "toast should be ON by default");
    }

    #[test]
    fn draw_advanced_dry_run_off_by_default() {
        let opts = default_opts();
        assert!(!opts.dry_run, "dry_run defaults to false");
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &opts)
        });
        assert!(output.contains("[OFF]"), "dry_run/verbose should be OFF");
    }

    #[test]
    fn draw_advanced_toggles_reflect_options() {
        let opts = Options {
            toast: false,
            dry_run: true,
            verbose: true,
        };
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &opts)
        });
        // With toast=false, dry_run=true, verbose=true
        // We should see two ON and one OFF
        let on_count = output.matches("[ON ]").count();
        let off_count = output.matches("[OFF]").count();
        assert_eq!(on_count, 2, "dry_run+verbose should be ON");
        assert_eq!(off_count, 1, "toast should be OFF");
    }

    #[test]
    fn draw_advanced_section_headers() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("NOTIFICATIONS"));
        assert!(output.contains("TESTING"));
        assert!(output.contains("NAVIGATION"));
    }

    #[test]
    fn draw_advanced_info_text() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("toggles affect main menu"));
    }

    #[test]
    fn draw_advanced_title() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(output.contains("ADVANCED OPTIONS"));
    }

    // ── Header drawing ───────────────────────────────────────────

    #[test]
    fn draw_header_contains_title() {
        let output = render_to_string(|buf| {
            draw_header(buf, &default_status())
        });
        assert!(output.contains(TITLE));
    }

    #[test]
    fn draw_header_contains_version() {
        let output = render_to_string(|buf| {
            draw_header(buf, &default_status())
        });
        assert!(
            output.contains(env!("CARGO_PKG_VERSION")),
            "header should show crate version"
        );
    }

    #[test]
    fn draw_header_contains_repo() {
        let output = render_to_string(|buf| {
            draw_header(buf, &default_status())
        });
        assert!(output.contains(REPO));
    }

    #[test]
    fn draw_header_shows_current_status_label() {
        let output = render_to_string(|buf| {
            draw_header(buf, &default_status())
        });
        assert!(output.contains("CURRENT STATUS"));
    }

    #[test]
    fn draw_header_shows_profile_not_installed() {
        let output = render_to_string(|buf| {
            draw_header(buf, &test_status(false, false, false, 0))
        });
        assert!(output.contains("Not Installed"));
    }

    #[test]
    fn draw_header_shows_profile_installed() {
        let output = render_to_string(|buf| {
            draw_header(buf, &test_status(true, false, false, 0))
        });
        assert!(output.contains("Installed"));
    }

    #[test]
    fn draw_header_shows_service_not_installed() {
        let output = render_to_string(|buf| {
            draw_header(buf, &test_status(false, false, false, 0))
        });
        // Service not installed → "Not Installed"
        assert!(output.contains("Not Installed"));
    }

    #[test]
    fn draw_header_shows_service_running() {
        let output = render_to_string(|buf| {
            draw_header(buf, &test_status(true, true, true, 1))
        });
        assert!(output.contains("Running"));
    }

    #[test]
    fn draw_header_shows_service_stopped() {
        let output = render_to_string(|buf| {
            draw_header(buf, &test_status(true, true, false, 1))
        });
        assert!(output.contains("Stopped"));
    }

    #[test]
    fn draw_header_shows_monitors_detected() {
        let output = render_to_string(|buf| {
            draw_header(buf, &test_status(true, true, true, 3))
        });
        assert!(output.contains("3 monitor(s) detected"));
    }

    #[test]
    fn draw_header_shows_no_monitors() {
        let output = render_to_string(|buf| {
            draw_header(buf, &test_status(false, false, false, 0))
        });
        assert!(output.contains("None detected"));
    }

    #[test]
    fn draw_header_shows_status_labels() {
        let output = render_to_string(|buf| {
            draw_header(buf, &default_status())
        });
        assert!(output.contains("Color Profile:"));
        assert!(output.contains("Service:"));
        assert!(output.contains("LG UltraGear:"));
    }

    // ── Goodbye screen ───────────────────────────────────────────

    #[test]
    fn draw_goodbye_contains_thank_you() {
        let output = render_to_string(draw_goodbye);
        assert!(output.contains("Thank you"));
    }

    #[test]
    fn draw_goodbye_contains_repo() {
        let output = render_to_string(draw_goodbye);
        assert!(output.contains(REPO));
    }

    #[test]
    fn draw_goodbye_contains_title_reference() {
        let output = render_to_string(draw_goodbye);
        assert!(output.contains("Auto-Dimming Fix"));
    }

    #[test]
    fn draw_goodbye_has_box_drawing() {
        let output = render_to_string(draw_goodbye);
        assert!(output.contains('\u{2554}'));
        assert!(output.contains('\u{255A}'));
    }

    // ── Draw with different status combos ────────────────────────

    #[test]
    fn draw_main_with_all_installed_status() {
        let output = render_to_string(|buf| {
            draw_main(buf, &all_good_status(), &default_opts())
        });
        assert!(output.contains("Installed"));
        assert!(output.contains("Running"));
        assert!(output.contains("1 monitor(s) detected"));
    }

    #[test]
    fn draw_main_with_service_stopped() {
        let s = test_status(true, true, false, 2);
        let output = render_to_string(|buf| {
            draw_main(buf, &s, &default_opts())
        });
        assert!(output.contains("Stopped"));
        assert!(output.contains("2 monitor(s) detected"));
    }

    #[test]
    fn draw_advanced_with_all_good_status() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &all_good_status(), &default_opts())
        });
        assert!(output.contains("Running"));
    }

    // ── Options toggling logic ───────────────────────────────────

    #[test]
    fn options_toggle_toast() {
        let mut opts = default_opts();
        assert!(opts.toast);
        opts.toast = !opts.toast;
        assert!(!opts.toast);
        opts.toast = !opts.toast;
        assert!(opts.toast);
    }

    #[test]
    fn options_toggle_dry_run() {
        let mut opts = default_opts();
        assert!(!opts.dry_run);
        opts.dry_run = !opts.dry_run;
        assert!(opts.dry_run);
    }

    #[test]
    fn options_toggle_verbose() {
        let mut opts = default_opts();
        assert!(!opts.verbose);
        opts.verbose = !opts.verbose;
        assert!(opts.verbose);
    }

    // ── Active toggles display ───────────────────────────────────

    #[test]
    fn active_toggles_none_when_defaults() {
        let opts = default_opts();
        let mut active: Vec<&str> = Vec::new();
        if !opts.toast {
            active.push("NoToast");
        }
        if opts.dry_run {
            active.push("DryRun");
        }
        if opts.verbose {
            active.push("Verbose");
        }
        assert!(active.is_empty());
    }

    #[test]
    fn active_toggles_all_when_everything_changed() {
        let opts = Options {
            toast: false,
            dry_run: true,
            verbose: true,
        };
        let mut active: Vec<&str> = Vec::new();
        if !opts.toast {
            active.push("NoToast");
        }
        if opts.dry_run {
            active.push("DryRun");
        }
        if opts.verbose {
            active.push("Verbose");
        }
        assert_eq!(active.len(), 3);
        assert_eq!(active, vec!["NoToast", "DryRun", "Verbose"]);
    }

    // ── Rendering consistency ────────────────────────────────────

    #[test]
    fn draw_main_produces_nonempty_output() {
        let output = render_to_string(|buf| {
            draw_main(buf, &default_status(), &default_opts())
        });
        assert!(!output.is_empty());
        assert!(output.len() > 500, "main menu should produce substantial output");
    }

    #[test]
    fn draw_advanced_produces_nonempty_output() {
        let output = render_to_string(|buf| {
            draw_advanced(buf, &default_status(), &default_opts())
        });
        assert!(!output.is_empty());
        assert!(output.len() > 300, "advanced menu should produce substantial output");
    }

    #[test]
    fn draw_goodbye_produces_nonempty_output() {
        let output = render_to_string(draw_goodbye);
        assert!(!output.is_empty());
        assert!(output.len() > 100);
    }

    #[test]
    fn draw_header_produces_nonempty_output() {
        let output = render_to_string(|buf| {
            draw_header(buf, &default_status())
        });
        assert!(!output.is_empty());
    }

    // ── Main and advanced both render without errors for all status combos ─

    #[test]
    fn draw_main_all_status_combos() {
        for profile in [false, true] {
            for svc_installed in [false, true] {
                for svc_running in [false, true] {
                    for count in [0, 1, 5] {
                        let s = test_status(profile, svc_installed, svc_running, count);
                        let output = render_to_string(|buf| {
                            draw_main(buf, &s, &default_opts())
                        });
                        assert!(!output.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn draw_advanced_all_option_combos() {
        for toast in [false, true] {
            for dry in [false, true] {
                for verb in [false, true] {
                    let opts = Options {
                        toast,
                        dry_run: dry,
                        verbose: verb,
                    };
                    let output = render_to_string(|buf| {
                        draw_advanced(buf, &default_status(), &opts)
                    });
                    assert!(!output.is_empty());
                }
            }
        }
    }
}
