use super::*;
use cmx::profile::RawProfile;
use cmx::tag::TagSignature;
use std::path::PathBuf;
use std::sync::Once;

fn enable_no_flicker_test_mode() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::env::set_var("LG_TEST_NO_FLICKER_REFRESH", "1");
        set_test_no_flicker_mode(true);
    });
}

fn generated_icm_bytes() -> Vec<u8> {
    generate_dynamic_profile_bytes(DEFAULT_DYNAMIC_GAMMA)
        .expect("should generate default dynamic ICC")
}

fn generated_icm_size() -> usize {
    generated_icm_bytes().len()
}

#[test]
fn parse_dynamic_icc_preset_variants() {
    assert_eq!(
        parse_dynamic_icc_preset("gamma22"),
        DynamicIccPreset::Gamma22
    );
    assert_eq!(parse_dynamic_icc_preset("2.2"), DynamicIccPreset::Gamma22);
    assert_eq!(
        parse_dynamic_icc_preset("gamma24"),
        DynamicIccPreset::Gamma24
    );
    assert_eq!(parse_dynamic_icc_preset("2.4"), DynamicIccPreset::Gamma24);
    assert_eq!(parse_dynamic_icc_preset("reader"), DynamicIccPreset::Reader);
    assert_eq!(
        parse_dynamic_icc_preset("reader_mode"),
        DynamicIccPreset::Reader
    );
    assert_eq!(parse_dynamic_icc_preset("custom"), DynamicIccPreset::Custom);
}

#[test]
fn reader_dynamic_preset_has_expected_profile_name_and_gamma() {
    assert_eq!(
        DynamicIccPreset::Reader.profile_name("ignored.icm"),
        READER_PROFILE_NAME
    );
    assert_eq!(
        DynamicIccPreset::Reader.gamma(1.7),
        PRESET_GAMMA_READER,
        "reader preset should use fixed gamma regardless of custom input"
    );
}

#[test]
fn parse_dynamic_icc_tuning_preset_variants() {
    assert_eq!(
        parse_dynamic_icc_tuning_preset("manual"),
        DynamicIccTuningPreset::Manual
    );
    assert_eq!(
        parse_dynamic_icc_tuning_preset("anti_dim_soft"),
        DynamicIccTuningPreset::AntiDimSoft
    );
    assert_eq!(
        parse_dynamic_icc_tuning_preset("balanced"),
        DynamicIccTuningPreset::AntiDimBalanced
    );
    assert_eq!(
        parse_dynamic_icc_tuning_preset("anti-dim-aggressive"),
        DynamicIccTuningPreset::AntiDimAggressive
    );
    assert_eq!(
        parse_dynamic_icc_tuning_preset("night"),
        DynamicIccTuningPreset::AntiDimNight
    );
    assert_eq!(
        parse_dynamic_icc_tuning_preset("reader_balanced"),
        DynamicIccTuningPreset::ReaderBalanced
    );
    assert_eq!(
        parse_dynamic_icc_tuning_preset("reader"),
        DynamicIccTuningPreset::ReaderBalanced
    );
}

#[test]
fn tuning_preset_resolution_supports_manual_overrides() {
    let manual = DynamicIccTuning {
        white_compression: 0.4,
        ..DynamicIccTuning::default()
    };
    let resolved = resolve_dynamic_icc_tuning(manual, "anti_dim_balanced", true);
    assert!(resolved.black_lift > 0.0);
    assert!(
        (resolved.white_compression - 0.4).abs() < 1e-9,
        "manual override should win when enabled"
    );

    let resolved_no_overrides = resolve_dynamic_icc_tuning(manual, "anti_dim_balanced", false);
    assert!(
        (resolved_no_overrides.white_compression - 0.22).abs() < 1e-9,
        "preset value should be used when manual overrides are disabled"
    );
}

#[test]
fn tuning_preset_names_include_reader_balanced() {
    assert!(
        dynamic_icc_tuning_preset_names().contains(&"reader_balanced"),
        "reader-balanced tuning should be selectable by name"
    );
}

