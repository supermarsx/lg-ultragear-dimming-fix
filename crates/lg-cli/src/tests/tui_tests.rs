use super::*;
use std::sync::{Mutex, OnceLock};

fn enable_no_flicker_test_mode() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        std::env::set_var("LG_TEST_NO_FLICKER_REFRESH", "1");
        lg_profile::set_test_no_flicker_mode(true);
    });
}

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
        ddc_brightness: false,
        ddc_brightness_value: 50,
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

fn state_file_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("state-file test lock poisoned")
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
    let _maint2 = Page::Maintenance2;
    let _adv = Page::Advanced;
    let _icc_studio = Page::IccStudio;
    let _icc_studio_tuning = Page::IccStudioTuning;
    let _icc_tags = Page::IccTags;
    let _icc_tags2 = Page::IccTags2;
}

// ── Main menu drawing ────────────────────────────────────────

#[test]
fn draw_main_contains_all_menu_items() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    // 6 numbered install/uninstall items + M, D, I, A, Q keys
    assert!(output.contains("[1]"), "should contain item 1");
    assert!(output.contains("[2]"), "should contain item 2");
    assert!(output.contains("[3]"), "should contain item 3");
    assert!(output.contains("[4]"), "should contain item 4");
    assert!(output.contains("[5]"), "should contain item 5");
    assert!(output.contains("[6]"), "should contain item 6");
    assert!(output.contains("[M]"), "should contain Maintenance key");
    assert!(output.contains("[D]"), "should contain DDC/CI Studio key");
    assert!(output.contains("[I]"), "should contain ICC Studio key");
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
        ddc_brightness: false,
        ddc_brightness_value: 50,
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
        ddc_brightness: true,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
    // ON: dry_run, verbose, hdr, sdr, ddc_brightness = 5 ON; OFF: toast, per_user, generic_default = 3 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(
        on_count, 5,
        "dry_run+verbose+hdr+sdr+ddc_brightness should be ON"
    );
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
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
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
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("PROFILE"));
}

#[test]
fn draw_maintenance_diagnostics_section() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("DIAGNOSTICS"));
}

#[test]
fn draw_maintenance_force_refresh_section() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("FORCE REFRESH"));
}

#[test]
fn draw_maintenance_navigation_section() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("NAVIGATION"));
    assert!(output.contains("Back to Main Menu"));
    assert!(output.contains("Quit"));
    assert!(output.contains("[N]"), "should have Next Page key");
    assert!(
        output.contains("DDC/CI Studio"),
        "should mention DDC/CI Studio"
    );
}

#[test]
fn draw_maintenance_item_labels() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("Refresh"), "should have Refresh");
    assert!(output.contains("Reinstall"), "should have Reinstall");
    assert!(output.contains("Detect Monitors"), "should have Detect");
    assert!(
        output.contains("Service Diagnostics"),
        "should have Service Status"
    );
    assert!(
        output.contains("Recheck Service"),
        "should have Recheck Service"
    );
    assert!(
        output.contains("Check Applicability + Conflict Detector"),
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
    assert!(
        output.contains("Safe Recovery Rollback"),
        "should have Safe Recovery"
    );
}

#[test]
fn draw_maintenance_title() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("MAINTENANCE"));
}

#[test]
fn draw_maintenance_produces_nonempty_output() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(!output.is_empty());
    assert!(
        output.len() > 300,
        "maintenance menu should produce substantial output"
    );
}

#[test]
fn draw_maintenance_contains_box_drawing_chars() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains('\u{2554}'), "top-left corner");
    assert!(output.contains('\u{2557}'), "top-right corner");
    assert!(output.contains('\u{255A}'), "bottom-left corner");
    assert!(output.contains('\u{255D}'), "bottom-right corner");
    assert!(output.contains('\u{2551}'), "vertical line");
}

#[test]
fn draw_maintenance_select_option_prompt() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("Select option"));
}

#[test]
fn draw_maintenance_all_status_combos() {
    for profile in [false, true] {
        for svc_installed in [false, true] {
            for svc_running in [false, true] {
                for count in [0, 1, 5] {
                    let s = test_status(profile, svc_installed, svc_running, count);
                    let output = render_to_string(|buf| draw_maintenance(buf, &s, &default_opts()));
                    assert!(!output.is_empty());
                }
            }
        }
    }
}

#[test]
fn draw_maintenance_with_all_good_status() {
    let output = render_to_string(|buf| draw_maintenance(buf, &all_good_status(), &default_opts()));
    assert!(output.contains("Running"));
}

#[test]
fn draw_maintenance_contains_ddc_section() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("DDC/CI"), "should contain DDC/CI section");
    assert!(output.contains("[0]"), "should contain item 0 for DDC test");
    assert!(
        output.contains("Set DDC Brightness"),
        "should have DDC brightness label"
    );
}

// ── Maintenance Page 2 (DDC/CI Studio) drawing ────────────

#[test]
fn draw_maintenance2_contains_all_items() {
    let output =
        render_to_string(|buf| draw_maintenance2(buf, &default_status(), &default_opts(), None));
    assert!(output.contains("[1]"), "item 1 — VCP version");
    assert!(output.contains("[2]"), "item 2 — color preset read");
    assert!(output.contains("[3]"), "item 3 — color preset cycle");
    assert!(output.contains("[4]"), "item 4 — display mode read");
    assert!(output.contains("[5]"), "item 5 — display mode cycle");
    assert!(output.contains("[6]"), "item 6 — reset brightness+contrast");
    assert!(output.contains("[7]"), "item 7 — reset color");
    assert!(output.contains("[8]"), "item 8 — list monitors");
    assert!(output.contains("[9]"), "item 9 — cycle target");
    assert!(output.contains("[0]"), "item 0 — reset target");
    assert!(output.contains("[A]"), "item A — read brightness");
    assert!(output.contains("[B]"), "item B — set brightness");
    assert!(output.contains("[C]"), "item C — read custom VCP");
    assert!(output.contains("[D]"), "item D — write custom VCP");
    assert!(output.contains("[E]"), "item E — guardrails");
    assert!(output.contains("[P]"), "prev page key");
    assert!(output.contains("[Z]"), "back key");
    assert!(output.contains("[Q]"), "quit key");
}

#[test]
fn draw_maintenance2_title() {
    let output =
        render_to_string(|buf| draw_maintenance2(buf, &default_status(), &default_opts(), None));
    assert!(
        output.contains("DDC/CI STUDIO"),
        "should show DDC/CI STUDIO title"
    );
}

#[test]
fn draw_maintenance2_sections() {
    let output =
        render_to_string(|buf| draw_maintenance2(buf, &default_status(), &default_opts(), None));
    assert!(output.contains("READ"), "should have READ section");
    assert!(output.contains("WRITE"), "should have WRITE section");
    assert!(output.contains("RESET"), "should have RESET section");
    assert!(output.contains("INFO"), "should have INFO section");
    assert!(output.contains("TARGET"), "should have TARGET section");
    assert!(
        output.contains("NAVIGATION"),
        "should have NAVIGATION section"
    );
}

