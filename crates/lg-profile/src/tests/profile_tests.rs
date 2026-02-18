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
    const {
        assert!(
            EMBEDDED_ICM_SIZE > 40,
            "Embedded ICM too small to be a valid ICC profile"
        )
    };
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
    let result = reapply_profile(r"DISPLAY\TEST\001", &path, 100, false);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Profile not found"),
        "Error should mention missing profile: {}",
        err_msg
    );
}

#[test]
fn reapply_profile_per_user_fails_with_missing_profile() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-test-profile-00001.icm",
    );
    let result = reapply_profile(r"DISPLAY\TEST\001", &path, 100, true);
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

// ── WCS scope constants ──────────────────────────────────────────

#[test]
fn wcs_scope_current_user_value() {
    assert_eq!(WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER, 1);
}

#[test]
fn wcs_cpt_and_cpst_constants() {
    assert_eq!(CPT_ICC, 1);
    assert_eq!(CPST_NONE, 1);
}

// ── Profile path validation ──────────────────────────────────────

#[test]
fn profile_path_for_reapply_check() {
    let path = PathBuf::from(r"C:\Windows\System32\spool\drivers\color\test.icm");
    assert!(path.to_string_lossy().ends_with("test.icm"));
}

// ================================================================
// Edge case tests — extended coverage
// ================================================================

// ── to_wide edge cases ───────────────────────────────────────────

#[test]
fn to_wide_unicode_characters() {
    let result = to_wide("日本語テスト");
    assert!(!result.is_empty());
    assert_eq!(*result.last().unwrap(), 0u16);
    // Each character maps to at least one u16
    assert!(result.len() >= 7); // 6 chars + null
}

#[test]
fn to_wide_backslashes_in_device_path() {
    let result = to_wide(r"DISPLAY\LG\ULTRAGEAR_001\INSTANCE_0");
    assert_eq!(*result.last().unwrap(), 0u16);
    // Count backslashes ('\' = 0x5C)
    let backslash_count = result.iter().filter(|&&c| c == 0x5C).count();
    assert_eq!(backslash_count, 3, "should encode 3 backslashes");
}

#[test]
fn to_wide_spaces_in_path() {
    let result = to_wide(r"C:\Program Files\Some App\profile.icm");
    assert_eq!(*result.last().unwrap(), 0u16);
    let space_count = result.iter().filter(|&&c| c == 0x20).count();
    assert_eq!(space_count, 2, "should encode 2 spaces");
}

#[test]
fn to_wide_very_long_string() {
    let long = "A".repeat(1000);
    let result = to_wide(&long);
    assert_eq!(result.len(), 1001); // 1000 chars + null
    assert_eq!(*result.last().unwrap(), 0u16);
}

#[test]
fn to_wide_mixed_ascii_and_unicode() {
    let result = to_wide("Monitor-LG-日本語");
    assert_eq!(*result.last().unwrap(), 0u16);
    assert!(result.len() > 1);
}

// ── Embedded ICM edge cases ──────────────────────────────────────

#[test]
fn embedded_icm_has_icc_header_signature() {
    // ICC profiles have "acsp" at offset 36
    if EMBEDDED_ICM_SIZE > 40 {
        let sig = &EMBEDDED_ICM[36..40];
        assert_eq!(sig, b"acsp", "embedded ICM should have ICC 'acsp' signature");
    }
}

#[test]
fn embedded_icm_first_4_bytes_is_size() {
    // ICC profile spec: first 4 bytes = big-endian profile size
    if EMBEDDED_ICM_SIZE >= 4 {
        let size_bytes = &EMBEDDED_ICM[0..4];
        let reported_size =
            u32::from_be_bytes([size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]]);
        assert_eq!(
            reported_size as usize, EMBEDDED_ICM_SIZE,
            "ICC header size should match EMBEDDED_ICM_SIZE"
        );
    }
}

#[test]
fn embedded_icm_is_not_all_zeros() {
    let all_zero = EMBEDDED_ICM.iter().all(|&b| b == 0);
    assert!(!all_zero, "embedded ICM should not be all zeros");
}

#[test]
fn embedded_icm_has_nonzero_size() {
    assert!(EMBEDDED_ICM_SIZE > 100, "ICM file should be > 100 bytes");
}

