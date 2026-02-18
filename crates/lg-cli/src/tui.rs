//! Interactive TUI for the LG UltraGear dimming fix tool.
//!
//! Provides a box-drawing terminal menu replicating the PowerShell
//! installer's interactive experience: live status display, numbered
//! actions, and toggle-based advanced settings.

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    queue,
    style::{Color, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use lg_core::config::{self, Config};
use std::io::{self, Write};

// ── UTF-8 console support (Windows) ──────────────────────────────────────

/// Ensure the Windows console uses UTF-8 for output so box-drawing and
/// other Unicode characters render correctly, even in cmd.exe or legacy
/// PowerShell hosts that default to an OEM code page.
///
/// This does three things:
/// 1. Sets the input and output code pages to 65001 (UTF-8).
/// 2. Switches the console font to Consolas (a TrueType font with full
///    Unicode box-drawing support). The default "Raster Fonts" in cmd.exe
///    cannot render ╔═╗║ etc.
/// 3. Enables Virtual Terminal Processing so ANSI escape sequences (used by
///    crossterm for colours and cursor movement) work correctly.
pub fn enable_utf8_console() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::{
            GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
            SetCurrentConsoleFontEx, CONSOLE_FONT_INFOEX, COORD, ENABLE_PROCESSED_OUTPUT,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
        };

        unsafe {
            // 1. UTF-8 code pages
            let _ = SetConsoleOutputCP(65001);
            let _ = SetConsoleCP(65001);

            let handle = match GetStdHandle(STD_OUTPUT_HANDLE) {
                Ok(h) => h,
                Err(_) => return,
            };

            // 2. TrueType font with Unicode support
            let mut font = CONSOLE_FONT_INFOEX {
                cbSize: std::mem::size_of::<CONSOLE_FONT_INFOEX>() as u32,
                dwFontSize: COORD { X: 0, Y: 18 },
                FontWeight: 400, // FW_NORMAL
                ..Default::default()
            };
            let name: Vec<u16> = "Consolas\0".encode_utf16().collect();
            font.FaceName[..name.len()].copy_from_slice(&name);
            let _ = SetCurrentConsoleFontEx(handle, false, &font);

            // 3. Enable VT processing (ANSI escape sequences)
            let mut mode = Default::default();
            if GetConsoleMode(handle, &mut mode).is_ok() {
                let _ = SetConsoleMode(
                    handle,
                    mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
                );
            }
        }
    }
}

/// Resize the console window so the TUI fits without scrolling.
/// Targets 40 rows × 80 columns — enough for the main menu with
/// status header, all menu sections, and the prompt line.
fn ensure_console_size() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::{
            GetConsoleScreenBufferInfo, GetStdHandle, SetConsoleScreenBufferSize,
            SetConsoleWindowInfo, CONSOLE_SCREEN_BUFFER_INFO, COORD, SMALL_RECT,
            STD_OUTPUT_HANDLE,
        };
        unsafe {
            let handle = match GetStdHandle(STD_OUTPUT_HANDLE) {
                Ok(h) => h,
                Err(_) => return,
            };
            let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
            if GetConsoleScreenBufferInfo(handle, &mut info).is_err() {
                return;
            }
            let current_cols = info.dwSize.X;
            let current_rows = info.srWindow.Bottom - info.srWindow.Top + 1;
            let want_cols = current_cols.max(80);
            let want_rows = current_rows.max(40);

            // Shrink window first so buffer resize doesn't fail
            let small_rect = SMALL_RECT {
                Top: 0,
                Left: 0,
                Right: want_cols - 1,
                Bottom: want_rows - 1,
            };
            // Set buffer large enough
            let buf_size = COORD {
                X: want_cols,
                Y: want_rows.max(300), // large buffer for scroll-back
            };
            let _ = SetConsoleScreenBufferSize(handle, buf_size);
            let _ = SetConsoleWindowInfo(handle, true, &small_rect);
        }
    }
}

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
    pub(crate) hdr: bool,
    pub(crate) sdr: bool,
    pub(crate) per_user: bool,
    pub(crate) generic_default: bool,
}

impl Default for Options {
    fn default() -> Self {
        let cfg = Config::load();
        Self {
            toast: cfg.toast_enabled,
            dry_run: false,
            verbose: cfg.verbose,
            hdr: false,
            sdr: true,
            per_user: false,
            generic_default: false,
        }
    }
}

pub(crate) struct Status {
    pub(crate) profile_installed: bool,
    pub(crate) service_installed: bool,
    pub(crate) service_running: bool,
    pub(crate) monitor_count: usize,
    pub(crate) hdr_enabled: bool,
    pub(crate) sdr_enabled: bool,
}

pub(crate) enum Page {
    Main,
    Maintenance,
    Advanced,
}