#[test]
fn draw_maintenance2_default_target() {
    let output =
        render_to_string(|buf| draw_maintenance2(buf, &default_status(), &default_opts(), None));
    assert!(
        output.contains("config default"),
        "should show config default when no target selected"
    );
}

#[test]
fn draw_maintenance2_specific_target() {
    let target = (0usize, "LG ULTRAGEAR".to_string());
    let output = render_to_string(|buf| {
        draw_maintenance2(buf, &default_status(), &default_opts(), Some(&target))
    });
    assert!(
        output.contains("LG ULTRAGEAR"),
        "should show specific target name"
    );
    assert!(output.contains("#0"), "should show monitor index");
}

#[test]
fn draw_maintenance2_item_labels() {
    let output =
        render_to_string(|buf| draw_maintenance2(buf, &default_status(), &default_opts(), None));
    assert!(output.contains("VCP Version"), "should have VCP Version");
    assert!(output.contains("Color Preset"), "should have Color Preset");
    assert!(output.contains("Display Mode"), "should have Display Mode");
    assert!(
        output.contains("Reset Brightness"),
        "should have Reset Brightness"
    );
    assert!(output.contains("Reset Color"), "should have Reset Color");
    assert!(
        output.contains("List Physical Monitors"),
        "should have List Monitors"
    );
    assert!(
        output.contains("Read Brightness"),
        "should have Read Brightness"
    );
    assert!(
        output.contains("Set Brightness"),
        "should have Set Brightness"
    );
    assert!(
        output.contains("Read Custom VCP"),
        "should have Read Custom VCP"
    );
    assert!(
        output.contains("Write Custom VCP"),
        "should have Write Custom VCP"
    );
    assert!(
        output.contains("DDC Guardrails"),
        "should have DDC Guardrails"
    );
    assert!(
        output.contains("Select Target"),
        "should have Select Target"
    );
    assert!(output.contains("Reset Target"), "should have Reset Target");
}

#[test]
fn draw_maintenance2_navigation() {
    let output =
        render_to_string(|buf| draw_maintenance2(buf, &default_status(), &default_opts(), None));
    assert!(
        output.contains("Previous Page"),
        "should have Previous Page"
    );
    assert!(output.contains("Back to Main Menu"), "should have Back");
    assert!(output.contains("Quit"), "should have Quit");
}

#[test]
fn draw_maintenance2_produces_nonempty_output() {
    let output =
        render_to_string(|buf| draw_maintenance2(buf, &default_status(), &default_opts(), None));
    assert!(!output.is_empty());
    assert!(
        output.len() > 200,
        "DDC/CI Studio page should produce substantial output"
    );
}

#[test]
fn draw_maintenance2_all_status_combos() {
    for profile in [false, true] {
        for svc_installed in [false, true] {
            for svc_running in [false, true] {
                let s = test_status(profile, svc_installed, svc_running, 1);
                let output =
                    render_to_string(|buf| draw_maintenance2(buf, &s, &default_opts(), None));
                assert!(!output.is_empty());
            }
        }
    }
}

#[test]
fn draw_advanced_contains_ddc_section() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(
        output.contains("DDC/CI BRIGHTNESS"),
        "should contain DDC section"
    );
    assert!(output.contains("[8]"), "toggle 8 for DDC auto");
    assert!(output.contains("[9]"), "item 9 for brightness value");
    assert!(
        output.contains("Auto-Set Brightness"),
        "should have auto label"
    );
    assert!(
        output.contains("Brightness Value"),
        "should have value label"
    );
}

#[test]
fn options_default_ddc_brightness_is_false() {
    let opts = Options::default();
    assert!(!opts.ddc_brightness);
}

#[test]
fn options_default_ddc_brightness_value_is_50() {
    let opts = Options::default();
    assert_eq!(opts.ddc_brightness_value, 50);
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
    let output = render_to_string(|buf| draw_advanced(buf, &all_good_status(), &default_opts()));
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
        ddc_brightness: false,
        ddc_brightness_value: 50,
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
    assert_eq!(
        active,
        vec![
            "NoToast",
            "DryRun",
            "Verbose",
            "NoHDR",
            "NoSDR",
            "PerUser",
            "GenericDef"
        ]
    );
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
                            ddc_brightness: false,
                            ddc_brightness_value: 50,
                        };
                        let output =
                            render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
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
    assert!(output.contains("[OFF]"), "toast should be OFF after toggle");
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
    // dry_run ON, toast ON, sdr ON = 3 ON; verbose OFF, hdr OFF, per_user OFF, generic_default OFF, ddc_brightness OFF = 5 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(on_count, 3, "toast+dry_run+sdr ON");
    assert_eq!(
        off_count, 5,
        "verbose+hdr+per_user+generic_default+ddc_brightness OFF"
    );
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
    // With hdr=true: toast ON, dry_run OFF, verbose OFF, hdr ON, sdr ON, per_user OFF, generic_default OFF, ddc_brightness OFF → 3 ON, 5 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(on_count, 3);
    assert_eq!(off_count, 5);
}

#[test]
fn toggle_sdr_flips_correctly() {
    let mut opts = default_opts();
    assert!(opts.sdr, "SDR should default ON");
    opts.sdr = !opts.sdr;
    assert!(!opts.sdr);
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
    // With sdr=false: toast ON, dry_run OFF, verbose OFF, hdr OFF, sdr OFF, per_user OFF, generic_default OFF, ddc_brightness OFF → 1 ON, 7 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(on_count, 1);
    assert_eq!(off_count, 7);
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
    // Default: toast=ON, dry_run=OFF, verbose=OFF, hdr=OFF, sdr=ON, per_user=OFF, generic_default=OFF, ddc_brightness=OFF → 2 ON, 6 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(on_count, 2, "toast+sdr should be ON");
    assert_eq!(
        off_count, 6,
        "dry_run+verbose+hdr+per_user+generic_default+ddc_brightness should be OFF"
    );
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
        ddc_brightness: false,
        ddc_brightness_value: 50,
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
        ddc_brightness: false,
        ddc_brightness_value: 50,
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
        ddc_brightness: false,
        ddc_brightness_value: 50,
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
    assert!(output.contains("\x1b[0m"), "should reset color after tag");
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

// ================================================================
// TUI ITEM EXISTENCE — Exhaustive checks for every menu item
// ================================================================

// ── Main menu: every numbered item, M, A, Q ──────────────────

#[test]
fn main_menu_has_item_1_default_install() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[1]"), "main menu missing [1]");
    assert!(
        output.contains("Default Install (Profile + Service)"),
        "main menu missing Default Install label"
    );
}

#[test]
fn main_menu_has_item_2_profile_only() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[2]"), "main menu missing [2]");
    assert!(
        output.contains("Profile Only (Install ICC without service)"),
        "main menu missing Profile Only label"
    );
}

#[test]
fn main_menu_has_item_3_service_only() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[3]"), "main menu missing [3]");
    assert!(
        output.contains("Service Only (Install service only)"),
        "main menu missing Service Only label"
    );
}

#[test]
fn main_menu_has_item_4_remove_service() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[4]"), "main menu missing [4]");
    assert!(
        output.contains("Remove Service (Keep profile)"),
        "main menu missing Remove Service label"
    );
}

