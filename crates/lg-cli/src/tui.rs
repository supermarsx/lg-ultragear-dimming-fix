//! Interactive TUI for the LG UltraGear dimming fix tool.
//!
//! Provides a box-drawing terminal menu replicating the PowerShell
//! installer's interactive experience: live status display, numbered
//! actions, and toggle-based advanced settings.

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Color, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use lg_core::{
    config::{self, Config},
    state as app_state,
};
use std::io::{self, IsTerminal, Write};

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
/// Targets 45 rows × 80 columns — enough for the tallest menu page
/// (Advanced with all toggles) plus the status header and prompt.
fn ensure_console_size() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::{
            GetConsoleScreenBufferInfo, GetStdHandle, SetConsoleScreenBufferSize,
            SetConsoleWindowInfo, CONSOLE_SCREEN_BUFFER_INFO, COORD, SMALL_RECT, STD_OUTPUT_HANDLE,
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
            let want_rows = current_rows.max(45);

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
    pub(crate) ddc_brightness: bool,
    pub(crate) ddc_brightness_value: u32,
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
            ddc_brightness: cfg.ddc_brightness_on_reapply,
            ddc_brightness_value: cfg.ddc_brightness_value,
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
    Maintenance2,
    ServiceDiagnostics,
    Advanced,
    IccStudio,
    IccStudioTuning,
    IccTags,
    IccTags2,
}

// ── Entry point ──────────────────────────────────────────────────────────

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    ensure_console_size();
    let mut out = io::stdout();

    // Enter the alternate screen buffer so TUI output never pollutes
    // the main scrollback — scrolling up won't show stale content.
    execute!(out, EnterAlternateScreen)?;

    let result = run_inner(&mut out);

    // Always leave the alternate screen, even on error
    let _ = execute!(out, LeaveAlternateScreen);
    result
}