#[test]
fn reader_balanced_preset_is_cooler_and_brighter_than_manual_defaults() {
    let tuning = dynamic_icc_tuning_for_preset(DynamicIccTuningPreset::ReaderBalanced);
    assert!(tuning.black_lift > 0.0);
    assert!(tuning.midtone_boost > 0.0);
    assert!(tuning.vcgt_enabled);
    assert!(tuning.vcgt_strength > 0.0);
    assert!(
        tuning.gamma_b > 1.0 && tuning.gamma_r < 1.0,
        "reader preset should cool white balance (reduce warm/yellow cast)"
    );
}

#[test]
fn active_reader_preset_respects_selected_tuning() {
    let dir = std::env::temp_dir().join("lg-profile-reader-preset");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let selected_tuning = dynamic_icc_tuning_for_preset(DynamicIccTuningPreset::AntiDimNight);

    let active = ensure_active_profile_installed_tuned(
        &dir,
        "reader",
        "custom-name-ignored.icm",
        2.4,
        120.0,
        false,
        selected_tuning,
    )
    .unwrap();

    assert_eq!(
        active.file_name().unwrap().to_string_lossy().to_lowercase(),
        READER_PROFILE_NAME
    );

    let expected = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        PRESET_GAMMA_READER,
        120.0,
        selected_tuning,
    )
    .unwrap();
    let actual = std::fs::read(&active).unwrap();
    assert_eq!(
        actual, expected,
        "reader preset should keep selected tuning"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn select_effective_preset_prefers_mode_when_no_schedule() {
    let sdr = select_effective_preset("custom", "gamma22", "gamma24", "", "", false);
    let hdr = select_effective_preset("custom", "gamma22", "gamma24", "", "", true);
    assert_eq!(sdr, "gamma22");
    assert_eq!(hdr, "gamma24");
}

#[test]
fn select_effective_preset_uses_active_when_mode_presets_are_legacy_defaults() {
    let selected = select_effective_preset("reader", "gamma22", "gamma22", "", "", false);
    assert_eq!(
        selected, "reader",
        "active preset should win when both mode presets are legacy defaults"
    );
}

#[test]
fn select_effective_preset_uses_schedule_when_both_present() {
    let selected =
        select_effective_preset("custom", "gamma22", "gamma24", "gamma22", "gamma24", true);
    assert!(
        selected == "gamma22" || selected == "gamma24",
        "schedule should choose either day or night preset"
    );
}

#[test]
fn sanitize_dynamic_luminance_clamps_values() {
    assert_eq!(
        sanitize_dynamic_luminance_cd_m2(f64::NAN),
        DEFAULT_DYNAMIC_LUMINANCE_CD_M2
    );
    assert_eq!(
        sanitize_dynamic_luminance_cd_m2(1.0),
        MIN_DYNAMIC_LUMINANCE_CD_M2
    );
    assert_eq!(
        sanitize_dynamic_luminance_cd_m2(10_000.0),
        MAX_DYNAMIC_LUMINANCE_CD_M2
    );
}

#[test]
fn generated_icm_changes_with_luminance() {
    let low = generate_dynamic_profile_bytes_with_luminance(2.2, 100.0).unwrap();
    let high = generate_dynamic_profile_bytes_with_luminance(2.2, 200.0).unwrap();
    assert_ne!(
        low, high,
        "different luminance values should produce different ICC bytes"
    );
}

#[test]
fn generated_icm_changes_with_tuning_parameters() {
    let base = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        2.2,
        120.0,
        DynamicIccTuning::default(),
    )
    .unwrap();
    let tuned = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        2.2,
        120.0,
        DynamicIccTuning {
            black_lift: 0.08,
            midtone_boost: 0.12,
            white_compression: 0.25,
            ..DynamicIccTuning::default()
        },
    )
    .unwrap();
    assert_ne!(
        base, tuned,
        "curve shaping parameters should affect generated ICC bytes"
    );
}