#[test]
fn main_menu_has_item_5_remove_profile() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[5]"), "main menu missing [5]");
    assert!(
        output.contains("Remove Profile Only"),
        "main menu missing Remove Profile Only label"
    );
}

#[test]
fn main_menu_has_item_6_full_uninstall() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[6]"), "main menu missing [6]");
    assert!(
        output.contains("Full Uninstall (Remove everything)"),
        "main menu missing Full Uninstall label"
    );
}

#[test]
fn main_menu_has_item_m_maintenance() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[M]"), "main menu missing [M]");
    assert!(
        output.contains("Maintenance (Diagnostics & refresh tools)"),
        "main menu missing Maintenance label"
    );
}

#[test]
fn main_menu_has_item_d_ddc_studio() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[D]"), "main menu missing [D]");
    assert!(
        output.contains("DDC/CI Studio (Direct monitor controls)"),
        "main menu missing DDC/CI Studio label"
    );
}

#[test]
fn main_menu_has_item_i_icc_studio() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[I]"), "main menu missing [I]");
    assert!(
        output.contains("ICC Studio (Presets, tuning, and on-the-fly optimize)"),
        "main menu missing ICC Studio label"
    );
}

#[test]
fn main_menu_has_item_a_advanced() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[A]"), "main menu missing [A]");
    assert!(
        output.contains("Advanced Options"),
        "main menu missing Advanced Options label"
    );
}

#[test]
fn main_menu_has_item_q_quit() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(output.contains("[Q]"), "main menu missing [Q]");
    assert!(output.contains("Quit"), "main menu missing Quit label");
}

#[test]
fn main_menu_has_all_sections() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(
        output.contains("INSTALL OPTIONS"),
        "missing INSTALL OPTIONS"
    );
    assert!(output.contains("UNINSTALL"), "missing UNINSTALL");
    assert!(output.contains("MORE"), "missing MORE");
}

#[test]
fn main_menu_has_select_option_prompt() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    assert!(
        output.contains("Select option:"),
        "main menu missing 'Select option:' prompt"
    );
}

#[test]
fn main_menu_total_bracketed_items_count() {
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &default_opts()));
    // Items: [1], [2], [3], [4], [5], [6], [M], [D], [I], [A], [Q] = 11 items
    let count = output.matches("[1]").count()
        + output.matches("[2]").count()
        + output.matches("[3]").count()
        + output.matches("[4]").count()
        + output.matches("[5]").count()
        + output.matches("[6]").count()
        + output.matches("[M]").count()
        + output.matches("[D]").count()
        + output.matches("[I]").count()
        + output.matches("[A]").count()
        + output.matches("[Q]").count();
    assert_eq!(
        count, 11,
        "main menu should have exactly 11 bracketed items"
    );
}

// ── Maintenance menu: every numbered item 1-9, B, Q ──────────

#[test]
fn maintenance_menu_has_item_1_refresh() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[1]"), "maintenance missing [1]");
    assert!(
        output.contains("Refresh (Re-apply profile now)"),
        "maintenance missing Refresh label"
    );
}

#[test]
fn maintenance_menu_has_item_2_reinstall() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[2]"), "maintenance missing [2]");
    assert!(
        output.contains("Reinstall (Clean reinstall everything)"),
        "maintenance missing Reinstall label"
    );
}

#[test]
fn maintenance_menu_has_item_3_detect() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[3]"), "maintenance missing [3]");
    assert!(
        output.contains("Detect Monitors"),
        "maintenance missing Detect Monitors label"
    );
}

#[test]
fn maintenance_menu_has_item_4_service_status() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[4]"), "maintenance missing [4]");
    assert!(
        output.contains("Service Diagnostics"),
        "maintenance missing Service Diagnostics label"
    );
}

#[test]
fn maintenance_menu_has_item_5_recheck() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[5]"), "maintenance missing [5]");
    assert!(
        output.contains("Recheck Service (Stop + Start)"),
        "maintenance missing Recheck Service label"
    );
}

#[test]
fn maintenance_menu_has_item_6_applicability() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[6]"), "maintenance missing [6]");
    assert!(
        output.contains("Check Applicability + Conflict Detector"),
        "maintenance missing Check Applicability label"
    );
}

#[test]
fn maintenance_menu_has_item_7_test_toast() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[7]"), "maintenance missing [7]");
    assert!(
        output.contains("Test Toast Notification"),
        "maintenance missing Test Toast Notification label"
    );
}

#[test]
fn maintenance_menu_has_item_8_force_profile() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[8]"), "maintenance missing [8]");
    assert!(
        output.contains("Force Refresh Color Profile"),
        "maintenance missing Force Refresh Color Profile label"
    );
}

#[test]
fn maintenance_menu_has_item_9_force_color_mgmt() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[9]"), "maintenance missing [9]");
    assert!(
        output.contains("Force Refresh Color Management"),
        "maintenance missing Force Refresh Color Management label"
    );
}

#[test]
fn maintenance_menu_has_item_a_safe_recovery() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[A]"), "maintenance missing [A]");
    assert!(
        output.contains("Safe Recovery Rollback"),
        "maintenance missing Safe Recovery label"
    );
}

#[test]
fn maintenance_menu_has_item_b_back() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[B]"), "maintenance missing [B]");
    assert!(
        output.contains("Back to Main Menu"),
        "maintenance missing Back label"
    );
}

#[test]
fn maintenance_menu_has_item_q_quit() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("[Q]"), "maintenance missing [Q]");
    assert!(output.contains("Quit"), "maintenance missing Quit");
}

#[test]
fn maintenance_menu_has_all_sections() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    assert!(output.contains("PROFILE"), "missing PROFILE section");
    assert!(
        output.contains("DIAGNOSTICS"),
        "missing DIAGNOSTICS section"
    );
    assert!(
        output.contains("FORCE REFRESH"),
        "missing FORCE REFRESH section"
    );
    assert!(output.contains("NAVIGATION"), "missing NAVIGATION section");
}

#[test]
fn maintenance_menu_total_bracketed_items_count() {
    let output = render_to_string(|buf| draw_maintenance(buf, &default_status(), &default_opts()));
    // [1]-[9], [B], [Q] = 11
    let count = output.matches("[1]").count()
        + output.matches("[2]").count()
        + output.matches("[3]").count()
        + output.matches("[4]").count()
        + output.matches("[5]").count()
        + output.matches("[6]").count()
        + output.matches("[7]").count()
        + output.matches("[8]").count()
        + output.matches("[9]").count()
        + output.matches("[B]").count()
        + output.matches("[Q]").count();
    assert_eq!(
        count, 11,
        "maintenance menu should have exactly 11 bracketed items"
    );
}

// ── Advanced menu: every numbered item 1-7, B, Q ─────────────

#[test]
fn advanced_menu_has_item_1_toast() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[1]"), "advanced missing [1]");
    assert!(
        output.contains("Toast Notifications (Show reapply alerts)"),
        "advanced missing Toast label"
    );
}