// ── Entry point ──────────────────────────────────────────────────────────

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    ensure_console_size();
    let mut out = io::stdout();
    let mut page = Page::Main;
    let mut opts = Options::default();

    loop {
        let status = gather_status(&opts);

        match page {
            Page::Main => draw_main(&mut out, &status, &opts)?,
            Page::Maintenance => draw_maintenance(&mut out, &status, &opts)?,
            Page::Advanced => draw_advanced(&mut out, &status, &opts)?,
        }
        out.flush()?;

        let ch = read_key()?;

        match (&page, ch) {
            // ── Main menu ──────────────────────────────────
            (Page::Main, '1') => run_action(&mut out, "Installing profile + service...", || {
                action_default_install(&opts)
            })?,
            (Page::Main, '2') => run_action(&mut out, "Installing profile only...", || {
                action_profile_only(&opts)
            })?,
            (Page::Main, '3') => run_action(&mut out, "Installing service only...", || {
                action_service_only(&opts)
            })?,
            (Page::Main, '4') => run_action(&mut out, "Removing service...", || {
                action_remove_service(&opts)
            })?,
            (Page::Main, '5') => run_action(&mut out, "Removing profile...", || {
                action_remove_profile(&opts)
            })?,
            (Page::Main, '6') => run_action(&mut out, "Full uninstall...", || {
                action_full_uninstall(&opts)
            })?,
            (Page::Main, 'm') => page = Page::Maintenance,
            (Page::Main, 'a') => page = Page::Advanced,
            (Page::Main, 'q') => break,

            // ── Maintenance menu ────────────────────────────
            (Page::Maintenance, '1') => {
                run_action(&mut out, "Refreshing profile...", || action_refresh(&opts))?
            }
            (Page::Maintenance, '2') => run_action(&mut out, "Reinstalling everything...", || {
                action_reinstall(&opts)
            })?,
            (Page::Maintenance, '3') => {
                run_action(&mut out, "Detecting monitors...", action_detect)?
            }
            (Page::Maintenance, '4') => {
                run_action(&mut out, "Checking service status...", action_service_status)?
            }
            (Page::Maintenance, '5') => run_action(&mut out, "Rechecking service...", || {
                action_recheck_service(&opts)
            })?,
            (Page::Maintenance, '6') => {
                run_action(&mut out, "Checking applicability...", action_check_applicability)?
            }
            (Page::Maintenance, '7') => run_action(
                &mut out,
                "Sending test toast notification...",
                || action_test_toast(&opts),
            )?,
            (Page::Maintenance, '8') => run_action(
                &mut out,
                "Force refreshing color profile...",
                || action_force_refresh_profile(&opts),
            )?,
            (Page::Maintenance, '9') => run_action(
                &mut out,
                "Force refreshing color management...",
                action_force_refresh_color_mgmt,
            )?,
            (Page::Maintenance, 'b') => page = Page::Main,
            (Page::Maintenance, 'q') => break,

            // ── Advanced menu ──────────────────────────────
            (Page::Advanced, '1') => opts.toast = !opts.toast,
            (Page::Advanced, '2') => opts.dry_run = !opts.dry_run,
            (Page::Advanced, '3') => opts.verbose = !opts.verbose,
            (Page::Advanced, '4') => opts.hdr = !opts.hdr,
            (Page::Advanced, '5') => opts.sdr = !opts.sdr,
            (Page::Advanced, '6') => opts.per_user = !opts.per_user,
            (Page::Advanced, '7') => opts.generic_default = !opts.generic_default,
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
            // Only react to Press events — on Windows crossterm also emits
            // Release and Repeat events which would double-toggle options.
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => break 'q',
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                kind: KeyEventKind::Press,
                ..
            }) => break c.to_ascii_lowercase(),
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press,
                ..
            }) => break 'q',
            _ => continue,
        }
    };
    terminal::disable_raw_mode()?;
    Ok(ch)
}

// ── Status gathering ─────────────────────────────────────────────────────