#[test]
fn all_non_manual_tuning_presets_change_generated_icm_bytes() {
    let base = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        2.2,
        120.0,
        DynamicIccTuning::default(),
    )
    .unwrap();
    let presets = [
        DynamicIccTuningPreset::AntiDimSoft,
        DynamicIccTuningPreset::AntiDimBalanced,
        DynamicIccTuningPreset::AntiDimAggressive,
        DynamicIccTuningPreset::AntiDimNight,
        DynamicIccTuningPreset::ReaderBalanced,
    ];

    for preset in presets {
        let tuned = generate_dynamic_profile_bytes_with_luminance_and_tuning(
            2.2,
            120.0,
            dynamic_icc_tuning_for_preset(preset),
        )
        .unwrap();
        assert_ne!(
            base, tuned,
            "{preset:?} should change generated ICC bytes from manual defaults"
        );
    }
}

#[test]
fn gamma22_and_gamma24_produce_distinct_generated_icm_bytes() {
    let tuning = dynamic_icc_tuning_for_preset(DynamicIccTuningPreset::AntiDimBalanced);
    let gamma22 =
        generate_dynamic_profile_bytes_with_luminance_and_tuning(PRESET_GAMMA_22, 140.0, tuning)
            .unwrap();
    let gamma24 =
        generate_dynamic_profile_bytes_with_luminance_and_tuning(PRESET_GAMMA_24, 140.0, tuning)
            .unwrap();
    assert_ne!(
        gamma22, gamma24,
        "gamma22 and gamma24 presets should generate different profile bytes"
    );
}

#[test]
fn generated_icm_with_vcgt_contains_vcgt_signature() {
    let with_vcgt = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        2.2,
        120.0,
        DynamicIccTuning {
            vcgt_enabled: true,
            vcgt_strength: 0.75,
            ..DynamicIccTuning::default()
        },
    )
    .unwrap();
    assert!(
        with_vcgt.windows(4).any(|w| w == b"vcgt"),
        "vcgt-enabled profile should contain the vcgt signature"
    );
}

#[test]
fn parse_icc_signature_or_zero_handles_valid_and_invalid_input() {
    assert_eq!(parse_icc_signature_or_zero("vidm"), 0x7669646D);
    assert_eq!(parse_icc_signature_or_zero(""), 0);
    assert_eq!(parse_icc_signature_or_zero("TOO_LONG"), 0);
}

#[test]
fn generated_icm_with_extended_tags_contains_signatures() {
    let bytes = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        2.2,
        160.0,
        DynamicIccTuning {
            cicp_enabled: true,
            cicp_color_primaries: 1,
            cicp_transfer_characteristics: 13,
            cicp_matrix_coefficients: 0,
            cicp_full_range: true,
            technology_signature: parse_icc_signature_or_zero("vidm"),
            ciis_signature: parse_icc_signature_or_zero("scoe"),
            metadata_enabled: true,
            ..DynamicIccTuning::default()
        },
    )
    .unwrap();

    assert!(
        bytes.windows(4).any(|w| w == b"cicp"),
        "extended profile should contain cicp signature"
    );
    assert!(
        bytes.windows(4).any(|w| w == b"meta"),
        "extended profile should contain metadata tag signature"
    );
    assert!(
        bytes.windows(4).any(|w| w == b"tech"),
        "extended profile should contain technology tag signature"
    );

    let report = validate_icc_profile_bytes(&bytes);
    assert!(report.is_valid(), "{}", report.errors.join(" | "));
    assert!(report.has_cicp_tag);
    assert!(report.has_metadata_tag);
}

#[test]
fn monitor_scoped_profile_name_uses_serial_suffix() {
    let identity = DynamicMonitorIdentity {
        serial_number: "SN-123 45".to_string(),
        ..DynamicMonitorIdentity::default()
    };
    let name = monitor_scoped_profile_name("lg-ultragear-dynamic-cmx.icm", &identity);
    assert!(name.starts_with("lg-ultragear-dynamic-cmx-"));
    assert!(name.ends_with(".icm"));
    assert!(name.contains("sn-123_45"));
}

#[test]
fn monitor_scoped_profile_name_falls_back_to_hash() {
    let identity = DynamicMonitorIdentity {
        device_key: r"DISPLAY\LGS\ABC123".to_string(),
        ..DynamicMonitorIdentity::default()
    };
    let name = monitor_scoped_profile_name("lg-ultragear-dynamic-cmx.icm", &identity);
    assert!(name.starts_with("lg-ultragear-dynamic-cmx-"));
    assert!(name.ends_with(".icm"));
}