fn run_inner(mut out: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
    let mut page = Page::Main;
    let mut opts = Options::default();
    let mut icc_cfg = Config::load();
    let mut icc_dirty = false;
    let mut ddc_guardrails = app_state::load_ddc_guardrails();
    // DDC/CI Studio target: None = use config monitor_match, Some((idx, name)) = by monitor index
    let mut ddc_target: Option<(usize, String)> = None;

    loop {
        let status = gather_status(&opts);

        match page {
            Page::Main => draw_main(&mut out, &status, &opts)?,
            Page::Maintenance => draw_maintenance(&mut out, &status, &opts)?,
            Page::Maintenance2 => draw_maintenance2(&mut out, &status, &opts, ddc_target.as_ref())?,
            Page::ServiceDiagnostics => draw_service_diagnostics(&mut out, &status)?,
            Page::Advanced => draw_advanced(&mut out, &status, &opts)?,
            Page::IccStudio => draw_icc_studio(&mut out, &status, &icc_cfg, icc_dirty)?,
            Page::IccStudioTuning => {
                draw_icc_studio_tuning(&mut out, &status, &icc_cfg, icc_dirty)?
            }
            Page::IccTags => draw_icc_tags(&mut out, &status, &icc_cfg, icc_dirty)?,
            Page::IccTags2 => draw_icc_tags2(&mut out, &status, &icc_cfg, icc_dirty)?,
        }
        out.flush()?;

        let ch = read_key()?;

        let color_dir = lg_profile::color_directory();
        let program_dir = config::config_dir();
        let both_folders: Vec<(char, &str, std::path::PathBuf)> = vec![
            ('1', "Color profiles", color_dir.clone()),
            ('2', "Program / config", program_dir.clone()),
        ];
        let profile_folder: Vec<(char, &str, std::path::PathBuf)> =
            vec![('1', "Color profiles", color_dir.clone())];
        let service_folder: Vec<(char, &str, std::path::PathBuf)> =
            vec![('1', "Program / config", program_dir.clone())];

        match (&page, ch) {
            // ── Main menu ──────────────────────────────────
            (Page::Main, '1') => run_action_offer_folder(
                &mut out,
                "Installing profile + service...",
                || action_default_install(&opts),
                &both_folders,
            )?,
            (Page::Main, '2') => run_action_offer_folder(
                &mut out,
                "Installing profile only...",
                || action_profile_only(&opts),
                &profile_folder,
            )?,
            (Page::Main, '3') => run_action_offer_folder(
                &mut out,
                "Installing service only...",
                || action_service_only(&opts),
                &service_folder,
            )?,
            (Page::Main, '4') => run_action_offer_folder(
                &mut out,
                "Removing service...",
                || action_remove_service(&opts),
                &service_folder,
            )?,
            (Page::Main, '5') => run_action_offer_folder(
                &mut out,
                "Removing profile...",
                || action_remove_profile(&opts),
                &profile_folder,
            )?,
            (Page::Main, '6') => run_action_offer_folder(
                &mut out,
                "Full uninstall...",
                || action_full_uninstall(&opts),
                &both_folders,
            )?,
            (Page::Main, 'm') => page = Page::Maintenance,
            (Page::Main, 'd') => page = Page::Maintenance2,
            (Page::Main, 'a') => page = Page::Advanced,
            (Page::Main, 'i') => page = Page::IccStudio,
            (Page::Main, 'q') => break,

            // ── Maintenance menu ────────────────────────────
            (Page::Maintenance, '1') => {
                run_action(&mut out, "Refreshing profile...", || action_refresh(&opts))?
            }
            (Page::Maintenance, '2') => run_action_offer_folder(
                &mut out,
                "Reinstalling everything...",
                || action_reinstall(&opts),
                &both_folders,
            )?,
            (Page::Maintenance, '3') => {
                run_action(&mut out, "Detecting monitors...", action_detect)?
            }
            (Page::Maintenance, '4') => page = Page::ServiceDiagnostics,
            (Page::Maintenance, '5') => run_action(&mut out, "Rechecking service...", || {
                action_recheck_service(&opts)
            })?,
            (Page::Maintenance, '6') => run_action(
                &mut out,
                "Checking applicability...",
                action_check_applicability,
            )?,
            (Page::Maintenance, '7') => {
                run_action(&mut out, "Sending test toast notification...", || {
                    action_test_toast(&opts)
                })?
            }
            (Page::Maintenance, '8') => {
                run_action(&mut out, "Force refreshing color profile...", || {
                    action_force_refresh_profile(&opts)
                })?
            }
            (Page::Maintenance, '9') => run_action(
                &mut out,
                "Force refreshing color management...",
                action_force_refresh_color_mgmt,
            )?,
            (Page::Maintenance, '0') => run_action(&mut out, "Setting DDC brightness...", || {
                action_set_ddc_brightness(&opts)
            })?,
            (Page::Maintenance, 'a') => {
                run_action(&mut out, "Running safe recovery rollback...", || {
                    action_safe_recovery(&opts)
                })?
            }
            (Page::Maintenance, 'n') => page = Page::Maintenance2,
            (Page::Maintenance, 'b') => page = Page::Main,
            (Page::Maintenance, 'q') => break,

            // ── Service Diagnostics ──────────────────────────
            (Page::ServiceDiagnostics, '1') => {}
            (Page::ServiceDiagnostics, '2') => run_action(
                &mut out,
                "Opening diagnostics folder...",
                action_open_state_folder,
            )?,
            (Page::ServiceDiagnostics, '3') => run_action(
                &mut out,
                "Clearing diagnostics log...",
                action_clear_diagnostics,
            )?,
            (Page::ServiceDiagnostics, '4') => run_action(
                &mut out,
                "Checking service status...",
                action_service_status,
            )?,
            (Page::ServiceDiagnostics, '5') => run_action(
                &mut out,
                "Running conflict detector...",
                action_detect_conflicts,
            )?,
            (Page::ServiceDiagnostics, 'b') => page = Page::Maintenance,
            (Page::ServiceDiagnostics, 'z') => page = Page::Main,
            (Page::ServiceDiagnostics, 'q') => break,

            // ── Maintenance Page 2 (DDC/CI Studio) ──────────
            (Page::Maintenance2, '1') => run_action(&mut out, "Reading VCP version...", || {
                action_ddc_vcp_version(&ddc_target)
            })?,
            (Page::Maintenance2, '2') => run_action(&mut out, "Reading color preset...", || {
                action_ddc_read_color_preset(&ddc_target)
            })?,
            (Page::Maintenance2, '3') => {
                let cur = ddc_get_vcp(&ddc_target, lg_monitor::ddc::VCP_COLOR_PRESET)
                    .map(|v| v.current)
                    .unwrap_or(0);

                const PRESETS: &[(char, &str, u32)] = &[
                    ('1', "sRGB", 1),
                    ('2', "Native", 2),
                    ('3', "4000 K", 4),
                    ('4', "5000 K", 5),
                    ('5', "6500 K", 6),
                    ('6', "7500 K", 8),
                    ('7', "8200 K", 9),
                    ('8', "9300 K", 10),
                    ('9', "User 1", 11),
                    ('a', "User 2", 12),
                    ('b', "User 3", 13),
                ];
                let items: Vec<(char, &str, bool)> = PRESETS
                    .iter()
                    .map(|&(k, label, val)| (k, label, val == cur))
                    .collect();

                if let Some(idx) = run_submenu(&mut out, " COLOR PRESET ", &items)? {
                    let (_, name, value) = PRESETS[idx];
                    if !confirm_ddc_write_if_risky(
                        &mut out,
                        &ddc_guardrails,
                        lg_monitor::ddc::VCP_COLOR_PRESET,
                        value,
                        &format!("Set color preset to {}", name),
                    )? {
                        continue;
                    }
                    run_action(
                        &mut out,
                        &format!("Setting color preset to {}...", name),
                        || {
                            ddc_set_vcp(&ddc_target, lg_monitor::ddc::VCP_COLOR_PRESET, value)?;
                            log_ok(&format!("Color preset set to {} (value {})", name, value));
                            log_done("Color preset updated.");
                            Ok(())
                        },
                    )?;
                }
            }
            (Page::Maintenance2, '4') => run_action(&mut out, "Reading display mode...", || {
                action_ddc_read_display_mode(&ddc_target)
            })?,
            (Page::Maintenance2, '5') => {
                match ddc_get_vcp(&ddc_target, lg_monitor::ddc::VCP_DISPLAY_MODE) {
                    Ok(val) => {
                        let max = (val.max as usize).max(1);
                        let current = val.current;
                        let keys = b"123456789abcdefghijklmnop";
                        let count = max.min(keys.len());

                        let labels: Vec<String> =
                            (1..=count).map(|i| format!("Mode {}", i)).collect();
                        let items: Vec<(char, &str, bool)> = (0..count)
                            .map(|i| {
                                (
                                    keys[i] as char,
                                    labels[i].as_str(),
                                    (i + 1) as u32 == current,
                                )
                            })
                            .collect();

                        if let Some(idx) = run_submenu(&mut out, " DISPLAY MODE ", &items)? {
                            let mode = (idx + 1) as u32;
                            if !confirm_ddc_write_if_risky(
                                &mut out,
                                &ddc_guardrails,
                                lg_monitor::ddc::VCP_DISPLAY_MODE,
                                mode,
                                &format!("Set display mode to {}", mode),
                            )? {
                                continue;
                            }
                            run_action(
                                &mut out,
                                &format!("Setting display mode to {}...", mode),
                                || {
                                    ddc_set_vcp(
                                        &ddc_target,
                                        lg_monitor::ddc::VCP_DISPLAY_MODE,
                                        mode,
                                    )?;
                                    log_ok(&format!("Display mode set to {}", mode));
                                    log_done("Display mode updated.");
                                    Ok(())
                                },
                            )?;
                        }
                    }
                    Err(e) => {
                        run_action(&mut out, "Reading display mode...", || {
                            Err(format!("Could not read display modes: {}", e).into())
                        })?;
                    }
                }
            }
            (Page::Maintenance2, '6') => {
                if !confirm_ddc_write_if_risky(
                    &mut out,
                    &ddc_guardrails,
                    lg_monitor::ddc::VCP_RESET_BRIGHTNESS_CONTRAST,
                    1,
                    "Factory reset brightness + contrast",
                )? {
                    continue;
                }
                run_action(&mut out, "Resetting brightness + contrast...", || {
                    action_ddc_reset_brightness_contrast(&ddc_target)
                })?
            }
            (Page::Maintenance2, '7') => {
                if !confirm_ddc_write_if_risky(
                    &mut out,
                    &ddc_guardrails,
                    lg_monitor::ddc::VCP_RESET_COLOR,
                    1,
                    "Factory reset color channels",
                )? {
                    continue;
                }
                run_action(&mut out, "Resetting color...", || {
                    action_ddc_reset_color(&ddc_target)
                })?
            }
            (Page::Maintenance2, '8') => run_action(
                &mut out,
                "Listing physical monitors...",
                action_ddc_list_monitors,
            )?,
            (Page::Maintenance2, 'a') => run_action(&mut out, "Reading brightness...", || {
                action_ddc_read_brightness(&ddc_target)
            })?,
            (Page::Maintenance2, 'b') => {
                if let Some(value) =
                    prompt_u8(&mut out, "SET BRIGHTNESS", "Enter brightness 0..100", 50)?
                {
                    if let Err(message) = validate_guarded_ddc_write(
                        &ddc_guardrails,
                        lg_monitor::ddc::VCP_BRIGHTNESS,
                        value as u32,
                    ) {
                        run_action(&mut out, "DDC guardrails blocked write...", || {
                            Err(message.into())
                        })?;
                        continue;
                    }
                    run_action(
                        &mut out,
                        &format!("Setting brightness to {}...", value),
                        || action_ddc_set_brightness(&ddc_target, value as u32),
                    )?;
                }
            }
            (Page::Maintenance2, 'c') => {
                if let Some(code) = prompt_vcp_code(
                    &mut out,
                    "READ CUSTOM VCP",
                    "Enter VCP code (hex like DC or 0xDC, or decimal)",
                    lg_monitor::ddc::VCP_BRIGHTNESS,
                )? {
                    run_action(&mut out, &format!("Reading VCP 0x{:02X}...", code), || {
                        action_ddc_read_custom_vcp(&ddc_target, code)
                    })?;
                }
            }
            (Page::Maintenance2, 'd') => {
                if let Some(code) = prompt_vcp_code(
                    &mut out,
                    "WRITE CUSTOM VCP",
                    "Enter VCP code (hex like DC or 0xDC, or decimal)",
                    lg_monitor::ddc::VCP_BRIGHTNESS,
                )? {
                    if let Some(value) =
                        prompt_u32(&mut out, "WRITE CUSTOM VCP", "Enter value (u32)", 50)?
                    {
                        if let Err(message) =
                            validate_guarded_ddc_write(&ddc_guardrails, code, value)
                        {
                            run_action(&mut out, "DDC guardrails blocked write...", || {
                                Err(message.into())
                            })?;
                            continue;
                        }
                        if !confirm_ddc_write_if_risky(
                            &mut out,
                            &ddc_guardrails,
                            code,
                            value,
                            &format!("Write custom VCP 0x{:02X}={}", code, value),
                        )? {
                            continue;
                        }
                        run_action(
                            &mut out,
                            &format!("Writing VCP 0x{:02X} = {}...", code, value),
                            || action_ddc_write_custom_vcp(&ddc_target, code, value),
                        )?;
                    }
                }
            }
            (Page::Maintenance2, '9') => match lg_monitor::ddc::list_physical_monitors() {
                Ok(monitors) if !monitors.is_empty() => {
                    let keys = b"123456789abcdefghijklmnop";
                    let count = monitors.len().min(keys.len());

                    let labels: Vec<String> = monitors[..count]
                        .iter()
                        .map(|(idx, desc)| format!("#{} {}", idx, desc))
                        .collect();
                    let items: Vec<(char, &str, bool)> = monitors[..count]
                        .iter()
                        .enumerate()
                        .map(|(i, (idx, _))| {
                            let is_cur = ddc_target
                                .as_ref()
                                .is_some_and(|(cur_idx, _)| cur_idx == idx);
                            (keys[i] as char, labels[i].as_str(), is_cur)
                        })
                        .collect();

                    if let Some(sel) = run_submenu(&mut out, " SELECT MONITOR ", &items)? {
                        let (idx, name) = &monitors[sel];
                        ddc_target = Some((*idx, name.clone()));
                    }
                }
                _ => {
                    run_action(&mut out, "Listing monitors...", || {
                        Err("No physical monitors found via DDC".into())
                    })?;
                }
            },
            (Page::Maintenance2, '0') => {
                ddc_target = None; // reset to config default
            }
            (Page::Maintenance2, 'e') => {
                configure_ddc_guardrails(&mut out, &mut ddc_guardrails)?;
            }
            (Page::Maintenance2, 'p') => page = Page::Maintenance,
            (Page::Maintenance2, 'z') => page = Page::Main,
            (Page::Maintenance2, 'q') => break,

            // ── Advanced menu ──────────────────────────────
            (Page::Advanced, '1') => opts.toast = !opts.toast,
            (Page::Advanced, '2') => opts.dry_run = !opts.dry_run,
            (Page::Advanced, '3') => opts.verbose = !opts.verbose,
            (Page::Advanced, '4') => opts.hdr = !opts.hdr,
            (Page::Advanced, '5') => opts.sdr = !opts.sdr,
            (Page::Advanced, '6') => opts.per_user = !opts.per_user,
            (Page::Advanced, '7') => opts.generic_default = !opts.generic_default,
            (Page::Advanced, '8') => opts.ddc_brightness = !opts.ddc_brightness,
            (Page::Advanced, '9') => {
                let items: Vec<(char, &str, bool)> = vec![
                    ('1', "10 %", opts.ddc_brightness_value == 10),
                    ('2', "20 %", opts.ddc_brightness_value == 20),
                    ('3', "30 %", opts.ddc_brightness_value == 30),
                    ('4', "40 %", opts.ddc_brightness_value == 40),
                    ('5', "50 %", opts.ddc_brightness_value == 50),
                    ('6', "60 %", opts.ddc_brightness_value == 60),
                    ('7', "70 %", opts.ddc_brightness_value == 70),
                    ('8', "80 %", opts.ddc_brightness_value == 80),
                    ('9', "90 %", opts.ddc_brightness_value == 90),
                    ('0', "100 %", opts.ddc_brightness_value == 100),
                ];
                if let Some(idx) = run_submenu(&mut out, " BRIGHTNESS ", &items)? {
                    opts.ddc_brightness_value = (idx as u32 + 1) * 10;
                }
            }
            (Page::Advanced, 'b') => page = Page::Main,
            (Page::Advanced, 'q') => break,

            // ── ICC Studio ──────────────────────────────────
            (Page::IccStudio, '1') => {
                let items = vec![
                    ('1', "gamma22", icc_cfg.icc_active_preset == "gamma22"),
                    ('2', "gamma24", icc_cfg.icc_active_preset == "gamma24"),
                    ('3', "reader", icc_cfg.icc_active_preset == "reader"),
                    ('4', "custom", icc_cfg.icc_active_preset == "custom"),
                ];
                if let Some(idx) = run_submenu(&mut out, " ICC ACTIVE PRESET ", &items)? {
                    icc_cfg.icc_active_preset = match idx {
                        0 => "gamma22".to_string(),
                        1 => "gamma24".to_string(),
                        2 => "reader".to_string(),
                        _ => "custom".to_string(),
                    };
                    sync_mode_presets_to_active(&mut icc_cfg);
                    if icc_cfg.icc_active_preset == "reader" {
                        // Reader preset should immediately counter warm/yellow cast.
                        icc_cfg.icc_tuning_preset = "reader_balanced".to_string();
                    }
                    icc_dirty = true;
                }
            }
            (Page::IccStudio, '2') => {
                let names = lg_profile::dynamic_icc_tuning_preset_names();
                let mut items = Vec::with_capacity(names.len());
                for (i, name) in names.iter().enumerate() {
                    let key = if i < 9 {
                        (b'1' + i as u8) as char
                    } else {
                        (b'A' + (i - 9) as u8) as char
                    };
                    items.push((key, *name, icc_cfg.icc_tuning_preset == *name));
                }
                if let Some(idx) = run_submenu(&mut out, " ICC TUNING PRESET ", &items)? {
                    icc_cfg.icc_tuning_preset = names[idx].to_string();
                    icc_cfg.icc_tuning_overlay_manual = false;
                    icc_dirty = true;
                }
            }
            (Page::IccStudio, '3') => {
                icc_cfg.icc_tuning_overlay_manual = !icc_cfg.icc_tuning_overlay_manual;
                icc_dirty = true;
            }
            (Page::IccStudio, '4') => {
                if let Some(value) = prompt_f64(
                    &mut out,
                    "ICC GAMMA",
                    "Master tone curve. Lower brightens mids/shadows, higher darkens. Range: 1.2..3.0",
                    icc_cfg.icc_gamma,
                )? {
                    icc_cfg.icc_gamma = lg_profile::sanitize_dynamic_gamma(value);
                    icc_dirty = true;
                }
            }
            (Page::IccStudio, '5') => {
                if let Some(value) = prompt_f64(
                    &mut out,
                    "ICC LUMINANCE",
                    "Target white level used in ICC tags. Higher feels brighter. Range: 80..600 cd/m^2",
                    icc_cfg.icc_luminance_cd_m2,
                )? {
                    icc_cfg.icc_luminance_cd_m2 =
                        lg_profile::sanitize_dynamic_luminance_cd_m2(value);
                    icc_dirty = true;
                }
            }
            (Page::IccStudio, '6') => {
                icc_cfg.icc_generate_specialized_profiles =
                    !icc_cfg.icc_generate_specialized_profiles;
                icc_dirty = true;
            }
            (Page::IccStudio, '7') => {
                icc_cfg.icc_per_monitor_profiles = !icc_cfg.icc_per_monitor_profiles;
                icc_dirty = true;
            }
            (Page::IccStudio, '8') => page = Page::IccStudioTuning,
            (Page::IccStudio, '9') => page = Page::IccTags,
            (Page::IccStudio, 'a') => {
                run_action(&mut out, "Generating and applying optimized ICC...", || {
                    action_icc_generate_and_apply(&icc_cfg, &opts)
                })?
            }
            (Page::IccStudio, 'b') => {
                run_reader_calibration_wizard(&mut out, &mut icc_cfg, &mut icc_dirty, &opts)?
            }
            (Page::IccStudio, 'c') => {
                run_snapshot_manager(&mut out, &mut icc_cfg, &mut icc_dirty, &opts)?
            }
            (Page::IccStudio, 'd') => run_action(&mut out, "Toggling A/B compare...", || {
                action_toggle_ab_compare(&opts, &icc_cfg)
            })?,
            (Page::IccStudio, 'e') => run_action(&mut out, "Opening Color Management...", || {
                open_color_management_panel()?;
                log_ok("Opened Color Management control panel.");
                Ok(())
            })?,
            (Page::IccStudio, 'r') => {
                icc_cfg = Config::load();
                icc_dirty = false;
            }
            (Page::IccStudio, 's') => run_action(&mut out, "Saving ICC config...", || {
                Config::write_config(&icc_cfg)?;
                icc_dirty = false;
                log_ok("ICC settings saved to config.toml");
                Ok(())
            })?,
            (Page::IccStudio, 'z') => page = Page::Main,
            (Page::IccStudio, 'q') => break,

            // ── ICC Studio Tuning ───────────────────────────
            (Page::IccStudioTuning, '1') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "BLACK LIFT",
                    "Raises near-black detail to reduce crush and dim scenes. Range: 0.0..0.25",
                    current.black_lift,
                )? {
                    icc_cfg.icc_black_lift = lg_profile::sanitize_black_lift(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, '2') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "MIDTONE BOOST",
                    "Adjusts perceived scene brightness in the middle tones. + boosts, - darkens. Range: -0.5..0.5",
                    current.midtone_boost,
                )? {
                    icc_cfg.icc_midtone_boost = lg_profile::sanitize_midtone_boost(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, '3') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "WHITE COMPRESSION",
                    "Softens top-end highlights after lifting shadows/mids. Helps prevent clipping. Range: 0.0..1.0",
                    current.white_compression,
                )? {
                    icc_cfg.icc_white_compression = lg_profile::sanitize_white_compression(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, '4') => {
                icc_cfg.icc_vcgt_enabled = !icc_cfg.icc_vcgt_enabled;
                enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                icc_dirty = true;
            }
            (Page::IccStudioTuning, '5') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "VCGT STRENGTH",
                    "How strongly LUT calibration is applied when VCGT is enabled. Range: 0.0..1.0",
                    current.vcgt_strength,
                )? {
                    icc_cfg.icc_vcgt_strength = lg_profile::sanitize_vcgt_strength(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, '6') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "TARGET BLACK",
                    "Absolute black floor target used to shape curve floors. Range: 0.0..5.0 cd/m^2",
                    current.target_black_cd_m2,
                )? {
                    icc_cfg.icc_target_black_cd_m2 = lg_profile::sanitize_target_black_cd_m2(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, '7') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "CHANNEL GAMMA R",
                    "Red channel balance. Lower reduces warm/yellow tint; higher adds warmth. Range: 0.5..1.5",
                    current.gamma_r,
                )? {
                    icc_cfg.icc_gamma_r = lg_profile::sanitize_channel_gamma_multiplier(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, '8') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "CHANNEL GAMMA G",
                    "Green channel balance. Tune neutrality after red/blue changes. Range: 0.5..1.5",
                    current.gamma_g,
                )? {
                    icc_cfg.icc_gamma_g = lg_profile::sanitize_channel_gamma_multiplier(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, '9') => {
                let current = tuning_from_config(&icc_cfg);
                if let Some(value) = prompt_f64(
                    &mut out,
                    "CHANNEL GAMMA B",
                    "Blue channel balance. Raise to cool/unyellow whites, lower for warmer whites. Range: 0.5..1.5",
                    current.gamma_b,
                )? {
                    icc_cfg.icc_gamma_b = lg_profile::sanitize_channel_gamma_multiplier(value);
                    enable_manual_overlay_for_tuning_edits(&mut icc_cfg);
                    icc_dirty = true;
                }
            }
            (Page::IccStudioTuning, 'r') => {
                icc_cfg = Config::load();
                icc_dirty = false;
            }
            (Page::IccStudioTuning, 's') => run_action(&mut out, "Saving ICC config...", || {
                Config::write_config(&icc_cfg)?;
                icc_dirty = false;
                log_ok("ICC settings saved to config.toml");
                Ok(())
            })?,
            (Page::IccStudioTuning, 'a') => page = Page::IccStudio,
            (Page::IccStudioTuning, 'z') => page = Page::Main,
            (Page::IccStudioTuning, 'q') => break,

            // ── ICC Tags ────────────────────────────────────
            (Page::IccTags, '1') => {
                icc_cfg.icc_include_media_black_point = !icc_cfg.icc_include_media_black_point;
                icc_dirty = true;
            }
            (Page::IccTags, '2') => {
                icc_cfg.icc_include_device_descriptions = !icc_cfg.icc_include_device_descriptions;
                icc_dirty = true;
            }
            (Page::IccTags, '3') => {
                icc_cfg.icc_include_characterization_target =
                    !icc_cfg.icc_include_characterization_target;
                icc_dirty = true;
            }
            (Page::IccTags, '4') => {
                icc_cfg.icc_include_viewing_cond_desc = !icc_cfg.icc_include_viewing_cond_desc;
                icc_dirty = true;
            }
            (Page::IccTags, '5') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "TECH SIGNATURE",
                    "4-char ICC signature (empty disables)",
                    &icc_cfg.icc_technology_signature,
                )? {
                    icc_cfg.icc_technology_signature = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags, '6') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "CIIS SIGNATURE",
                    "4-char ICC signature (empty disables)",
                    &icc_cfg.icc_ciis_signature,
                )? {
                    icc_cfg.icc_ciis_signature = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags, '7') => {
                icc_cfg.icc_cicp_enabled = !icc_cfg.icc_cicp_enabled;
                icc_dirty = true;
            }
            (Page::IccTags, '8') => {
                if let Some(value) = prompt_u8(
                    &mut out,
                    "CICP PRIMARIES",
                    "Integer 0..255",
                    icc_cfg.icc_cicp_primaries,
                )? {
                    icc_cfg.icc_cicp_primaries = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags, '9') => {
                if let Some(value) = prompt_u8(
                    &mut out,
                    "CICP TRANSFER",
                    "Integer 0..255",
                    icc_cfg.icc_cicp_transfer,
                )? {
                    icc_cfg.icc_cicp_transfer = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags, '0') => {
                if let Some(value) = prompt_u8(
                    &mut out,
                    "CICP MATRIX",
                    "Integer 0..255",
                    icc_cfg.icc_cicp_matrix,
                )? {
                    icc_cfg.icc_cicp_matrix = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags, 'a') => {
                icc_cfg.icc_cicp_full_range = !icc_cfg.icc_cicp_full_range;
                icc_dirty = true;
            }
            (Page::IccTags, 'n') => page = Page::IccTags2,
            (Page::IccTags, 's') => run_action(&mut out, "Saving ICC config...", || {
                Config::write_config(&icc_cfg)?;
                icc_dirty = false;
                log_ok("ICC settings saved to config.toml");
                Ok(())
            })?,
            (Page::IccTags, 'p') => page = Page::IccStudio,
            (Page::IccTags, 'z') => page = Page::Main,
            (Page::IccTags, 'q') => break,

            // ── ICC Tags (Page 2) ───────────────────────────
            (Page::IccTags2, '1') => {
                icc_cfg.icc_metadata_enabled = !icc_cfg.icc_metadata_enabled;
                icc_dirty = true;
            }
            (Page::IccTags2, '2') => {
                icc_cfg.icc_include_calibration_datetime =
                    !icc_cfg.icc_include_calibration_datetime;
                icc_dirty = true;
            }
            (Page::IccTags2, '3') => {
                icc_cfg.icc_include_chromatic_adaptation =
                    !icc_cfg.icc_include_chromatic_adaptation;
                icc_dirty = true;
            }
            (Page::IccTags2, '4') => {
                icc_cfg.icc_include_chromaticity = !icc_cfg.icc_include_chromaticity;
                icc_dirty = true;
            }
            (Page::IccTags2, '5') => {
                icc_cfg.icc_include_measurement = !icc_cfg.icc_include_measurement;
                icc_dirty = true;
            }
            (Page::IccTags2, '6') => {
                icc_cfg.icc_include_viewing_conditions = !icc_cfg.icc_include_viewing_conditions;
                icc_dirty = true;
            }
            (Page::IccTags2, '7') => {
                icc_cfg.icc_include_spectral_scaffold = !icc_cfg.icc_include_spectral_scaffold;
                icc_dirty = true;
            }
            (Page::IccTags2, '8') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "HDR PRESET",
                    "gamma22|gamma24|reader|custom",
                    &icc_cfg.icc_hdr_preset,
                )? {
                    icc_cfg.icc_hdr_preset = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags2, '9') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "SDR PRESET",
                    "gamma22|gamma24|reader|custom",
                    &icc_cfg.icc_sdr_preset,
                )? {
                    icc_cfg.icc_sdr_preset = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags2, '0') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "DAY PRESET",
                    "gamma22|gamma24|reader|custom|empty",
                    &icc_cfg.icc_schedule_day_preset,
                )? {
                    icc_cfg.icc_schedule_day_preset = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags2, 'a') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "NIGHT PRESET",
                    "gamma22|gamma24|reader|custom|empty",
                    &icc_cfg.icc_schedule_night_preset,
                )? {
                    icc_cfg.icc_schedule_night_preset = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags2, 'b') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "PROFILE NAME",
                    "ICC filename in color store",
                    &icc_cfg.profile_name,
                )? {
                    icc_cfg.profile_name = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags2, 'c') => {
                if let Some(value) = prompt_text(
                    &mut out,
                    "MONITOR MATCH",
                    "Monitor name pattern",
                    &icc_cfg.monitor_match,
                )? {
                    icc_cfg.monitor_match = value;
                    icc_dirty = true;
                }
            }
            (Page::IccTags2, 'd') => {
                icc_cfg.monitor_match_regex = !icc_cfg.monitor_match_regex;
                icc_dirty = true;
            }
            (Page::IccTags2, 's') => run_action(&mut out, "Saving ICC config...", || {
                Config::write_config(&icc_cfg)?;
                icc_dirty = false;
                log_ok("ICC settings saved to config.toml");
                Ok(())
            })?,
            (Page::IccTags2, 'p') => page = Page::IccTags,
            (Page::IccTags2, 'z') => page = Page::Main,
            (Page::IccTags2, 'q') => break,

            _ => {} // ignore unknown keys
        }
    }

    draw_goodbye(out)?;
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

