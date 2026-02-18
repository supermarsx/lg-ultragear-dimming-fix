use super::*;
use std::path::PathBuf;

// ── to_wide helper ───────────────────────────────────────────────

#[test]
fn to_wide_empty_string() {
    let result = to_wide("");
    assert_eq!(result, vec![0]); // just the null terminator
}

#[test]
fn to_wide_ascii() {
    let result = to_wide("ABC");
    assert_eq!(result, vec![65, 66, 67, 0]);
}

#[test]
fn to_wide_null_terminated() {
    let result = to_wide("test");
    assert_eq!(*result.last().unwrap(), 0u16);
}

#[test]
fn to_wide_path() {
    let result = to_wide(r"C:\Windows\System32\spool\drivers\color\test.icm");
    assert!(!result.is_empty());
    assert_eq!(*result.last().unwrap(), 0u16);
    assert_eq!(result[0], 67u16); // 'C'
}

#[test]
fn to_wide_unicode() {
    let result = to_wide("日本語");
    assert_eq!(*result.last().unwrap(), 0u16);
    assert_eq!(result.len(), 4); // 3 chars + null
}

#[test]
fn to_wide_spaces_and_special() {
    let result = to_wide("LG ULTRAGEAR (27GP950)");
    assert_eq!(*result.last().unwrap(), 0u16);
    assert_eq!(result.len(), 23); // 22 chars + null
}

// ── is_profile_installed ─────────────────────────────────────────

#[test]
fn is_profile_installed_nonexistent_profile() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\this-profile-definitely-does-not-exist-12345.icm",
    );
    assert!(!is_profile_installed(&path));
}

#[test]
fn is_profile_installed_default_path() {
    // May or may not exist on the test machine — just verify no panic
    let path = PathBuf::from(r"C:\Windows\System32\spool\drivers\color\lg-ultragear-full-cal.icm");
    let _ = is_profile_installed(&path);
}

// ── WCS scope constant ───────────────────────────────────────────

#[test]
fn wcs_scope_system_wide_value() {
    assert_eq!(WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE, 2);
}

// ── Profile reapply ──────────────────────────────────────────────

#[test]
fn reapply_profile_fails_with_missing_profile() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-test-profile-00000.icm",
    );
    let result = reapply_profile(r"DISPLAY\TEST\001", &path, 100);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Profile not found"),
        "Error should mention missing profile: {}",
        err_msg
    );
}

#[test]
fn refresh_display_with_all_methods_disabled_does_not_panic() {
    // All false = complete no-op
    refresh_display(false, false, false);
}

#[test]
fn trigger_calibration_loader_disabled_does_not_panic() {
    trigger_calibration_loader(false);
}

// ── Profile path validation ──────────────────────────────────────

#[test]
fn profile_path_for_reapply_check() {
    let path = PathBuf::from(r"C:\Windows\System32\spool\drivers\color\test.icm");
    assert!(path.to_string_lossy().ends_with("test.icm"));
}