#[test]
fn advanced_menu_has_item_2_dry_run() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[2]"), "advanced missing [2]");
    assert!(
        output.contains("Dry Run (Simulate without changes)"),
        "advanced missing Dry Run label"
    );
}

#[test]
fn advanced_menu_has_item_3_verbose() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[3]"), "advanced missing [3]");
    assert!(
        output.contains("Verbose Logging (Detailed output)"),
        "advanced missing Verbose label"
    );
}

#[test]
fn advanced_menu_has_item_4_hdr() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[4]"), "advanced missing [4]");
    assert!(
        output.contains("HDR Mode (Advanced color association)"),
        "advanced missing HDR Mode label"
    );
}

#[test]
fn advanced_menu_has_item_5_sdr() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[5]"), "advanced missing [5]");
    assert!(
        output.contains("SDR Mode (Standard color association)"),
        "advanced missing SDR Mode label"
    );
}

#[test]
fn advanced_menu_has_item_6_per_user() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[6]"), "advanced missing [6]");
    assert!(
        output.contains("Per-User Install (User scope, not system)"),
        "advanced missing Per-User Install label"
    );
}

#[test]
fn advanced_menu_has_item_7_generic_default() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[7]"), "advanced missing [7]");
    assert!(
        output.contains("Generic Default (Legacy default profile API)"),
        "advanced missing Generic Default label"
    );
}

#[test]
fn advanced_menu_has_item_b_back() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[B]"), "advanced missing [B]");
    assert!(
        output.contains("Back to Main Menu"),
        "advanced missing Back label"
    );
}

#[test]
fn advanced_menu_has_item_q_quit() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(output.contains("[Q]"), "advanced missing [Q]");
    assert!(output.contains("Quit"), "advanced missing Quit");
}

#[test]
fn advanced_menu_has_all_sections() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(
        output.contains("NOTIFICATIONS"),
        "missing NOTIFICATIONS section"
    );
    assert!(output.contains("TESTING"), "missing TESTING section");
    assert!(output.contains("COLOR MODE"), "missing COLOR MODE section");
    assert!(
        output.contains("INSTALL MODE"),
        "missing INSTALL MODE section"
    );
    assert!(output.contains("NAVIGATION"), "missing NAVIGATION section");
}

#[test]
fn advanced_menu_total_bracketed_items_count() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    // [1]-[7], [B], [Q] = 9
    let count = output.matches("[1]").count()
        + output.matches("[2]").count()
        + output.matches("[3]").count()
        + output.matches("[4]").count()
        + output.matches("[5]").count()
        + output.matches("[6]").count()
        + output.matches("[7]").count()
        + output.matches("[B]").count()
        + output.matches("[Q]").count();
    assert_eq!(
        count, 9,
        "advanced menu should have exactly 9 bracketed items"
    );
}

#[test]
fn advanced_menu_info_text_present() {
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &default_opts()));
    assert!(
        output.contains("These toggles affect main menu install options"),
        "advanced missing info text"
    );
}

// ================================================================
// TOGGLE EDGE CASES — all 7 toggles exhaustive combinations
// ================================================================