pub(crate) fn gather_status(opts: &Options) -> Status {
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
        hdr_enabled: opts.hdr,
        sdr_enabled: opts.sdr,
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

    draw_section(out, "UNINSTALL")?;
    draw_item(out, "4", "Remove Service (Keep profile)")?;
    draw_item(out, "5", "Remove Profile Only")?;
    draw_item(out, "6", "Full Uninstall (Remove everything)")?;
    draw_empty(out)?;

    draw_section(out, "MORE")?;
    draw_item(out, "M", "Maintenance (Diagnostics & refresh tools)")?;
    draw_empty(out)?;

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
    if !opts.hdr {
        active.push("NoHDR");
    }
    if !opts.sdr {
        active.push("NoSDR");
    }
    if opts.per_user {
        active.push("PerUser");
    }
    if opts.generic_default {
        active.push("GenericDef");
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
// Drawing — Maintenance menu
// ============================================================================

pub(crate) fn draw_maintenance(
    out: &mut impl Write,
    status: &Status,
    _opts: &Options,
) -> io::Result<()> {
    queue!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    draw_header(out, status)?;
    draw_sep(out, " MAINTENANCE ")?;

    draw_empty(out)?;
    draw_section(out, "PROFILE")?;
    draw_item(out, "1", "Refresh (Re-apply profile now)")?;
    draw_item(out, "2", "Reinstall (Clean reinstall everything)")?;
    draw_empty(out)?;

    draw_section(out, "DIAGNOSTICS")?;
    draw_item(out, "3", "Detect Monitors")?;
    draw_item(out, "4", "Check Service Status")?;
    draw_item(out, "5", "Recheck Service (Stop + Start)")?;
    draw_item(out, "6", "Check Applicability")?;
    draw_item(out, "7", "Test Toast Notification")?;
    draw_empty(out)?;

    draw_section(out, "FORCE REFRESH")?;
    draw_item(out, "8", "Force Refresh Color Profile")?;
    draw_item(out, "9", "Force Refresh Color Management")?;
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
// Drawing — Advanced menu
// ============================================================================

pub(crate) fn draw_advanced(
    out: &mut impl Write,
    status: &Status,
    opts: &Options,
) -> io::Result<()> {
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

    draw_section(out, "COLOR MODE")?;
    draw_toggle(out, "4", "HDR Mode (Advanced color association)", opts.hdr)?;
    draw_toggle(out, "5", "SDR Mode (Standard color association)", opts.sdr)?;
    draw_empty(out)?;

    draw_section(out, "INSTALL MODE")?;
    draw_toggle(
        out,
        "6",
        "Per-User Install (User scope, not system)",
        opts.per_user,
    )?;
    draw_toggle(
        out,
        "7",
        "Generic Default (Legacy default profile API)",
        opts.generic_default,
    )?;
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

    let version_line = format!("Version {}  \u{2502}  {}", env!("APP_VERSION"), REPO);
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
            format!("\u{25CF} {} monitor(s) detected", status.monitor_count),
            Color::Green,
        )
    } else {
        ("\u{25CB} None detected".to_string(), Color::DarkGrey)
    };
    draw_status(out, "LG UltraGear: ", &monitor_text, monitor_color)?;

    // HDR mode status
    let (hdr_text, hdr_color) = if status.hdr_enabled {
        ("\u{25CF} Enabled", Color::Green)
    } else {
        ("\u{25CB} Disabled", Color::DarkGrey)
    };
    draw_status(out, "HDR Mode:     ", hdr_text, hdr_color)?;

    // SDR mode status
    let (sdr_text, sdr_color) = if status.sdr_enabled {
        ("\u{25CF} Enabled", Color::Green)
    } else {
        ("\u{25CB} Disabled", Color::DarkGrey)
    };
    draw_status(out, "SDR Mode:     ", sdr_text, sdr_color)?;

    // Status sub-box bottom
    let status_bottom = format!("\u{2514}{}\u{2518}", "\u{2500}".repeat(INNER - 2));
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

fn draw_toggle(out: &mut impl Write, key: &str, text: &str, enabled: bool) -> io::Result<()> {
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

fn draw_status(out: &mut impl Write, label: &str, value: &str, color: Color) -> io::Result<()> {
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
// Colored log tags — used by action functions for consistent output
// ============================================================================

/// Print a log line with a colored tag prefix: `  [TAG] message`.
fn log_tag(tag: &str, color: Color, msg: &str) {
    let mut out = io::stdout();
    let _ = queue!(out, SetForegroundColor(color));
    let _ = write!(out, "  {}", tag);
    let _ = queue!(out, ResetColor);
    let _ = writeln!(out, " {}", msg);
    let _ = out.flush();
}

fn log_ok(msg: &str) {
    log_tag("[ OK ]", Color::Green, msg);
}
fn log_dry(msg: &str) {
    log_tag("[DRY RUN]", Color::Cyan, msg);
}
fn log_done(msg: &str) {
    println!(); // blank line before completion tag
    log_tag("[DONE]", Color::Green, msg);
}
fn log_info(msg: &str) {
    log_tag("[INFO]", Color::Blue, msg);
}
fn log_warn(msg: &str) {
    log_tag("[WARN]", Color::Yellow, msg);
}
fn log_note(msg: &str) {
    log_tag("[NOTE]", Color::DarkGrey, msg);
}
fn log_skip(msg: &str) {
    log_tag("[SKIP]", Color::DarkGrey, msg);
}
#[allow(dead_code)] // Part of the log helpers API; used in tests
fn log_err(msg: &str) {
    log_tag("[ERR ]", Color::Red, msg);
}

/// Write a colored error tag to an arbitrary `Write` sink (used by
/// `run_action` which writes to `out` rather than stdout).
fn write_err(out: &mut impl Write, msg: &str) -> io::Result<()> {
    queue!(out, SetForegroundColor(Color::Red))?;
    write!(out, "  [ERR ]")?;
    queue!(out, ResetColor)?;
    writeln!(out, " {}", msg)?;
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
            write_err(out, &e.to_string())?;
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
        log_dry("Would extract ICC profile to color store");
        log_dry("Would write default config");
        log_dry("Would install Windows service");
        log_dry("Would start service");
        return Ok(());
    }

    let cfg = Config::load();

    // Extract ICC profile
    let profile_path = cfg.profile_path();
    match lg_profile::ensure_profile_installed(&profile_path)? {
        true => log_ok(&format!("ICC profile installed to {}", profile_path.display())),
        false => log_ok("ICC profile already present"),
    }

    // Write default config
    let cfg_path = config::config_path();
    if !cfg_path.exists() {
        Config::write_default()?;
        log_ok(&format!("Default config written to {}", cfg_path.display()));
    } else {
        log_ok(&format!("Config already exists at {}", cfg_path.display()));
    }

    // Install service
    lg_service::install(&cfg.monitor_match)?;
    log_ok("Service installed");

    // Start service
    lg_service::start_service()?;
    log_ok("Service started");

    log_done("Default install complete!");
    Ok(())
}

fn action_profile_only(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would extract ICC profile to color store");
        return Ok(());
    }

    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    match lg_profile::ensure_profile_installed(&profile_path)? {
        true => log_ok(&format!("ICC profile installed to {}", profile_path.display())),
        false => log_ok("ICC profile already present"),
    }

    log_done("Profile install complete!");
    Ok(())
}

fn action_service_only(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would write default config");
        log_dry("Would install Windows service");
        log_dry("Would start service");
        return Ok(());
    }

    let cfg = Config::load();
    let cfg_path = config::config_path();
    if !cfg_path.exists() {
        Config::write_default()?;
        log_ok("Default config written");
    }

    lg_service::install(&cfg.monitor_match)?;
    log_ok("Service installed");

    lg_service::start_service()?;
    log_ok("Service started");

    log_done("Service install complete!");
    Ok(())
}

fn action_refresh(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would re-apply profile to matching monitors");
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
        log_skip("No matching monitors found.");
    } else {
        for device in &devices {
            log_info(&format!("Found: {}", device.name));
            lg_profile::reapply_profile(
                &device.device_key,
                &profile_path,
                cfg.toggle_delay_ms,
                opts.per_user,
            )?;
            log_ok(&format!("Profile reapplied for {}", device.name));
            if opts.generic_default {
                lg_profile::set_generic_default(
                    &device.device_key,
                    &profile_path,
                    opts.per_user,
                )?;
                log_ok(&format!("Generic default set for {}", device.name));
            }
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

        log_done(&format!("Profile refreshed for {} monitor(s).", devices.len()));
    }

    Ok(())
}

fn action_reinstall(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would uninstall service");
        log_dry("Would reinstall profile + service");
        return Ok(());
    }

    // Best-effort uninstall first
    match lg_service::uninstall() {
        Ok(()) => log_ok("Service uninstalled"),
        Err(e) => log_note(&format!("Service removal: {} (continuing)", e)),
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
        log_dry("Would uninstall Windows service");
        return Ok(());
    }

    lg_service::uninstall()?;
    log_ok("Service uninstalled");
    log_note("ICC profile preserved in color store");
    Ok(())
}

fn action_remove_profile(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would remove ICC profile from color store");
        return Ok(());
    }

    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    match lg_profile::remove_profile(&profile_path)? {
        true => log_ok(&format!("ICC profile removed from {}", profile_path.display())),
        false => log_note("ICC profile not found (already removed)"),
    }
    Ok(())
}

fn action_full_uninstall(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would uninstall service");
        log_dry("Would remove ICC profile");
        log_dry("Would remove config directory");
        return Ok(());
    }

    // Remove service (best-effort)
    match lg_service::uninstall() {
        Ok(()) => log_ok("Service uninstalled"),
        Err(e) => log_note(&format!("Service removal: {} (continuing)", e)),
    }

    // Remove profile
    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    match lg_profile::remove_profile(&profile_path)? {
        true => log_ok("ICC profile removed"),
        false => log_note("ICC profile not found (already removed)"),
    }

    // Remove config directory
    let cfg_dir = config::config_dir();
    if cfg_dir.exists() {
        match std::fs::remove_dir_all(&cfg_dir) {
            Ok(()) => log_ok(&format!("Config directory removed: {}", cfg_dir.display())),
            Err(e) => log_warn(&format!(
                "Could not remove config dir: {} (clean up manually)",
                e
            )),
        }
    }

    log_done("Full uninstall complete!");
    Ok(())
}