fn ensure_active_profile_for_mode(
    cfg: &Config,
    hdr_mode: bool,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let active_preset = effective_preset_for_mode(cfg, hdr_mode);
    lg_profile::ensure_active_profile_installed_tuned(
        &lg_profile::color_directory(),
        &active_preset,
        &cfg.profile_name,
        cfg.icc_gamma,
        cfg.icc_luminance_cd_m2,
        cfg.icc_generate_specialized_profiles,
        tuning_from_config(cfg),
    )
}

fn mode_presets_for_cfg(cfg: &Config) -> (String, String) {
    (
        effective_preset_for_mode(cfg, false),
        effective_preset_for_mode(cfg, true),
    )
}

fn ensure_shared_mode_profiles(
    cfg: &Config,
) -> Result<(std::path::PathBuf, std::path::PathBuf), Box<dyn std::error::Error>> {
    let (sdr_preset, hdr_preset) = mode_presets_for_cfg(cfg);
    lg_profile::ensure_mode_profiles_installed_tuned(
        &lg_profile::color_directory(),
        &sdr_preset,
        &hdr_preset,
        &cfg.profile_name,
        cfg.icc_gamma,
        cfg.icc_luminance_cd_m2,
        cfg.icc_generate_specialized_profiles,
        tuning_from_config(cfg),
    )
}

fn ensure_mode_profiles_for_monitor(
    cfg: &Config,
    device: &lg_monitor::MatchedMonitor,
) -> Result<(std::path::PathBuf, std::path::PathBuf), Box<dyn std::error::Error>> {
    let (sdr_preset, hdr_preset) = mode_presets_for_cfg(cfg);
    let identity = lg_profile::DynamicMonitorIdentity {
        monitor_name: device.name.clone(),
        device_key: device.device_key.clone(),
        serial_number: device.serial.clone(),
        manufacturer_id: device.manufacturer_id.clone(),
        product_code: device.product_code.clone(),
    };
    lg_profile::ensure_mode_profiles_installed_tuned_for_monitor(
        &lg_profile::color_directory(),
        &sdr_preset,
        &hdr_preset,
        &cfg.profile_name,
        cfg.icc_gamma,
        cfg.icc_luminance_cd_m2,
        cfg.icc_generate_specialized_profiles,
        tuning_from_config(cfg),
        &identity,
    )
}

pub(crate) fn gather_status(opts: &Options) -> Status {
    let cfg = Config::load();
    let profile_installed =
        lg_profile::is_profile_installed(&resolve_active_profile_path_for_mode(&cfg, opts.hdr));
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
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

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
    draw_item(out, "D", "DDC/CI Studio (Direct monitor controls)")?;
    draw_item(
        out,
        "I",
        "ICC Studio (Presets, tuning, and on-the-fly optimize)",
    )?;
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
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_header(out, status)?;
    draw_sep(out, " MAINTENANCE ")?;

    draw_empty(out)?;
    draw_section(out, "PROFILE")?;
    draw_item(out, "1", "Refresh (Re-apply profile now)")?;
    draw_item(out, "2", "Reinstall (Clean reinstall everything)")?;
    draw_empty(out)?;

    draw_section(out, "DIAGNOSTICS")?;
    draw_item(out, "3", "Detect Monitors")?;
    draw_item(out, "4", "Service Diagnostics (Recent apply history)")?;
    draw_item(out, "5", "Recheck Service (Stop + Start)")?;
    draw_item(out, "6", "Check Applicability + Conflict Detector")?;
    draw_item(out, "7", "Test Toast Notification")?;
    draw_empty(out)?;

    draw_section(out, "FORCE REFRESH")?;
    draw_item(out, "8", "Force Refresh Color Profile")?;
    draw_item(out, "9", "Force Refresh Color Management")?;
    draw_empty(out)?;

    draw_section(out, "DDC/CI")?;
    draw_item(out, "0", "Set DDC Brightness (Test)")?;
    draw_item(out, "A", "Safe Recovery Rollback (Last known good)")?;
    draw_empty(out)?;

    draw_section(out, "NAVIGATION")?;
    draw_item(out, "N", "Open DDC/CI Studio")?;
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
// Drawing — Maintenance Page 2 (DDC/CI Studio)
// ============================================================================

pub(crate) fn draw_maintenance2(
    out: &mut impl Write,
    status: &Status,
    _opts: &Options,
    ddc_target: Option<&(usize, String)>,
) -> io::Result<()> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_header(out, status)?;
    draw_sep(out, " DDC/CI STUDIO ")?;

    draw_empty(out)?;
    let target_label = match ddc_target {
        Some((idx, name)) => format!("  Target: #{} ({})", idx, name),
        None => format!(
            "  Target: config default ({})",
            Config::load().monitor_match
        ),
    };
    draw_line(out, &target_label, Color::Green)?;
    draw_empty(out)?;

    draw_section(out, "READ")?;
    draw_item(out, "1", "View VCP Version")?;
    draw_item(out, "2", "Read Color Preset (VCP 0x14)")?;
    draw_item(out, "4", "Read Display Mode (VCP 0xDC)")?;
    draw_item(out, "A", "Read Brightness (VCP 0x10)")?;
    draw_item(out, "C", "Read Custom VCP (Any code)")?;
    draw_empty(out)?;

    draw_section(out, "WRITE")?;
    draw_item(out, "3", "Cycle Color Preset (sRGB→6500K→9300K→User1)")?;
    draw_item(out, "5", "Set Display Mode (VCP 0xDC)")?;
    draw_item(out, "B", "Set Brightness (VCP 0x10)")?;
    draw_item(out, "D", "Write Custom VCP (Any code/value)")?;
    draw_item(out, "E", "DDC Guardrails (Limits + confirmations)")?;
    draw_empty(out)?;

    draw_section(out, "RESET")?;
    draw_item(out, "6", "Reset Brightness + Contrast (VCP 0x06)")?;
    draw_item(out, "7", "Reset Color (VCP 0x0A)")?;
    draw_empty(out)?;

    draw_section(out, "INFO")?;
    draw_item(out, "8", "List Physical Monitors (DDC)")?;
    draw_empty(out)?;

    draw_section(out, "TARGET")?;
    draw_item(out, "9", "Select Target Monitor")?;
    draw_item(out, "0", "Reset Target (use config default)")?;
    draw_empty(out)?;

    draw_section(out, "NAVIGATION")?;
    draw_item(out, "P", "← Previous Page (Maintenance)")?;
    draw_item(out, "Z", "Back to Main Menu")?;
    draw_item_quit(out)?;
    draw_empty(out)?;
    draw_bottom(out)?;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "  Select option: ")?;
    queue!(out, ResetColor)?;
    Ok(())
}

pub(crate) fn draw_service_diagnostics(out: &mut impl Write, status: &Status) -> io::Result<()> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_header(out, status)?;
    draw_sep(out, " SERVICE DIAGNOSTICS ")?;
    draw_empty(out)?;

    draw_section(out, "SUMMARY")?;
    let (installed, running) = lg_service::query_service_info();
    draw_line(
        out,
        &format!(
            "  Service: {} / {}",
            if installed { "installed" } else { "missing" },
            if running { "running" } else { "stopped" }
        ),
        if installed && running {
            Color::Green
        } else {
            Color::Yellow
        },
    )?;
    let latest_success =
        app_state::latest_success_timestamp().unwrap_or_else(|| "none recorded".to_string());
    draw_line(
        out,
        &format!("  Last successful apply: {}", latest_success),
        Color::White,
    )?;
    let metrics_cfg = app_state::load_automation_config().metrics;
    if metrics_cfg.enabled && metrics_cfg.collect_latency {
        let metrics = app_state::compute_apply_latency_metrics(metrics_cfg.rolling_window);
        draw_line(
            out,
            &format!(
                "  Apply latency (window={}): samples={} avg={:.1}ms p95={}ms last={}ms success={} fail={}",
                metrics_cfg.rolling_window,
                metrics.samples,
                metrics.avg_ms,
                metrics.p95_ms,
                metrics.last_ms,
                metrics.success_count,
                metrics.failure_count
            ),
            if metrics.failure_count > 0 {
                Color::Yellow
            } else {
                Color::Green
            },
        )?;
    }
    draw_empty(out)?;

    draw_section(out, "RECENT EVENTS")?;
    match app_state::read_recent_diagnostic_events(8) {
        Ok(events) if events.is_empty() => {
            draw_line(
                out,
                "  No diagnostics yet. Trigger Apply/Refresh or wait for service events.",
                Color::DarkGrey,
            )?;
        }
        Ok(events) => {
            for event in events {
                let mut line = format!(
                    "  {} [{}:{}] {}",
                    event.timestamp, event.source, event.level, event.event
                );
                if !event.details.trim().is_empty() {
                    line.push_str(": ");
                    line.push_str(&event.details);
                }
                let color = match event.level.as_str() {
                    "ERROR" => Color::Red,
                    "WARN" => Color::Yellow,
                    _ => Color::White,
                };
                for wrapped in wrap_text(&line, INNER.saturating_sub(2)) {
                    draw_line(out, &wrapped, color)?;
                }
            }
        }
        Err(e) => {
            draw_line(
                out,
                &format!("  Could not read diagnostics log: {}", e),
                Color::Red,
            )?;
        }
    }
    draw_empty(out)?;

    draw_section(out, "ACTIONS")?;
    draw_item(out, "1", "Refresh this page")?;
    draw_item(out, "2", "Open state folder")?;
    draw_item(out, "3", "Clear diagnostics log")?;
    draw_item(out, "4", "Check service status")?;
    draw_item(out, "5", "Run conflict detector")?;
    draw_empty(out)?;

    draw_section(out, "NAVIGATION")?;
    draw_item(out, "B", "Back to Maintenance")?;
    draw_item(out, "Z", "Back to Main Menu")?;
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
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

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

    draw_section(out, "DDC/CI BRIGHTNESS")?;
    draw_toggle(
        out,
        "8",
        "Auto-Set Brightness on Reapply",
        opts.ddc_brightness,
    )?;
    {
        let label = format!(
            "Brightness Value: {} (press to select)",
            opts.ddc_brightness_value
        );
        draw_item(out, "9", &label)?;
    }
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
// Drawing — ICC Studio
// ============================================================================