#[test]
fn advanced_all_7_toggles_off_shows_7_off_markers() {
    let opts = Options {
        toast: false,
        dry_run: false,
        verbose: false,
        hdr: false,
        sdr: false,
        per_user: false,
        generic_default: false,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
    assert_eq!(
        output.matches("[OFF]").count(),
        8,
        "all toggles OFF should show 8 [OFF] markers"
    );
    assert_eq!(
        output.matches("[ON ]").count(),
        0,
        "all toggles OFF should show 0 [ON ] markers"
    );
}

#[test]
fn advanced_all_7_toggles_on_shows_7_on_markers() {
    let opts = Options {
        toast: true,
        dry_run: true,
        verbose: true,
        hdr: true,
        sdr: true,
        per_user: true,
        generic_default: true,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
    assert_eq!(
        output.matches("[ON ]").count(),
        7,
        "all toggles ON should show 7 [ON ] markers (ddc_brightness is false)"
    );
    assert_eq!(
        output.matches("[OFF]").count(),
        1,
        "all toggles ON should show 1 [OFF] marker (ddc_brightness)"
    );
}

#[test]
fn advanced_only_per_user_on() {
    let mut opts = default_opts();
    opts.per_user = true;
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
    // toast=ON, sdr=ON, per_user=ON → 3 ON; dry_run OFF, verbose OFF, hdr OFF, generic_default OFF, ddc_brightness OFF → 5 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(on_count, 3, "per_user ON only: expected 3 ON markers");
    assert_eq!(off_count, 5, "per_user ON only: expected 5 OFF markers");
}

#[test]
fn advanced_only_generic_default_on() {
    let mut opts = default_opts();
    opts.generic_default = true;
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
    // toast=ON, sdr=ON, generic_default=ON → 3 ON; dry_run OFF, verbose OFF, hdr OFF, per_user OFF, ddc_brightness OFF → 5 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(on_count, 3, "generic_default ON: expected 3 ON markers");
    assert_eq!(off_count, 5, "generic_default ON: expected 5 OFF markers");
}

#[test]
fn advanced_both_install_mode_toggles_on() {
    let mut opts = default_opts();
    opts.per_user = true;
    opts.generic_default = true;
    let output = render_to_string(|buf| draw_advanced(buf, &default_status(), &opts));
    // toast=ON, sdr=ON, per_user=ON, generic_default=ON → 4 ON; dry_run OFF, verbose OFF, hdr OFF, ddc_brightness OFF → 4 OFF
    let on_count = output.matches("[ON ]").count();
    let off_count = output.matches("[OFF]").count();
    assert_eq!(on_count, 4, "both install mode: expected 4 ON markers");
    assert_eq!(off_count, 4, "both install mode: expected 4 OFF markers");
}

#[test]
fn draw_advanced_all_128_toggle_combos() {
    // Exhaustive: iterate all 2^7 = 128 combinations of the 7 toggles
    let status = default_status();
    for bits in 0u8..128 {
        let opts = Options {
            toast: bits & 1 != 0,
            dry_run: bits & 2 != 0,
            verbose: bits & 4 != 0,
            hdr: bits & 8 != 0,
            sdr: bits & 16 != 0,
            per_user: bits & 32 != 0,
            generic_default: bits & 64 != 0,
            ddc_brightness: false,
            ddc_brightness_value: 50,
        };
        let output = render_to_string(|buf| draw_advanced(buf, &status, &opts));
        let on_count = output.matches("[ON ]").count();
        let off_count = output.matches("[OFF]").count();
        assert_eq!(
            on_count + off_count,
            8,
            "combo {:07b}: expected 8 total toggles, got {} ON + {} OFF",
            bits,
            on_count,
            off_count
        );
        let expected_on = (bits as u32).count_ones() as usize;
        assert_eq!(
            on_count, expected_on,
            "combo {:07b}: expected {} ON markers, got {}",
            bits, expected_on, on_count
        );
    }
}

// ================================================================
// MAIN MENU — Active toggles edge cases
// ================================================================

#[test]
fn main_active_toggles_all_seven_active() {
    let opts = Options {
        toast: false, // NoToast appears when toast=false
        dry_run: true,
        verbose: true,
        hdr: false, // NoHDR appears when hdr=false
        sdr: false, // NoSDR appears when sdr=false
        per_user: true,
        generic_default: true,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
    assert!(output.contains("NoToast"), "missing NoToast");
    assert!(output.contains("DryRun"), "missing DryRun");
    assert!(output.contains("Verbose"), "missing Verbose");
    assert!(output.contains("NoHDR"), "missing NoHDR");
    assert!(output.contains("NoSDR"), "missing NoSDR");
    assert!(output.contains("PerUser"), "missing PerUser");
    assert!(output.contains("GenericDef"), "missing GenericDef");
}

#[test]
fn main_active_toggles_none_shows_none_active() {
    // Default: toast=true, dry_run=false, verbose=false, hdr=false, sdr=true,
    // per_user=false, generic_default=false
    // Active toggles: NoHDR (hdr=false counts as active)
    // Actually let's create "no active" state:
    let opts = Options {
        toast: true,
        dry_run: false,
        verbose: false,
        hdr: true, // hdr=true → not active
        sdr: true,
        per_user: false,
        generic_default: false,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
    assert!(output.contains("None active"), "should show (None active)");
}

#[test]
fn main_active_per_user_only() {
    let opts = Options {
        toast: true,
        dry_run: false,
        verbose: false,
        hdr: true,
        sdr: true,
        per_user: true,
        generic_default: false,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
    assert!(output.contains("PerUser"), "should show PerUser");
    assert!(!output.contains("GenericDef"), "should NOT show GenericDef");
    assert!(
        !output.contains("None active"),
        "should NOT show None active"
    );
}

#[test]
fn main_active_generic_def_only() {
    let opts = Options {
        toast: true,
        dry_run: false,
        verbose: false,
        hdr: true,
        sdr: true,
        per_user: false,
        generic_default: true,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
    assert!(output.contains("GenericDef"), "should show GenericDef");
    assert!(!output.contains("PerUser"), "should NOT show PerUser");
}

#[test]
fn main_active_both_install_mode_toggles() {
    let opts = Options {
        toast: true,
        dry_run: false,
        verbose: false,
        hdr: true,
        sdr: true,
        per_user: true,
        generic_default: true,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let output = render_to_string(|buf| draw_main(buf, &default_status(), &opts));
    assert!(output.contains("PerUser"), "should show PerUser");
    assert!(output.contains("GenericDef"), "should show GenericDef");
}

#[test]
fn main_all_128_active_toggle_combos() {
    let status = default_status();
    for bits in 0u8..128 {
        let opts = Options {
            toast: bits & 1 != 0,
            dry_run: bits & 2 != 0,
            verbose: bits & 4 != 0,
            hdr: bits & 8 != 0,
            sdr: bits & 16 != 0,
            per_user: bits & 32 != 0,
            generic_default: bits & 64 != 0,
            ddc_brightness: false,
            ddc_brightness_value: 50,
        };
        let output = render_to_string(|buf| draw_main(buf, &status, &opts));
        // Should always contain [A] and Advanced Options
        assert!(output.contains("[A]"), "combo {:07b}: missing [A]", bits);
        assert!(
            output.contains("Advanced Options"),
            "combo {:07b}: missing Advanced Options label",
            bits
        );

        // Count active toggles
        let mut expected_active = 0;
        if bits & 1 == 0 {
            expected_active += 1;
        } // NoToast
        if bits & 2 != 0 {
            expected_active += 1;
        } // DryRun
        if bits & 4 != 0 {
            expected_active += 1;
        } // Verbose
        if bits & 8 == 0 {
            expected_active += 1;
        } // NoHDR
        if bits & 16 == 0 {
            expected_active += 1;
        } // NoSDR
        if bits & 32 != 0 {
            expected_active += 1;
        } // PerUser
        if bits & 64 != 0 {
            expected_active += 1;
        } // GenericDef

        if expected_active == 0 {
            assert!(
                output.contains("None active"),
                "combo {:07b}: should show None active",
                bits
            );
        } else {
            assert!(
                !output.contains("None active"),
                "combo {:07b}: should NOT show None active",
                bits
            );
        }
    }
}

// ================================================================
// HEADER EDGE CASES — status combinations
// ================================================================

#[test]
fn header_all_status_false_shows_not_installed() {
    let status = test_status(false, false, false, 0);
    let output = render_to_string(|buf| draw_header(buf, &status));
    assert!(
        output.contains("Not Installed"),
        "should show Not Installed for profile"
    );
    assert!(
        output.contains("None detected"),
        "should show None detected"
    );
}

#[test]
fn header_service_installed_not_running_shows_stopped() {
    let status = test_status(false, true, false, 0);
    let output = render_to_string(|buf| draw_header(buf, &status));
    assert!(
        output.contains("Stopped"),
        "should show Stopped for service"
    );
}

#[test]
fn header_service_installed_and_running_shows_running() {
    let status = test_status(false, true, true, 0);
    let output = render_to_string(|buf| draw_header(buf, &status));
    assert!(output.contains("Running"), "should show Running");
}

#[test]
fn header_multiple_monitors() {
    let status = test_status(true, true, true, 5);
    let output = render_to_string(|buf| draw_header(buf, &status));
    assert!(
        output.contains("5 monitor(s) detected"),
        "should show 5 monitors: {}",
        output
    );
}

#[test]
fn header_hdr_and_sdr_status_reflect_status_struct() {
    let mut status = default_status();
    status.hdr_enabled = true;
    status.sdr_enabled = false;
    let output = render_to_string(|buf| draw_header(buf, &status));
    // HDR should show Enabled, SDR should show Disabled
    // Both labels are in the output
    assert!(output.contains("HDR Mode:"), "should have HDR Mode label");
    assert!(output.contains("SDR Mode:"), "should have SDR Mode label");
}

// ================================================================
// OPTIONS STRUCT — boundary and default edge cases
// ================================================================

#[test]
fn options_default_has_correct_defaults() {
    let opts = default_opts();
    assert!(opts.toast, "toast default should be true");
    assert!(!opts.dry_run, "dry_run default should be false");
    assert!(!opts.verbose, "verbose default should be false");
    assert!(!opts.hdr, "hdr default should be false");
    assert!(opts.sdr, "sdr default should be true");
    assert!(!opts.per_user, "per_user default should be false");
    assert!(
        !opts.generic_default,
        "generic_default default should be false"
    );
}

#[test]
fn options_all_fields_are_independent() {
    let mut opts = Options {
        toast: false,
        dry_run: false,
        verbose: false,
        hdr: false,
        sdr: false,
        per_user: false,
        generic_default: false,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    // Toggle each field independently and verify no side effects
    opts.toast = true;
    assert!(opts.toast);
    assert!(!opts.dry_run);
    assert!(!opts.per_user);

    opts.per_user = true;
    assert!(opts.toast);
    assert!(opts.per_user);
    assert!(!opts.generic_default);

    opts.generic_default = true;
    assert!(opts.toast);
    assert!(opts.per_user);
    assert!(opts.generic_default);
    assert!(!opts.dry_run);
}

#[test]
fn options_toggle_roundtrip_all_fields() {
    let mut opts = default_opts();
    let original = Options {
        toast: opts.toast,
        dry_run: opts.dry_run,
        verbose: opts.verbose,
        hdr: opts.hdr,
        sdr: opts.sdr,
        per_user: opts.per_user,
        generic_default: opts.generic_default,
        ddc_brightness: opts.ddc_brightness,
        ddc_brightness_value: opts.ddc_brightness_value,
    };

    // Toggle all fields
    opts.toast = !opts.toast;
    opts.dry_run = !opts.dry_run;
    opts.verbose = !opts.verbose;
    opts.hdr = !opts.hdr;
    opts.sdr = !opts.sdr;
    opts.per_user = !opts.per_user;
    opts.generic_default = !opts.generic_default;

    // All should be opposite now
    assert_ne!(opts.toast, original.toast);
    assert_ne!(opts.dry_run, original.dry_run);
    assert_ne!(opts.verbose, original.verbose);
    assert_ne!(opts.hdr, original.hdr);
    assert_ne!(opts.sdr, original.sdr);
    assert_ne!(opts.per_user, original.per_user);
    assert_ne!(opts.generic_default, original.generic_default);

    // Toggle all back
    opts.toast = !opts.toast;
    opts.dry_run = !opts.dry_run;
    opts.verbose = !opts.verbose;
    opts.hdr = !opts.hdr;
    opts.sdr = !opts.sdr;
    opts.per_user = !opts.per_user;
    opts.generic_default = !opts.generic_default;

    // All should match original
    assert_eq!(opts.toast, original.toast);
    assert_eq!(opts.dry_run, original.dry_run);
    assert_eq!(opts.verbose, original.verbose);
    assert_eq!(opts.hdr, original.hdr);
    assert_eq!(opts.sdr, original.sdr);
    assert_eq!(opts.per_user, original.per_user);
    assert_eq!(opts.generic_default, original.generic_default);
}

// ================================================================
// STATUS STRUCT — edge cases
// ================================================================

#[test]
fn status_service_running_without_installed_is_representable() {
    // Although logically nonsensical, the struct allows it
    let s = Status {
        profile_installed: false,
        service_installed: false,
        service_running: true,
        monitor_count: 0,
        hdr_enabled: false,
        sdr_enabled: false,
    };
    assert!(s.service_running);
    assert!(!s.service_installed);
}

#[test]
fn status_large_monitor_count() {
    let s = test_status(true, true, true, 99);
    let output = render_to_string(|buf| draw_header(buf, &s));
    assert!(
        output.contains("99 monitor(s) detected"),
        "should handle large monitor count"
    );
}

#[test]
fn status_zero_monitors_with_all_else_good() {
    let s = test_status(true, true, true, 0);
    let output = render_to_string(|buf| draw_header(buf, &s));
    // Profile installed, service running, but no monitors
    assert!(
        output.contains("Installed"),
        "profile should show Installed"
    );
    assert!(output.contains("Running"), "service should show Running");
    assert!(
        output.contains("None detected"),
        "monitors should show None"
    );
}

// ================================================================
// PAGE ENUM — edge cases
// ================================================================

#[test]
fn page_main_is_default_start_page() {
    let page = Page::Main;
    matches!(page, Page::Main);
}

#[test]
fn page_variants_are_distinct() {
    assert!(
        !matches!(Page::Main, Page::Maintenance),
        "Main should not match Maintenance"
    );
    assert!(
        !matches!(Page::Main, Page::Advanced),
        "Main should not match Advanced"
    );
    assert!(
        !matches!(Page::Maintenance, Page::Advanced),
        "Maintenance should not match Advanced"
    );
}

// ================================================================
// GOODBYE SCREEN — content checks
// ================================================================

#[test]
fn goodbye_contains_thank_you() {
    let output = render_to_string(draw_goodbye);
    assert!(
        output.contains("Thank you"),
        "goodbye should contain Thank you"
    );
}

#[test]
fn goodbye_contains_repo_link() {
    let output = render_to_string(draw_goodbye);
    assert!(
        output.contains("github.com"),
        "goodbye should contain repo link"
    );
}

#[test]
fn goodbye_has_box_drawing_characters() {
    let output = render_to_string(draw_goodbye);
    assert!(
        output.contains('\u{2554}') && output.contains('\u{255D}'),
        "goodbye should have box corners"
    );
}

// ================================================================
// DRAW CONSISTENCY — render same content twice, get same result
// ================================================================

#[test]
fn main_menu_render_is_deterministic() {
    let status = default_status();
    let opts = default_opts();
    let a = render_to_string(|buf| draw_main(buf, &status, &opts));
    let b = render_to_string(|buf| draw_main(buf, &status, &opts));
    assert_eq!(a, b, "rendering should be deterministic");
}

#[test]
fn maintenance_menu_render_is_deterministic() {
    let status = default_status();
    let opts = default_opts();
    let a = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    let b = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    assert_eq!(a, b, "rendering should be deterministic");
}

#[test]
fn advanced_menu_render_is_deterministic() {
    let status = default_status();
    let opts = default_opts();
    let a = render_to_string(|buf| draw_advanced(buf, &status, &opts));
    let b = render_to_string(|buf| draw_advanced(buf, &status, &opts));
    assert_eq!(a, b, "rendering should be deterministic");
}

#[test]
fn header_render_is_deterministic() {
    let status = all_good_status();
    let a = render_to_string(|buf| draw_header(buf, &status));
    let b = render_to_string(|buf| draw_header(buf, &status));
    assert_eq!(a, b, "header rendering should be deterministic");
}

#[test]
fn goodbye_render_is_deterministic() {
    let a = render_to_string(draw_goodbye);
    let b = render_to_string(draw_goodbye);
    assert_eq!(a, b, "goodbye rendering should be deterministic");
}

// ================================================================
// WRITE_ERR — edge cases
// ================================================================

#[test]
fn write_err_empty_message() {
    let mut buf = Vec::new();
    write_err(&mut buf, "").unwrap();
    let output = String::from_utf8_lossy(&buf).to_string();
    assert!(output.contains("[ERR ]"), "should still have ERR tag");
}

#[test]
fn write_err_long_message() {
    let long_msg = "x".repeat(500);
    let mut buf = Vec::new();
    write_err(&mut buf, &long_msg).unwrap();
    let output = String::from_utf8_lossy(&buf).to_string();
    assert!(output.contains("[ERR ]"), "should have ERR tag");
    assert!(output.contains(&long_msg), "should contain full message");
}

#[test]
fn write_err_message_with_special_characters() {
    let msg = "error: file \"C:\\path\\to\\file\" not found <&>";
    let mut buf = Vec::new();
    write_err(&mut buf, msg).unwrap();
    let output = String::from_utf8_lossy(&buf).to_string();
    assert!(output.contains(msg), "should preserve special chars");
}

#[test]
fn write_err_unicode_message() {
    let msg = "操作失败: プロファイル не найден → ❌";
    let mut buf = Vec::new();
    write_err(&mut buf, msg).unwrap();
    let output = String::from_utf8_lossy(&buf).to_string();
    assert!(output.contains("操作失败"), "should handle CJK characters");
}

// ================================================================
// CROSS-PAGE — structural checks
// ================================================================

#[test]
fn all_pages_have_header_with_title() {
    let status = default_status();
    let opts = default_opts();
    let main_output = render_to_string(|buf| draw_main(buf, &status, &opts));
    let maint_output = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    let adv_output = render_to_string(|buf| draw_advanced(buf, &status, &opts));

    for (name, output) in [
        ("main", &main_output),
        ("maintenance", &maint_output),
        ("advanced", &adv_output),
    ] {
        assert!(
            output.contains(TITLE),
            "{} page should contain title '{}'",
            name,
            TITLE
        );
    }
}

#[test]
fn all_pages_have_version() {
    let status = default_status();
    let opts = default_opts();
    let main_output = render_to_string(|buf| draw_main(buf, &status, &opts));
    let maint_output = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    let adv_output = render_to_string(|buf| draw_advanced(buf, &status, &opts));

    for (name, output) in [
        ("main", &main_output),
        ("maintenance", &maint_output),
        ("advanced", &adv_output),
    ] {
        assert!(
            output.contains("Version"),
            "{} page should contain version",
            name
        );
    }
}

#[test]
fn all_pages_have_box_drawing_top_and_bottom() {
    let status = default_status();
    let opts = default_opts();
    let main_output = render_to_string(|buf| draw_main(buf, &status, &opts));
    let maint_output = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    let adv_output = render_to_string(|buf| draw_advanced(buf, &status, &opts));

    for (name, output) in [
        ("main", &main_output),
        ("maintenance", &maint_output),
        ("advanced", &adv_output),
    ] {
        assert!(
            output.contains('\u{2554}'),
            "{} page missing top-left corner",
            name
        );
        assert!(
            output.contains('\u{255D}'),
            "{} page missing bottom-right corner",
            name
        );
    }
}

#[test]
fn all_pages_have_select_option_prompt() {
    let status = default_status();
    let opts = default_opts();
    let main_output = render_to_string(|buf| draw_main(buf, &status, &opts));
    let maint_output = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    let adv_output = render_to_string(|buf| draw_advanced(buf, &status, &opts));

    for (name, output) in [
        ("main", &main_output),
        ("maintenance", &maint_output),
        ("advanced", &adv_output),
    ] {
        assert!(
            output.contains("Select option:"),
            "{} page missing 'Select option:' prompt",
            name
        );
    }
}

#[test]
fn all_pages_have_current_status_section() {
    let status = default_status();
    let opts = default_opts();
    let main_output = render_to_string(|buf| draw_main(buf, &status, &opts));
    let maint_output = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    let adv_output = render_to_string(|buf| draw_advanced(buf, &status, &opts));

    for (name, output) in [
        ("main", &main_output),
        ("maintenance", &maint_output),
        ("advanced", &adv_output),
    ] {
        assert!(
            output.contains("CURRENT STATUS"),
            "{} page missing CURRENT STATUS",
            name
        );
    }
}

#[test]
fn all_pages_show_all_five_status_lines() {
    let status = all_good_status();
    let opts = default_opts();
    let main_output = render_to_string(|buf| draw_main(buf, &status, &opts));
    let maint_output = render_to_string(|buf| draw_maintenance(buf, &status, &opts));
    let adv_output = render_to_string(|buf| draw_advanced(buf, &status, &opts));

    for (name, output) in [
        ("main", &main_output),
        ("maintenance", &maint_output),
        ("advanced", &adv_output),
    ] {
        assert!(
            output.contains("Color Profile:"),
            "{} page missing Color Profile status",
            name
        );
        assert!(
            output.contains("Service:"),
            "{} page missing Service status",
            name
        );
        assert!(
            output.contains("LG UltraGear:"),
            "{} page missing LG UltraGear status",
            name
        );
        assert!(
            output.contains("HDR Mode:"),
            "{} page missing HDR Mode status",
            name
        );
        assert!(
            output.contains("SDR Mode:"),
            "{} page missing SDR Mode status",
            name
        );
    }
}

// ================================================================
// INSTALL PIPELINE — dry-run action function tests
// ================================================================

#[test]
fn action_default_install_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_default_install(&opts);
    assert!(result.is_ok(), "dry-run default install should succeed");
}

#[test]
fn action_profile_only_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_profile_only(&opts);
    assert!(result.is_ok(), "dry-run profile-only should succeed");
}

#[test]
fn action_service_only_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_service_only(&opts);
    assert!(result.is_ok(), "dry-run service-only should succeed");
}

#[test]
fn action_refresh_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_refresh(&opts);
    assert!(result.is_ok(), "dry-run refresh should succeed");
}

#[test]
fn action_reinstall_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_reinstall(&opts);
    assert!(result.is_ok(), "dry-run reinstall should succeed");
}

#[test]
fn action_remove_service_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_remove_service(&opts);
    assert!(result.is_ok(), "dry-run remove service should succeed");
}

#[test]
fn action_remove_profile_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_remove_profile(&opts);
    assert!(result.is_ok(), "dry-run remove profile should succeed");
}

#[test]
fn action_full_uninstall_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_full_uninstall(&opts);
    assert!(result.is_ok(), "dry-run full uninstall should succeed");
}