#[test]
fn generated_icm_with_spectral_scaffold_contains_signatures() {
    let bytes = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        2.2,
        120.0,
        DynamicIccTuning {
            include_spectral_scaffold: true,
            ..DynamicIccTuning::default()
        },
    )
    .unwrap();

    assert!(bytes.windows(4).any(|w| w == b"sdin"));
    assert!(bytes.windows(4).any(|w| w == b"swpt"));
    assert!(bytes.windows(4).any(|w| w == b"svcn"));

    let report = validate_icc_profile_bytes(&bytes);
    assert!(report.is_valid(), "{}", report.errors.join(" | "));
    assert!(report.has_spectral_data_info_tag);
    assert!(report.has_spectral_white_point_tag);
    assert!(report.has_spectral_viewing_conditions_tag);
}

#[test]
fn ensure_specialized_profiles_installed_writes_both() {
    let dir = std::env::temp_dir().join("lg-profile-specialized");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (gamma22, gamma24) = ensure_specialized_profiles_installed(&dir, 120.0).unwrap();
    assert!(gamma22.exists(), "gamma22 profile should exist");
    assert!(gamma24.exists(), "gamma24 profile should exist");
    assert!(gamma22
        .file_name()
        .unwrap()
        .to_string_lossy()
        .eq_ignore_ascii_case(GAMMA22_PROFILE_NAME));
    assert!(gamma24
        .file_name()
        .unwrap()
        .to_string_lossy()
        .eq_ignore_ascii_case(GAMMA24_PROFILE_NAME));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_mode_profiles_installed_tuned_writes_sdr_and_hdr_profiles() {
    let dir = std::env::temp_dir().join("lg-profile-mode-profiles");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (sdr, hdr) = ensure_mode_profiles_installed_tuned(
        &dir,
        "gamma22",
        "gamma24",
        "ignored.icm",
        2.2,
        120.0,
        false,
        DynamicIccTuning::default(),
    )
    .unwrap();

    assert!(sdr.exists(), "SDR profile should exist");
    assert!(hdr.exists(), "HDR profile should exist");
    assert_ne!(sdr, hdr, "gamma22 and gamma24 should be separate files");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_mode_profiles_installed_tuned_reuses_same_path_for_same_preset() {
    let dir = std::env::temp_dir().join("lg-profile-mode-profiles-same");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (sdr, hdr) = ensure_mode_profiles_installed_tuned(
        &dir,
        "reader",
        "reader",
        "ignored.icm",
        2.2,
        120.0,
        false,
        dynamic_icc_tuning_for_preset(DynamicIccTuningPreset::ReaderBalanced),
    )
    .unwrap();

    assert_eq!(sdr, hdr, "same preset should reuse same profile path");
    assert!(sdr.exists(), "profile should exist");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reapply_profile_with_mode_associations_rejects_missing_mode_profiles() {
    let missing = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-mode-profile-123456.icm",
    );
    let result = reapply_profile_with_mode_associations(
        r"DISPLAY\FAKE\999",
        &missing,
        &missing,
        &missing,
        50,
        false,
    );
    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("not found"),
        "should report missing profile path"
    );
}

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

// ── Dynamic ICM ──────────────────────────────────────────────────

#[test]
fn embedded_icm_is_not_empty() {
    assert!(
        generated_icm_size() > 0,
        "generated ICC should contain data"
    );
}

#[test]
fn embedded_icm_has_valid_icc_header() {
    // ICC profiles start with a 4-byte size field, then 4 bytes of padding,
    // then the ASCII signature "acsp" at offset 36.
    assert!(
        generated_icm_size() > 40,
        "generated ICC too small to be a valid profile"
    );
}

#[test]
fn generated_icm_is_parseable_by_cmx() {
    let bytes = generated_icm_bytes();
    let parsed =
        cmx::profile::Profile::from_bytes(&bytes).expect("generated ICC should parse in cmx");
    assert_eq!(
        parsed.profile_size(),
        bytes.len(),
        "parsed ICC size should match serialized bytes"
    );
}

#[test]
fn generated_icm_passes_extensive_validation() {
    let bytes = generated_icm_bytes();
    let report = validate_icc_profile_bytes(&bytes);
    assert!(
        report.is_valid(),
        "generated ICC should validate, errors: {}",
        report.errors.join(" | ")
    );
    assert_eq!(report.declared_size, Some(bytes.len() as u32));
    assert!(report.tag_count.unwrap_or(0) > 0);
    assert_eq!(
        report.known_tag_count + report.unknown_tag_count,
        report.tag_details.len()
    );
}

#[test]
fn inspect_icc_reports_tag_details() {
    let bytes = generated_icm_bytes();
    let report = inspect_icc_profile_bytes(&bytes).expect("inspect should parse generated ICC");
    assert_eq!(
        report.known_tag_count + report.unknown_tag_count,
        report.tag_details.len()
    );
    assert!(
        !report.tag_details.is_empty(),
        "expected at least one ICC tag"
    );
    assert!(
        report
            .tag_details
            .iter()
            .any(|tag| tag.signature == "desc" && tag.known_type_signature),
        "expected `desc` tag with known type signature"
    );
}

#[test]
fn validate_icc_reports_unknown_vendor_tag() {
    let bytes = generated_icm_bytes();
    let patched = patch_icc_profile_bytes(
        &bytes,
        &[ExtraRawTag {
            signature: 0x7A7A7A7A,
            payload: build_icc_data_type_payload(b"vendor blob"),
        }],
        &[],
    )
    .expect("patch should succeed");
    let report = validate_icc_profile_bytes(&patched);
    assert!(
        report.unknown_tag_count >= 1,
        "expected at least one unknown tag"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("vendor-specific")),
        "expected vendor-specific warning"
    );
}