pub(crate) fn draw_icc_studio(
    out: &mut impl Write,
    status: &Status,
    cfg: &Config,
    dirty: bool,
) -> io::Result<()> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_header(out, status)?;
    draw_sep(out, " ICC STUDIO ")?;

    draw_empty(out)?;
    draw_section(out, "PRESET + CORE")?;
    draw_item(
        out,
        "1",
        &format!("Active ICC Preset: {}", cfg.icc_active_preset),
    )?;
    draw_item(
        out,
        "2",
        &format!("Tuning Preset: {}", cfg.icc_tuning_preset),
    )?;
    draw_toggle(
        out,
        "3",
        "Overlay Manual Overrides",
        cfg.icc_tuning_overlay_manual,
    )?;
    draw_item(out, "4", &format!("Gamma: {:.3}", cfg.icc_gamma))?;
    draw_item(
        out,
        "5",
        &format!("Luminance cd/m^2: {:.1}", cfg.icc_luminance_cd_m2),
    )?;
    draw_empty(out)?;

    draw_section(out, "PROFILE STRATEGY")?;
    draw_toggle(
        out,
        "6",
        "Generate Specialized gamma22/gamma24",
        cfg.icc_generate_specialized_profiles,
    )?;
    draw_toggle(
        out,
        "7",
        "Per-Monitor Profiles",
        cfg.icc_per_monitor_profiles,
    )?;
    draw_item(out, "8", "Next Page -> Manual Curve Tuning")?;
    draw_item(out, "9", "Next Page -> ICC Tags (Page 1)")?;
    draw_item(out, "A", "Generate + Apply Optimized ICC Now")?;
    draw_item(out, "B", "Guided Reader Calibration Wizard")?;
    draw_item(out, "C", "Profile Snapshots (Save/Restore/Diff)")?;
    draw_item(out, "D", "A/B Compare Toggle (Current vs Baseline)")?;
    draw_item(out, "E", "Open Color Management (ColorCPL)")?;
    draw_item(out, "R", "Reload ICC settings from disk")?;
    draw_item(out, "S", "Save ICC settings to config.toml")?;
    draw_empty(out)?;

    let dirty_text = if dirty { "unsaved changes" } else { "saved" };
    let dirty_color = if dirty { Color::Yellow } else { Color::Green };
    draw_line(
        out,
        &format!("  ICC Studio state: {}", dirty_text),
        dirty_color,
    )?;
    draw_empty(out)?;
    draw_section(out, "NAVIGATION")?;
    draw_item(out, "Z", "Back to Main Menu")?;
    draw_item_quit(out)?;
    draw_empty(out)?;
    draw_bottom(out)?;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "  Select option: ")?;
    queue!(out, ResetColor)?;
    Ok(())
}

pub(crate) fn draw_icc_studio_tuning(
    out: &mut impl Write,
    status: &Status,
    cfg: &Config,
    dirty: bool,
) -> io::Result<()> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_header(out, status)?;
    draw_sep(out, " ICC STUDIO - MANUAL TUNING ")?;
    let effective = tuning_from_config(cfg);

    draw_empty(out)?;
    draw_section(out, "CURVE SHAPING")?;
    draw_item(
        out,
        "1",
        &format!(
            "Black Lift manual/effective: {:.3} / {:.3}",
            cfg.icc_black_lift, effective.black_lift
        ),
    )?;
    draw_item(
        out,
        "2",
        &format!(
            "Midtone Boost manual/effective: {:.3} / {:.3}",
            cfg.icc_midtone_boost, effective.midtone_boost
        ),
    )?;
    draw_item(
        out,
        "3",
        &format!(
            "White Compression manual/effective: {:.3} / {:.3}",
            cfg.icc_white_compression, effective.white_compression
        ),
    )?;
    draw_toggle(out, "4", "VCGT Enabled", cfg.icc_vcgt_enabled)?;
    draw_item(
        out,
        "5",
        &format!(
            "VCGT Strength manual/effective: {:.3} / {:.3}",
            cfg.icc_vcgt_strength, effective.vcgt_strength
        ),
    )?;
    draw_item(
        out,
        "6",
        &format!(
            "Target Black cd/m^2 manual/effective: {:.3} / {:.3}",
            cfg.icc_target_black_cd_m2, effective.target_black_cd_m2
        ),
    )?;
    draw_item(
        out,
        "7",
        &format!(
            "Gamma Mult R manual/effective: {:.3} / {:.3}",
            cfg.icc_gamma_r, effective.gamma_r
        ),
    )?;
    draw_item(
        out,
        "8",
        &format!(
            "Gamma Mult G manual/effective: {:.3} / {:.3}",
            cfg.icc_gamma_g, effective.gamma_g
        ),
    )?;
    draw_item(
        out,
        "9",
        &format!(
            "Gamma Mult B manual/effective: {:.3} / {:.3}",
            cfg.icc_gamma_b, effective.gamma_b
        ),
    )?;
    draw_empty(out)?;
    if !cfg.icc_tuning_overlay_manual {
        for line in wrap_text(
            "Manual overlay is OFF: effective values are coming from the selected tuning preset.",
            INNER.saturating_sub(2),
        ) {
            draw_line(out, &format!("  {}", line), Color::DarkGrey)?;
        }
        for line in wrap_text(
            "Editing any field here will automatically enable manual overlay.",
            INNER.saturating_sub(2),
        ) {
            draw_line(out, &format!("  {}", line), Color::DarkGrey)?;
        }
        draw_empty(out)?;
    }

    let dirty_text = if dirty { "unsaved changes" } else { "saved" };
    let dirty_color = if dirty { Color::Yellow } else { Color::Green };
    draw_line(
        out,
        &format!("  ICC Studio tuning state: {}", dirty_text),
        dirty_color,
    )?;
    draw_empty(out)?;
    draw_section(out, "NAVIGATION")?;
    draw_item(out, "A", "<- Back to ICC Studio (Main)")?;
    draw_item(out, "R", "Reload ICC settings from disk")?;
    draw_item(out, "S", "Save ICC settings to config.toml")?;
    draw_item(out, "Z", "Back to Main Menu")?;
    draw_item_quit(out)?;
    draw_empty(out)?;
    draw_bottom(out)?;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "  Select option: ")?;
    queue!(out, ResetColor)?;
    Ok(())
}

pub(crate) fn draw_icc_tags(
    out: &mut impl Write,
    status: &Status,
    cfg: &Config,
    dirty: bool,
) -> io::Result<()> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_header(out, status)?;
    draw_sep(out, " ICC TAGS - PAGE 1 ")?;
    draw_empty(out)?;

    draw_section(out, "CORE TAGS + SIGNATURES")?;
    draw_toggle(
        out,
        "1",
        "Media Black Point",
        cfg.icc_include_media_black_point,
    )?;
    draw_toggle(
        out,
        "2",
        "Device Descriptions (dmnd/dmdd)",
        cfg.icc_include_device_descriptions,
    )?;
    draw_toggle(
        out,
        "3",
        "Characterization Target (targ)",
        cfg.icc_include_characterization_target,
    )?;
    draw_toggle(
        out,
        "4",
        "Viewing Cond Description (vued)",
        cfg.icc_include_viewing_cond_desc,
    )?;
    draw_item(
        out,
        "5",
        &format!("Technology Signature: {}", cfg.icc_technology_signature),
    )?;
    draw_item(
        out,
        "6",
        &format!("CIIS Signature: {}", cfg.icc_ciis_signature),
    )?;
    draw_toggle(out, "7", "CICP Enabled", cfg.icc_cicp_enabled)?;
    draw_item(
        out,
        "8",
        &format!("CICP Primaries: {}", cfg.icc_cicp_primaries),
    )?;
    draw_item(
        out,
        "9",
        &format!("CICP Transfer: {}", cfg.icc_cicp_transfer),
    )?;
    draw_item(out, "0", &format!("CICP Matrix: {}", cfg.icc_cicp_matrix))?;
    draw_toggle(out, "A", "CICP Full Range", cfg.icc_cicp_full_range)?;
    draw_item(out, "N", "Next Page -> ICC Tags (Page 2)")?;
    draw_item(out, "S", "Save ICC settings to config.toml")?;
    draw_empty(out)?;

    let dirty_text = if dirty { "unsaved changes" } else { "saved" };
    let dirty_color = if dirty { Color::Yellow } else { Color::Green };
    draw_line(
        out,
        &format!("  ICC Tags state: {}", dirty_text),
        dirty_color,
    )?;
    draw_empty(out)?;
    draw_section(out, "NAVIGATION")?;
    draw_item(out, "P", "<- Previous Page (ICC Studio)")?;
    draw_item(out, "Z", "Back to Main Menu")?;
    draw_item_quit(out)?;
    draw_empty(out)?;
    draw_bottom(out)?;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "  Select option: ")?;
    queue!(out, ResetColor)?;
    Ok(())
}

pub(crate) fn draw_icc_tags2(
    out: &mut impl Write,
    status: &Status,
    cfg: &Config,
    dirty: bool,
) -> io::Result<()> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_header(out, status)?;
    draw_sep(out, " ICC TAGS - PAGE 2 ")?;
    draw_empty(out)?;

    draw_section(out, "EXTENDED TAGS")?;
    draw_toggle(out, "1", "Metadata Tag (meta)", cfg.icc_metadata_enabled)?;
    draw_toggle(
        out,
        "2",
        "Calibration DateTime (calt)",
        cfg.icc_include_calibration_datetime,
    )?;
    draw_toggle(
        out,
        "3",
        "Chromatic Adaptation (chad)",
        cfg.icc_include_chromatic_adaptation,
    )?;
    draw_toggle(
        out,
        "4",
        "Chromaticity (chrm)",
        cfg.icc_include_chromaticity,
    )?;
    draw_toggle(out, "5", "Measurement (meas)", cfg.icc_include_measurement)?;
    draw_toggle(
        out,
        "6",
        "Viewing Conditions (view)",
        cfg.icc_include_viewing_conditions,
    )?;
    draw_toggle(
        out,
        "7",
        "Spectral Scaffolding (sdin/swpt/svcn)",
        cfg.icc_include_spectral_scaffold,
    )?;
    draw_empty(out)?;

    draw_section(out, "MODE + FILE")?;
    draw_item(out, "8", &format!("HDR Preset: {}", cfg.icc_hdr_preset))?;
    draw_item(out, "9", &format!("SDR Preset: {}", cfg.icc_sdr_preset))?;
    draw_item(
        out,
        "0",
        &format!("Schedule Day Preset: {}", cfg.icc_schedule_day_preset),
    )?;
    draw_item(
        out,
        "A",
        &format!("Schedule Night Preset: {}", cfg.icc_schedule_night_preset),
    )?;
    draw_item(out, "B", &format!("Profile Filename: {}", cfg.profile_name))?;
    draw_item(out, "C", &format!("Monitor Match: {}", cfg.monitor_match))?;
    draw_toggle(out, "D", "Monitor Match Regex", cfg.monitor_match_regex)?;
    draw_item(out, "S", "Save ICC settings to config.toml")?;
    draw_empty(out)?;

    let dirty_text = if dirty { "unsaved changes" } else { "saved" };
    let dirty_color = if dirty { Color::Yellow } else { Color::Green };
    draw_line(
        out,
        &format!("  ICC Tags state: {}", dirty_text),
        dirty_color,
    )?;
    draw_empty(out)?;
    draw_section(out, "NAVIGATION")?;
    draw_item(out, "P", "<- Previous Page (ICC Tags Page 1)")?;
    draw_item(out, "Z", "Back to Main Menu")?;
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
        ("\u{25CB} Not Installed", Color::Red)
    };
    draw_status(out, "Service:      ", service_text, service_color)?;

    // Monitor status
    let (monitor_text, monitor_color) = if status.monitor_count > 0 {
        (
            format!("\u{25CF} {} monitor(s) detected", status.monitor_count),
            Color::Green,
        )
    } else {
        ("\u{25CB} None detected".to_string(), Color::Red)
    };
    draw_status(out, "LG UltraGear: ", &monitor_text, monitor_color)?;

    // HDR mode status
    let (hdr_text, hdr_color) = if status.hdr_enabled {
        ("\u{25CF} Enabled", Color::Green)
    } else {
        ("\u{25CB} Disabled", Color::Yellow)
    };
    draw_status(out, "HDR Mode:     ", hdr_text, hdr_color)?;

    // SDR mode status
    let (sdr_text, sdr_color) = if status.sdr_enabled {
        ("\u{25CF} Enabled", Color::Green)
    } else {
        ("\u{25CB} Disabled", Color::Yellow)
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
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

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

fn log_service_binary_placement(after_failed_install: bool) {
    let path = config::install_path();
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_file() => {
            let msg = format!(
                "Service binary placed at {} ({} bytes)",
                path.display(),
                meta.len()
            );
            if after_failed_install {
                log_note(&format!(
                    "{}; failure happened in a later install step",
                    msg
                ));
            } else {
                log_ok(&msg);
            }
        }
        Ok(_) => log_warn(&format!(
            "Service install path exists but is not a file: {}",
            path.display()
        )),
        Err(e) => log_warn(&format!(
            "Service binary not found at {} ({})",
            path.display(),
            e
        )),
    }
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
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;
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

/// Like `run_action`, but on failure offers to open one or two relevant
/// folders in Explorer so the user can inspect them manually.
///
/// `folders` is a slice of `(key, label, path)`:
///   - `('1', "color profile", color_dir)`
///   - `('2', "program",       config_dir)`
fn run_action_offer_folder<F>(
    out: &mut impl Write,
    banner: &str,
    action: F,
    folders: &[(char, &str, std::path::PathBuf)],
) -> io::Result<()>
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error>>,
{
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;
    draw_top(out, " PROCESSING ")?;
    draw_empty(out)?;
    draw_line(out, banner, Color::Yellow)?;
    draw_empty(out)?;
    draw_bottom(out)?;
    writeln!(out)?;
    out.flush()?;

    let failed = match action() {
        Ok(()) => false,
        Err(e) => {
            write_err(out, &e.to_string())?;
            queue!(out, SetForegroundColor(Color::DarkYellow))?;
            writeln!(
                out,
                "  Tip: If Event Viewer or another MMC snap-in is open, close it and retry."
            )?;
            queue!(out, ResetColor)?;
            true
        }
    };

    writeln!(out)?;
    if failed && !folders.is_empty() {
        queue!(out, SetForegroundColor(Color::Yellow))?;
        writeln!(out, "  Open folder in Explorer?")?;
        queue!(out, ResetColor)?;
        for &(key, label, ref path) in folders {
            queue!(out, SetForegroundColor(Color::Yellow))?;
            write!(out, "    [{}]", key.to_ascii_uppercase())?;
            queue!(out, SetForegroundColor(Color::White))?;
            writeln!(out, " {} — {}", label, path.display())?;
        }
        queue!(out, SetForegroundColor(Color::DarkGrey))?;
        writeln!(out, "    Any other key to skip")?;
        queue!(out, ResetColor)?;
        out.flush()?;
        let ch = read_key()?;
        for &(key, _, ref path) in folders {
            if ch == key {
                let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
                break;
            }
        }
    } else {
        queue!(out, SetForegroundColor(Color::DarkGrey))?;
        write!(out, "  Press any key to continue...")?;
        queue!(out, ResetColor)?;
        out.flush()?;
        let _ = read_key();
    }
    Ok(())
}

// ============================================================================
// Sub-menu — interactive selection screen
// ============================================================================

/// Show a sub-menu with selectable items. Returns the index of the selected
/// item, or `None` if the user pressed Esc to cancel.
///
/// Each item is `(key, label, is_current)`. The `is_current` flag highlights
/// the item that matches the active value.
fn run_submenu(
    out: &mut impl Write,
    title: &str,
    items: &[(char, &str, bool)],
) -> io::Result<Option<usize>> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_top(out, title)?;
    draw_empty(out)?;

    for &(key, label, current) in items {
        let key_str = key.to_ascii_uppercase().to_string();
        if current {
            draw_item_colored(out, &key_str, &format!("{} \u{25C4}", label), Color::Green)?;
        } else {
            draw_item(out, &key_str, label)?;
        }
    }

    draw_empty(out)?;
    draw_line(out, "  Press Esc to cancel", Color::DarkGrey)?;
    draw_empty(out)?;
    draw_bottom(out)?;

    writeln!(out)?;
    queue!(out, SetForegroundColor(Color::White))?;
    write!(out, "  Select: ")?;
    queue!(out, ResetColor)?;
    out.flush()?;

    loop {
        let ch = read_key()?;
        // Esc is mapped to 'q' by read_key
        if ch == 'q' {
            return Ok(None);
        }
        if let Some(idx) = items.iter().position(|&(k, _, _)| k == ch) {
            return Ok(Some(idx));
        }
    }
}