#[test]
fn action_recheck_service_dry_run_succeeds() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    let result = action_recheck_service(&opts);
    assert!(result.is_ok(), "dry-run recheck service should succeed");
}

// ── Pipeline: non-dry actions that are safe (read-only) ──────

#[test]
fn action_detect_succeeds() {
    let result = action_detect();
    assert!(result.is_ok(), "detect should succeed: {:?}", result.err());
}

#[test]
fn action_service_status_succeeds() {
    let result = action_service_status();
    assert!(
        result.is_ok(),
        "service status should succeed: {:?}",
        result.err()
    );
}

#[test]
fn action_check_applicability_succeeds() {
    let result = action_check_applicability();
    assert!(
        result.is_ok(),
        "check applicability should succeed: {:?}",
        result.err()
    );
}

#[test]
fn action_force_refresh_color_mgmt_succeeds() {
    enable_no_flicker_test_mode();
    let result = action_force_refresh_color_mgmt();
    assert!(
        result.is_ok(),
        "force refresh color mgmt should succeed: {:?}",
        result.err()
    );
}

// ── Pipeline: dry-run with per_user and generic_default ──────

#[test]
fn action_refresh_dry_run_with_per_user() {
    let opts = Options {
        dry_run: true,
        per_user: true,
        ..default_opts()
    };
    let result = action_refresh(&opts);
    assert!(
        result.is_ok(),
        "dry-run refresh with per_user should succeed"
    );
}