// ============================================================================
// Maintenance actions
// ============================================================================

fn action_service_status() -> Result<(), Box<dyn std::error::Error>> {
    let (installed, running) = lg_service::query_service_info();
    if installed {
        if running {
            log_ok("Service is installed and running");
        } else {
            log_warn("Service is installed but NOT running");
        }
    } else {
        log_warn("Service is NOT installed");
    }
    println!();
    lg_service::print_status()?;
    Ok(())
}

fn action_recheck_service(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would stop then start the service");
        return Ok(());
    }

    log_info("Stopping service...");
    match lg_service::stop_service() {
        Ok(()) => log_ok("Service stopped"),
        Err(e) => log_note(&format!("Stop: {} (continuing)", e)),
    }

    log_info("Starting service...");
    lg_service::start_service()?;
    log_ok("Service started");

    log_done("Service rechecked and restarted.");
    Ok(())
}

fn action_check_applicability() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load();

    // Check monitor
    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    if devices.is_empty() {
        log_warn(&format!("No monitors matching \"{}\"", cfg.monitor_match));
    } else {
        log_ok(&format!(
            "{} monitor(s) matching \"{}\"",
            devices.len(),
            cfg.monitor_match
        ));
        for d in &devices {
            println!("         - {}", d.name);
        }
    }

    // Check profile
    let profile_path = cfg.profile_path();
    if lg_profile::is_profile_installed(&profile_path) {
        log_ok(&format!("ICC profile installed at {}", profile_path.display()));
    } else {
        log_warn(&format!(
            "ICC profile NOT found at {}",
            profile_path.display()
        ));
    }

    // Check service
    let (installed, running) = lg_service::query_service_info();
    if installed {
        if running {
            log_ok("Service installed and running");
        } else {
            log_warn("Service installed but NOT running");
        }
    } else {
        log_warn("Service NOT installed");
    }

    // Check config
    let cfg_path = config::config_path();
    if cfg_path.exists() {
        log_ok(&format!("Config file exists at {}", cfg_path.display()));
    } else {
        log_info("No config file (using defaults)");
    }

    // Summary
    let all_good = !devices.is_empty()
        && lg_profile::is_profile_installed(&profile_path)
        && installed
        && running;
    if all_good {
        log_done("Everything looks good!");
    } else {
        log_done("Some issues detected — see warnings above.");
    }

    Ok(())
}

fn action_test_toast(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load();
    log_info("Sending test toast notification...");
    lg_notify::show_reapply_toast(true, &cfg.toast_title, &cfg.toast_body, opts.verbose);
    if opts.toast {
        log_ok("Toast notification sent (check your notification area)");
    } else {
        log_note("Toast toggle is OFF in Advanced Options — sent anyway for testing");
    }
    log_done("Test toast complete.");
    Ok(())
}

fn action_force_refresh_profile(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load();
    let profile_path = cfg.profile_path();
    lg_profile::ensure_profile_installed(&profile_path)?;

    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    if devices.is_empty() {
        log_skip("No matching monitors found.");
    } else {
        for device in &devices {
            log_info(&format!("Force reapplying to: {}", device.name));
            lg_profile::reapply_profile(
                &device.device_key,
                &profile_path,
                cfg.toggle_delay_ms,
                opts.per_user,
            )?;
            log_ok(&format!("Profile reapplied for {}", device.name));
            if opts.generic_default {
                lg_profile::set_generic_default(
                    &device.device_key,
                    &profile_path,
                    opts.per_user,
                )?;
                log_ok(&format!("Generic default set for {}", device.name));
            }
        }
        log_done(&format!(
            "Color profile force-refreshed for {} monitor(s).",
            devices.len()
        ));
    }
    Ok(())
}