fn prompt_text(
    out: &mut impl Write,
    title: &str,
    hint: &str,
    current: &str,
) -> io::Result<Option<String>> {
    queue!(
        out,
        Clear(ClearType::Purge),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    out.flush()?;

    draw_top(out, " ICC INPUT ")?;
    draw_empty(out)?;
    draw_line(out, &format!("  {}", title), Color::Yellow)?;
    draw_line(out, &format!("  Current: {}", current), Color::Green)?;
    for line in wrap_text(hint, INNER.saturating_sub(2)) {
        draw_line(out, &format!("  {}", line), Color::DarkGrey)?;
    }
    draw_line(out, "  Enter blank to cancel", Color::DarkGrey)?;
    draw_empty(out)?;
    draw_bottom(out)?;
    writeln!(out)?;
    write!(out, "  New value: ")?;
    out.flush()?;

    terminal::disable_raw_mode().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let value = input.trim().to_string();
    if value.is_empty() {
        return Ok(None);
    }
    Ok(Some(value))
}

fn wrap_text(input: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![input.to_string()];
    }
    let mut out = Vec::new();
    let mut line = String::new();

    for word in input.split_whitespace() {
        if line.is_empty() {
            if word.len() <= max_width {
                line.push_str(word);
            } else {
                // Break overlong tokens so the TUI border stays aligned.
                for chunk in word.as_bytes().chunks(max_width) {
                    out.push(String::from_utf8_lossy(chunk).to_string());
                }
            }
            continue;
        }

        let needed = line.len() + 1 + word.len();
        if needed <= max_width {
            line.push(' ');
            line.push_str(word);
        } else {
            out.push(line);
            line = String::new();
            if word.len() <= max_width {
                line.push_str(word);
            } else {
                for chunk in word.as_bytes().chunks(max_width) {
                    out.push(String::from_utf8_lossy(chunk).to_string());
                }
            }
        }
    }

    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn prompt_f64(
    out: &mut impl Write,
    title: &str,
    hint: &str,
    current: f64,
) -> io::Result<Option<f64>> {
    let Some(text) = prompt_text(out, title, hint, &format!("{:.6}", current))? else {
        return Ok(None);
    };
    match text.parse::<f64>() {
        Ok(value) => Ok(Some(value)),
        Err(_) => {
            run_action(out, "Invalid numeric input...", || {
                Err(format!("Could not parse '{}' as number", text).into())
            })?;
            Ok(None)
        }
    }
}

fn prompt_u8(out: &mut impl Write, title: &str, hint: &str, current: u8) -> io::Result<Option<u8>> {
    let Some(text) = prompt_text(out, title, hint, &current.to_string())? else {
        return Ok(None);
    };
    match text.parse::<u16>() {
        Ok(value) if value <= u8::MAX as u16 => Ok(Some(value as u8)),
        _ => {
            run_action(out, "Invalid integer input...", || {
                Err(format!("Could not parse '{}' as u8", text).into())
            })?;
            Ok(None)
        }
    }
}

fn prompt_u32(
    out: &mut impl Write,
    title: &str,
    hint: &str,
    current: u32,
) -> io::Result<Option<u32>> {
    let Some(text) = prompt_text(out, title, hint, &current.to_string())? else {
        return Ok(None);
    };
    match text.parse::<u32>() {
        Ok(value) => Ok(Some(value)),
        Err(_) => {
            run_action(out, "Invalid integer input...", || {
                Err(format!("Could not parse '{}' as u32", text).into())
            })?;
            Ok(None)
        }
    }
}

fn parse_vcp_code(text: &str) -> Result<u8, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("empty VCP code".to_string());
    }

    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).map_err(|e| format!("invalid hex '{}': {}", t, e));
    }

    if t.chars()
        .any(|c| c.is_ascii_hexdigit() && c.is_ascii_alphabetic())
    {
        return u8::from_str_radix(t, 16).map_err(|e| format!("invalid hex '{}': {}", t, e));
    }

    t.parse::<u8>()
        .or_else(|_| u8::from_str_radix(t, 16))
        .map_err(|e| format!("invalid VCP code '{}': {}", t, e))
}

fn prompt_vcp_code(
    out: &mut impl Write,
    title: &str,
    hint: &str,
    current: u8,
) -> io::Result<Option<u8>> {
    let Some(text) = prompt_text(out, title, hint, &format!("0x{:02X}", current))? else {
        return Ok(None);
    };
    match parse_vcp_code(&text) {
        Ok(value) => Ok(Some(value)),
        Err(e) => {
            run_action(out, "Invalid VCP code...", || Err(e.into()))?;
            Ok(None)
        }
    }
}

// ============================================================================
// Actions — called from TUI menu selections
// ============================================================================

fn maybe_capture_last_good(
    cfg: &Config,
    profile_path: &std::path::Path,
    note: &str,
    ddc_brightness: Option<u32>,
) {
    if let Ok(snapshot) = app_state::create_profile_snapshot(
        cfg,
        "Auto Last Good (TUI)",
        "auto",
        profile_path,
        ddc_brightness,
        note,
    ) {
        let _ = app_state::mark_snapshot_last_good(&snapshot, ddc_brightness, note);
    }
}

fn action_open_state_folder() -> Result<(), Box<dyn std::error::Error>> {
    let state_dir = app_state::state_dir();
    if !state_dir.exists() {
        std::fs::create_dir_all(&state_dir)?;
    }
    std::process::Command::new("explorer.exe")
        .arg(&state_dir)
        .spawn()?;
    log_ok(&format!("Opened {}", state_dir.display()));
    Ok(())
}

fn open_color_management_panel() -> Result<(), Box<dyn std::error::Error>> {
    if std::process::Command::new("colorcpl.exe").spawn().is_ok() {
        return Ok(());
    }

    std::process::Command::new("control.exe")
        .arg("/name")
        .arg("Microsoft.ColorManagement")
        .spawn()?;
    Ok(())
}

fn action_clear_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    app_state::clear_diagnostics_log()?;
    log_ok("Diagnostics log cleared.");
    Ok(())
}

fn detect_conflicting_processes() -> Vec<String> {
    let candidates = [
        "flux",
        "f.lux",
        "displaycal",
        "calibrilla",
        "iris",
        "lg calibration",
        "onscreencontrol",
        "truetone",
    ];
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Process | Select-Object -ExpandProperty ProcessName",
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
    let mut matches = Vec::new();
    for name in candidates {
        if text.contains(name) {
            matches.push(name.to_string());
        }
    }
    matches
}

fn night_light_maybe_enabled() -> bool {
    // Best-effort heuristic based on the Night Light CloudStore state key.
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\Cache\DefaultAccount\$$windows.data.bluelightreduction.bluelightreductionstate\Current",
            "/v",
            "Data",
        ])
        .output();
    let Ok(output) = output else {
        return false;
    };
    output.status.success() && !output.stdout.is_empty()
}

fn action_detect_conflicts() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load();
    let mut warnings = 0usize;

    let hdr_enabled = lg_monitor::is_any_display_hdr_enabled().unwrap_or(false);
    if hdr_enabled {
        warnings += 1;
        log_warn("HDR is currently enabled. Windows can remap SDR ICC behavior in HDR mode.");
    } else {
        log_ok("HDR is currently disabled.");
    }

    if night_light_maybe_enabled() {
        warnings += 1;
        log_warn("Night Light state key is present; warm tint tools can override ICC appearance.");
    }

    let processes = detect_conflicting_processes();
    if processes.is_empty() {
        log_ok("No obvious color-control helper processes detected.");
    } else {
        warnings += 1;
        log_warn(&format!(
            "Potential color-control process(es) detected: {}",
            processes.join(", ")
        ));
    }

    let (svc_installed, svc_running) = lg_service::query_service_info();
    if !svc_installed || !svc_running {
        warnings += 1;
        log_warn("Service not fully active; profile reapply may not happen on wake/unlock.");
    }

    if cfg.icc_active_preset == "custom" && cfg.profile_name.trim().is_empty() {
        warnings += 1;
        log_warn("Custom preset is active but profile_name is empty.");
    }

    if warnings == 0 {
        log_done("No likely ICC/DDC conflicts detected.");
    } else {
        log_done(&format!(
            "Detected {} potential conflict area(s).",
            warnings
        ));
    }

    app_state::append_diagnostic_event(
        "tui",
        if warnings == 0 { "INFO" } else { "WARN" },
        "conflict_scan",
        &format!("warnings={}", warnings),
    );

    Ok(())
}

fn ddc_risky_reason(vcp_code: u8) -> Option<&'static str> {
    match vcp_code {
        lg_monitor::ddc::VCP_FACTORY_RESET => Some("factory reset affects most monitor settings"),
        lg_monitor::ddc::VCP_RESET_BRIGHTNESS_CONTRAST => {
            Some("reset restores brightness/contrast defaults")
        }
        lg_monitor::ddc::VCP_RESET_COLOR => Some("reset restores color defaults"),
        lg_monitor::ddc::VCP_POWER_MODE => Some("power mode can blank or power off the monitor"),
        lg_monitor::ddc::VCP_INPUT_SOURCE => Some("input source switch can move away from this PC"),
        _ => None,
    }
}

fn validate_guarded_ddc_write(
    guardrails: &app_state::DdcGuardrails,
    vcp_code: u8,
    value: u32,
) -> Result<(), String> {
    if guardrails.enabled && vcp_code == lg_monitor::ddc::VCP_BRIGHTNESS {
        let min = guardrails.min_brightness.min(100);
        let max = guardrails.max_brightness.min(100).max(min);
        if value < min || value > max {
            return Err(format!(
                "Guardrails blocked brightness {} (allowed {}..{}).",
                value, min, max
            ));
        }
    }
    Ok(())
}

fn confirm_ddc_write_if_risky(
    out: &mut impl Write,
    guardrails: &app_state::DdcGuardrails,
    vcp_code: u8,
    value: u32,
    action_desc: &str,
) -> io::Result<bool> {
    if !guardrails.confirm_risky_writes {
        return Ok(true);
    }
    let Some(reason) = ddc_risky_reason(vcp_code) else {
        return Ok(true);
    };
    let hint = format!(
        "This is a risky VCP write: {}. Type YES to continue.",
        reason
    );
    let prompt = format!("{} (0x{:02X}={})", action_desc, vcp_code, value);
    let text = prompt_text(out, &prompt, &hint, "NO")?;
    Ok(matches!(text.as_deref(), Some("YES") | Some("yes")))
}