#[test]
fn action_refresh_dry_run_with_generic_default() {
    let opts = Options {
        dry_run: true,
        generic_default: true,
        ..default_opts()
    };
    let result = action_refresh(&opts);
    assert!(
        result.is_ok(),
        "dry-run refresh with generic_default should succeed"
    );
}

#[test]
fn action_refresh_dry_run_with_both_install_mode_flags() {
    let opts = Options {
        dry_run: true,
        per_user: true,
        generic_default: true,
        ..default_opts()
    };
    let result = action_refresh(&opts);
    assert!(
        result.is_ok(),
        "dry-run refresh with both install mode flags should succeed"
    );
}

#[test]
fn action_default_install_dry_run_with_all_toggles() {
    let opts = Options {
        toast: false,
        dry_run: true,
        verbose: true,
        hdr: true,
        sdr: false,
        per_user: true,
        generic_default: true,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let result = action_default_install(&opts);
    assert!(
        result.is_ok(),
        "dry-run install with all toggles should succeed"
    );
}

#[test]
fn action_full_uninstall_dry_run_with_all_toggles() {
    let opts = Options {
        toast: false,
        dry_run: true,
        verbose: true,
        hdr: true,
        sdr: false,
        per_user: true,
        generic_default: true,
        ddc_brightness: false,
        ddc_brightness_value: 50,
    };
    let result = action_full_uninstall(&opts);
    assert!(
        result.is_ok(),
        "dry-run uninstall with all toggles should succeed"
    );
}

// ── Pipeline: full dry-run install → uninstall sequence ──────

#[test]
fn pipeline_dry_run_full_install_then_uninstall() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    // Install pipeline
    assert!(action_default_install(&opts).is_ok(), "install");
    // Verify detect works between
    assert!(action_detect().is_ok(), "detect");
    // Check service status
    assert!(action_service_status().is_ok(), "status");
    // Full uninstall
    assert!(action_full_uninstall(&opts).is_ok(), "uninstall");
}