fn action_force_refresh_color_mgmt() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load();

    log_info("Broadcasting display settings refresh...");
    lg_profile::refresh_display(true, true, true);
    log_ok("ChangeDisplaySettingsEx sent");
    log_ok("WM_SETTINGCHANGE \"Color\" broadcast sent");
    log_ok("InvalidateRect sent");

    log_info("Triggering Calibration Loader...");
    lg_profile::trigger_calibration_loader(cfg.refresh_calibration_loader);
    log_ok("Calibration Loader task triggered");

    log_done("Color management force-refreshed.");
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
            hdr_enabled: true,
            sdr_enabled: true,
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
            hdr: false,
            sdr: true,
            per_user: false,
            generic_default: false,
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
        let opts = default_opts();
        let _s = gather_status(&opts);
    }

    #[test]
    fn gather_status_returns_valid_data() {
        let opts = default_opts();
        let s = gather_status(&opts);
        // If service not installed, it can't be running
        if !s.service_installed {
            assert!(!s.service_running);
        }
    }

    // ── Page enum ────────────────────────────────────────────────

    #[test]
    fn page_variants_exist() {
        let _main = Page::Main;
        let _maint = Page::Maintenance;
        let _adv = Page::Advanced;
    }

    // ── Main menu drawing ────────────────────────────────────────

    #[test]
    fn draw_main_contains_all_menu_items() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        // 6 numbered install/uninstall items + M, A, Q keys
        assert!(output.contains("[1]"), "should contain item 1");
        assert!(output.contains("[2]"), "should contain item 2");
        assert!(output.contains("[3]"), "should contain item 3");
        assert!(output.contains("[4]"), "should contain item 4");
        assert!(output.contains("[5]"), "should contain item 5");
        assert!(output.contains("[6]"), "should contain item 6");
        assert!(output.contains("[M]"), "should contain Maintenance key");
        assert!(output.contains("[A]"), "should contain Advanced key");
        assert!(output.contains("[Q]"), "should contain Quit key");
    }

    #[test]
    fn draw_main_contains_install_section() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("INSTALL OPTIONS"));
    }

    #[test]
    fn draw_main_contains_more_section() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("MORE"));
    }

    #[test]
    fn draw_main_contains_uninstall_section() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("UNINSTALL"));
    }

    #[test]
    fn draw_main_contains_advanced_item() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("Advanced Options"));
    }

    #[test]
    fn draw_main_contains_quit_option() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("[Q]"));
        assert!(output.contains("Quit"));
    }

    #[test]
    fn draw_main_contains_advanced_key() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("[A]"));
        assert!(output.contains("Advanced Options"));
    }

    #[test]
    fn draw_main_install_labels() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("Default Install"));
        assert!(output.contains("Profile Only"));
        assert!(output.contains("Service Only"));
    }

    #[test]
    fn draw_main_maintenance_link() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("Maintenance"));
        assert!(output.contains("Diagnostics"));
    }

    #[test]
    fn draw_main_uninstall_labels() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("Remove Service"));
        assert!(output.contains("Remove Profile Only"));
        assert!(output.contains("Full Uninstall"));
    }

    #[test]
    fn draw_main_shows_no_active_toggles_by_default() {
        // Default opts have hdr=false so "NoHDR" will be active.
        // Verify the main menu shows the active toggle indicator.
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(
            output.contains("NoHDR"),
            "Default opts should show NoHDR since hdr defaults to false"
        );
    }

    #[test]
    fn draw_main_shows_active_toggles_when_set() {
        let opts = Options {
            toast: false, // toggled off → shows NoToast
            dry_run: true,
            verbose: true,
            hdr: true,
            sdr: true,
            per_user: false,
            generic_default: false,
        };
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
        assert!(output.contains("NoToast"), "should show NoToast");
        assert!(output.contains("DryRun"), "should show DryRun");
        assert!(output.contains("Verbose"), "should show Verbose");
    }

    #[test]
    fn draw_main_select_option_prompt() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains("Select option"));
    }

    // ── Box drawing characters in main menu ──────────────────────

    #[test]
    fn draw_main_contains_box_drawing_chars() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(output.contains('\u{2554}'), "top-left corner \u{2554}");
        assert!(output.contains('\u{2557}'), "top-right corner \u{2557}");
        assert!(output.contains('\u{255A}'), "bottom-left corner \u{255a}");
        assert!(output.contains('\u{255D}'), "bottom-right corner \u{255d}");
        assert!(output.contains('\u{2551}'), "vertical line \u{2551}");
    }

    // ── Advanced menu drawing ────────────────────────────────────

    #[test]
    fn draw_advanced_contains_3_toggles() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("[1]"), "toggle 1");
        assert!(output.contains("[2]"), "toggle 2");
        assert!(output.contains("[3]"), "toggle 3");
        assert!(output.contains("[4]"), "toggle 4 (HDR)");
        assert!(output.contains("[5]"), "toggle 5 (SDR)");
    }

    #[test]
    fn draw_advanced_contains_toggle_labels() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("Toast Notifications"));
        assert!(output.contains("Dry Run"));
        assert!(output.contains("Verbose Logging"));
    }

    #[test]
    fn draw_advanced_contains_back_option() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("[B]"));
        assert!(output.contains("Back to Main Menu"));
    }

    #[test]
    fn draw_advanced_contains_quit_option() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("[Q]"));
        assert!(output.contains("Quit"));
    }

    #[test]
    fn draw_advanced_toast_on_by_default() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        // Toast should be ON by default (assuming config has toast_enabled=true)
        assert!(output.contains("[ON ]"), "toast should be ON by default");
    }

    #[test]
    fn draw_advanced_dry_run_off_by_default() {
        let opts = default_opts();
        assert!(!opts.dry_run, "dry_run defaults to false");
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
        assert!(output.contains("[OFF]"), "dry_run/verbose should be OFF");
    }

    #[test]
    fn draw_advanced_toggles_reflect_options() {
        let opts = Options {
            toast: false,
            dry_run: true,
            verbose: true,
            hdr: true,
            sdr: true,
            per_user: false,
            generic_default: false,
        };
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
        // With toast=false, dry_run=true, verbose=true, hdr=true, sdr=true, per_user=false, generic_default=false
        // ON: dry_run, verbose, hdr, sdr = 4 ON; OFF: toast, per_user, generic_default = 3 OFF
        let on_count = output.matches("[ON ]").count();
        let off_count = output.matches("[OFF]").count();
        assert_eq!(on_count, 4, "dry_run+verbose+hdr+sdr should be ON");
        assert_eq!(off_count, 3, "toast+per_user+generic_default should be OFF");
    }

    #[test]
    fn draw_advanced_section_headers() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("NOTIFICATIONS"));
        assert!(output.contains("TESTING"));
        assert!(output.contains("NAVIGATION"));
    }

    #[test]
    fn draw_advanced_info_text() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("toggles affect main menu"));
    }

    #[test]
    fn draw_advanced_title() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("ADVANCED OPTIONS"));
    }

    // ── Maintenance menu drawing ─────────────────────────────────

    #[test]
    fn draw_maintenance_contains_all_items() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("[1]"), "item 1");
        assert!(output.contains("[2]"), "item 2");
        assert!(output.contains("[3]"), "item 3");
        assert!(output.contains("[4]"), "item 4");
        assert!(output.contains("[5]"), "item 5");
        assert!(output.contains("[6]"), "item 6");
        assert!(output.contains("[7]"), "item 7");
        assert!(output.contains("[8]"), "item 8");
        assert!(output.contains("[9]"), "item 9");
        assert!(output.contains("[B]"), "back key");
        assert!(output.contains("[Q]"), "quit key");
    }

    #[test]
    fn draw_maintenance_profile_section() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("PROFILE"));
    }

    #[test]
    fn draw_maintenance_diagnostics_section() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("DIAGNOSTICS"));
    }

    #[test]
    fn draw_maintenance_force_refresh_section() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("FORCE REFRESH"));
    }

    #[test]
    fn draw_maintenance_navigation_section() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("NAVIGATION"));
        assert!(output.contains("Back to Main Menu"));
        assert!(output.contains("Quit"));
    }

    #[test]
    fn draw_maintenance_item_labels() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("Refresh"), "should have Refresh");
        assert!(output.contains("Reinstall"), "should have Reinstall");
        assert!(output.contains("Detect Monitors"), "should have Detect");
        assert!(
            output.contains("Check Service Status"),
            "should have Service Status"
        );
        assert!(
            output.contains("Recheck Service"),
            "should have Recheck Service"
        );
        assert!(
            output.contains("Check Applicability"),
            "should have Applicability"
        );
        assert!(
            output.contains("Test Toast Notification"),
            "should have Test Toast"
        );
        assert!(
            output.contains("Force Refresh Color Profile"),
            "should have Force Refresh Profile"
        );
        assert!(
            output.contains("Force Refresh Color Management"),
            "should have Force Refresh Color Mgmt"
        );
    }

    #[test]
    fn draw_maintenance_title() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("MAINTENANCE"));
    }

    #[test]
    fn draw_maintenance_produces_nonempty_output() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(!output.is_empty());
        assert!(
            output.len() > 300,
            "maintenance menu should produce substantial output"
        );
    }

    #[test]
    fn draw_maintenance_contains_box_drawing_chars() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains('\u{2554}'), "top-left corner");
        assert!(output.contains('\u{2557}'), "top-right corner");
        assert!(output.contains('\u{255A}'), "bottom-left corner");
        assert!(output.contains('\u{255D}'), "bottom-right corner");
        assert!(output.contains('\u{2551}'), "vertical line");
    }

    #[test]
    fn draw_maintenance_select_option_prompt() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
        assert!(output.contains("Select option"));
    }

    #[test]
    fn draw_maintenance_all_status_combos() {
        for profile in [false, true] {
            for svc_installed in [false, true] {
                for svc_running in [false, true] {
                    for count in [0, 1, 5] {
                        let s = test_status(profile, svc_installed, svc_running, count);
                        let output = render_to_string(|buf| {
                            draw_maintenance(buf, &s, &default_opts())
                        });
                        assert!(!output.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn draw_maintenance_with_all_good_status() {
        let output =
            render_to_string(|buf| draw_maintenance(buf, &all_good_status(), &default_opts()));
        assert!(output.contains("Running"));
    }

    // ── Header drawing ───────────────────────────────────────────

    #[test]
    fn draw_header_contains_title() {
        let output = render_to_string(|buf| draw_header(buf, &default_status()));
        assert!(output.contains(TITLE));
    }

    #[test]
    fn draw_header_contains_version() {
        let output = render_to_string(|buf| draw_header(buf, &default_status()));
        assert!(
            output.contains(env!("APP_VERSION")),
            "header should show version from VERSION file"
        );
    }

    #[test]
    fn draw_header_contains_repo() {
        let output = render_to_string(|buf| draw_header(buf, &default_status()));
        assert!(output.contains(REPO));
    }

    #[test]
    fn draw_header_shows_current_status_label() {
        let output = render_to_string(|buf| draw_header(buf, &default_status()));
        assert!(output.contains("CURRENT STATUS"));
    }

    #[test]
    fn draw_header_shows_profile_not_installed() {
        let output = render_to_string(|buf| draw_header(buf, &test_status(false, false, false, 0)));
        assert!(output.contains("Not Installed"));
    }

    #[test]
    fn draw_header_shows_profile_installed() {
        let output = render_to_string(|buf| draw_header(buf, &test_status(true, false, false, 0)));
        assert!(output.contains("Installed"));
    }

    #[test]
    fn draw_header_shows_service_not_installed() {
        let output = render_to_string(|buf| draw_header(buf, &test_status(false, false, false, 0)));
        // Service not installed → "Not Installed"
        assert!(output.contains("Not Installed"));
    }

    #[test]
    fn draw_header_shows_service_running() {
        let output = render_to_string(|buf| draw_header(buf, &test_status(true, true, true, 1)));
        assert!(output.contains("Running"));
    }

    #[test]
    fn draw_header_shows_service_stopped() {
        let output = render_to_string(|buf| draw_header(buf, &test_status(true, true, false, 1)));
        assert!(output.contains("Stopped"));
    }

    #[test]
    fn draw_header_shows_monitors_detected() {
        let output = render_to_string(|buf| draw_header(buf, &test_status(true, true, true, 3)));
        assert!(output.contains("3 monitor(s) detected"));
    }

    #[test]
    fn draw_header_shows_no_monitors() {
        let output = render_to_string(|buf| draw_header(buf, &test_status(false, false, false, 0)));
        assert!(output.contains("None detected"));
    }

    #[test]
    fn draw_header_shows_status_labels() {
        let output = render_to_string(|buf| draw_header(buf, &default_status()));
        assert!(output.contains("Color Profile:"));
        assert!(output.contains("Service:"));
        assert!(output.contains("LG UltraGear:"));
        assert!(output.contains("HDR Mode:"));
        assert!(output.contains("SDR Mode:"));
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
        let output = render_to_string(|buf| draw_main(buf, &all_good_status(), &default_opts()));
        assert!(output.contains("Installed"));
        assert!(output.contains("Running"));
        assert!(output.contains("1 monitor(s) detected"));
    }

    #[test]
    fn draw_main_with_service_stopped() {
        let s = test_status(true, true, false, 2);
        let output = render_to_string(|buf| draw_main(buf, &s, &default_opts()));
        assert!(output.contains("Stopped"));
        assert!(output.contains("2 monitor(s) detected"));
    }

    #[test]
    fn draw_advanced_with_all_good_status() {
        let output =
            render_to_string(|buf| draw_advanced(buf, &all_good_status(), &default_opts()));
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
            hdr: false,
            sdr: false,
            per_user: true,
            generic_default: true,
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
        if !opts.hdr {
            active.push("NoHDR");
        }
        if !opts.sdr {
            active.push("NoSDR");
        }
        if opts.per_user {
            active.push("PerUser");
        }
        if opts.generic_default {
            active.push("GenericDef");
        }
        assert_eq!(active.len(), 7);
        assert_eq!(active, vec!["NoToast", "DryRun", "Verbose", "NoHDR", "NoSDR", "PerUser", "GenericDef"]);
    }

    // ── Rendering consistency ────────────────────────────────────

    #[test]
    fn draw_main_produces_nonempty_output() {
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
        assert!(!output.is_empty());
        assert!(
            output.len() > 500,
            "main menu should produce substantial output"
        );
    }

    #[test]
    fn draw_advanced_produces_nonempty_output() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(!output.is_empty());
        assert!(
            output.len() > 300,
            "advanced menu should produce substantial output"
        );
    }

    #[test]
    fn draw_goodbye_produces_nonempty_output() {
        let output = render_to_string(draw_goodbye);
        assert!(!output.is_empty());
        assert!(output.len() > 100);
    }

    #[test]
    fn draw_header_produces_nonempty_output() {
        let output = render_to_string(|buf| draw_header(buf, &default_status()));
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
                        let output = render_to_string(|buf| draw_main(buf, &s, &default_opts()));
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
                    for hdr in [false, true] {
                        for sdr in [false, true] {
                            let opts = Options {
                                toast,
                                dry_run: dry,
                                verbose: verb,
                                hdr,
                                sdr,
                                per_user: false,
                                generic_default: false,
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
    }

    // ── Toggle mechanics (simulates the run() key handling) ──────

    #[test]
    fn toggle_toast_flips_correctly() {
        let mut opts = default_opts();
        assert!(opts.toast);
        // Simulate pressing '1' on Advanced page
        opts.toast = !opts.toast;
        assert!(!opts.toast);
        // Re-draw should show OFF
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
        // Toast is item [1]; find its toggle state
        assert!(
            output.contains("[OFF]"),
            "toast should be OFF after toggle"
        );
        // Toggle back
        opts.toast = !opts.toast;
        assert!(opts.toast);
    }

    #[test]
    fn toggle_dry_run_flips_correctly() {
        let mut opts = default_opts();
        assert!(!opts.dry_run);
        opts.dry_run = !opts.dry_run;
        assert!(opts.dry_run);
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
        // dry_run ON, toast ON, sdr ON = 3 ON; verbose OFF, hdr OFF, per_user OFF, generic_default OFF = 4 OFF
        let on_count = output.matches("[ON ]").count();
        let off_count = output.matches("[OFF]").count();
        assert_eq!(on_count, 3, "toast+dry_run+sdr ON");
        assert_eq!(off_count, 4, "verbose+hdr+per_user+generic_default OFF");
    }

    #[test]
    fn toggle_verbose_flips_correctly() {
        let mut opts = default_opts();
        assert!(!opts.verbose);
        opts.verbose = !opts.verbose;
        assert!(opts.verbose);
        opts.verbose = !opts.verbose;
        assert!(!opts.verbose);
    }

    #[test]
    fn toggle_hdr_flips_correctly() {
        let mut opts = default_opts();
        assert!(!opts.hdr, "HDR should default OFF");
        opts.hdr = !opts.hdr;
        assert!(opts.hdr);
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
        // With hdr=true: toast ON, dry_run OFF, verbose OFF, hdr ON, sdr ON, per_user OFF, generic_default OFF → 3 ON, 4 OFF
        let on_count = output.matches("[ON ]").count();
        let off_count = output.matches("[OFF]").count();
        assert_eq!(on_count, 3);
        assert_eq!(off_count, 4);
    }

    #[test]
    fn toggle_sdr_flips_correctly() {
        let mut opts = default_opts();
        assert!(opts.sdr, "SDR should default ON");
        opts.sdr = !opts.sdr;
        assert!(!opts.sdr);
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
        // With sdr=false: toast ON, dry_run OFF, verbose OFF, hdr OFF, sdr OFF, per_user OFF, generic_default OFF → 1 ON, 6 OFF
        let on_count = output.matches("[ON ]").count();
        let off_count = output.matches("[OFF]").count();
        assert_eq!(on_count, 1);
        assert_eq!(off_count, 6);
    }

    #[test]
    fn toggle_sequence_round_trips() {
        let mut opts = default_opts();
        // Toggle all to opposite
        opts.toast = !opts.toast;
        opts.dry_run = !opts.dry_run;
        opts.verbose = !opts.verbose;
        opts.hdr = !opts.hdr;
        opts.sdr = !opts.sdr;
        opts.per_user = !opts.per_user;
        opts.generic_default = !opts.generic_default;
        assert!(!opts.toast);
        assert!(opts.dry_run);
        assert!(opts.verbose);
        assert!(opts.hdr); // was false, now true
        assert!(!opts.sdr);
        assert!(opts.per_user);
        assert!(opts.generic_default);
        // Toggle all back
        opts.toast = !opts.toast;
        opts.dry_run = !opts.dry_run;
        opts.verbose = !opts.verbose;
        opts.hdr = !opts.hdr;
        opts.sdr = !opts.sdr;
        opts.per_user = !opts.per_user;
        opts.generic_default = !opts.generic_default;
        assert!(opts.toast);
        assert!(!opts.dry_run);
        assert!(!opts.verbose);
        assert!(!opts.hdr); // back to false
        assert!(opts.sdr);
        assert!(!opts.per_user);
        assert!(!opts.generic_default);
    }

    // ── HDR/SDR status display in header ─────────────────────────

    #[test]
    fn draw_header_shows_hdr_enabled() {
        let mut s = default_status();
        s.hdr_enabled = true;
        let output = render_to_string(|buf| draw_header(buf, &s));
        assert!(output.contains("HDR Mode:"));
        assert!(output.contains("Enabled"));
    }

    #[test]
    fn draw_header_shows_hdr_disabled() {
        let mut s = default_status();
        s.hdr_enabled = false;
        let output = render_to_string(|buf| draw_header(buf, &s));
        assert!(output.contains("HDR Mode:"));
        assert!(output.contains("Disabled"));
    }

    #[test]
    fn draw_header_shows_sdr_enabled() {
        let mut s = default_status();
        s.sdr_enabled = true;
        let output = render_to_string(|buf| draw_header(buf, &s));
        assert!(output.contains("SDR Mode:"));
        assert!(output.contains("Enabled"));
    }

    #[test]
    fn draw_header_shows_sdr_disabled() {
        let mut s = default_status();
        s.sdr_enabled = false;
        let output = render_to_string(|buf| draw_header(buf, &s));
        assert!(output.contains("SDR Mode:"));
        assert!(output.contains("Disabled"));
    }

    #[test]
    fn draw_header_hdr_sdr_both_enabled_by_default() {
        let s = default_status(); // hdr_enabled=true, sdr_enabled=true
        let output = render_to_string(|buf| draw_header(buf, &s));
        // Should show both as Enabled
        let enabled_count = output.matches("Enabled").count();
        assert!(
            enabled_count >= 2,
            "HDR and SDR should both show Enabled; got {} occurrences",
            enabled_count
        );
    }

    // ── HDR/SDR in advanced menu ─────────────────────────────────

    #[test]
    fn draw_advanced_contains_5_toggles() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("[1]"), "toggle 1");
        assert!(output.contains("[2]"), "toggle 2");
        assert!(output.contains("[3]"), "toggle 3");
        assert!(output.contains("[4]"), "toggle 4 (HDR)");
        assert!(output.contains("[5]"), "toggle 5 (SDR)");
    }

    #[test]
    fn draw_advanced_contains_hdr_sdr_labels() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("HDR Mode"));
        assert!(output.contains("SDR Mode"));
    }

    #[test]
    fn draw_advanced_contains_color_mode_section() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(output.contains("COLOR MODE"));
    }

    #[test]
    fn draw_advanced_hdr_sdr_on_by_default() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        // Default: toast=ON, dry_run=OFF, verbose=OFF, hdr=OFF, sdr=ON, per_user=OFF, generic_default=OFF → 2 ON, 5 OFF
        let on_count = output.matches("[ON ]").count();
        let off_count = output.matches("[OFF]").count();
        assert_eq!(on_count, 2, "toast+sdr should be ON");
        assert_eq!(off_count, 5, "dry_run+verbose+hdr+per_user+generic_default should be OFF");
    }

    // ── Active toggles in main menu with HDR/SDR ─────────────────

    #[test]
    fn draw_main_shows_no_hdr_when_toggled_off() {
        let opts = Options {
            toast: true,
            dry_run: false,
            verbose: false,
            hdr: false,
            sdr: true,
            per_user: false,
            generic_default: false,
        };
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
        assert!(output.contains("NoHDR"), "should show NoHDR");
        assert!(!output.contains("NoSDR"), "should not show NoSDR");
    }

    #[test]
    fn draw_main_shows_no_sdr_when_toggled_off() {
        let opts = Options {
            toast: true,
            dry_run: false,
            verbose: false,
            hdr: true,
            sdr: false,
            per_user: false,
            generic_default: false,
        };
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
        assert!(!output.contains("NoHDR"), "should not show NoHDR");
        assert!(output.contains("NoSDR"), "should show NoSDR");
    }

    #[test]
    fn draw_main_no_active_when_hdr_sdr_on() {
        // All defaults produce "NoHDR" since hdr defaults to false,
        // so provide explicit all-on opts to test the "None active" path.
        let opts = Options {
            toast: true,
            dry_run: false,
            verbose: false,
            hdr: true,
            sdr: true,
            per_user: false,
            generic_default: false,
        };
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
        assert!(
            output.contains("None active"),
            "all-on opts should show 'None active'"
        );
    }

    // ── Status struct with HDR/SDR fields ────────────────────────

    #[test]
    fn status_hdr_sdr_defaults() {
        let s = default_status();
        assert!(s.hdr_enabled, "HDR should default enabled");
        assert!(s.sdr_enabled, "SDR should default enabled");
    }

    #[test]
    fn gather_status_reflects_options_hdr_sdr() {
        let mut opts = default_opts();
        opts.hdr = false;
        opts.sdr = false;
        let s = gather_status(&opts);
        assert!(!s.hdr_enabled, "status should mirror opts.hdr=false");
        assert!(!s.sdr_enabled, "status should mirror opts.sdr=false");
    }

    // ── Options defaults include HDR/SDR ─────────────────────────

    #[test]
    fn options_default_hdr_is_false() {
        let opts = Options::default();
        assert!(!opts.hdr);
    }

    #[test]
    fn options_default_sdr_is_true() {
        let opts = Options::default();
        assert!(opts.sdr);
    }

    #[test]
    fn options_default_per_user_is_false() {
        let opts = Options::default();
        assert!(!opts.per_user);
    }

    #[test]
    fn options_default_generic_default_is_false() {
        let opts = Options::default();
        assert!(!opts.generic_default);
    }

    #[test]
    fn toggle_per_user_flips_correctly() {
        let mut opts = default_opts();
        assert!(!opts.per_user);
        opts.per_user = !opts.per_user;
        assert!(opts.per_user);
        opts.per_user = !opts.per_user;
        assert!(!opts.per_user);
    }

    #[test]
    fn toggle_generic_default_flips_correctly() {
        let mut opts = default_opts();
        assert!(!opts.generic_default);
        opts.generic_default = !opts.generic_default;
        assert!(opts.generic_default);
        opts.generic_default = !opts.generic_default;
        assert!(!opts.generic_default);
    }

    #[test]
    fn draw_main_shows_per_user_when_toggled_on() {
        let mut opts = default_opts();
        opts.per_user = true;
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
        assert!(output.contains("PerUser"), "should show PerUser");
    }

    #[test]
    fn draw_main_shows_generic_def_when_toggled_on() {
        let mut opts = default_opts();
        opts.generic_default = true;
        let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
        assert!(output.contains("GenericDef"), "should show GenericDef");
    }

    #[test]
    fn draw_advanced_shows_install_mode_section() {
        let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
        assert!(
            output.contains("INSTALL MODE"),
            "should have INSTALL MODE section"
        );
        assert!(
            output.contains("Per-User Install"),
            "should show Per-User toggle"
        );
        assert!(
            output.contains("Generic Default"),
            "should show Generic Default toggle"
        );
    }

    // ── Colored log tag helpers ──────────────────────────────────

    #[test]
    fn log_tag_produces_ansi_colored_output() {
        // log_tag writes to stdout which we can't capture easily,
        // but we can verify write_err writes the correct structure.
        let mut buf = Vec::new();
        write_err(&mut buf, "something broke").unwrap();
        let output = String::from_utf8_lossy(&buf).to_string();
        assert!(output.contains("[ERR ]"), "should contain ERR tag");
        assert!(output.contains("something broke"), "should contain message");
    }

    #[test]
    fn write_err_contains_ansi_sequences() {
        let mut buf = Vec::new();
        write_err(&mut buf, "test error").unwrap();
        let output = String::from_utf8_lossy(&buf).to_string();
        // crossterm ANSI sequences start with ESC [
        assert!(
            output.contains("\x1b["),
            "should contain ANSI escape sequences"
        );
    }

    #[test]
    fn write_err_resets_color() {
        let mut buf = Vec::new();
        write_err(&mut buf, "oops").unwrap();
        let output = String::from_utf8_lossy(&buf).to_string();
        // ResetColor emits ESC[0m
        assert!(
            output.contains("\x1b[0m"),
            "should reset color after tag"
        );
    }

    #[test]
    fn log_tag_helpers_do_not_panic() {
        // These write to stdout; just verify they don't panic.
        log_ok("test ok");
        log_dry("test dry");
        log_done("test done");
        log_info("test info");
        log_warn("test warn");
        log_note("test note");
        log_skip("test skip");
        log_err("test err");
    }
}