fn configure_ddc_guardrails(
    out: &mut impl Write,
    guardrails: &mut app_state::DdcGuardrails,
) -> io::Result<()> {
    let items = vec![
        ('1', "Toggle Guardrails Enabled", guardrails.enabled),
        ('2', "Set Min Brightness", false),
        ('3', "Set Max Brightness", false),
        (
            '4',
            "Toggle Confirm Risky Writes",
            guardrails.confirm_risky_writes,
        ),
        ('5', "Reset to Defaults", false),
    ];
    let Some(choice) = run_submenu(out, " DDC GUARDRAILS ", &items)? else {
        return Ok(());
    };
    match choice {
        0 => {
            guardrails.enabled = !guardrails.enabled;
        }
        1 => {
            if let Some(min) = prompt_u32(
                out,
                "DDC GUARDRAILS",
                "Minimum brightness allowed when guardrails are enabled (0..100)",
                guardrails.min_brightness,
            )? {
                guardrails.min_brightness = min.min(100);
            }
        }
        2 => {
            if let Some(max) = prompt_u32(
                out,
                "DDC GUARDRAILS",
                "Maximum brightness allowed when guardrails are enabled (0..100)",
                guardrails.max_brightness,
            )? {
                guardrails.max_brightness = max.min(100);
            }
        }
        3 => {
            guardrails.confirm_risky_writes = !guardrails.confirm_risky_writes;
        }
        4 => {
            *guardrails = app_state::DdcGuardrails::default();
        }
        _ => {}
    }
    *guardrails = guardrails.clone().sanitized();
    match app_state::save_ddc_guardrails(guardrails) {
        Ok(()) => run_action(out, "DDC guardrails updated.", || Ok(()))?,
        Err(e) => run_action(out, "Could not save DDC guardrails...", || Err(e))?,
    }
    Ok(())
}

fn sync_mode_presets_to_active(cfg: &mut Config) {
    cfg.icc_sdr_preset = cfg.icc_active_preset.clone();
    cfg.icc_hdr_preset = cfg.icc_active_preset.clone();
}

fn enable_manual_overlay_for_tuning_edits(cfg: &mut Config) {
    cfg.icc_tuning_overlay_manual = true;
}

fn run_reader_calibration_wizard(
    out: &mut impl Write,
    cfg: &mut Config,
    dirty: &mut bool,
    opts: &Options,
) -> io::Result<()> {
    let Some(target_luminance) = prompt_f64(
        out,
        "READER WIZARD - STEP 1",
        "Target white luminance in cd/m^2. Typical text comfort: 120..220",
        cfg.icc_luminance_cd_m2,
    )?
    else {
        return Ok(());
    };
    let Some(unyellow_strength) = prompt_u32(
        out,
        "READER WIZARD - STEP 2",
        "Unyellow strength 0..100. Higher = cooler/less warm white.",
        60,
    )?
    else {
        return Ok(());
    };
    let Some(comfort_brightness) = prompt_u32(
        out,
        "READER WIZARD - STEP 3",
        "Text comfort 0..100. Higher lifts mids/shadows more.",
        50,
    )?
    else {
        return Ok(());
    };

    let uny = (unyellow_strength.min(100) as f64) / 100.0;
    let comfort = (comfort_brightness.min(100) as f64) / 100.0;

    cfg.icc_active_preset = "reader".to_string();
    sync_mode_presets_to_active(cfg);
    cfg.icc_tuning_preset = "reader_balanced".to_string();
    cfg.icc_tuning_overlay_manual = true;
    cfg.icc_luminance_cd_m2 = lg_profile::sanitize_dynamic_luminance_cd_m2(target_luminance);
    cfg.icc_gamma = lg_profile::sanitize_dynamic_gamma(2.18 - 0.18 * comfort);
    cfg.icc_gamma_r = lg_profile::sanitize_channel_gamma_multiplier(1.0 - 0.18 * uny);
    cfg.icc_gamma_g = lg_profile::sanitize_channel_gamma_multiplier(1.0 - 0.05 * uny);
    cfg.icc_gamma_b = lg_profile::sanitize_channel_gamma_multiplier(1.0 + 0.22 * uny);
    cfg.icc_black_lift = lg_profile::sanitize_black_lift(0.015 + 0.07 * comfort);
    cfg.icc_midtone_boost = lg_profile::sanitize_midtone_boost(0.05 + 0.16 * comfort);
    cfg.icc_white_compression = lg_profile::sanitize_white_compression(0.04 + 0.20 * comfort);
    *dirty = true;

    if let Some(answer) = prompt_text(
        out,
        "READER WIZARD",
        "Apply these settings now? (type YES to apply)",
        "NO",
    )? {
        if answer.eq_ignore_ascii_case("YES") {
            run_action(out, "Applying reader wizard ICC profile...", || {
                action_icc_generate_and_apply(cfg, opts)
            })?;
        }
    }

    if let Some(answer) = prompt_text(
        out,
        "READER WIZARD",
        "Save these wizard values to config.toml now? (type YES to save)",
        "YES",
    )? {
        if answer.eq_ignore_ascii_case("YES") {
            run_action(out, "Saving reader wizard settings...", || {
                Config::write_config(cfg)?;
                Ok(())
            })?;
            *dirty = false;
        }
    }

    Ok(())
}

fn log_snapshot_diff(snapshot: &app_state::ProfileSnapshot, cfg: &Config) {
    let mut changed = 0usize;
    if snapshot.profile_name != cfg.profile_name {
        changed += 1;
        log_info(&format!(
            "profile_name: '{}' -> '{}'",
            cfg.profile_name, snapshot.profile_name
        ));
    }
    if snapshot.active_preset != cfg.icc_active_preset {
        changed += 1;
        log_info(&format!(
            "icc_active_preset: '{}' -> '{}'",
            cfg.icc_active_preset, snapshot.active_preset
        ));
    }
    if snapshot.tuning_preset != cfg.icc_tuning_preset {
        changed += 1;
        log_info(&format!(
            "icc_tuning_preset: '{}' -> '{}'",
            cfg.icc_tuning_preset, snapshot.tuning_preset
        ));
    }
    if (snapshot.gamma - cfg.icc_gamma).abs() > f64::EPSILON {
        changed += 1;
        log_info(&format!(
            "icc_gamma: {:.3} -> {:.3}",
            cfg.icc_gamma, snapshot.gamma
        ));
    }
    if (snapshot.luminance_cd_m2 - cfg.icc_luminance_cd_m2).abs() > f64::EPSILON {
        changed += 1;
        log_info(&format!(
            "icc_luminance_cd_m2: {:.1} -> {:.1}",
            cfg.icc_luminance_cd_m2, snapshot.luminance_cd_m2
        ));
    }
    if snapshot.per_monitor_profiles != cfg.icc_per_monitor_profiles {
        changed += 1;
        log_info(&format!(
            "icc_per_monitor_profiles: {} -> {}",
            cfg.icc_per_monitor_profiles, snapshot.per_monitor_profiles
        ));
    }
    if changed == 0 {
        log_ok("Snapshot and current config core fields match.");
    } else {
        log_ok(&format!("{} core field(s) differ.", changed));
    }
}

fn run_snapshot_manager(
    out: &mut impl Write,
    cfg: &mut Config,
    dirty: &mut bool,
    opts: &Options,
) -> io::Result<()> {
    let items = vec![
        ('1', "Save Snapshot", false),
        ('2', "Restore Snapshot", false),
        ('3', "Diff Latest Snapshot", false),
        ('4', "Open Snapshot Folder", false),
    ];
    let Some(choice) = run_submenu(out, " SNAPSHOT MANAGER ", &items)? else {
        return Ok(());
    };
    match choice {
        0 => {
            let label = prompt_text(
                out,
                "SAVE SNAPSHOT",
                "Label for this snapshot",
                "Manual Snapshot",
            )?
            .unwrap_or_else(|| "Manual Snapshot".to_string());
            run_action(out, "Saving profile snapshot...", || {
                let profile = ensure_active_profile_for_mode(cfg, opts.hdr)?;
                let snapshot = app_state::create_profile_snapshot(
                    cfg,
                    &label,
                    "manual",
                    &profile,
                    if cfg.ddc_brightness_on_reapply {
                        Some(cfg.ddc_brightness_value)
                    } else {
                        None
                    },
                    "Saved from ICC Studio",
                )?;
                log_ok(&format!(
                    "Snapshot saved: {} ({})",
                    snapshot.label, snapshot.created_at
                ));
                Ok(())
            })?;
        }
        1 => {
            let snapshots = match app_state::list_profile_snapshots() {
                Ok(s) => s,
                Err(e) => {
                    run_action(out, "Loading snapshots...", || Err(e))?;
                    return Ok(());
                }
            };
            if snapshots.is_empty() {
                run_action(out, "Loading snapshots...", || {
                    Err("No snapshots available yet.".into())
                })?;
                return Ok(());
            }
            let keys = b"123456789abcdefghijklmnop";
            let count = snapshots.len().min(keys.len());
            let labels = snapshots[..count]
                .iter()
                .map(|s| format!("{} [{}] {}", s.label, s.kind, s.created_at))
                .collect::<Vec<_>>();
            let items = (0..count)
                .map(|i| (keys[i] as char, labels[i].as_str(), false))
                .collect::<Vec<_>>();
            if let Some(idx) = run_submenu(out, " RESTORE SNAPSHOT ", &items)? {
                let snapshot = snapshots[idx].clone();
                run_action(
                    out,
                    &format!("Restoring snapshot '{}'...", snapshot.label),
                    || {
                        let profile_path = app_state::restore_profile_snapshot(&snapshot)?;
                        cfg.profile_name = snapshot.profile_name.clone();
                        cfg.icc_active_preset = snapshot.active_preset.clone();
                        cfg.icc_tuning_preset = snapshot.tuning_preset.clone();
                        cfg.icc_gamma = lg_profile::sanitize_dynamic_gamma(snapshot.gamma);
                        cfg.icc_luminance_cd_m2 =
                            lg_profile::sanitize_dynamic_luminance_cd_m2(snapshot.luminance_cd_m2);
                        cfg.icc_per_monitor_profiles = snapshot.per_monitor_profiles;
                        Config::write_config(cfg)?;
                        *dirty = false;

                        let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
                        for device in &devices {
                            lg_profile::reapply_profile_with_mode_associations(
                                &device.device_key,
                                &profile_path,
                                &profile_path,
                                &profile_path,
                                cfg.toggle_delay_ms,
                                opts.per_user,
                            )?;
                        }
                        lg_profile::refresh_display(
                            cfg.refresh_display_settings,
                            cfg.refresh_broadcast_color,
                            cfg.refresh_invalidate,
                        );
                        lg_profile::trigger_calibration_loader(cfg.refresh_calibration_loader);

                        if let Some(level) = snapshot.ddc_brightness {
                            let _ = lg_monitor::ddc::set_brightness_all(level);
                        }

                        maybe_capture_last_good(
                            cfg,
                            &profile_path,
                            "Snapshot restore apply",
                            snapshot.ddc_brightness,
                        );
                        app_state::append_diagnostic_event(
                            "tui",
                            "INFO",
                            "apply_success",
                            &format!("snapshot restored: {}", snapshot.label),
                        );
                        log_ok(&format!(
                            "Snapshot restored and applied to {} monitor(s).",
                            devices.len()
                        ));
                        Ok(())
                    },
                )?;
            }
        }
        2 => {
            run_action(out, "Comparing latest snapshot...", || {
                let snapshots = app_state::list_profile_snapshots()?;
                let Some(snapshot) = snapshots.first() else {
                    return Err("No snapshots available yet.".into());
                };
                log_ok(&format!(
                    "Latest snapshot: '{}' ({})",
                    snapshot.label, snapshot.created_at
                ));
                log_snapshot_diff(snapshot, cfg);
                Ok(())
            })?;
        }
        3 => {
            run_action(out, "Opening snapshot folder...", || {
                let dir = app_state::snapshots_dir();
                if !dir.exists() {
                    std::fs::create_dir_all(&dir)?;
                }
                std::process::Command::new("explorer.exe")
                    .arg(&dir)
                    .spawn()?;
                log_ok(&format!("Opened {}", dir.display()));
                Ok(())
            })?;
        }
        _ => {}
    }
    Ok(())
}

fn action_toggle_ab_compare(
    opts: &Options,
    cfg: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would toggle A/B compare profile");
        return Ok(());
    }

    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    if devices.is_empty() {
        return Err("No matching monitors found for A/B compare.".into());
    }

    let state_dir = app_state::state_dir();
    if !state_dir.exists() {
        std::fs::create_dir_all(&state_dir)?;
    }
    let mut state = app_state::load_ab_compare_state();
    let profile_a = state_dir.join("ab_profile_a.icm");
    let profile_b = state_dir.join("ab_profile_b.icm");

    if !state.enabled || !profile_a.exists() || !profile_b.exists() {
        let current = ensure_active_profile_for_mode(cfg, opts.hdr)?;
        std::fs::copy(&current, &profile_a)?;

        let baseline = lg_profile::ensure_active_profile_installed_tuned(
            &lg_profile::color_directory(),
            "gamma22",
            &cfg.profile_name,
            2.2,
            cfg.icc_luminance_cd_m2,
            cfg.icc_generate_specialized_profiles,
            tuning_from_config(cfg),
        )?;
        std::fs::copy(&baseline, &profile_b)?;

        state.enabled = true;
        state.profile_a_path = profile_a.to_string_lossy().to_string();
        state.profile_b_path = profile_b.to_string_lossy().to_string();
        state.profile_a_label = "Current".to_string();
        state.profile_b_label = "Gamma22 baseline".to_string();
        state.current_side = "A".to_string();
    }

    let next_side = if state.current_side.eq_ignore_ascii_case("A") {
        "B"
    } else {
        "A"
    };
    let next_path = if next_side == "A" {
        std::path::PathBuf::from(&state.profile_a_path)
    } else {
        std::path::PathBuf::from(&state.profile_b_path)
    };
    if !next_path.exists() {
        return Err(format!("A/B profile path missing: {}", next_path.display()).into());
    }

    for device in &devices {
        lg_profile::reapply_profile_with_mode_associations(
            &device.device_key,
            &next_path,
            &next_path,
            &next_path,
            cfg.toggle_delay_ms,
            opts.per_user,
        )?;
    }
    // Use a non-disruptive refresh first to avoid monitor-wide flicker.
    lg_profile::refresh_display(false, true, false);
    lg_profile::trigger_calibration_loader(true);

    state.current_side = next_side.to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    state.last_switch_at = format!("unix:{}", now);
    app_state::save_ab_compare_state(&state)?;

    app_state::append_diagnostic_event(
        "tui",
        "INFO",
        "ab_compare_toggle",
        &format!("side={} monitors={}", next_side, devices.len()),
    );
    log_done(&format!(
        "A/B compare switched to side {} for {} monitor(s).",
        next_side,
        devices.len()
    ));
    Ok(())
}

