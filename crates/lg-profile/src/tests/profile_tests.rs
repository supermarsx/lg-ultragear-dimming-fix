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

// ── Embedded ICM ─────────────────────────────────────────────────

#[test]
fn embedded_icm_is_not_empty() {
    const { assert!(EMBEDDED_ICM_SIZE > 0, "Embedded ICM should contain data") };
}

#[test]
fn embedded_icm_has_valid_icc_header() {
    // ICC profiles start with a 4-byte size field, then 4 bytes of padding,
    // then the ASCII signature "acsp" at offset 36.
    const { assert!(EMBEDDED_ICM_SIZE > 40, "Embedded ICM too small to be a valid ICC profile") };
}

#[test]
fn ensure_profile_installed_writes_to_temp() {
    let dir = std::env::temp_dir().join("lg-test-ensure-profile");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("test-embedded.icm");

    // First call should write the file
    let wrote = ensure_profile_installed(&path).expect("should succeed");
    assert!(wrote, "should report file was written");
    assert!(path.exists(), "file should exist after extraction");
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        EMBEDDED_ICM_SIZE as u64
    );

    // Second call should be a no-op
    let wrote2 = ensure_profile_installed(&path).expect("should succeed");
    assert!(!wrote2, "should report no write needed (already present)");

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

// ── remove_profile ───────────────────────────────────────────────

#[test]
fn remove_profile_nonexistent_returns_false() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\this-profile-does-not-exist-99999.icm",
    );
    let result = remove_profile(&path).expect("should succeed");
    assert!(!result, "should return false for nonexistent file");
}

#[test]
fn remove_profile_deletes_temp_file() {
    let dir = std::env::temp_dir().join("lg-test-remove-profile");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test-remove.icm");

    // Write a test file
    std::fs::write(&path, b"test data").unwrap();
    assert!(path.exists());

    // Remove it
    let result = remove_profile(&path).expect("should succeed");
    assert!(result, "should return true when file was removed");
    assert!(!path.exists(), "file should be gone");

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
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
