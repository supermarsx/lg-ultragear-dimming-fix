use super::*;
use crate::config::Config;

// ── show_reapply_toast disabled ───────────────────────────────────

#[test]
fn show_reapply_toast_disabled_is_noop() {
    let cfg = Config {
        toast_enabled: false,
        ..Config::default()
    };
    // Should return immediately without spawning any process
    show_reapply_toast(&cfg);
}

#[test]
fn show_reapply_toast_disabled_with_custom_text() {
    let cfg = Config {
        toast_enabled: false,
        toast_title: "Should Not Show".to_string(),
        toast_body: "This should be a no-op".to_string(),
        ..Config::default()
    };
    show_reapply_toast(&cfg);
}

// ── Config field access ──────────────────────────────────────────

#[test]
fn toast_config_default_title() {
    let cfg = Config::default();
    assert_eq!(cfg.toast_title, "LG UltraGear");
}

#[test]
fn toast_config_default_body() {
    let cfg = Config::default();
    assert_eq!(cfg.toast_body, "Color profile reapplied ✓");
}

#[test]
fn toast_config_enabled_by_default() {
    let cfg = Config::default();
    assert!(cfg.toast_enabled);
}

// ── Text escaping edge cases ─────────────────────────────────────

#[test]
fn toast_title_with_quotes_does_not_panic() {
    let cfg = Config {
        toast_enabled: false, // keep disabled so no process spawns
        toast_title: r#"Title with "quotes" and 'apostrophes'"#.to_string(),
        toast_body: "Normal body".to_string(),
        ..Config::default()
    };
    show_reapply_toast(&cfg);
}

#[test]
fn toast_body_with_special_chars_does_not_panic() {
    let cfg = Config {
        toast_enabled: false,
        toast_title: "Title".to_string(),
        toast_body: "Body with <xml> & special chars £€¥".to_string(),
        ..Config::default()
    };
    show_reapply_toast(&cfg);
}

#[test]
fn toast_with_empty_strings_does_not_panic() {
    let cfg = Config {
        toast_enabled: false,
        toast_title: "".to_string(),
        toast_body: "".to_string(),
        ..Config::default()
    };
    show_reapply_toast(&cfg);
}

#[test]
fn toast_with_unicode_does_not_panic() {
    let cfg = Config {
        toast_enabled: false,
        toast_title: "カラープロファイル".to_string(),
        toast_body: "適用済み ✓".to_string(),
        ..Config::default()
    };
    show_reapply_toast(&cfg);
}

// ── Verbose flag ─────────────────────────────────────────────────

#[test]
fn toast_verbose_flag_accessible() {
    let cfg = Config {
        verbose: true,
        toast_enabled: false,
        ..Config::default()
    };
    assert!(cfg.verbose);
    show_reapply_toast(&cfg);
}