fn action_safe_recovery(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let Some(recovery_state) = app_state::load_recovery_state() else {
        return Err("No last-good recovery state found yet.".into());
    };
    let Some(snapshot) = app_state::load_last_good_snapshot()? else {
        return Err("Recovery state exists but snapshot metadata is missing.".into());
    };
    let mut cfg = Config::load();

    if opts.dry_run {
        log_dry(&format!(
            "Would recover snapshot '{}' ({})",
            snapshot.label, recovery_state.created_at
        ));
        return Ok(());
    }

    let profile_path = app_state::restore_profile_snapshot(&snapshot)?;
    cfg.profile_name = snapshot.profile_name.clone();
    cfg.icc_active_preset = snapshot.active_preset.clone();
    cfg.icc_tuning_preset = snapshot.tuning_preset.clone();
    cfg.icc_gamma = lg_profile::sanitize_dynamic_gamma(snapshot.gamma);
    cfg.icc_luminance_cd_m2 =
        lg_profile::sanitize_dynamic_luminance_cd_m2(snapshot.luminance_cd_m2);
    cfg.icc_per_monitor_profiles = snapshot.per_monitor_profiles;
    Config::write_config(&cfg)?;

    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    if devices.is_empty() {
        log_warn("No matching monitors found during recovery.");
    } else {
        for device in &devices {
            lg_profile::reapply_profile_with_mode_associations(
                &device.device_key,
                &profile_path,
                &profile_path,
                &profile_path,
                cfg.toggle_delay_ms,
                opts.per_user,
            )?;
        }
        lg_profile::refresh_display(
            cfg.refresh_display_settings,
            cfg.refresh_broadcast_color,
            cfg.refresh_invalidate,
        );
        lg_profile::trigger_calibration_loader(cfg.refresh_calibration_loader);
        if let Some(level) = recovery_state.ddc_brightness {
            let _ = lg_monitor::ddc::set_brightness_all(level);
        }
    }

    app_state::append_diagnostic_event(
        "tui",
        "INFO",
        "safe_recovery",
        &format!(
            "snapshot='{}' monitors={} timestamp={}",
            snapshot.label,
            devices.len(),
            recovery_state.created_at
        ),
    );
    log_done(&format!(
        "Safe recovery applied from '{}' to {} monitor(s).",
        snapshot.label,
        devices.len()
    ));
    Ok(())
}

fn action_default_install(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would generate dynamic ICC profile to color store");
        log_dry("Would write default config");
        log_dry("Would install Windows service");
        log_dry("Would start service");
        return Ok(());
    }

    let cfg = Config::load();

    // Generate/install ICC profile
    let profile_path = ensure_active_profile_for_mode(&cfg, opts.hdr)?;
    match lg_profile::is_profile_installed(&profile_path) {
        true => log_ok(&format!(
            "ICC profile installed to {}",
            profile_path.display()
        )),
        false => log_ok("ICC profile already present"),
    }
    maybe_capture_last_good(&cfg, &profile_path, "Default install profile ready", None);

    // Write default config
    let cfg_path = config::config_path();
    if !cfg_path.exists() {
        Config::write_default()?;
        log_ok(&format!("Default config written to {}", cfg_path.display()));
    } else {
        log_ok(&format!("Config already exists at {}", cfg_path.display()));
    }

    // Install service
    match lg_service::install(&cfg.monitor_match) {
        Ok(()) => {
            log_ok("Service installed");
            log_service_binary_placement(false);
        }
        Err(e) => {
            log_service_binary_placement(true);
            return Err(e);
        }
    }

    // Start service
    lg_service::start_service()?;
    log_ok("Service started");

    log_done("Default install complete!");
    Ok(())
}

fn action_profile_only(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        log_dry("Would generate dynamic ICC profile to color store");
        return Ok(());
    }

    let cfg = Config::load();
    let profile_path = ensure_active_profile_for_mode(&cfg, opts.hdr)?;
    match lg_profile::is_profile_installed(&profile_path) {
        true => log_ok(&format!(
            "ICC profile installed to {}",
            profile_path.display()
        )),
        false => log_ok("ICC profile already present"),
    }
    maybe_capture_last_good(&cfg, &profile_path, "Profile-only install", None);

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

    match lg_service::install(&cfg.monitor_match) {
        Ok(()) => {
            log_ok("Service installed");
            log_service_binary_placement(false);
        }
        Err(e) => {
            log_service_binary_placement(true);
            return Err(e);
        }
    }

    lg_service::start_service()?;
    log_ok("Service started");

    log_done("Service install complete!");
    Ok(())
}