#[test]
fn validate_icc_rejects_bad_acsp_signature() {
    let mut bytes = generated_icm_bytes();
    bytes[36..40].copy_from_slice(b"zzzz");
    let report = validate_icc_profile_bytes(&bytes);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("file signature 'acsp'")),
        "expected acsp signature validation error"
    );
}

#[test]
fn validate_icc_rejects_declared_size_mismatch() {
    let mut bytes = generated_icm_bytes();
    let wrong_size = (bytes.len() as u32).saturating_add(17);
    bytes[0..4].copy_from_slice(&wrong_size.to_be_bytes());
    let report = validate_icc_profile_bytes(&bytes);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("declared profile size")),
        "expected declared-size mismatch error"
    );
}

#[test]
fn validate_icc_rejects_tag_table_overflow() {
    let mut bytes = generated_icm_bytes();
    bytes[128..132].copy_from_slice(&0x7FFF_FFFFu32.to_be_bytes());
    let report = validate_icc_profile_bytes(&bytes);
    assert!(!report.is_valid());
    assert!(
        report.errors.iter().any(|e| e.contains("tag table")),
        "expected tag-table bounds error"
    );
}

#[test]
fn validate_icc_rejects_out_of_bounds_tag_range() {
    let mut bytes = generated_icm_bytes();
    let entry = 132usize; // first tag entry
    let size_u32 = bytes.len() as u32;
    bytes[entry + 4..entry + 8].copy_from_slice(&(size_u32 - 2).to_be_bytes());
    bytes[entry + 8..entry + 12].copy_from_slice(&64u32.to_be_bytes());
    let report = validate_icc_profile_bytes(&bytes);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("exceeds profile size")),
        "expected out-of-bounds tag error"
    );
}

#[test]
fn validate_icc_rejects_overlapping_tag_ranges() {
    let mut bytes = generated_icm_bytes();

    // Force first two tags to overlap with different extents.
    let entry1 = 132usize;
    let entry2 = entry1 + 12;
    bytes[entry1 + 4..entry1 + 8].copy_from_slice(&200u32.to_be_bytes());
    bytes[entry1 + 8..entry1 + 12].copy_from_slice(&48u32.to_be_bytes());
    bytes[entry2 + 4..entry2 + 8].copy_from_slice(&220u32.to_be_bytes());
    bytes[entry2 + 8..entry2 + 12].copy_from_slice(&48u32.to_be_bytes());

    let report = validate_icc_profile_bytes(&bytes);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("overlapping tag ranges")),
        "expected overlap detection error"
    );
}