// ── ensure_profile_installed edge cases ──────────────────────────

#[test]
fn ensure_profile_installed_to_temp_directory() {
    let dir = std::env::temp_dir().join("lg-profile-edge-test-ensure");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("edge-test.icm");

    // First install: should write
    let wrote = ensure_profile_installed(&path).expect("first install");
    assert!(wrote, "should write on first install");
    assert!(path.exists(), "file should exist after install");
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        EMBEDDED_ICM_SIZE as u64,
    );

    // Second install: should skip (same size)
    let wrote = ensure_profile_installed(&path).expect("second install");
    assert!(!wrote, "should skip when size matches");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_profile_installed_overwrites_wrong_size() {
    let dir = std::env::temp_dir().join("lg-profile-edge-test-overwrite");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wrong-size.icm");

    // Write a wrong-size file
    std::fs::write(&path, b"too short").unwrap();
    assert!(path.exists());

    let wrote = ensure_profile_installed(&path).expect("should overwrite");
    assert!(wrote, "should overwrite wrong-size file");
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        EMBEDDED_ICM_SIZE as u64,
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_profile_installed_creates_parent_directories() {
    let dir = std::env::temp_dir()
        .join("lg-profile-edge-test-nested")
        .join("a")
        .join("b")
        .join("c");
    let _ = std::fs::remove_dir_all(
        std::env::temp_dir().join("lg-profile-edge-test-nested"),
    );
    let path = dir.join("nested.icm");

    let wrote = ensure_profile_installed(&path).expect("should create nested dirs");
    assert!(wrote, "should write to nested path");
    assert!(path.exists());

    let _ = std::fs::remove_dir_all(
        std::env::temp_dir().join("lg-profile-edge-test-nested"),
    );
}

// ── remove_profile edge cases ────────────────────────────────────

#[test]
fn remove_profile_nonexistent_edge_returns_false() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-edge-test-99999.icm",
    );
    let result = remove_profile(&path).expect("should not error");
    assert!(!result, "removing nonexistent profile should return false");
}

#[test]
fn remove_profile_after_ensure_installed() {
    let dir = std::env::temp_dir().join("lg-profile-edge-test-remove");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("remove-test.icm");

    // Install
    ensure_profile_installed(&path).unwrap();
    assert!(path.exists());

    // Remove
    let removed = remove_profile(&path).expect("should remove");
    assert!(removed, "should return true on removal");
    assert!(!path.exists(), "file should be gone after removal");

    // Remove again (already gone)
    let removed = remove_profile(&path).expect("should not error");
    assert!(!removed, "second removal should return false");

    let _ = std::fs::remove_dir_all(&dir);
}

// ── is_profile_installed edge cases ──────────────────────────────

#[test]
fn is_profile_installed_correct_size_returns_true() {
    let dir = std::env::temp_dir().join("lg-profile-edge-test-is-installed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("check.icm");

    ensure_profile_installed(&path).unwrap();
    assert!(is_profile_installed(&path), "should report installed");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_profile_installed_wrong_size_still_returns_true() {
    // is_profile_installed only checks existence, not size
    let dir = std::env::temp_dir().join("lg-profile-edge-test-wrong-size-check");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wrong.icm");

    std::fs::write(&path, b"not a real profile").unwrap();
    assert!(
        is_profile_installed(&path),
        "is_profile_installed checks only existence, not size"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_profile_installed_missing_returns_false() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\definitely-missing-99999.icm",
    );
    assert!(!is_profile_installed(&path), "missing file should not be installed");
}

// ── reapply_profile edge cases ───────────────────────────────────

#[test]
fn reapply_profile_empty_device_key_fails() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-empty-key-99999.icm",
    );
    let result = reapply_profile("", &path, 100, false);
    // Should fail because profile doesn't exist (profile check comes first)
    assert!(result.is_err());
}

#[test]
fn reapply_profile_zero_delay_still_fails_on_missing_profile() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-zero-delay-99999.icm",
    );
    let result = reapply_profile(r"DISPLAY\TEST\001", &path, 0, false);
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Profile not found"),
        "should mention missing profile"
    );
}