fn action_refresh(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    if opts.dry_run {
        log_dry("Would re-apply SDR/HDR mode profiles to matching monitors");
        return Ok(());
    }

    let cfg = Config::load();
    let shared_mode_profiles = if cfg.icc_per_monitor_profiles {
        None
    } else {
        Some(ensure_shared_mode_profiles(&cfg)?)
    };

    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    let success = if devices.is_empty() {
        log_skip("No matching monitors found.");
        app_state::append_diagnostic_event("tui", "WARN", "apply_skip", "refresh: no monitors");
        false
    } else {
        let mut last_applied_profile: Option<std::path::PathBuf> = None;
        for device in &devices {
            let (sdr_profile_path, hdr_profile_path) =
                if let Some((sdr, hdr)) = &shared_mode_profiles {
                    (sdr.clone(), hdr.clone())
                } else {
                    ensure_mode_profiles_for_monitor(&cfg, device)?
                };
            let active_profile_path = if opts.hdr {
                &hdr_profile_path
            } else {
                &sdr_profile_path
            };
            log_info(&format!("Found: {}", device.name));
            lg_profile::reapply_profile_with_mode_associations(
                &device.device_key,
                active_profile_path,
                &sdr_profile_path,
                &hdr_profile_path,
                cfg.toggle_delay_ms,
                opts.per_user,
            )?;
            last_applied_profile = Some(active_profile_path.clone());
            log_ok(&format!("SDR/HDR profiles associated for {}", device.name));
            if opts.generic_default {
                lg_profile::set_generic_default(
                    &device.device_key,
                    active_profile_path,
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

        // DDC/CI brightness (if enabled)
        if cfg.ddc_brightness_on_reapply {
            match lg_monitor::ddc::set_brightness_all(cfg.ddc_brightness_value) {
                Ok(n) => log_ok(&format!(
                    "DDC brightness set to {} on {} monitor(s)",
                    cfg.ddc_brightness_value, n
                )),
                Err(e) => log_note(&format!("DDC brightness failed: {}", e)),
            }
        }

        if opts.toast && cfg.toast_enabled {
            lg_notify::show_reapply_toast(true, &cfg.toast_title, &cfg.toast_body, opts.verbose);
        }

        if let Some(profile_path) = last_applied_profile.as_ref() {
            maybe_capture_last_good(
                &cfg,
                profile_path,
                "Manual refresh apply",
                if cfg.ddc_brightness_on_reapply {
                    Some(cfg.ddc_brightness_value)
                } else {
                    None
                },
            );
        }
        app_state::append_diagnostic_event(
            "tui",
            "INFO",
            "apply_success",
            &format!("refresh applied to {} monitor(s)", devices.len()),
        );

        log_done(&format!(
            "Profile refreshed for {} monitor(s).",
            devices.len()
        ));
        true
    };

    emit_apply_latency_tui(
        started,
        success,
        &format!(
            "action=refresh monitors={}",
            if success { devices.len() } else { 0 }
        ),
    );

    Ok(())
}

fn action_icc_generate_and_apply(
    cfg: &Config,
    opts: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    if opts.dry_run {
        log_dry("Would generate optimized ICC from ICC Studio settings");
        log_dry("Would apply profile to all matching monitors");
        return Ok(());
    }

    let shared_mode_profiles = if cfg.icc_per_monitor_profiles {
        None
    } else {
        Some(ensure_shared_mode_profiles(cfg)?)
    };

    let match_mode = if cfg.monitor_match_regex {
        lg_monitor::MonitorMatchMode::Regex
    } else {
        lg_monitor::MonitorMatchMode::Substring
    };
    let devices = lg_monitor::find_matching_monitors_with_mode(&cfg.monitor_match, match_mode)?;
    if devices.is_empty() {
        log_skip("No matching monitors found.");
        app_state::append_diagnostic_event(
            "tui",
            "WARN",
            "apply_skip",
            "icc optimize: no monitors",
        );
        emit_apply_latency_tui(started, false, "action=icc_optimize monitors=0");
        return Ok(());
    }

    let mut last_applied_profile: Option<std::path::PathBuf> = None;
    for device in &devices {
        let (sdr_profile_path, hdr_profile_path) = if let Some((sdr, hdr)) = &shared_mode_profiles {
            (sdr.clone(), hdr.clone())
        } else {
            ensure_mode_profiles_for_monitor(cfg, device)?
        };
        let active_profile_path = if opts.hdr {
            &hdr_profile_path
        } else {
            &sdr_profile_path
        };

        log_info(&format!(
            "Applying optimized ICC to {} (active={} sdr={} hdr={})",
            device.name,
            active_profile_path.display(),
            sdr_profile_path.display(),
            hdr_profile_path.display()
        ));
        lg_profile::reapply_profile_with_mode_associations(
            &device.device_key,
            active_profile_path,
            &sdr_profile_path,
            &hdr_profile_path,
            cfg.toggle_delay_ms,
            opts.per_user,
        )?;
        last_applied_profile = Some(active_profile_path.clone());
        if opts.generic_default {
            lg_profile::set_generic_default(
                &device.device_key,
                active_profile_path,
                opts.per_user,
            )?;
            log_ok(&format!("Generic default set for {}", device.name));
        }
    }

    // Use a non-disruptive refresh first to avoid monitor-wide flicker.
    lg_profile::refresh_display(false, true, false);
    lg_profile::trigger_calibration_loader(true);

    if cfg.ddc_brightness_on_reapply {
        match lg_monitor::ddc::set_brightness_all(cfg.ddc_brightness_value) {
            Ok(n) => log_ok(&format!(
                "DDC brightness set to {} on {} monitor(s)",
                cfg.ddc_brightness_value, n
            )),
            Err(e) => log_note(&format!("DDC brightness failed: {}", e)),
        }
    }

    if opts.toast && cfg.toast_enabled {
        lg_notify::show_reapply_toast(true, &cfg.toast_title, &cfg.toast_body, opts.verbose);
    }

    let tuning = tuning_from_config(cfg);
    log_ok(&format!(
        "Tuning: preset='{}' gamma={:.3} Yw={:.1} lift={:.3} mid={:.3} comp={:.3} vcgt={} {:.3}",
        cfg.icc_tuning_preset,
        cfg.icc_gamma,
        cfg.icc_luminance_cd_m2,
        tuning.black_lift,
        tuning.midtone_boost,
        tuning.white_compression,
        tuning.vcgt_enabled,
        tuning.vcgt_strength
    ));
    if let Some(profile_path) = last_applied_profile.as_ref() {
        maybe_capture_last_good(
            cfg,
            profile_path,
            "ICC Studio optimized apply",
            if cfg.ddc_brightness_on_reapply {
                Some(cfg.ddc_brightness_value)
            } else {
                None
            },
        );
    }
    app_state::append_diagnostic_event(
        "tui",
        "INFO",
        "apply_success",
        &format!("icc optimize applied to {} monitor(s)", devices.len()),
    );
    emit_apply_latency_tui(
        started,
        true,
        &format!("action=icc_optimize monitors={}", devices.len()),
    );
    log_done(&format!(
        "Optimized ICC generated and applied for {} monitor(s).",
        devices.len()
    ));
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

    let profile_path = resolve_active_profile_path(&cfg);
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
    let profile_path = resolve_active_profile_path(&cfg);
    match lg_profile::remove_profile(&profile_path)? {
        true => log_ok(&format!(
            "ICC profile removed from {}",
            profile_path.display()
        )),
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
    let profile_path = resolve_active_profile_path(&cfg);
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
    app_state::append_diagnostic_event(
        "tui",
        "INFO",
        "service_status",
        &format!("installed={} running={}", installed, running),
    );
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
    let mut conflict_hits = 0usize;

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
    let profile_path = resolve_active_profile_path(&cfg);
    if lg_profile::is_profile_installed(&profile_path) {
        log_ok(&format!(
            "ICC profile installed at {}",
            profile_path.display()
        ));
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

    // Conflict scan
    let hdr_enabled = lg_monitor::is_any_display_hdr_enabled().unwrap_or(false);
    if hdr_enabled {
        conflict_hits += 1;
        log_warn("HDR mode appears enabled; SDR ICC behavior can be altered.");
    }
    if night_light_maybe_enabled() {
        conflict_hits += 1;
        log_warn("Night Light state key detected; warm tint controls can override ICC look.");
    }
    let conflicting_processes = detect_conflicting_processes();
    if !conflicting_processes.is_empty() {
        conflict_hits += 1;
        log_warn(&format!(
            "Potential color-control process(es): {}",
            conflicting_processes.join(", ")
        ));
    }

    // Summary
    let all_good = !devices.is_empty()
        && lg_profile::is_profile_installed(&profile_path)
        && installed
        && running;
    if all_good && conflict_hits == 0 {
        log_done("Everything looks good!");
    } else {
        log_done("Some issues detected — see warnings above.");
    }

    app_state::append_diagnostic_event(
        "tui",
        if all_good && conflict_hits == 0 {
            "INFO"
        } else {
            "WARN"
        },
        "applicability_check",
        &format!(
            "monitors={} profile_installed={} service_installed={} service_running={} conflicts={}",
            devices.len(),
            lg_profile::is_profile_installed(&profile_path),
            installed,
            running,
            conflict_hits
        ),
    );

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
    let started = std::time::Instant::now();
    let cfg = Config::load();
    let shared_mode_profiles = if cfg.icc_per_monitor_profiles {
        None
    } else {
        Some(ensure_shared_mode_profiles(&cfg)?)
    };

    let devices = lg_monitor::find_matching_monitors(&cfg.monitor_match)?;
    let success = if devices.is_empty() {
        log_skip("No matching monitors found.");
        app_state::append_diagnostic_event(
            "tui",
            "WARN",
            "apply_skip",
            "force refresh profile: no monitors",
        );
        false
    } else {
        let mut last_applied_profile: Option<std::path::PathBuf> = None;
        for device in &devices {
            let (sdr_profile_path, hdr_profile_path) =
                if let Some((sdr, hdr)) = &shared_mode_profiles {
                    (sdr.clone(), hdr.clone())
                } else {
                    ensure_mode_profiles_for_monitor(&cfg, device)?
                };
            let active_profile_path = if opts.hdr {
                &hdr_profile_path
            } else {
                &sdr_profile_path
            };
            log_info(&format!("Force reapplying to: {}", device.name));
            lg_profile::reapply_profile_with_mode_associations(
                &device.device_key,
                active_profile_path,
                &sdr_profile_path,
                &hdr_profile_path,
                cfg.toggle_delay_ms,
                opts.per_user,
            )?;
            last_applied_profile = Some(active_profile_path.clone());
            log_ok(&format!("SDR/HDR profiles associated for {}", device.name));
            if opts.generic_default {
                lg_profile::set_generic_default(
                    &device.device_key,
                    active_profile_path,
                    opts.per_user,
                )?;
                log_ok(&format!("Generic default set for {}", device.name));
            }
        }
        // DDC/CI brightness (if enabled)
        if cfg.ddc_brightness_on_reapply {
            match lg_monitor::ddc::set_brightness_all(cfg.ddc_brightness_value) {
                Ok(n) => log_ok(&format!(
                    "DDC brightness set to {} on {} monitor(s)",
                    cfg.ddc_brightness_value, n
                )),
                Err(e) => log_note(&format!("DDC brightness failed: {}", e)),
            }
        }
        log_done(&format!(
            "Color profile force-refreshed for {} monitor(s).",
            devices.len()
        ));
        maybe_capture_last_good(
            &cfg,
            last_applied_profile
                .as_deref()
                .ok_or("missing applied profile path after force refresh")?,
            "Force refresh profile",
            if cfg.ddc_brightness_on_reapply {
                Some(cfg.ddc_brightness_value)
            } else {
                None
            },
        );
        app_state::append_diagnostic_event(
            "tui",
            "INFO",
            "apply_success",
            &format!("force refresh applied to {} monitor(s)", devices.len()),
        );
        true
    };
    emit_apply_latency_tui(
        started,
        success,
        &format!(
            "action=force_refresh monitors={}",
            if success { devices.len() } else { 0 }
        ),
    );
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

fn emit_apply_latency_tui(started: std::time::Instant, success: bool, details: &str) {
    let metrics_cfg = app_state::load_automation_config().metrics;
    if !metrics_cfg.enabled || !metrics_cfg.collect_latency {
        return;
    }
    let ms = started.elapsed().as_millis() as u64;
    app_state::append_diagnostic_event(
        "tui",
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

fn action_set_ddc_brightness(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let value = opts.ddc_brightness_value;

    if opts.dry_run {
        log_dry(&format!("Would set DDC brightness to {}", value));
        return Ok(());
    }

    log_info("Reading current brightness levels...");
    match lg_monitor::ddc::get_brightness_all() {
        Ok(infos) if infos.is_empty() => {
            log_skip("No DDC/CI-capable monitors found.");
        }
        Ok(infos) => {
            for info in &infos {
                log_info(&format!(
                    "  {} — current: {}/{} ({}%)",
                    if info.description.is_empty() {
                        "Monitor"
                    } else {
                        &info.description
                    },
                    info.current,
                    info.max,
                    if info.max > 0 {
                        info.current * 100 / info.max
                    } else {
                        0
                    },
                ));
            }
        }
        Err(e) => log_note(&format!("Could not read brightness: {}", e)),
    }

    log_info(&format!("Setting DDC brightness to {}...", value));
    match lg_monitor::ddc::set_brightness_all(value) {
        Ok(0) => log_skip("No monitors responded to DDC brightness set."),
        Ok(n) => log_ok(&format!(
            "DDC brightness set to {} on {} monitor(s)",
            value, n
        )),
        Err(e) => return Err(format!("DDC brightness set failed: {}", e).into()),
    }

    log_done("DDC brightness test complete.");
    Ok(())
}

// ── DDC/CI Studio actions (LG UltraGear only) ────────────────────────────

/// Read a VCP feature using the current DDC target.
/// If a monitor index is selected, reads by index. Otherwise falls back
/// to the config `monitor_match` pattern.
fn ddc_get_vcp(
    target: &Option<(usize, String)>,
    vcp_code: u8,
) -> Result<lg_monitor::ddc::VcpValue, Box<dyn std::error::Error>> {
    match target {
        Some((idx, _)) => lg_monitor::ddc::get_vcp_by_index(*idx, vcp_code),
        None => {
            let pat = Config::load().monitor_match;
            lg_monitor::ddc::get_vcp_by_pattern(&pat, vcp_code)
        }
    }
}

fn wait_for_keep_key_tui(timeout_ms: u64, keep_key: &str) -> io::Result<bool> {
    let expected = keep_key
        .trim()
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
        .unwrap_or('k');
    terminal::enable_raw_mode()?;
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(timeout_ms) {
        if event::poll(std::time::Duration::from_millis(200))? {
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

/// Write a VCP feature using the current DDC target.
/// If a monitor index is selected, writes by index. Otherwise falls back
/// to the config `monitor_match` pattern.
fn ddc_set_vcp(
    target: &Option<(usize, String)>,
    vcp_code: u8,
    value: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let guardrails = app_state::load_ddc_guardrails();
    if let Err(message) = validate_guarded_ddc_write(&guardrails, vcp_code, value) {
        return Err(message.into());
    }
    let auto_cfg = app_state::load_automation_config();
    let risky = app_state::risky_vcp_codes_from_csv(&auto_cfg.ddc_safety.risky_vcp_codes)
        .into_iter()
        .any(|code| code == vcp_code);
    let previous = if risky && auto_cfg.ddc_safety.rollback_timer_enabled {
        ddc_get_vcp(target, vcp_code).ok().map(|v| v.current)
    } else {
        None
    };

    match target {
        Some((idx, _)) => lg_monitor::ddc::set_vcp_by_index(*idx, vcp_code, value),
        None => {
            let pat = Config::load().monitor_match;
            lg_monitor::ddc::set_vcp_by_pattern(&pat, vcp_code, value)
        }
    }?;

    if risky
        && auto_cfg.ddc_safety.rollback_timer_enabled
        && std::io::stdout().is_terminal()
        && previous.is_some()
    {
        let keep_key = if auto_cfg.ddc_safety.keep_key.trim().is_empty() {
            "K".to_string()
        } else {
            auto_cfg.ddc_safety.keep_key.trim().to_string()
        };
        log_warn(&format!(
            "Risky write VCP 0x{:02X}={} sent. Press {} within {}ms to keep, or it rolls back.",
            vcp_code, value, keep_key, auto_cfg.ddc_safety.rollback_timeout_ms
        ));
        let keep = wait_for_keep_key_tui(auto_cfg.ddc_safety.rollback_timeout_ms, &keep_key)
            .unwrap_or(false);
        if !keep {
            if let Some(prev) = previous {
                match target {
                    Some((idx, _)) => {
                        let _ = lg_monitor::ddc::set_vcp_by_index(*idx, vcp_code, prev);
                    }
                    None => {
                        let pat = Config::load().monitor_match;
                        let _ = lg_monitor::ddc::set_vcp_by_pattern(&pat, vcp_code, prev);
                    }
                }
                app_state::append_diagnostic_event(
                    "tui",
                    "WARN",
                    "ddc_rollback",
                    &format!("rolled back 0x{:02X} from {} to {}", vcp_code, value, prev),
                );
                log_warn(&format!(
                    "Rollback applied: VCP 0x{:02X} -> {}",
                    vcp_code, prev
                ));
            }
        } else {
            app_state::append_diagnostic_event(
                "tui",
                "INFO",
                "ddc_rollback_keep",
                &format!("kept risky write 0x{:02X}={}", vcp_code, value),
            );
        }
    }

    Ok(())
}

/// Human-readable label for the current DDC target.
fn ddc_target_label(target: &Option<(usize, String)>) -> String {
    match target {
        Some((idx, name)) => format!("#{} ({})", idx, name),
        None => format!("config default ({})", Config::load().monitor_match),
    }
}

fn action_ddc_vcp_version(
    target: &Option<(usize, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));

    match ddc_get_vcp(target, lg_monitor::ddc::VCP_VERSION) {
        Ok(val) => {
            let major = (val.current >> 8) & 0xFF;
            let minor = val.current & 0xFF;
            log_ok(&format!(
                "VCP Version: {}.{} (raw current={}, max={})",
                major, minor, val.current, val.max
            ));
        }
        Err(e) => log_note(&format!("Could not read VCP version: {}", e)),
    }

    log_done("VCP version check complete.");
    Ok(())
}

fn action_ddc_read_color_preset(
    target: &Option<(usize, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));

    match ddc_get_vcp(target, lg_monitor::ddc::VCP_COLOR_PRESET) {
        Ok(val) => {
            let name = match val.current {
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
            };
            log_ok(&format!(
                "Color Preset: {} (value={}, max={})",
                name, val.current, val.max
            ));
        }
        Err(e) => log_note(&format!("Could not read color preset: {}", e)),
    }

    log_done("Color preset read complete.");
    Ok(())
}

fn action_ddc_read_display_mode(
    target: &Option<(usize, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));

    match ddc_get_vcp(target, lg_monitor::ddc::VCP_DISPLAY_MODE) {
        Ok(val) => {
            log_ok(&format!(
                "Display Mode: current={}, max={} (type={})",
                val.current, val.max, val.vcp_type
            ));
        }
        Err(e) => log_note(&format!("Could not read display mode: {}", e)),
    }

    log_done("Display mode read complete.");
    Ok(())
}

fn action_ddc_read_brightness(
    target: &Option<(usize, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));

    match ddc_get_vcp(target, lg_monitor::ddc::VCP_BRIGHTNESS) {
        Ok(val) => {
            let pct = if val.max > 0 {
                (val.current as f64 / val.max as f64) * 100.0
            } else {
                0.0
            };
            log_ok(&format!(
                "Brightness: current={} max={} ({:.0}%)",
                val.current, val.max, pct
            ));
        }
        Err(e) => log_note(&format!("Could not read brightness: {}", e)),
    }

    log_done("Brightness read complete.");
    Ok(())
}

fn action_ddc_set_brightness(
    target: &Option<(usize, String)>,
    value: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    if value > 100 {
        return Err("brightness must be 0..100".into());
    }

    log_info(&format!("Target: {}", ddc_target_label(target)));
    log_info(&format!("Setting brightness to {}...", value));

    ddc_set_vcp(target, lg_monitor::ddc::VCP_BRIGHTNESS, value)?;
    log_ok("Brightness set.");
    log_done("Brightness write complete.");
    Ok(())
}

fn action_ddc_read_custom_vcp(
    target: &Option<(usize, String)>,
    code: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));
    log_info(&format!("Reading VCP 0x{:02X}...", code));

    match ddc_get_vcp(target, code) {
        Ok(val) => {
            log_ok(&format!(
                "VCP 0x{:02X}: current={} max={} type={}",
                code, val.current, val.max, val.vcp_type
            ));
        }
        Err(e) => log_note(&format!("Could not read VCP 0x{:02X}: {}", code, e)),
    }

    log_done("Custom VCP read complete.");
    Ok(())
}

fn action_ddc_write_custom_vcp(
    target: &Option<(usize, String)>,
    code: u8,
    value: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));
    log_info(&format!("Writing VCP 0x{:02X} = {}...", code, value));

    ddc_set_vcp(target, code, value)?;
    log_ok(&format!("VCP 0x{:02X} written.", code));
    log_done("Custom VCP write complete.");
    Ok(())
}

fn action_ddc_reset_brightness_contrast(
    target: &Option<(usize, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));
    log_info("Sending VCP 0x06 reset (brightness + contrast)...");

    match ddc_set_vcp(target, lg_monitor::ddc::VCP_RESET_BRIGHTNESS_CONTRAST, 1) {
        Ok(()) => log_ok("Brightness + Contrast reset sent"),
        Err(e) => return Err(format!("Reset failed: {}", e).into()),
    }

    log_done("Brightness/Contrast reset complete.");
    Ok(())
}

fn action_ddc_reset_color(
    target: &Option<(usize, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Target: {}", ddc_target_label(target)));
    log_info("Sending VCP 0x0A reset (color)...");

    match ddc_set_vcp(target, lg_monitor::ddc::VCP_RESET_COLOR, 1) {
        Ok(()) => log_ok("Color reset sent"),
        Err(e) => return Err(format!("Reset failed: {}", e).into()),
    }

    log_done("Color reset complete.");
    Ok(())
}

fn action_ddc_list_monitors() -> Result<(), Box<dyn std::error::Error>> {
    log_info("Enumerating physical monitors via DDC/CI...");

    match lg_monitor::ddc::list_physical_monitors() {
        Ok(monitors) if monitors.is_empty() => {
            log_skip("No physical monitors found.");
        }
        Ok(monitors) => {
            for (idx, desc) in &monitors {
                let label = if desc.is_empty() {
                    "(no description)".to_string()
                } else {
                    desc.clone()
                };
                log_info(&format!("  [{}] {}", idx, label));
            }
            log_ok(&format!("{} physical monitor(s) found", monitors.len()));
        }
        Err(e) => return Err(format!("Monitor enumeration failed: {}", e).into()),
    }

    log_done("Monitor list complete.");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "tests/tui_tests.rs"]
mod tests;