#[test]
fn pipeline_dry_run_profile_service_separate() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    // Profile first
    assert!(action_profile_only(&opts).is_ok(), "profile");
    // Then service
    assert!(action_service_only(&opts).is_ok(), "service");
    // Refresh
    assert!(action_refresh(&opts).is_ok(), "refresh");
    // Reinstall
    assert!(action_reinstall(&opts).is_ok(), "reinstall");
    // Remove separately
    assert!(action_remove_service(&opts).is_ok(), "remove service");
    assert!(action_remove_profile(&opts).is_ok(), "remove profile");
}

#[test]
fn pipeline_dry_run_maintenance_sequence() {
    enable_no_flicker_test_mode();
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    // Run through all safe maintenance actions
    assert!(action_detect().is_ok(), "detect");
    assert!(action_service_status().is_ok(), "status");
    assert!(action_recheck_service(&opts).is_ok(), "recheck");
    assert!(action_check_applicability().is_ok(), "applicability");
    assert!(action_force_refresh_color_mgmt().is_ok(), "force refresh");
}

#[test]
fn pipeline_dry_run_all_install_variants() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    // Try all three install paths
    assert!(action_default_install(&opts).is_ok(), "default install");
    assert!(action_profile_only(&opts).is_ok(), "profile only");
    assert!(action_service_only(&opts).is_ok(), "service only");
}

#[test]
fn pipeline_dry_run_all_uninstall_variants() {
    let opts = Options {
        dry_run: true,
        ..default_opts()
    };
    // Try all three uninstall paths
    assert!(action_remove_service(&opts).is_ok(), "remove service");
    assert!(action_remove_profile(&opts).is_ok(), "remove profile");
    assert!(action_full_uninstall(&opts).is_ok(), "full uninstall");
}

// ── run_action wrapper tests ─────────────────────────────────

#[test]
fn run_action_success_renders_banner() {
    let output = render_to_string(|buf| {
        // We avoid read_key by directly testing the wrapper output
        // before it hits the "Press any key" logic. We test the
        // write path only.
        queue!(
            buf,
            Clear(ClearType::Purge),
            Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .unwrap();
        draw_top(buf, " PROCESSING ").unwrap();
        draw_empty(buf).unwrap();
        draw_line(buf, "Test banner text", Color::Yellow).unwrap();
        draw_empty(buf).unwrap();
        draw_bottom(buf).unwrap();
        Ok(())
    });
    assert!(output.contains("PROCESSING"), "should show PROCESSING");
    assert!(
        output.contains("Test banner text"),
        "should show banner text"
    );
}

#[test]
fn run_action_error_renders_err_tag() {
    let mut buf = Vec::new();
    write_err(&mut buf, "action failed with error XYZ").unwrap();
    let output = String::from_utf8_lossy(&buf).to_string();
    assert!(output.contains("[ERR ]"), "should show ERR tag");
    assert!(
        output.contains("action failed with error XYZ"),
        "should show error message"
    );
}

struct FileBackup {
    path: std::path::PathBuf,
    original: Option<Vec<u8>>,
}

impl FileBackup {
    fn capture(path: std::path::PathBuf) -> Self {
        let original = std::fs::read(&path).ok();
        Self { path, original }
    }
}

impl Drop for FileBackup {
    fn drop(&mut self) {
        if let Some(bytes) = &self.original {
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&self.path, bytes);
        } else if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[test]
fn emit_apply_latency_tui_records_metrics_event_when_enabled() {
    let _lock = state_file_test_lock();
    let auto_path = app_state::automation_config_path();
    let diag_path = app_state::diagnostics_log_path();
    let _auto_backup = FileBackup::capture(auto_path);
    let _diag_backup = FileBackup::capture(diag_path);
    let cfg = app_state::AutomationConfig {
        metrics: app_state::MetricsConfig {
            enabled: true,
            collect_latency: true,
            collect_success_rate: true,
            rolling_window: 32,
        },
        ..app_state::AutomationConfig::default()
    };
    app_state::save_automation_config(&cfg).expect("save automation");
    let start = std::time::Instant::now();
    app_state::clear_diagnostics_log().expect("clear diagnostics");
    emit_apply_latency_tui(start, true, "action=tui-latency-test");

    let events = app_state::read_recent_diagnostic_events(256).expect("read diagnostics");
    assert!(
        events.iter().any(|e| {
            e.event == "apply_latency"
                && e.source == "tui"
                && e.details.contains("action=tui-latency-test")
        }),
        "expected tui apply_latency diagnostics entry"
    );
}

#[test]
fn draw_service_diagnostics_shows_latency_summary() {
    let _lock = state_file_test_lock();
    let auto_path = app_state::automation_config_path();
    let diag_path = app_state::diagnostics_log_path();
    let _auto_backup = FileBackup::capture(auto_path);
    let _diag_backup = FileBackup::capture(diag_path);
    let cfg = app_state::AutomationConfig {
        metrics: app_state::MetricsConfig {
            enabled: true,
            collect_latency: true,
            collect_success_rate: true,
            rolling_window: 16,
        },
        ..app_state::AutomationConfig::default()
    };
    app_state::save_automation_config(&cfg).expect("save automation");
    app_state::append_diagnostic_event(
        "service",
        "INFO",
        "apply_latency",
        "ms=45 success=1 trigger=test",
    );
    let output = render_to_string(|buf| draw_service_diagnostics(buf, &default_status()));
    assert!(
        output.contains("Apply latency (window="),
        "diagnostics should show apply latency summary: {}",
        output
    );
}