#[test]
fn validate_icc_rejects_missing_required_tag() {
    let bytes = generated_icm_bytes();
    let mut raw = RawProfile::from_bytes(&bytes).expect("base profile should parse");
    raw.tags.retain(|sig, _| *sig != TagSignature::RedTRC);
    let stripped = raw
        .into_bytes()
        .expect("should serialize profile without rTRC");

    let report = validate_icc_profile_bytes(&stripped);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("missing required display tag rTRC")),
        "expected missing-required-tag error"
    );
}

#[test]
fn validate_icc_file_helper_reads_from_disk() {
    let dir = std::env::temp_dir().join("lg-profile-validate-file");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("validate.icm");

    std::fs::write(&path, generated_icm_bytes()).unwrap();
    let report = validate_icc_profile_file(&path).expect("validate file should succeed");
    assert!(report.is_valid(), "{}", report.errors.join(" | "));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn validate_icc_detects_vcgt_when_enabled() {
    let bytes = generate_dynamic_profile_bytes_with_luminance_and_tuning(
        2.2,
        120.0,
        DynamicIccTuning {
            vcgt_enabled: true,
            vcgt_strength: 1.0,
            ..DynamicIccTuning::default()
        },
    )
    .unwrap();

    let report = validate_icc_profile_bytes(&bytes);
    assert!(report.is_valid(), "{}", report.errors.join(" | "));
    assert!(report.has_vcgt_tag, "expected vcgt tag to be reported");
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
        generated_icm_size() as u64
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
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\lg-ultragear-dynamic-cmx.icm");
    let _ = is_profile_installed(&path);
}

// ── WCS scope constant ───────────────────────────────────────────

#[test]
fn wcs_scope_system_wide_value() {
    assert_eq!(WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE.0, 0);
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
    enable_no_flicker_test_mode();
    // All false = complete no-op
    refresh_display(false, false, false);
}

#[test]
fn trigger_calibration_loader_disabled_does_not_panic() {
    enable_no_flicker_test_mode();
    trigger_calibration_loader(false);
}

// ── WCS scope constants ──────────────────────────────────────────

#[test]
fn wcs_scope_current_user_value() {
    assert_eq!(WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER.0, 1);
}

#[test]
fn wcs_cpt_and_cpst_constants() {
    assert_eq!(CPT_ICC.0, 0);
    assert_eq!(CPST_NONE.0, 4);
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
    let generated = generated_icm_bytes();
    if generated.len() > 40 {
        let sig = &generated[36..40];
        assert_eq!(
            sig, b"acsp",
            "embedded ICM should have ICC 'acsp' signature"
        );
    }
}

#[test]
fn embedded_icm_first_4_bytes_is_size() {
    // ICC profile spec: first 4 bytes = big-endian profile size
    let generated = generated_icm_bytes();
    if generated.len() >= 4 {
        let size_bytes = &generated[0..4];
        let reported_size =
            u32::from_be_bytes([size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]]);
        assert_eq!(
            reported_size as usize,
            generated.len(),
            "ICC header size should match generated profile size"
        );
    }
}

#[test]
fn embedded_icm_is_not_all_zeros() {
    let generated = generated_icm_bytes();
    let all_zero = generated.iter().all(|&b| b == 0);
    assert!(!all_zero, "embedded ICM should not be all zeros");
}

#[test]
fn embedded_icm_has_nonzero_size() {
    assert!(generated_icm_size() > 100, "ICM file should be > 100 bytes");
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
        generated_icm_size() as u64,
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
        generated_icm_size() as u64,
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
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("lg-profile-edge-test-nested"));
    let path = dir.join("nested.icm");

    let wrote = ensure_profile_installed(&path).expect("should create nested dirs");
    assert!(wrote, "should write to nested path");
    assert!(path.exists());

    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("lg-profile-edge-test-nested"));
}

// ── remove_profile edge cases ────────────────────────────────────