#[test]
fn reapply_profile_per_user_true_still_fails_on_missing_profile() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-per-user-99999.icm",
    );
    let result = reapply_profile(r"DISPLAY\TEST\001", &path, 100, true);
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("Profile not found"),
        "per_user=true should still check profile existence"
    );
}

#[test]
fn reapply_profile_very_long_device_key_fails_on_missing_profile() {
    let long_key = format!(r"DISPLAY\{}\001", "X".repeat(500));
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-long-key-99999.icm",
    );
    let result = reapply_profile(&long_key, &path, 100, false);
    assert!(result.is_err());
}

// ── refresh_display edge cases ───────────────────────────────────

#[test]
fn refresh_display_all_enabled_does_not_panic() {
    refresh_display(true, true, true);
}

#[test]
fn refresh_display_only_settings_does_not_panic() {
    refresh_display(true, false, false);
}

#[test]
fn refresh_display_only_broadcast_does_not_panic() {
    refresh_display(false, true, false);
}

#[test]
fn refresh_display_only_invalidate_does_not_panic() {
    refresh_display(false, false, true);
}

// ── trigger_calibration_loader edge cases ────────────────────────

#[test]
fn trigger_calibration_loader_enabled_does_not_panic() {
    trigger_calibration_loader(true);
}

// ── WCS constants boundary validation ────────────────────────────

#[test]
fn wcs_scope_constants_are_distinct() {
    assert_ne!(
        WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
        WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
        "system-wide and current-user scopes must differ"
    );
}

#[test]
fn wcs_scope_values_are_small_positive_integers() {
    assert!(WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE > 0);
    assert!(WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE < 256);
    assert!(WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER > 0);
    assert!(WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER < 256);
}

#[test]
fn cpt_icc_is_one() {
    assert_eq!(CPT_ICC, 1);
}

#[test]
fn cpst_none_is_one() {
    assert_eq!(CPST_NONE, 1);
}

// ── Display association constants ────────────────────────────────

#[test]
fn color_profile_type_sdr_is_zero() {
    assert_eq!(COLOR_PROFILE_TYPE_SDR, 0);
}

#[test]
fn color_profile_subtype_sdr_is_zero() {
    assert_eq!(COLOR_PROFILE_SUBTYPE_SDR, 0);
}

// ── register_color_profile ───────────────────────────────────────

#[test]
fn register_color_profile_nonexistent_does_not_panic() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-register-test-99999.icm",
    );
    // Should not panic — returns Ok even if the API warns (non-fatal)
    let result = register_color_profile(&path);
    assert!(result.is_ok());
}

#[test]
fn register_color_profile_temp_file() {
    let dir = std::env::temp_dir().join("lg-profile-register-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("register-test.icm");

    // Write the embedded profile
    ensure_profile_installed(&path).unwrap();

    // register_color_profile should not panic (may warn if no admin rights)
    let result = register_color_profile(&path);
    assert!(result.is_ok());

    let _ = std::fs::remove_dir_all(&dir);
}

// ── set_display_default_association ──────────────────────────────

#[test]
fn set_display_default_association_nonexistent_device_does_not_panic() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-sdr-assoc-99999.icm",
    );
    // Should not panic — API calls are non-fatal
    let result = set_display_default_association(r"DISPLAY\FAKE\999", &path, false);
    assert!(result.is_ok());
}

#[test]
fn set_display_default_association_per_user_does_not_panic() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-sdr-assoc-per-user-99999.icm",
    );
    let result = set_display_default_association(r"DISPLAY\FAKE\999", &path, true);
    assert!(result.is_ok());
}

// ── add_hdr_display_association ──────────────────────────────────

#[test]
fn add_hdr_display_association_nonexistent_device_does_not_panic() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-hdr-assoc-99999.icm",
    );
    let result = add_hdr_display_association(r"DISPLAY\FAKE\999", &path, false);
    assert!(result.is_ok());
}

#[test]
fn add_hdr_display_association_per_user_does_not_panic() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-hdr-assoc-per-user-99999.icm",
    );
    let result = add_hdr_display_association(r"DISPLAY\FAKE\999", &path, true);
    assert!(result.is_ok());
}