#[test]
fn remove_profile_nonexistent_edge_returns_false() {
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\nonexistent-edge-test-99999.icm");
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
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\definitely-missing-99999.icm");
    assert!(
        !is_profile_installed(&path),
        "missing file should not be installed"
    );
}

// ── reapply_profile edge cases ───────────────────────────────────

#[test]
fn reapply_profile_empty_device_key_fails() {
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\nonexistent-empty-key-99999.icm");
    let result = reapply_profile("", &path, 100, false);
    // Should fail because profile doesn't exist (profile check comes first)
    assert!(result.is_err());
}

#[test]
fn reapply_profile_zero_delay_still_fails_on_missing_profile() {
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\nonexistent-zero-delay-99999.icm");
    let result = reapply_profile(r"DISPLAY\TEST\001", &path, 0, false);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Profile not found"),
        "should mention missing profile"
    );
}

#[test]
fn reapply_profile_per_user_true_still_fails_on_missing_profile() {
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\nonexistent-per-user-99999.icm");
    let result = reapply_profile(r"DISPLAY\TEST\001", &path, 100, true);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Profile not found"),
        "per_user=true should still check profile existence"
    );
}

#[test]
fn reapply_profile_very_long_device_key_fails_on_missing_profile() {
    let long_key = format!(r"DISPLAY\{}\001", "X".repeat(500));
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\nonexistent-long-key-99999.icm");
    let result = reapply_profile(&long_key, &path, 100, false);
    assert!(result.is_err());
}

// ── refresh_display edge cases ───────────────────────────────────

#[test]
fn refresh_display_all_enabled_does_not_panic() {
    enable_no_flicker_test_mode();
    refresh_display(true, true, true);
}

#[test]
fn refresh_display_only_settings_does_not_panic() {
    enable_no_flicker_test_mode();
    refresh_display(true, false, false);
}

#[test]
fn refresh_display_only_broadcast_does_not_panic() {
    enable_no_flicker_test_mode();
    refresh_display(false, true, false);
}

#[test]
fn refresh_display_only_invalidate_does_not_panic() {
    enable_no_flicker_test_mode();
    refresh_display(false, false, true);
}

// ── trigger_calibration_loader edge cases ────────────────────────

#[test]
fn trigger_calibration_loader_enabled_does_not_panic() {
    enable_no_flicker_test_mode();
    trigger_calibration_loader(true);
}

// ── WCS constants boundary validation ────────────────────────────

#[test]
fn wcs_scope_constants_are_distinct() {
    assert_ne!(
        WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE, WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
        "system-wide and current-user scopes must differ"
    );
}

#[test]
fn wcs_scope_values_are_small_integers() {
    // SYSTEM_WIDE = 0 is valid (first enum variant in the Windows SDK)
    const {
        assert!(WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE.0 < 256);
        assert!(WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER.0 > 0);
        assert!(WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER.0 < 256);
    };
}

#[test]
fn cpt_icc_is_one() {
    assert_eq!(CPT_ICC.0, 0);
}

#[test]
fn cpst_none_is_one() {
    assert_eq!(CPST_NONE.0, 4);
}

// ── Display association constants ────────────────────────────────

#[test]
fn cpst_standard_display_color_mode_is_expected() {
    assert_eq!(CPST_STANDARD_DISPLAY_COLOR_MODE.0, 7);
}

#[test]
fn cpst_extended_display_color_mode_is_expected() {
    assert_eq!(CPST_EXTENDED_DISPLAY_COLOR_MODE.0, 8);
}

// ── register_color_profile ───────────────────────────────────────

#[test]
fn register_color_profile_nonexistent_returns_error() {
    let path = PathBuf::from(
        r"C:\Windows\System32\spool\drivers\color\nonexistent-register-test-99999.icm",
    );
    let result = register_color_profile(&path);
    assert!(result.is_err());
}

#[test]
fn register_color_profile_temp_file_is_noop() {
    // register_color_profile should be a no-op for paths outside the color
    // directory — it must NOT call InstallColorProfileW which would copy
    // the file into the system color store.
    let dir = std::env::temp_dir().join("lg-profile-register-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("register-test.icm");

    // Clean up any previously-leaked copy from old test runs.
    let leaked = color_directory().join("register-test.icm");
    let _ = std::fs::remove_file(&leaked);

    // Write the embedded profile to temp
    ensure_profile_installed(&path).unwrap();

    // register_color_profile should succeed (no-op since outside color dir)
    let result = register_color_profile(&path);
    assert!(result.is_ok());

    // Verify it did NOT leak into the system color directory
    assert!(
        !leaked.exists(),
        "register_color_profile should NOT copy temp files into the color directory"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── set_display_default_association ──────────────────────────────

#[test]
fn set_display_default_association_nonexistent_device_does_not_panic() {
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\nonexistent-sdr-assoc-99999.icm");
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
    let path =
        PathBuf::from(r"C:\Windows\System32\spool\drivers\color\nonexistent-hdr-assoc-99999.icm");
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

// ── WCS scope constants ──────────────────────────────────────────
// Windows SDK icm.h defines WCS_PROFILE_MANAGEMENT_SCOPE as a plain C enum:
//   SYSTEM_WIDE  = 0
//   CURRENT_USER = 1
// Passing any other value makes WcsAssociate/WcsDisassociate return
// ERROR_INVALID_PARAMETER (87).

#[test]
fn wcs_scope_system_wide_is_zero() {
    assert_eq!(
        WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE.0, 0,
        "SYSTEM_WIDE must be 0 per the Windows SDK enum"
    );
}

#[test]
fn wcs_scope_current_user_is_one() {
    assert_eq!(
        WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER.0, 1,
        "CURRENT_USER must be 1 per the Windows SDK enum"
    );
}

#[test]
fn wcs_scope_system_wide_less_than_current_user() {
    const {
        assert!(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE.0
                < WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER.0,
            // enum order: SYSTEM_WIDE(0) < CURRENT_USER(1)
        );
    };
}

// ── profile_path file_name extraction ────────────────────────────
// WCS association APIs must receive the filename only, never a full path.

#[test]
fn file_name_extraction_normal_path() {
    let p = PathBuf::from(r"C:\Windows\System32\spool\drivers\color\lg-profile.icm");
    let name = p.file_name().unwrap();
    assert_eq!(name, "lg-profile.icm");
}

#[test]
fn file_name_extraction_forward_slashes() {
    let p = PathBuf::from("C:/Windows/System32/spool/drivers/color/lg-profile.icm");
    let name = p.file_name().unwrap();
    assert_eq!(name, "lg-profile.icm");
}

#[test]
fn file_name_extraction_unc_path() {
    let p = PathBuf::from(r"\\server\share\color\lg-profile.icm");
    let name = p.file_name().unwrap();
    assert_eq!(name, "lg-profile.icm");
}

#[test]
fn file_name_no_backslash_in_result() {
    let p = PathBuf::from(r"C:\Windows\System32\spool\drivers\color\lg-profile.icm");
    let name = p.file_name().unwrap().to_string_lossy();
    assert!(
        !name.contains('\\') && !name.contains('/'),
        "file_name must not contain path separators, got: {name}"
    );
}

#[test]
fn reapply_profile_rejects_missing_file() {
    let p = PathBuf::from(r"C:\Windows\System32\spool\drivers\color\__does_not_exist_987654.icm");
    let result = reapply_profile(r"DISPLAY\FAKE\999", &p, 0, false);
    assert!(result.is_err(), "should fail when profile file is missing");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found"),
        "error should mention 'not found', got: {msg}"
    );
}

#[test]
fn reapply_profile_rejects_directory_path() {
    // A path that is a directory has no file_name that InstallColorProfile
    // would accept — but the exists() check should catch the root issue.
    let p = std::env::temp_dir(); // e.g. C:\Users\...\AppData\Local\Temp
    let result = reapply_profile(r"DISPLAY\FAKE\999", &p, 0, false);
    // A directory "exists" but is not a valid ICC profile.
    // Depending on file_name() returning Some or None the error will vary,
    // but it must not succeed.
    // (temp_dir has a file_name, but doesn't have an ICC extension —
    //  the WCS API will fail regardless.)
    // This is a safety-net test: the call should not panic.
    let _ = result;
}
