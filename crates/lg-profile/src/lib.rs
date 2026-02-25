//! Color profile management — disassociate, reassociate, and refresh.
//!
//! Uses `mscms.dll` (WCS APIs) directly via the `windows` crate for reliable
//! color profile toggling, plus display refresh via `user32.dll`.
//!
//! All functions take raw parameters (no Config dependency) so this crate
//! can be used independently.

use chrono::{TimeZone, Timelike};
use cmx::profile::{DisplayProfile, RawProfile};
use cmx::signatures::Signature;
use cmx::signatures::{ColorSpace, DeviceClass};
use cmx::tag::tagdata::DataSignature;
use cmx::tag::tags::{
    BlueTRCTag, CalibrationDateTimeTag, CharTargetTag, ChromaticAdaptationTag, ChromaticityTag,
    CicpTag, ColorimetricIntentImageStateTag, DeviceMfgDescTag, DeviceModelDescTag, GreenTRCTag,
    LuminanceTag, MeasurementTag, MediaBlackPointTag, MetadataTag, ProfileDescriptionTag,
    RedTRCTag, TechnologyTag, VcgtTag, ViewingCondDescTag, ViewingConditionsTag,
};
use cmx::tag::RenderingIntent;
use cmx::tag::TagSignature;
use log::{info, warn};
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{ptr, thread, time::Duration};
use windows::core::{BSTR, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig, SetDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS, SDC_ALLOW_CHANGES, SDC_APPLY,
    SDC_NO_OPTIMIZATION, SDC_USE_DATABASE_CURRENT, SDC_USE_SUPPLIED_DISPLAY_CONFIG,
    SET_DISPLAY_CONFIG_FLAGS,
};
use windows::Win32::Foundation::{
    LocalFree, BOOL, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HWND, LPARAM, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, CreateDCW, DeleteDC, InvalidateRect,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::TaskScheduler::{ITaskService, TaskScheduler};
use windows::Win32::UI::ColorSystem::{
    AssociateColorProfileWithDeviceW, ColorProfileAddDisplayAssociation,
    ColorProfileGetDisplayDefault, ColorProfileSetDisplayDefaultAssociation, GetICMProfileW,
    InstallColorProfileW, SetDeviceGammaRamp, SetICMProfileW, WcsAssociateColorProfileWithDevice,
    WcsDisassociateColorProfileFromDevice, WcsGetDefaultColorProfile,
    WcsGetDefaultColorProfileSize, WcsGetUsePerUserProfiles, WcsSetCalibrationManagementState,
    WcsSetDefaultColorProfile, WcsSetUsePerUserProfiles, CPST_EXTENDED_DISPLAY_COLOR_MODE,
    CPST_NONE, CPST_STANDARD_DISPLAY_COLOR_MODE, CPT_ICC, WCS_PROFILE_MANAGEMENT_SCOPE,
    WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER, WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

// ============================================================================
// Dynamic ICC profile generation
// ============================================================================

/// Default gamma used for generated ICC transfer curves.
pub const DEFAULT_DYNAMIC_GAMMA: f64 = 2.05;

/// Specialized preset gamma values.
pub const PRESET_GAMMA_22: f64 = 2.2;
pub const PRESET_GAMMA_24: f64 = 2.4;
pub const PRESET_GAMMA_READER: f64 = 2.2;

/// Minimum allowed gamma for dynamic ICC generation.
pub const MIN_DYNAMIC_GAMMA: f64 = 1.2;

/// Maximum allowed gamma for dynamic ICC generation.
pub const MAX_DYNAMIC_GAMMA: f64 = 3.0;

/// Default luminance (cd/m^2) encoded in the ICC luminance tag.
pub const DEFAULT_DYNAMIC_LUMINANCE_CD_M2: f64 = 120.0;

/// Minimum allowed luminance for ICC generation (cd/m^2).
pub const MIN_DYNAMIC_LUMINANCE_CD_M2: f64 = 80.0;

/// Maximum allowed luminance for ICC generation (cd/m^2).
pub const MAX_DYNAMIC_LUMINANCE_CD_M2: f64 = 600.0;

/// Specialized profile filenames.
pub const GAMMA22_PROFILE_NAME: &str = "lg-ultragear-gamma22-cmx.icm";
pub const GAMMA24_PROFILE_NAME: &str = "lg-ultragear-gamma24-cmx.icm";
pub const READER_PROFILE_NAME: &str = "lg-ultragear-reader-cmx.icm";

/// Curve table size used for generated ICC TRCs/VCGT LUTs.
const CURVE_TABLE_SIZE: usize = 256;
const ICC_HEADER_SIZE: usize = 128;
const ICC_TAG_COUNT_OFFSET: usize = ICC_HEADER_SIZE;
const ICC_TAG_RECORD_SIZE: usize = 12;
const ICC_MIN_SIZE: usize = ICC_HEADER_SIZE + 4;
const ICC_ACSP_OFFSET: usize = 36;
const TAG_SIG_SDIN: u32 = 0x7364_696E; // "sdin"
const TAG_SIG_SWPT: u32 = 0x7377_7074; // "swpt"
const TAG_SIG_SVCN: u32 = 0x7376_636E; // "svcn"

/// Daytime start hour for day/night preset selection (local time, 24h clock).
const DAY_PRESET_START_HOUR: u32 = 7;

/// Daytime end hour (exclusive) for day/night preset selection.
const NIGHT_PRESET_START_HOUR: u32 = 19;

/// Legacy profile names that should be removed when the dynamic profile is active.
const LEGACY_PROFILE_NAMES: &[&str] = &["lg-ultragear-full-cal.icm"];
const CLASS_MONITOR_SIGNATURE: u32 = 0x6D6E_7472; // 'mntr'
const TEST_NO_FLICKER_ENV: &str = "LG_TEST_NO_FLICKER_REFRESH";
static TEST_NO_FLICKER_MODE: AtomicBool = AtomicBool::new(false);

/// Dynamic ICC presets used by auto-generation and apply flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicIccPreset {
    Gamma22,
    Gamma24,
    Reader,
    Custom,
}

/// Curated anti-dimming, color-space, and readability tuning presets for
/// dynamic ICC generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicIccTuningPreset {
    Manual,
    AntiDimSoft,
    AntiDimBalanced,
    AntiDimAggressive,
    AntiDimNight,
    ReaderBalanced,
    ColorRgbFull,
    ColorRgbLimited,
    ColorYcbcr444,
    ColorYcbcr422,
    ColorYcbcr420,
    ColorBt2020Pq,
    UnyellowSoft,
    UnyellowBalanced,
    UnyellowAggressive,
    BlackDepth,
    WhiteClarity,
    AntiFadePunch,
    AntiFadeCinematic,
}

/// Optional monitor identity used for per-display ICC naming and embedded metadata.
#[derive(Debug, Clone, Default)]
pub struct DynamicMonitorIdentity {
    pub monitor_name: String,
    pub device_key: String,
    pub serial_number: String,
    pub manufacturer_id: String,
    pub product_code: String,
}

/// Additional shaping parameters for generated dynamic ICC/VCGT curves.
#[derive(Debug, Clone, Copy)]
pub struct DynamicIccTuning {
    /// Lift near-black output to counter aggressive dimming.
    pub black_lift: f64,
    /// Midtone boost/dip amount (positive = boost).
    pub midtone_boost: f64,
    /// Compress highlight region to reduce clipping when lifting midtones.
    pub white_compression: f64,
    /// Per-channel gamma multipliers.
    pub gamma_r: f64,
    pub gamma_g: f64,
    pub gamma_b: f64,
    /// Optional VCGT payload generation.
    pub vcgt_enabled: bool,
    /// Blend amount between identity LUT and generated LUT (0..1).
    pub vcgt_strength: f64,
    /// Target black floor in cd/m^2, mapped against white luminance.
    pub target_black_cd_m2: f64,
    /// Include media black point tag (`bkpt`) derived from target black luminance.
    pub include_media_black_point: bool,
    /// Include device manufacturer/model description tags.
    pub include_device_descriptions: bool,
    /// Include characterization target text tag (`targ`).
    pub include_characterization_target: bool,
    /// Include viewing condition description tag (`vued`).
    pub include_viewing_cond_desc: bool,
    /// Optional Technology tag (`tech`) signature (0 = disabled).
    pub technology_signature: u32,
    /// Optional Colorimetric Intent Image State tag (`ciis`) signature (0 = disabled).
    pub ciis_signature: u32,
    /// Include CICP tag (`cicp`) for video/HDR signaling.
    pub cicp_enabled: bool,
    /// CICP primaries code.
    pub cicp_color_primaries: u8,
    /// CICP transfer characteristics code.
    pub cicp_transfer_characteristics: u8,
    /// CICP matrix coefficients code.
    pub cicp_matrix_coefficients: u8,
    /// CICP full-range flag.
    pub cicp_full_range: bool,
    /// Include metadata tag (`meta`) with an empty dict payload.
    pub metadata_enabled: bool,
    /// Include calibration date/time tag (`calt`).
    pub include_calibration_datetime: bool,
    /// Include chromatic adaptation matrix tag (`chad`).
    pub include_chromatic_adaptation: bool,
    /// Include chromaticity primaries tag (`chrm`).
    pub include_chromaticity: bool,
    /// Include measurement condition tag (`meas`).
    pub include_measurement: bool,
    /// Include viewing conditions tag (`view`).
    pub include_viewing_conditions: bool,
    /// Include ICC v5 spectral scaffolding tags (`sdin`, `swpt`, `svcn`) as `dataType`.
    pub include_spectral_scaffold: bool,
}

impl Default for DynamicIccTuning {
    fn default() -> Self {
        Self {
            black_lift: 0.0,
            midtone_boost: 0.0,
            white_compression: 0.0,
            gamma_r: 1.0,
            gamma_g: 1.0,
            gamma_b: 1.0,
            vcgt_enabled: false,
            vcgt_strength: 0.0,
            target_black_cd_m2: 0.2,
            include_media_black_point: true,
            include_device_descriptions: true,
            include_characterization_target: true,
            include_viewing_cond_desc: true,
            technology_signature: 0x7669646D, // "vidm"
            ciis_signature: 0,
            cicp_enabled: false,
            cicp_color_primaries: 1,
            cicp_transfer_characteristics: 13,
            cicp_matrix_coefficients: 0,
            cicp_full_range: true,
            metadata_enabled: false,
            include_calibration_datetime: true,
            include_chromatic_adaptation: true,
            include_chromaticity: true,
            include_measurement: true,
            include_viewing_conditions: true,
            include_spectral_scaffold: false,
        }
    }
}

/// Validation report for a single ICC profile payload.
#[derive(Debug, Clone, Default)]
pub struct IccValidationReport {
    /// On-disk/in-memory size in bytes.
    pub actual_size: usize,
    /// Declared header size (`bytes[0..4]`) when available.
    pub declared_size: Option<u32>,
    /// Tag count from the tag table header when available.
    pub tag_count: Option<u32>,
    /// Whether a `vcgt` tag was found by parser-level inspection.
    pub has_vcgt_tag: bool,
    /// Whether a `cicp` tag was found.
    pub has_cicp_tag: bool,
    /// Whether a `meta` tag was found.
    pub has_metadata_tag: bool,
    /// Whether ICC v5 spectral scaffolding tags were found.
    pub has_spectral_data_info_tag: bool,
    pub has_spectral_white_point_tag: bool,
    pub has_spectral_viewing_conditions_tag: bool,
    /// Count of parser-recognized ICC tag signatures.
    pub known_tag_count: usize,
    /// Count of unrecognized/vendor-specific tag signatures.
    pub unknown_tag_count: usize,
    /// Detailed per-tag inspection data.
    pub tag_details: Vec<IccTagDetail>,
    /// Hard validation failures.
    pub errors: Vec<String>,
    /// Non-fatal anomalies.
    pub warnings: Vec<String>,
}

impl IccValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
struct IccTagRecord {
    signature: [u8; 4],
    offset: u32,
    size: u32,
}

/// Per-tag inspection metadata shared by `inspect` and `validate` flows.
#[derive(Debug, Clone, Default)]
pub struct IccTagDetail {
    pub signature_u32: u32,
    pub signature: String,
    pub known_signature: bool,
    pub payload_size: usize,
    pub type_signature_u32: Option<u32>,
    pub type_signature: Option<String>,
    pub known_type_signature: bool,
    pub reserved_bytes_zero: Option<bool>,
}

impl DynamicIccPreset {
    pub fn profile_name(self, custom_profile_name: &str) -> String {
        match self {
            DynamicIccPreset::Gamma22 => GAMMA22_PROFILE_NAME.to_string(),
            DynamicIccPreset::Gamma24 => GAMMA24_PROFILE_NAME.to_string(),
            DynamicIccPreset::Reader => READER_PROFILE_NAME.to_string(),
            DynamicIccPreset::Custom => {
                let trimmed = custom_profile_name.trim();
                if trimmed.is_empty() {
                    "lg-ultragear-dynamic-cmx.icm".to_string()
                } else {
                    trimmed.to_string()
                }
            }
        }
    }

    pub fn gamma(self, custom_gamma: f64) -> f64 {
        match self {
            DynamicIccPreset::Gamma22 => PRESET_GAMMA_22,
            DynamicIccPreset::Gamma24 => PRESET_GAMMA_24,
            DynamicIccPreset::Reader => PRESET_GAMMA_READER,
            DynamicIccPreset::Custom => sanitize_dynamic_gamma(custom_gamma),
        }
    }
}

/// Parse a textual preset name into a typed preset.
pub fn parse_dynamic_icc_preset(value: &str) -> DynamicIccPreset {
    match value.trim().to_ascii_lowercase().as_str() {
        "gamma22" | "gamma_22" | "2.2" | "22" | "g22" => DynamicIccPreset::Gamma22,
        "gamma24" | "gamma_24" | "2.4" | "24" | "g24" => DynamicIccPreset::Gamma24,
        "reader" | "reader_mode" | "reader-mode" => DynamicIccPreset::Reader,
        _ => DynamicIccPreset::Custom,
    }
}

/// Parse a textual anti-dimming tuning preset.
pub fn parse_dynamic_icc_tuning_preset(value: &str) -> DynamicIccTuningPreset {
    match value.trim().to_ascii_lowercase().as_str() {
        "manual" | "custom" | "off" => DynamicIccTuningPreset::Manual,
        "soft" | "anti_dim_soft" | "anti-dim-soft" => DynamicIccTuningPreset::AntiDimSoft,
        "balanced" | "anti_dim_balanced" | "anti-dim-balanced" => {
            DynamicIccTuningPreset::AntiDimBalanced
        }
        "aggressive" | "anti_dim_aggressive" | "anti-dim-aggressive" => {
            DynamicIccTuningPreset::AntiDimAggressive
        }
        "night" | "anti_dim_night" | "anti-dim-night" => DynamicIccTuningPreset::AntiDimNight,
        "reader" | "reader_balanced" | "reader-balanced" => DynamicIccTuningPreset::ReaderBalanced,
        "color_rgb_full" | "rgb_full" | "rgb-full" => DynamicIccTuningPreset::ColorRgbFull,
        "color_rgb_limited" | "rgb_limited" | "rgb-limited" => {
            DynamicIccTuningPreset::ColorRgbLimited
        }
        "color_ycbcr444" | "colorspace_ycbcr444" | "ycbcr444" | "ycbcr_444" | "ycbcr-444" => {
            DynamicIccTuningPreset::ColorYcbcr444
        }
        "color_ycbcr422" | "colorspace_ycbcr422" | "ycbcr422" | "ycbcr_422" | "ycbcr-422" => {
            DynamicIccTuningPreset::ColorYcbcr422
        }
        "color_ycbcr420" | "colorspace_ycbcr420" | "ycbcr420" | "ycbcr_420" | "ycbcr-420" => {
            DynamicIccTuningPreset::ColorYcbcr420
        }
        "color_bt2020_pq" | "colorspace_bt2020_pq" | "bt2020_pq" | "bt2020-pq" => {
            DynamicIccTuningPreset::ColorBt2020Pq
        }
        "unyellow_soft" | "unyellow-soft" => DynamicIccTuningPreset::UnyellowSoft,
        "unyellow_balanced" | "unyellow-balanced" => DynamicIccTuningPreset::UnyellowBalanced,
        "unyellow_aggressive" | "unyellow-aggressive" => {
            DynamicIccTuningPreset::UnyellowAggressive
        }
        "black_depth" | "black-depth" | "deep_black" | "deep-black" => {
            DynamicIccTuningPreset::BlackDepth
        }
        "white_clarity" | "white-clarity" | "bright_whites" | "bright-whites" => {
            DynamicIccTuningPreset::WhiteClarity
        }
        "anti_fade_punch" | "anti-fade-punch" | "fade_fix" | "fade-fix" => {
            DynamicIccTuningPreset::AntiFadePunch
        }
        "anti_fade_cinematic" | "anti-fade-cinematic" => {
            DynamicIccTuningPreset::AntiFadeCinematic
        }
        _ => DynamicIccTuningPreset::AntiDimBalanced,
    }
}

/// Return the list of supported tuning preset names.
pub fn dynamic_icc_tuning_preset_names() -> &'static [&'static str] {
    &[
        "manual",
        "anti_dim_soft",
        "anti_dim_balanced",
        "anti_dim_aggressive",
        "anti_dim_night",
        "reader_balanced",
        "color_rgb_full",
        "color_rgb_limited",
        "color_ycbcr444",
        "color_ycbcr422",
        "color_ycbcr420",
        "color_bt2020_pq",
        "unyellow_soft",
        "unyellow_balanced",
        "unyellow_aggressive",
        "black_depth",
        "white_clarity",
        "anti_fade_punch",
        "anti_fade_cinematic",
    ]
}

/// Build a tuned `DynamicIccTuning` profile from a preset.
pub fn dynamic_icc_tuning_for_preset(preset: DynamicIccTuningPreset) -> DynamicIccTuning {
    let mut tuning = DynamicIccTuning::default();
    match preset {
        DynamicIccTuningPreset::Manual => {}
        DynamicIccTuningPreset::AntiDimSoft => {
            tuning.black_lift = 0.025;
            tuning.midtone_boost = 0.08;
            tuning.white_compression = 0.12;
            tuning.target_black_cd_m2 = 0.30;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.25;
        }
        DynamicIccTuningPreset::AntiDimBalanced => {
            tuning.black_lift = 0.045;
            tuning.midtone_boost = 0.15;
            tuning.white_compression = 0.22;
            tuning.target_black_cd_m2 = 0.45;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.45;
        }
        DynamicIccTuningPreset::AntiDimAggressive => {
            tuning.black_lift = 0.08;
            tuning.midtone_boost = 0.26;
            tuning.white_compression = 0.36;
            tuning.target_black_cd_m2 = 0.75;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.68;
        }
        DynamicIccTuningPreset::AntiDimNight => {
            tuning.black_lift = 0.065;
            tuning.midtone_boost = 0.20;
            tuning.white_compression = 0.32;
            tuning.target_black_cd_m2 = 0.65;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.55;
        }
        DynamicIccTuningPreset::ReaderBalanced => {
            // Reader-first preset: intentionally cooler white balance to
            // counter warm/yellow cast, with moderate brightness uplift.
            tuning.black_lift = 0.040;
            tuning.midtone_boost = 0.20;
            tuning.white_compression = 0.14;
            tuning.target_black_cd_m2 = 0.30;
            tuning.gamma_r = 0.93;
            tuning.gamma_g = 0.97;
            tuning.gamma_b = 1.12;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.58;
        }
        DynamicIccTuningPreset::ColorRgbFull => {
            tuning.black_lift = 0.030;
            tuning.midtone_boost = 0.10;
            tuning.white_compression = 0.10;
            tuning.target_black_cd_m2 = 0.30;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.35;
            tuning.cicp_enabled = true;
            tuning.cicp_color_primaries = 1;
            tuning.cicp_transfer_characteristics = 13;
            tuning.cicp_matrix_coefficients = 0;
            tuning.cicp_full_range = true;
        }
        DynamicIccTuningPreset::ColorRgbLimited => {
            tuning.black_lift = 0.022;
            tuning.midtone_boost = 0.09;
            tuning.white_compression = 0.18;
            tuning.target_black_cd_m2 = 0.25;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.28;
            tuning.cicp_enabled = true;
            tuning.cicp_color_primaries = 1;
            tuning.cicp_transfer_characteristics = 13;
            tuning.cicp_matrix_coefficients = 0;
            tuning.cicp_full_range = false;
        }
        DynamicIccTuningPreset::ColorYcbcr444 => {
            tuning.black_lift = 0.035;
            tuning.midtone_boost = 0.13;
            tuning.white_compression = 0.18;
            tuning.target_black_cd_m2 = 0.38;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.40;
            tuning.gamma_r = 0.99;
            tuning.gamma_b = 1.03;
            tuning.cicp_enabled = true;
            tuning.cicp_color_primaries = 1;
            tuning.cicp_transfer_characteristics = 13;
            tuning.cicp_matrix_coefficients = 1;
            tuning.cicp_full_range = false;
        }
        DynamicIccTuningPreset::ColorYcbcr422 => {
            tuning.black_lift = 0.050;
            tuning.midtone_boost = 0.16;
            tuning.white_compression = 0.24;
            tuning.target_black_cd_m2 = 0.45;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.48;
            tuning.gamma_r = 0.98;
            tuning.gamma_b = 1.05;
            tuning.cicp_enabled = true;
            tuning.cicp_color_primaries = 1;
            tuning.cicp_transfer_characteristics = 13;
            tuning.cicp_matrix_coefficients = 1;
            tuning.cicp_full_range = false;
        }
        DynamicIccTuningPreset::ColorYcbcr420 => {
            tuning.black_lift = 0.065;
            tuning.midtone_boost = 0.22;
            tuning.white_compression = 0.30;
            tuning.target_black_cd_m2 = 0.55;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.58;
            tuning.gamma_r = 0.98;
            tuning.gamma_b = 1.07;
            tuning.cicp_enabled = true;
            tuning.cicp_color_primaries = 1;
            tuning.cicp_transfer_characteristics = 13;
            tuning.cicp_matrix_coefficients = 1;
            tuning.cicp_full_range = false;
        }
        DynamicIccTuningPreset::ColorBt2020Pq => {
            tuning.black_lift = 0.030;
            tuning.midtone_boost = 0.12;
            tuning.white_compression = 0.20;
            tuning.target_black_cd_m2 = 0.35;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.42;
            tuning.metadata_enabled = true;
            tuning.cicp_enabled = true;
            tuning.cicp_color_primaries = 9;
            tuning.cicp_transfer_characteristics = 16;
            tuning.cicp_matrix_coefficients = 9;
            tuning.cicp_full_range = false;
        }
        DynamicIccTuningPreset::UnyellowSoft => {
            tuning.black_lift = 0.030;
            tuning.midtone_boost = 0.12;
            tuning.white_compression = 0.14;
            tuning.target_black_cd_m2 = 0.32;
            tuning.gamma_r = 0.96;
            tuning.gamma_g = 0.99;
            tuning.gamma_b = 1.05;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.45;
        }
        DynamicIccTuningPreset::UnyellowBalanced => {
            tuning.black_lift = 0.040;
            tuning.midtone_boost = 0.18;
            tuning.white_compression = 0.18;
            tuning.target_black_cd_m2 = 0.34;
            tuning.gamma_r = 0.92;
            tuning.gamma_g = 0.96;
            tuning.gamma_b = 1.10;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.58;
        }
        DynamicIccTuningPreset::UnyellowAggressive => {
            tuning.black_lift = 0.050;
            tuning.midtone_boost = 0.23;
            tuning.white_compression = 0.24;
            tuning.target_black_cd_m2 = 0.38;
            tuning.gamma_r = 0.88;
            tuning.gamma_g = 0.93;
            tuning.gamma_b = 1.16;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.72;
        }
        DynamicIccTuningPreset::BlackDepth => {
            tuning.black_lift = 0.005;
            tuning.midtone_boost = 0.05;
            tuning.white_compression = 0.08;
            tuning.target_black_cd_m2 = 0.12;
            tuning.gamma_r = 1.05;
            tuning.gamma_g = 1.05;
            tuning.gamma_b = 1.05;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.22;
        }
        DynamicIccTuningPreset::WhiteClarity => {
            tuning.black_lift = 0.020;
            tuning.midtone_boost = 0.14;
            tuning.white_compression = 0.08;
            tuning.target_black_cd_m2 = 0.22;
            tuning.gamma_r = 0.99;
            tuning.gamma_b = 1.03;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.46;
        }
        DynamicIccTuningPreset::AntiFadePunch => {
            tuning.black_lift = 0.055;
            tuning.midtone_boost = 0.24;
            tuning.white_compression = 0.20;
            tuning.target_black_cd_m2 = 0.40;
            tuning.gamma_r = 0.98;
            tuning.gamma_b = 1.04;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.62;
        }
        DynamicIccTuningPreset::AntiFadeCinematic => {
            tuning.black_lift = 0.030;
            tuning.midtone_boost = 0.17;
            tuning.white_compression = 0.28;
            tuning.target_black_cd_m2 = 0.28;
            tuning.gamma_r = 1.03;
            tuning.gamma_g = 1.03;
            tuning.gamma_b = 1.03;
            tuning.vcgt_enabled = true;
            tuning.vcgt_strength = 0.50;
        }
    }
    tuning
}

fn float_differs(a: f64, b: f64) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return true;
    }
    (a - b).abs() > 1e-9
}

fn merge_manual_tuning_overrides(
    mut preset: DynamicIccTuning,
    manual: DynamicIccTuning,
) -> DynamicIccTuning {
    let defaults = DynamicIccTuning::default();

    if float_differs(manual.black_lift, defaults.black_lift) {
        preset.black_lift = manual.black_lift;
    }
    if float_differs(manual.midtone_boost, defaults.midtone_boost) {
        preset.midtone_boost = manual.midtone_boost;
    }
    if float_differs(manual.white_compression, defaults.white_compression) {
        preset.white_compression = manual.white_compression;
    }
    if float_differs(manual.gamma_r, defaults.gamma_r) {
        preset.gamma_r = manual.gamma_r;
    }
    if float_differs(manual.gamma_g, defaults.gamma_g) {
        preset.gamma_g = manual.gamma_g;
    }
    if float_differs(manual.gamma_b, defaults.gamma_b) {
        preset.gamma_b = manual.gamma_b;
    }
    if manual.vcgt_enabled != defaults.vcgt_enabled {
        preset.vcgt_enabled = manual.vcgt_enabled;
    }
    if float_differs(manual.vcgt_strength, defaults.vcgt_strength) {
        preset.vcgt_strength = manual.vcgt_strength;
    }
    if float_differs(manual.target_black_cd_m2, defaults.target_black_cd_m2) {
        preset.target_black_cd_m2 = manual.target_black_cd_m2;
    }
    if manual.include_media_black_point != defaults.include_media_black_point {
        preset.include_media_black_point = manual.include_media_black_point;
    }
    if manual.include_device_descriptions != defaults.include_device_descriptions {
        preset.include_device_descriptions = manual.include_device_descriptions;
    }
    if manual.include_characterization_target != defaults.include_characterization_target {
        preset.include_characterization_target = manual.include_characterization_target;
    }
    if manual.include_viewing_cond_desc != defaults.include_viewing_cond_desc {
        preset.include_viewing_cond_desc = manual.include_viewing_cond_desc;
    }
    if manual.technology_signature != defaults.technology_signature {
        preset.technology_signature = manual.technology_signature;
    }
    if manual.ciis_signature != defaults.ciis_signature {
        preset.ciis_signature = manual.ciis_signature;
    }
    if manual.cicp_enabled != defaults.cicp_enabled {
        preset.cicp_enabled = manual.cicp_enabled;
    }
    if manual.cicp_color_primaries != defaults.cicp_color_primaries {
        preset.cicp_color_primaries = manual.cicp_color_primaries;
    }
    if manual.cicp_transfer_characteristics != defaults.cicp_transfer_characteristics {
        preset.cicp_transfer_characteristics = manual.cicp_transfer_characteristics;
    }
    if manual.cicp_matrix_coefficients != defaults.cicp_matrix_coefficients {
        preset.cicp_matrix_coefficients = manual.cicp_matrix_coefficients;
    }
    if manual.cicp_full_range != defaults.cicp_full_range {
        preset.cicp_full_range = manual.cicp_full_range;
    }
    if manual.metadata_enabled != defaults.metadata_enabled {
        preset.metadata_enabled = manual.metadata_enabled;
    }
    if manual.include_calibration_datetime != defaults.include_calibration_datetime {
        preset.include_calibration_datetime = manual.include_calibration_datetime;
    }
    if manual.include_chromatic_adaptation != defaults.include_chromatic_adaptation {
        preset.include_chromatic_adaptation = manual.include_chromatic_adaptation;
    }
    if manual.include_chromaticity != defaults.include_chromaticity {
        preset.include_chromaticity = manual.include_chromaticity;
    }
    if manual.include_measurement != defaults.include_measurement {
        preset.include_measurement = manual.include_measurement;
    }
    if manual.include_viewing_conditions != defaults.include_viewing_conditions {
        preset.include_viewing_conditions = manual.include_viewing_conditions;
    }
    if manual.include_spectral_scaffold != defaults.include_spectral_scaffold {
        preset.include_spectral_scaffold = manual.include_spectral_scaffold;
    }

    preset
}

/// Resolve final dynamic ICC tuning from a preset name and optional manual overrides.
pub fn resolve_dynamic_icc_tuning(
    manual_tuning: DynamicIccTuning,
    preset_name: &str,
    overlay_manual_overrides: bool,
) -> DynamicIccTuning {
    let preset = parse_dynamic_icc_tuning_preset(preset_name);
    if matches!(preset, DynamicIccTuningPreset::Manual) {
        return manual_tuning;
    }

    let preset_tuning = dynamic_icc_tuning_for_preset(preset);
    if overlay_manual_overrides {
        return merge_manual_tuning_overrides(preset_tuning, manual_tuning);
    }
    preset_tuning
}

/// Resolve the effective preset name for a run.
///
/// Precedence:
/// 1) day/night schedule presets when both are configured
/// 2) HDR/SDR preset based on `hdr_mode`
/// 3) fallback `active_preset`
pub fn select_effective_preset(
    active_preset: &str,
    sdr_preset: &str,
    hdr_preset: &str,
    schedule_day_preset: &str,
    schedule_night_preset: &str,
    hdr_mode: bool,
) -> String {
    let day = schedule_day_preset.trim();
    let night = schedule_night_preset.trim();
    if !day.is_empty() && !night.is_empty() {
        let hour = chrono::Local::now().hour();
        if (DAY_PRESET_START_HOUR..NIGHT_PRESET_START_HOUR).contains(&hour) {
            return day.to_string();
        }
        return night.to_string();
    }

    let mode_preset = if hdr_mode {
        hdr_preset.trim()
    } else {
        sdr_preset.trim()
    };
    let active = active_preset.trim();

    // Backward-compatible behavior:
    // Older configs kept SDR/HDR pinned to gamma22 while users edited only
    // `active_preset`, which made active changes appear to do nothing.
    // If both mode presets are still the legacy gamma22 default and active is
    // explicitly non-default, treat active as the user's intent.
    if !active.is_empty()
        && !active.eq_ignore_ascii_case("gamma22")
        && sdr_preset.trim().eq_ignore_ascii_case("gamma22")
        && hdr_preset.trim().eq_ignore_ascii_case("gamma22")
    {
        return active.to_string();
    }

    if !mode_preset.is_empty() {
        return mode_preset.to_string();
    }

    if active.is_empty() {
        "custom".to_string()
    } else {
        active.to_string()
    }
}

/// Clamp/sanitize a caller-provided gamma value into a safe range.
pub fn sanitize_dynamic_gamma(gamma: f64) -> f64 {
    if !gamma.is_finite() {
        return DEFAULT_DYNAMIC_GAMMA;
    }
    gamma.clamp(MIN_DYNAMIC_GAMMA, MAX_DYNAMIC_GAMMA)
}

/// Clamp/sanitize luminance in cd/m^2 for ICC luminance tag generation.
pub fn sanitize_dynamic_luminance_cd_m2(luminance_cd_m2: f64) -> f64 {
    if !luminance_cd_m2.is_finite() {
        return DEFAULT_DYNAMIC_LUMINANCE_CD_M2;
    }
    luminance_cd_m2.clamp(MIN_DYNAMIC_LUMINANCE_CD_M2, MAX_DYNAMIC_LUMINANCE_CD_M2)
}

/// Clamp/sanitize black-lift amount.
pub fn sanitize_black_lift(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 0.25)
}

/// Clamp/sanitize midtone boost amount.
pub fn sanitize_midtone_boost(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(-0.5, 0.5)
}

/// Clamp/sanitize highlight compression amount.
pub fn sanitize_white_compression(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

/// Clamp/sanitize per-channel gamma multipliers.
pub fn sanitize_channel_gamma_multiplier(value: f64) -> f64 {
    if !value.is_finite() {
        return 1.0;
    }
    value.clamp(0.5, 1.5)
}

/// Clamp/sanitize VCGT blend strength.
pub fn sanitize_vcgt_strength(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

/// Clamp/sanitize target-black floor in cd/m^2.
pub fn sanitize_target_black_cd_m2(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.2;
    }
    value.clamp(0.0, 5.0)
}

/// Parse an ICC 4-character signature (or shorter, padded with spaces) into a u32.
/// Returns 0 when input is empty or invalid.
pub fn parse_icc_signature_or_zero(value: &str) -> u32 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0;
    }
    trimmed.parse::<Signature>().map(|sig| sig.0).unwrap_or(0)
}

fn signature_u32_to_ascii(value: u32) -> Option<String> {
    if value == 0 {
        return None;
    }
    let bytes = value.to_be_bytes();
    if !bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Sanitize a string for safe use as part of an ICC filename.
pub fn sanitize_profile_name_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    let out = out.trim_matches(|c| c == '_' || c == '-' || c == '.');
    if out.is_empty() {
        "monitor".to_string()
    } else {
        out.to_string()
    }
}

fn stable_short_hash(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}

fn monitor_identity_suffix(identity: &DynamicMonitorIdentity) -> String {
    let serial = sanitize_profile_name_component(&identity.serial_number);
    if !serial.is_empty() && serial != "monitor" {
        return serial;
    }

    let composite = format!(
        "{}-{}",
        sanitize_profile_name_component(&identity.manufacturer_id),
        sanitize_profile_name_component(&identity.product_code)
    );
    let composite = composite.trim_matches('-');
    if !composite.is_empty() {
        return composite.to_string();
    }

    stable_short_hash(&identity.device_key)
}

/// Create a monitor-scoped profile filename from a base profile name.
pub fn monitor_scoped_profile_name(
    base_profile_name: &str,
    identity: &DynamicMonitorIdentity,
) -> String {
    let path = Path::new(base_profile_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("lg-ultragear-dynamic-cmx");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("icm");
    let suffix = monitor_identity_suffix(identity);
    format!("{}-{}.{}", stem, suffix, ext)
}

/// Resolve a monitor-scoped profile path under the color directory.
pub fn resolve_monitor_scoped_profile_path(
    color_dir: &Path,
    base_profile_name: &str,
    identity: &DynamicMonitorIdentity,
) -> PathBuf {
    color_dir.join(monitor_scoped_profile_name(base_profile_name, identity))
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn build_trc_curve_points(
    gamma: f64,
    gamma_multiplier: f64,
    luminance_cd_m2: f64,
    tuning: DynamicIccTuning,
) -> [u16; CURVE_TABLE_SIZE] {
    let effective_gamma =
        sanitize_dynamic_gamma(gamma * sanitize_channel_gamma_multiplier(gamma_multiplier));
    let luminance_cd_m2 = sanitize_dynamic_luminance_cd_m2(luminance_cd_m2);
    let luminance_ratio = (luminance_cd_m2 / DEFAULT_DYNAMIC_LUMINANCE_CD_M2).clamp(0.25, 5.0);
    let luminance_bias = (luminance_ratio.log2() * 0.10).clamp(-0.18, 0.18);
    let black_lift = sanitize_black_lift(tuning.black_lift);
    let midtone_boost = sanitize_midtone_boost(tuning.midtone_boost);
    let white_compression = sanitize_white_compression(tuning.white_compression);
    let target_black_ratio =
        sanitize_target_black_cd_m2(tuning.target_black_cd_m2) / luminance_cd_m2.max(1.0);
    let floor = (black_lift + target_black_ratio).clamp(0.0, 0.25);

    let mut out = [0u16; CURVE_TABLE_SIZE];
    for (i, slot) in out.iter_mut().enumerate() {
        let x = i as f64 / (CURVE_TABLE_SIZE - 1) as f64;
        let mut y = if x <= 0.0 {
            0.0
        } else {
            x.powf(1.0 / effective_gamma)
        };

        // Bell-shaped midtone influence centered near 50%.
        let bell = 4.0 * x * (1.0 - x);
        y += midtone_boost * bell * (1.0 - y);

        // Lift floor after gamma shaping.
        y = floor + (1.0 - floor) * y;

        // Make white-luminance control affect perceived image brightness, not
        // only metadata tags. Higher Yw lifts tones; lower Yw gently darkens.
        if luminance_bias > 0.0 {
            y += luminance_bias * (1.0 - y);
        } else if luminance_bias < 0.0 {
            y *= 1.0 + luminance_bias;
        }

        // Compress highlights only near the top-end.
        let high = smoothstep((y - 0.70) / 0.30);
        y -= white_compression * high * (y - 0.70);

        y = y.clamp(0.0, 1.0);
        *slot = (y * 65535.0).round() as u16;
    }

    out
}

fn blend_curve_with_identity(
    curve: &[u16; CURVE_TABLE_SIZE],
    strength: f64,
) -> [u16; CURVE_TABLE_SIZE] {
    let strength = sanitize_vcgt_strength(strength);
    let mut out = [0u16; CURVE_TABLE_SIZE];
    for (i, slot) in out.iter_mut().enumerate() {
        let identity = ((i as f64 / (CURVE_TABLE_SIZE - 1) as f64) * 65535.0).round() as u16;
        let blended = (identity as f64) + ((curve[i] as f64) - (identity as f64)) * strength;
        *slot = blended.round().clamp(0.0, 65535.0) as u16;
    }
    out
}

fn build_vcgt_table_payload(
    red: &[u16; CURVE_TABLE_SIZE],
    green: &[u16; CURVE_TABLE_SIZE],
    blue: &[u16; CURVE_TABLE_SIZE],
    strength: f64,
) -> Vec<u8> {
    let red = blend_curve_with_identity(red, strength);
    let green = blend_curve_with_identity(green, strength);
    let blue = blend_curve_with_identity(blue, strength);

    let mut payload = Vec::with_capacity(4 + 6 + (3 * CURVE_TABLE_SIZE * 2));
    payload.extend_from_slice(&0u32.to_be_bytes()); // table mode
    payload.extend_from_slice(&(3u16).to_be_bytes()); // channels
    payload.extend_from_slice(&(CURVE_TABLE_SIZE as u16).to_be_bytes()); // entries
    payload.extend_from_slice(&(2u16).to_be_bytes()); // 16-bit entries
    for v in red {
        payload.extend_from_slice(&v.to_be_bytes());
    }
    for v in green {
        payload.extend_from_slice(&v.to_be_bytes());
    }
    for v in blue {
        payload.extend_from_slice(&v.to_be_bytes());
    }
    payload
}

fn d65_xyz_for_luminance(luminance_cd_m2: f64) -> [f64; 3] {
    let y = sanitize_dynamic_luminance_cd_m2(luminance_cd_m2);
    [0.950455 * y, y, 1.08905 * y]
}

fn d65_xyz_for_absolute_luminance(y_cd_m2: f64) -> [f64; 3] {
    let y = if y_cd_m2.is_finite() {
        y_cd_m2.max(0.0)
    } else {
        0.0
    };
    [0.950455 * y, y, 1.08905 * y]
}

fn sanitize_ascii_text(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect()
}

fn build_empty_dict_tag_payload() -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + 16);
    payload.extend_from_slice(b"dict");
    payload.extend_from_slice(&0u32.to_be_bytes()); // reserved
    payload.extend_from_slice(&0u32.to_be_bytes()); // record count
    payload.extend_from_slice(&0u32.to_be_bytes()); // record size
    payload.extend_from_slice(&0u32.to_be_bytes()); // names offset
    payload.extend_from_slice(&0u32.to_be_bytes()); // values offset
    payload
}

fn f64_to_s15fixed16(value: f64) -> i32 {
    (value * 65536.0).round() as i32
}

fn append_u16_be(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn append_u32_be(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn append_i32_be(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn build_date_time_tag_payload(
    year: u16,
    month: u16,
    day: u16,
    hour: u16,
    min: u16,
    sec: u16,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(20);
    payload.extend_from_slice(b"dtim");
    append_u32_be(&mut payload, 0);
    append_u16_be(&mut payload, year);
    append_u16_be(&mut payload, month);
    append_u16_be(&mut payload, day);
    append_u16_be(&mut payload, hour);
    append_u16_be(&mut payload, min);
    append_u16_be(&mut payload, sec);
    payload
}

fn build_measurement_tag_payload() -> Vec<u8> {
    // ICC measurementType:
    // observer=1 (CIE1931 2deg), backing=XYZ(D50), geometry=1 (45/0), flare=0, illuminant=2 (D65)
    let mut payload = Vec::with_capacity(36);
    payload.extend_from_slice(b"meas");
    append_u32_be(&mut payload, 0);
    append_u32_be(&mut payload, 1);
    append_i32_be(&mut payload, f64_to_s15fixed16(0.9642));
    append_i32_be(&mut payload, f64_to_s15fixed16(1.0));
    append_i32_be(&mut payload, f64_to_s15fixed16(0.8249));
    append_u32_be(&mut payload, 1);
    append_u32_be(&mut payload, 0);
    append_u32_be(&mut payload, 2);
    payload
}

fn build_viewing_conditions_tag_payload() -> Vec<u8> {
    // ICC viewingConditionsType:
    // illuminant XYZ(D50), surround XYZ(20% gray), illuminant enum 1 (D50).
    let mut payload = Vec::with_capacity(36);
    payload.extend_from_slice(b"view");
    append_u32_be(&mut payload, 0);
    append_i32_be(&mut payload, f64_to_s15fixed16(0.9642));
    append_i32_be(&mut payload, f64_to_s15fixed16(1.0));
    append_i32_be(&mut payload, f64_to_s15fixed16(0.8249));
    append_i32_be(&mut payload, f64_to_s15fixed16(0.2));
    append_i32_be(&mut payload, f64_to_s15fixed16(0.2));
    append_i32_be(&mut payload, f64_to_s15fixed16(0.2));
    append_u32_be(&mut payload, 1);
    payload
}

fn build_chromaticity_tag_payload() -> Vec<u8> {
    // ICC chromaticityType with explicit RGB xy primaries (BT.709 / sRGB).
    let mut payload = Vec::with_capacity(32);
    payload.extend_from_slice(b"chrm");
    append_u32_be(&mut payload, 0);
    append_u16_be(&mut payload, 3); // channels
    append_u16_be(&mut payload, 1); // ITU-R BT.709-2 primaries
    for (x, y) in [
        (0.640_f64, 0.330_f64),
        (0.300_f64, 0.600_f64),
        (0.150_f64, 0.060_f64),
    ] {
        append_u32_be(&mut payload, (x * 65535.0).round() as u32);
        append_u32_be(&mut payload, (y * 65535.0).round() as u32);
    }
    payload
}

fn build_data_type_payload(bytes: &[u8]) -> Vec<u8> {
    // ICC dataType: signature "data", reserved, data flag (1=binary), payload bytes.
    let mut payload = Vec::with_capacity(12 + bytes.len());
    payload.extend_from_slice(b"data");
    append_u32_be(&mut payload, 0);
    append_u32_be(&mut payload, 1);
    payload.extend_from_slice(bytes);
    payload
}

/// Generate a dynamic ICC profile using `cmx`, tuned with a single gamma value.
///
/// The generated profile is deterministic for a given gamma:
/// - fixed creation date (to avoid timestamp churn)
/// - explicit profile ID recalculation after TRC updates
pub fn generate_dynamic_profile_bytes(gamma: f64) -> Result<Vec<u8>, Box<dyn Error>> {
    generate_dynamic_profile_bytes_with_luminance_and_tuning(
        gamma,
        DEFAULT_DYNAMIC_LUMINANCE_CD_M2,
        DynamicIccTuning::default(),
    )
}

/// Generate a dynamic ICC profile with caller-controlled gamma and luminance.
///
/// Luminance is encoded via the ICC `lumi` tag (CIEXYZ, cd/m^2).
pub fn generate_dynamic_profile_bytes_with_luminance(
    gamma: f64,
    luminance_cd_m2: f64,
) -> Result<Vec<u8>, Box<dyn Error>> {
    generate_dynamic_profile_bytes_with_luminance_and_tuning(
        gamma,
        luminance_cd_m2,
        DynamicIccTuning::default(),
    )
}

/// Generate a dynamic ICC profile with caller-controlled gamma/luminance and advanced tuning.
pub fn generate_dynamic_profile_bytes_with_luminance_and_tuning(
    gamma: f64,
    luminance_cd_m2: f64,
    tuning: DynamicIccTuning,
) -> Result<Vec<u8>, Box<dyn Error>> {
    generate_dynamic_profile_bytes_with_luminance_tuning_identity_and_extra_tags(
        gamma,
        luminance_cd_m2,
        tuning,
        None,
        &[],
    )
}

/// Additional raw tags to write into a generated ICC profile.
#[derive(Debug, Clone)]
pub struct ExtraRawTag {
    pub signature: u32,
    pub payload: Vec<u8>,
}

/// Generate a dynamic ICC profile with tuning, optional monitor identity, and extra raw tags.
pub fn generate_dynamic_profile_bytes_with_luminance_tuning_identity_and_extra_tags(
    gamma: f64,
    luminance_cd_m2: f64,
    tuning: DynamicIccTuning,
    identity: Option<&DynamicMonitorIdentity>,
    extra_tags: &[ExtraRawTag],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let gamma = sanitize_dynamic_gamma(gamma);
    let luminance_cd_m2 = sanitize_dynamic_luminance_cd_m2(luminance_cd_m2);
    let red_curve = build_trc_curve_points(gamma, tuning.gamma_r, luminance_cd_m2, tuning);
    let green_curve = build_trc_curve_points(gamma, tuning.gamma_g, luminance_cd_m2, tuning);
    let blue_curve = build_trc_curve_points(gamma, tuning.gamma_b, luminance_cd_m2, tuning);
    let created = chrono::Utc
        .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .ok_or("invalid fixed ICC creation date")?;
    let target_black = sanitize_target_black_cd_m2(tuning.target_black_cd_m2)
        .min((luminance_cd_m2 * 0.95).max(0.0));
    let mut profile_desc = format!(
        "LG UltraGear Dynamic ICC (gamma {:.2}, Yw {:.1} cd/m^2)",
        gamma, luminance_cd_m2
    );
    let mut char_target = format!(
        "gamma={:.3};Yw={:.1};Yb={:.3};r={:.3};g={:.3};b={:.3}",
        gamma, luminance_cd_m2, target_black, tuning.gamma_r, tuning.gamma_g, tuning.gamma_b
    );
    if let Some(identity) = identity {
        let serial = sanitize_ascii_text(identity.serial_number.trim());
        if !serial.is_empty() {
            profile_desc.push_str(&format!("; SN={}", serial));
            char_target.push_str(&format!(";sn={}", serial));
        }
        let model = sanitize_ascii_text(identity.monitor_name.trim());
        if !model.is_empty() {
            char_target.push_str(&format!(";model={}", model));
        }
    }
    let profile_desc = sanitize_ascii_text(&profile_desc);
    let char_target = sanitize_ascii_text(&char_target);

    let mut profile = DisplayProfile::cmx_srgb(RenderingIntent::RelativeColorimetric)
        .with_creation_date(created)
        .with_tag(ProfileDescriptionTag)
        .as_text_description(|text| text.set_ascii(&profile_desc))
        .with_tag(RedTRCTag)
        .as_curve(|curve| curve.set_data(red_curve))
        .with_tag(GreenTRCTag)
        .as_curve(|curve| curve.set_data(green_curve))
        .with_tag(BlueTRCTag)
        .as_curve(|curve| curve.set_data(blue_curve))
        .with_tag(LuminanceTag)
        .as_xyz_array(|xyz| xyz.set(d65_xyz_for_luminance(luminance_cd_m2)));

    if tuning.include_media_black_point {
        let black_xyz = d65_xyz_for_absolute_luminance(target_black);
        profile = profile
            .with_tag(MediaBlackPointTag)
            .as_xyz_array(|xyz| xyz.set(black_xyz));
    }

    if tuning.include_device_descriptions {
        let mfg_text = identity
            .map(|i| i.manufacturer_id.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("supermarsx");
        let model_text = identity
            .map(|i| i.monitor_name.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("LG UltraGear Dynamic Profile");
        profile = profile
            .with_tag(DeviceMfgDescTag)
            .as_multi_localized_unicode(|mluc| {
                mluc.insert("en", Some("US"), &sanitize_ascii_text(mfg_text));
            })
            .with_tag(DeviceModelDescTag)
            .as_multi_localized_unicode(|mluc| {
                mluc.insert("en", Some("US"), &sanitize_ascii_text(model_text));
            });
    }

    if tuning.include_characterization_target {
        profile = profile
            .with_tag(CharTargetTag)
            .as_text(|text| text.set_text(&char_target));
    }

    if tuning.include_viewing_cond_desc {
        profile = profile
            .with_tag(ViewingCondDescTag)
            .as_multi_localized_unicode(|mluc| {
                mluc.insert(
                    "en",
                    Some("US"),
                    "Desktop SDR viewing conditions for LG UltraGear dimming mitigation",
                );
            });
    }

    if let Some(sig) = signature_u32_to_ascii(tuning.technology_signature) {
        profile = profile
            .with_tag(TechnologyTag)
            .as_signature(|signature| signature.set_signature(&sig));
    }

    if let Some(sig) = signature_u32_to_ascii(tuning.ciis_signature) {
        profile = profile
            .with_tag(ColorimetricIntentImageStateTag)
            .as_signature(|signature| signature.set_signature(&sig));
    }

    if tuning.cicp_enabled {
        let cicp_payload = [
            tuning.cicp_color_primaries,
            tuning.cicp_transfer_characteristics,
            tuning.cicp_matrix_coefficients,
            if tuning.cicp_full_range { 1 } else { 0 },
        ];
        profile = profile
            .with_tag(CicpTag)
            .as_raw(|raw| raw.set_bytes(&cicp_payload));
    }

    if tuning.metadata_enabled {
        let dict_payload = build_empty_dict_tag_payload();
        profile = profile.with_tag(MetadataTag).as_raw(|raw| {
            raw.0 = dict_payload;
        });
    }

    if tuning.include_calibration_datetime {
        profile = profile.with_tag(CalibrationDateTimeTag).as_raw(|raw| {
            raw.0 = build_date_time_tag_payload(2026, 1, 1, 0, 0, 0);
        });
    }

    if tuning.include_chromatic_adaptation {
        profile = profile
            .with_tag(ChromaticAdaptationTag)
            .as_sf15_fixed_16_array(|arr| {
                arr.set([
                    1.0479298, 0.0229468, -0.0501922, 0.0296278, 0.9904345, -0.0170738, -0.0092430,
                    0.0150552, 0.7518743,
                ]);
            });
    }

    if tuning.include_chromaticity {
        profile = profile.with_tag(ChromaticityTag).as_raw(|raw| {
            raw.0 = build_chromaticity_tag_payload();
        });
    }

    if tuning.include_measurement {
        profile = profile.with_tag(MeasurementTag).as_raw(|raw| {
            raw.0 = build_measurement_tag_payload();
        });
    }

    if tuning.include_viewing_conditions {
        profile = profile.with_tag(ViewingConditionsTag).as_raw(|raw| {
            raw.0 = build_viewing_conditions_tag_payload();
        });
    }

    if tuning.include_spectral_scaffold {
        let scaffolds = [
            (
                TAG_SIG_SDIN,
                build_data_type_payload(b"spectral-data-info placeholder"),
            ),
            (
                TAG_SIG_SWPT,
                build_data_type_payload(b"spectral-white-point placeholder"),
            ),
            (
                TAG_SIG_SVCN,
                build_data_type_payload(b"spectral-viewing-conditions placeholder"),
            ),
        ];
        for (sig, payload) in scaffolds {
            profile = profile.with_tag(TagSignature::Unknown(sig)).as_raw(|raw| {
                raw.0 = payload.clone();
            });
        }
    }

    if tuning.vcgt_enabled {
        let payload =
            build_vcgt_table_payload(&red_curve, &green_curve, &blue_curve, tuning.vcgt_strength);
        profile = profile
            .with_tag(VcgtTag)
            .as_raw(|raw| raw.set_bytes(&payload));
    }

    for tag in extra_tags {
        profile = profile
            .with_tag(TagSignature::Unknown(tag.signature))
            .as_raw(|raw| {
                raw.0 = tag.payload.clone();
            });
    }

    let profile = profile.with_profile_id();

    profile.to_bytes()
}

/// Default generated ICC profile size in bytes.
pub fn dynamic_profile_size() -> Result<usize, Box<dyn Error>> {
    Ok(generate_dynamic_profile_bytes(DEFAULT_DYNAMIC_GAMMA)?.len())
}

/// Validate ICC bytes with structural, parser-level, and required-tag checks.
pub fn validate_icc_profile_bytes(profile_bytes: &[u8]) -> IccValidationReport {
    let mut report = IccValidationReport {
        actual_size: profile_bytes.len(),
        ..IccValidationReport::default()
    };

    if profile_bytes.len() < ICC_MIN_SIZE {
        report.errors.push(format!(
            "profile is too small: {} bytes (minimum {})",
            profile_bytes.len(),
            ICC_MIN_SIZE
        ));
        return report;
    }

    let Some(declared_size) = read_be_u32(profile_bytes, 0) else {
        report
            .errors
            .push("could not read declared profile size".to_string());
        return report;
    };
    report.declared_size = Some(declared_size);
    if declared_size as usize != profile_bytes.len() {
        report.errors.push(format!(
            "declared profile size {} does not match actual size {}",
            declared_size,
            profile_bytes.len()
        ));
    }

    if profile_bytes
        .get(ICC_ACSP_OFFSET..ICC_ACSP_OFFSET + 4)
        .map(|s| s != b"acsp")
        .unwrap_or(true)
    {
        report
            .errors
            .push("missing ICC file signature 'acsp' at header offset 36".to_string());
    }

    let Some(tag_count) = read_be_u32(profile_bytes, ICC_TAG_COUNT_OFFSET) else {
        report
            .errors
            .push("could not read ICC tag count".to_string());
        return report;
    };
    report.tag_count = Some(tag_count);

    let Some(tag_table_bytes) = (tag_count as usize).checked_mul(ICC_TAG_RECORD_SIZE) else {
        report.errors.push(format!(
            "tag table byte count overflow for {} tags",
            tag_count
        ));
        return report;
    };
    let Some(tag_table_end) = (ICC_HEADER_SIZE + 4).checked_add(tag_table_bytes) else {
        report
            .errors
            .push("tag table end computation overflowed".to_string());
        return report;
    };
    if tag_table_end > profile_bytes.len() {
        report.errors.push(format!(
            "tag table end {} exceeds profile size {}",
            tag_table_end,
            profile_bytes.len()
        ));
        return report;
    }

    let mut tag_records = Vec::with_capacity(tag_count as usize);
    let mut signature_counts: HashMap<[u8; 4], usize> = HashMap::new();
    for i in 0..tag_count as usize {
        let entry_offset = ICC_HEADER_SIZE + 4 + i * ICC_TAG_RECORD_SIZE;
        let Some(entry) = profile_bytes.get(entry_offset..entry_offset + ICC_TAG_RECORD_SIZE)
        else {
            report
                .errors
                .push(format!("tag entry {} is out of bounds", i));
            continue;
        };

        let signature: [u8; 4] = [entry[0], entry[1], entry[2], entry[3]];
        let offset = u32::from_be_bytes([entry[4], entry[5], entry[6], entry[7]]);
        let size = u32::from_be_bytes([entry[8], entry[9], entry[10], entry[11]]);

        *signature_counts.entry(signature).or_insert(0) += 1;

        if size == 0 {
            report.warnings.push(format!(
                "tag {} has zero size",
                icc_tag_signature_to_string(signature)
            ));
        }

        if offset % 4 != 0 {
            report.warnings.push(format!(
                "tag {} offset {} is not 4-byte aligned",
                icc_tag_signature_to_string(signature),
                offset
            ));
        }

        if size > 0 && offset < tag_table_end as u32 {
            report.warnings.push(format!(
                "tag {} data offset {} points into header/tag-table region (< {})",
                icc_tag_signature_to_string(signature),
                offset,
                tag_table_end
            ));
        }

        if size > 0 {
            let Some(end) = (offset as usize).checked_add(size as usize) else {
                report.errors.push(format!(
                    "tag {} offset+size overflows (offset={}, size={})",
                    icc_tag_signature_to_string(signature),
                    offset,
                    size
                ));
                continue;
            };
            if end > profile_bytes.len() {
                report.errors.push(format!(
                    "tag {} range [{}..{}) exceeds profile size {}",
                    icc_tag_signature_to_string(signature),
                    offset,
                    end,
                    profile_bytes.len()
                ));
            }
        }

        tag_records.push(IccTagRecord {
            signature,
            offset,
            size,
        });
    }

    for (signature, count) in signature_counts {
        if count > 1 {
            report.warnings.push(format!(
                "duplicate tag signature {} appears {} times",
                icc_tag_signature_to_string(signature),
                count
            ));
        }
    }

    for i in 0..tag_records.len() {
        for j in (i + 1)..tag_records.len() {
            let a = tag_records[i];
            let b = tag_records[j];
            if a.size == 0 || b.size == 0 {
                continue;
            }

            let a_start = a.offset as usize;
            let b_start = b.offset as usize;
            let Some(a_end) = a_start.checked_add(a.size as usize) else {
                continue;
            };
            let Some(b_end) = b_start.checked_add(b.size as usize) else {
                continue;
            };

            let overlaps = a_start < b_end && b_start < a_end;
            if !overlaps {
                continue;
            }

            let identical_shared_block = a_start == b_start && a_end == b_end;
            if identical_shared_block {
                continue;
            }

            report.errors.push(format!(
                "overlapping tag ranges: {} [{}..{}) and {} [{}..{})",
                icc_tag_signature_to_string(a.signature),
                a_start,
                a_end,
                icc_tag_signature_to_string(b.signature),
                b_start,
                b_end
            ));
        }
    }

    match RawProfile::from_bytes(profile_bytes) {
        Ok(raw) => validate_rgb_display_profile_semantics(&raw, &mut report),
        Err(e) => report
            .errors
            .push(format!("cmx parser rejected profile: {}", e)),
    }

    report
}

/// Validate an ICC file on disk.
pub fn validate_icc_profile_file(
    profile_path: &Path,
) -> Result<IccValidationReport, Box<dyn Error>> {
    let bytes = std::fs::read(profile_path)?;
    Ok(validate_icc_profile_bytes(&bytes))
}

fn validate_rgb_display_profile_semantics(raw: &RawProfile, report: &mut IccValidationReport) {
    if let Err(e) = raw.check_file_signature() {
        report
            .errors
            .push(format!("invalid ICC file signature: {}", e));
    }

    match raw.version() {
        Ok((major, minor)) => {
            if !(2..=4).contains(&major) {
                report.warnings.push(format!(
                    "unusual ICC version {}.{} (expected major 2..4)",
                    major, minor
                ));
            }
        }
        Err(e) => report
            .errors
            .push(format!("failed to read ICC version: {}", e)),
    }

    if raw.profile_size() != report.actual_size {
        report.errors.push(format!(
            "parsed header profile size {} does not match actual size {}",
            raw.profile_size(),
            report.actual_size
        ));
    }

    report.tag_details = collect_icc_tag_details(raw);
    report.known_tag_count = report
        .tag_details
        .iter()
        .filter(|tag| tag.known_signature)
        .count();
    report.unknown_tag_count = report.tag_details.len() - report.known_tag_count;
    validate_generic_tag_semantics(report);

    report.has_vcgt_tag = raw.tags.contains_key(&TagSignature::Vcgt);
    report.has_cicp_tag = raw.tags.contains_key(&TagSignature::Cicp);
    report.has_metadata_tag = raw.tags.contains_key(&TagSignature::Metadata);
    report.has_spectral_data_info_tag = raw.tags.contains_key(&TagSignature::SpectralDataInfo)
        || raw.tags.contains_key(&TagSignature::Unknown(TAG_SIG_SDIN));
    report.has_spectral_white_point_tag = raw.tags.contains_key(&TagSignature::SpectralWhitePoint)
        || raw.tags.contains_key(&TagSignature::Unknown(TAG_SIG_SWPT));
    report.has_spectral_viewing_conditions_tag = raw
        .tags
        .contains_key(&TagSignature::SpectralViewingConditions)
        || raw.tags.contains_key(&TagSignature::Unknown(TAG_SIG_SVCN));

    let is_display_rgb = raw.device_class() == DeviceClass::Display
        && raw.data_color_space() == Some(ColorSpace::RGB);
    if !is_display_rgb {
        report.warnings.push(format!(
            "profile class/colorspace is {:?}/{:?}, expected Display/RGB for this project",
            raw.device_class(),
            raw.data_color_space()
        ));
        return;
    }

    let required_display_tags = [
        TagSignature::ProfileDescription,
        TagSignature::Copyright,
        TagSignature::MediaWhitePoint,
        TagSignature::RedMatrixColumn,
        TagSignature::GreenMatrixColumn,
        TagSignature::BlueMatrixColumn,
        TagSignature::RedTRC,
        TagSignature::GreenTRC,
        TagSignature::BlueTRC,
        TagSignature::Luminance,
    ];

    for tag in required_display_tags {
        if !raw.tags.contains_key(&tag) {
            report
                .errors
                .push(format!("missing required display tag {}", tag));
        }
    }

    validate_tag_type_signature(
        raw,
        TagSignature::ProfileDescription,
        &[*b"desc", *b"mluc"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::Copyright,
        &[*b"text", *b"mluc", *b"desc"],
        report,
    );
    validate_tag_type_signature(raw, TagSignature::MediaWhitePoint, &[*b"XYZ "], report);
    validate_tag_type_signature(raw, TagSignature::RedMatrixColumn, &[*b"XYZ "], report);
    validate_tag_type_signature(raw, TagSignature::GreenMatrixColumn, &[*b"XYZ "], report);
    validate_tag_type_signature(raw, TagSignature::BlueMatrixColumn, &[*b"XYZ "], report);
    validate_tag_type_signature(raw, TagSignature::Luminance, &[*b"XYZ "], report);
    validate_tag_type_signature(raw, TagSignature::RedTRC, &[*b"curv", *b"para"], report);
    validate_tag_type_signature(raw, TagSignature::GreenTRC, &[*b"curv", *b"para"], report);
    validate_tag_type_signature(raw, TagSignature::BlueTRC, &[*b"curv", *b"para"], report);
    validate_tag_type_signature(raw, TagSignature::MediaBlackPoint, &[*b"XYZ "], report);
    validate_tag_type_signature(
        raw,
        TagSignature::DeviceMfgDesc,
        &[*b"desc", *b"mluc"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::DeviceModelDesc,
        &[*b"desc", *b"mluc"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::ViewingCondDesc,
        &[*b"desc", *b"mluc"],
        report,
    );
    validate_tag_type_signature(raw, TagSignature::CharTarget, &[*b"text"], report);
    validate_tag_type_signature(raw, TagSignature::Technology, &[*b"sig "], report);
    validate_tag_type_signature(
        raw,
        TagSignature::ColorimetricIntentImageState,
        &[*b"sig "],
        report,
    );
    validate_tag_type_signature(raw, TagSignature::Cicp, &[*b"cicp"], report);
    validate_tag_type_signature(raw, TagSignature::Metadata, &[*b"dict", *b"meta"], report);
    validate_tag_type_signature(raw, TagSignature::CalibrationDateTime, &[*b"dtim"], report);
    validate_tag_type_signature(raw, TagSignature::ChromaticAdaptation, &[*b"sf32"], report);
    validate_tag_type_signature(raw, TagSignature::Chromaticity, &[*b"chrm"], report);
    validate_tag_type_signature(raw, TagSignature::Measurement, &[*b"meas"], report);
    validate_tag_type_signature(raw, TagSignature::ViewingConditions, &[*b"view"], report);
    validate_tag_type_signature(
        raw,
        TagSignature::Unknown(TAG_SIG_SDIN),
        &[*b"data", *b"text"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::Unknown(TAG_SIG_SWPT),
        &[*b"data", *b"text"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::Unknown(TAG_SIG_SVCN),
        &[*b"data", *b"text"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::SpectralDataInfo,
        &[*b"data", *b"sdin", *b"text"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::SpectralWhitePoint,
        &[*b"data", *b"XYZ ", *b"sf32"],
        report,
    );
    validate_tag_type_signature(
        raw,
        TagSignature::SpectralViewingConditions,
        &[*b"data", *b"svcn", *b"view"],
        report,
    );

    if raw.tags.contains_key(&TagSignature::Vcgt) {
        validate_tag_type_signature(raw, TagSignature::Vcgt, &[*b"vcgt"], report);
    }

    if raw.profile_id().iter().all(|b| *b == 0) {
        report
            .warnings
            .push("profile ID is all-zero; a computed profile ID is recommended".to_string());
    }
}

fn validate_generic_tag_semantics(report: &mut IccValidationReport) {
    for detail in &report.tag_details {
        if detail.payload_size < 8 {
            report.errors.push(format!(
                "tag {} has invalid payload size {} (minimum 8 bytes for type+reserved header)",
                detail.signature, detail.payload_size
            ));
            continue;
        }

        if matches!(detail.reserved_bytes_zero, Some(false)) {
            report.warnings.push(format!(
                "tag {} has non-zero reserved bytes at payload offset 4..8",
                detail.signature
            ));
        }

        let Some(type_signature_u32) = detail.type_signature_u32 else {
            report.errors.push(format!(
                "tag {} is missing type signature",
                detail.signature
            ));
            continue;
        };

        if !detail.known_signature {
            report.warnings.push(format!(
                "tag {} is not recognized by cmx and will be treated as vendor-specific",
                detail.signature
            ));
        }

        if !detail.known_type_signature {
            report.warnings.push(format!(
                "tag {} uses unknown type signature {}",
                detail.signature,
                icc_u32_signature_to_string(type_signature_u32)
            ));
        }

        if let Some(expected) = expected_type_signatures_for_tag(detail.signature_u32) {
            if !expected.contains(&type_signature_u32.to_be_bytes()) {
                let expected_text = expected
                    .iter()
                    .map(|sig| icc_tag_signature_to_string(*sig))
                    .collect::<Vec<_>>()
                    .join("|");
                report.warnings.push(format!(
                    "tag {} has type {} outside common set {}",
                    detail.signature,
                    icc_u32_signature_to_string(type_signature_u32),
                    expected_text
                ));
            }
        }
    }
}

fn validate_tag_type_signature(
    raw: &RawProfile,
    tag: TagSignature,
    allowed_signatures: &[[u8; 4]],
    report: &mut IccValidationReport,
) {
    let Some(record) = raw.tags.get(&tag) else {
        return;
    };

    let bytes = record.tag.as_slice();
    if bytes.len() < 4 {
        report.errors.push(format!(
            "tag {} has invalid payload size {}",
            tag,
            bytes.len()
        ));
        return;
    }

    let signature = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if allowed_signatures.contains(&signature) {
        return;
    }

    let expected = allowed_signatures
        .iter()
        .map(|s| icc_tag_signature_to_string(*s))
        .collect::<Vec<_>>()
        .join("|");
    report.errors.push(format!(
        "tag {} has unexpected data type {}, expected {}",
        tag,
        icc_tag_signature_to_string(signature),
        expected
    ));
}

/// Lightweight ICC inspection result for CLI/reporting tools.
#[derive(Debug, Clone, Default)]
pub struct IccInspectionReport {
    pub profile_size: usize,
    pub device_class: String,
    pub data_color_space: String,
    pub known_tag_count: usize,
    pub unknown_tag_count: usize,
    pub tag_signatures: Vec<String>,
    pub tag_details: Vec<IccTagDetail>,
}

/// Inspect basic ICC metadata and tag list.
pub fn inspect_icc_profile_bytes(
    profile_bytes: &[u8],
) -> Result<IccInspectionReport, Box<dyn Error>> {
    let raw = RawProfile::from_bytes(profile_bytes)?;
    let tag_details = collect_icc_tag_details(&raw);
    let known_tag_count = tag_details.iter().filter(|tag| tag.known_signature).count();
    let unknown_tag_count = tag_details.len() - known_tag_count;
    let mut tag_signatures = raw
        .tags
        .keys()
        .map(|sig| sig.to_string())
        .collect::<Vec<_>>();
    tag_signatures.sort();

    Ok(IccInspectionReport {
        profile_size: raw.profile_size(),
        device_class: format!("{:?}", raw.device_class()),
        data_color_space: format!("{:?}", raw.data_color_space()),
        known_tag_count,
        unknown_tag_count,
        tag_signatures,
        tag_details,
    })
}

/// Parse and reserialize an ICC profile to normalize layout/ordering.
pub fn normalize_icc_profile_bytes(profile_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let raw = RawProfile::from_bytes(profile_bytes)?;
    raw.into_bytes()
}

/// Apply raw tag patches to ICC bytes (set/replace + remove by signature).
pub fn patch_icc_profile_bytes(
    profile_bytes: &[u8],
    set_tags: &[ExtraRawTag],
    remove_signatures: &[u32],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut raw = RawProfile::from_bytes(profile_bytes)?;

    if !remove_signatures.is_empty() {
        raw.tags
            .retain(|sig, _| !remove_signatures.iter().any(|rm| sig.to_u32() == *rm));
    }

    for patch in set_tags {
        raw = raw
            .with_tag(TagSignature::new(patch.signature))
            .as_raw(|raw_data| {
                raw_data.0 = patch.payload.clone();
            });
    }

    raw.into_bytes()
}

/// Build an ICC `dataType` payload from raw bytes.
pub fn build_icc_data_type_payload(bytes: &[u8]) -> Vec<u8> {
    build_data_type_payload(bytes)
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    let mut arr = [0u8; 4];
    arr.copy_from_slice(slice);
    Some(u32::from_be_bytes(arr))
}

fn icc_tag_signature_to_string(signature: [u8; 4]) -> String {
    if signature.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        String::from_utf8_lossy(&signature).into_owned()
    } else {
        format!("{:08X}", u32::from_be_bytes(signature))
    }
}

fn icc_u32_signature_to_string(signature: u32) -> String {
    icc_tag_signature_to_string(signature.to_be_bytes())
}

fn is_known_tag_signature(signature: u32) -> bool {
    !matches!(TagSignature::new(signature), TagSignature::Unknown(_))
}

fn is_known_type_signature(signature: u32) -> bool {
    !matches!(
        DataSignature::from_u32(signature),
        DataSignature::Unknown(_)
    )
}

fn expected_type_signatures_for_tag(signature: u32) -> Option<Vec<[u8; 4]>> {
    macro_rules! sigs {
        ($($sig:literal),+ $(,)?) => {
            vec![$(*$sig),+]
        };
    }

    match TagSignature::new(signature) {
        TagSignature::AToB0
        | TagSignature::AToB1
        | TagSignature::AToB2
        | TagSignature::BToA0
        | TagSignature::BToA1
        | TagSignature::BToA2
        | TagSignature::DToB0
        | TagSignature::DToB1
        | TagSignature::DToB2
        | TagSignature::DToB3
        | TagSignature::BToD0
        | TagSignature::BToD1
        | TagSignature::BToD2
        | TagSignature::BToD3 => Some(sigs!(b"mAB ", b"mBA ", b"mft1", b"mft2", b"mpet")),
        TagSignature::BlueMatrixColumn
        | TagSignature::GreenMatrixColumn
        | TagSignature::RedMatrixColumn
        | TagSignature::MediaWhitePoint
        | TagSignature::MediaBlackPoint
        | TagSignature::Luminance => Some(sigs!(b"XYZ ")),
        TagSignature::BlueTRC
        | TagSignature::GreenTRC
        | TagSignature::RedTRC
        | TagSignature::GrayTRC => Some(sigs!(b"curv", b"para", b"curf")),
        TagSignature::CalibrationDateTime => Some(sigs!(b"dtim")),
        TagSignature::CharTarget => Some(sigs!(b"text")),
        TagSignature::ChromaticAdaptation => Some(sigs!(b"sf32")),
        TagSignature::Chromaticity => Some(sigs!(b"chrm")),
        TagSignature::Cicp => Some(sigs!(b"cicp")),
        TagSignature::ColorantOrder => Some(sigs!(b"clro")),
        TagSignature::ColorantTable | TagSignature::ColorantTableOut => Some(sigs!(b"clrt")),
        TagSignature::ColorimetricIntentImageState
        | TagSignature::Technology
        | TagSignature::ReferenceName => Some(sigs!(b"sig ")),
        TagSignature::Copyright
        | TagSignature::DeviceMfgDesc
        | TagSignature::DeviceModelDesc
        | TagSignature::ProfileDescription
        | TagSignature::ViewingCondDesc => Some(sigs!(b"desc", b"mluc", b"text")),
        TagSignature::Measurement => Some(sigs!(b"meas")),
        TagSignature::Metadata => Some(sigs!(b"dict", b"meta")),
        TagSignature::NativeDisplayInfo => Some(sigs!(b"ndin")),
        TagSignature::OutputResponse => Some(sigs!(b"rcs2")),
        TagSignature::ProfileSequenceDesc => Some(sigs!(b"pseq")),
        TagSignature::ProfileSequenceIdendtifier => Some(sigs!(b"psid")),
        TagSignature::ViewingConditions => Some(sigs!(b"view")),
        TagSignature::Vcgt => Some(sigs!(b"vcgt")),
        TagSignature::Vcgp => Some(sigs!(b"vcgp")),
        TagSignature::Unknown(TAG_SIG_SDIN) => Some(sigs!(b"sdin", b"data", b"text")),
        TagSignature::Unknown(TAG_SIG_SWPT) => Some(sigs!(b"XYZ ", b"sf32", b"data")),
        TagSignature::Unknown(TAG_SIG_SVCN) => Some(sigs!(b"svcn", b"view", b"data")),
        TagSignature::AToB3
        | TagSignature::AToM0
        | TagSignature::BRDFAToB0
        | TagSignature::BRDFAToB1
        | TagSignature::BRDFAToB2
        | TagSignature::BRDFAToB3
        | TagSignature::BRDFDToB0
        | TagSignature::BRDFDToB1
        | TagSignature::BRDFDToB2
        | TagSignature::BRDFDToB3
        | TagSignature::BRDFMToB0
        | TagSignature::BRDFMToB1
        | TagSignature::BRDFMToB2
        | TagSignature::BRDFMToB3
        | TagSignature::MToA0
        | TagSignature::MToB0
        | TagSignature::MToB1
        | TagSignature::MToB2
        | TagSignature::MToB3
        | TagSignature::BToA3 => Some(sigs!(b"mAB ", b"mBA ", b"mpet")),
        TagSignature::BRDFMToS0
        | TagSignature::BRDFMToS1
        | TagSignature::BRDFMToS2
        | TagSignature::BRDFMToS3
        | TagSignature::MToS0
        | TagSignature::MToS1
        | TagSignature::MToS2
        | TagSignature::MToS3 => Some(sigs!(b"smat", b"sf32", b"data")),
        TagSignature::BrdfColorimetricParameter0
        | TagSignature::BrdfColorimetricParameter1
        | TagSignature::BrdfColorimetricParameter2
        | TagSignature::BrdfColorimetricParameter3
        | TagSignature::BrdfSpectralParameter0
        | TagSignature::BrdfSpectralParameter1
        | TagSignature::BrdfSpectralParameter2
        | TagSignature::BrdfSpectralParameter3 => Some(sigs!(b"sf32", b"fl32", b"fl64")),
        TagSignature::ColorEncodingParams => Some(sigs!(b"cicp", b"dict", b"data")),
        TagSignature::ColorSpaceName | TagSignature::MakeAndModel => Some(sigs!(b"mluc", b"text")),
        TagSignature::ColorantInfo | TagSignature::ColorantInfoOut => Some(sigs!(b"clrt", b"clio")),
        TagSignature::ColorantOrderOut => Some(sigs!(b"cloo", b"clro")),
        TagSignature::CrdInfo => Some(sigs!(b"crdi", b"text", b"data")),
        TagSignature::CustomToStandardPcc | TagSignature::StandardToCustomPcc => {
            Some(sigs!(b"mAB ", b"mBA ", b"mpet"))
        }
        TagSignature::CxF => Some(sigs!(b"data", b"tstr", b"utf8")),
        TagSignature::Data => Some(sigs!(b"data")),
        TagSignature::DateTime => Some(sigs!(b"dtim")),
        TagSignature::DeviceMediaWhitePoint => Some(sigs!(b"XYZ ", b"sf32")),
        TagSignature::DeviceSettings => Some(sigs!(b"devs", b"dict", b"data")),
        TagSignature::GamutBoundaryDescription0
        | TagSignature::GamutBoundaryDescription1
        | TagSignature::GamutBoundaryDescription2
        | TagSignature::GamutBoundaryDescription3 => Some(sigs!(b"gbd ", b"sf32", b"smat")),
        TagSignature::MaterialDefaultValues | TagSignature::MaterialDataArray => {
            Some(sigs!(b"tary", b"tstr", b"dict", b"data"))
        }
        TagSignature::NamedColorV5 => Some(sigs!(b"ncl2", b"nmcl")),
        TagSignature::PrintCondition => Some(sigs!(b"text", b"mluc", b"dict")),
        TagSignature::Ps2CRD0
        | TagSignature::Ps2CRD1
        | TagSignature::Ps2CRD2
        | TagSignature::Ps2CRD3
        | TagSignature::Ps2CSA
        | TagSignature::Ps2RenderingIntent => Some(sigs!(b"text", b"data")),
        TagSignature::SaturationRenderingIntentGamut
        | TagSignature::PerceptualRenderingIntentGamut => Some(sigs!(b"sig ", b"data")),
        TagSignature::ScreeningDesc => Some(sigs!(b"text", b"desc", b"mluc")),
        TagSignature::Screening => Some(sigs!(b"scrn")),
        TagSignature::SpectralDataInfo => Some(sigs!(b"sdin", b"data")),
        TagSignature::SpectralWhitePoint => Some(sigs!(b"XYZ ", b"sf32", b"data")),
        TagSignature::SpectralViewingConditions => Some(sigs!(b"svcn", b"view", b"data")),
        TagSignature::SurfaceMap => Some(sigs!(b"smat", b"tary", b"tstr", b"data")),
        TagSignature::UcrBg => Some(sigs!(b"bfd ", b"text", b"data")),
        TagSignature::EmbeddedV5Profile => Some(sigs!(b"ICCp", b"data")),
        _ => None,
    }
}

fn collect_icc_tag_details(raw: &RawProfile) -> Vec<IccTagDetail> {
    let mut details = raw
        .tags
        .iter()
        .map(|(signature, record)| {
            let bytes = record.tag.as_slice();
            let type_signature_u32 = if bytes.len() >= 4 {
                Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            } else {
                None
            };
            let type_signature = type_signature_u32.map(icc_u32_signature_to_string);
            let signature_u32 = signature.to_u32();
            let known_signature = is_known_tag_signature(signature_u32);
            let known_type_signature = type_signature_u32
                .map(is_known_type_signature)
                .unwrap_or(false);
            let reserved_bytes_zero = if bytes.len() >= 8 {
                Some(bytes[4..8] == [0u8; 4])
            } else {
                None
            };

            IccTagDetail {
                signature_u32,
                signature: signature.to_string(),
                known_signature,
                payload_size: bytes.len(),
                type_signature_u32,
                type_signature,
                known_type_signature,
                reserved_bytes_zero,
            }
        })
        .collect::<Vec<_>>();
    details.sort_by(|a, b| a.signature_u32.cmp(&b.signature_u32));
    details
}

/// Ensure the ICC profile is installed in the Windows color store.
///
/// Uses the default dynamic gamma (`DEFAULT_DYNAMIC_GAMMA`).
pub fn ensure_profile_installed(profile_path: &Path) -> Result<bool, Box<dyn Error>> {
    ensure_profile_installed_with_gamma(profile_path, DEFAULT_DYNAMIC_GAMMA)
}

/// Ensure the ICC profile is installed in the Windows color store.
///
/// If the file already exists and matches the generated profile content for
/// the selected gamma, this is a no-op. Otherwise, writes (or overwrites) the
/// generated profile to the color directory.
///
/// After the file is placed, calls `InstallColorProfileW` to register the
/// profile with the Windows Color System — the WCS APIs (`WcsAssociate…`,
/// `WcsDisassociate…`) require profiles to be registered, not merely present
/// on disk.
///
/// Returns `Ok(true)` if a new file was written, `Ok(false)` if already present.
pub fn ensure_profile_installed_with_gamma(
    profile_path: &Path,
    gamma: f64,
) -> Result<bool, Box<dyn Error>> {
    ensure_profile_installed_with_gamma_and_luminance(
        profile_path,
        gamma,
        DEFAULT_DYNAMIC_LUMINANCE_CD_M2,
    )
}

/// Ensure the ICC profile is installed using a gamma + luminance pair.
pub fn ensure_profile_installed_with_gamma_and_luminance(
    profile_path: &Path,
    gamma: f64,
    luminance_cd_m2: f64,
) -> Result<bool, Box<dyn Error>> {
    ensure_profile_installed_with_gamma_luminance_and_tuning(
        profile_path,
        gamma,
        luminance_cd_m2,
        DynamicIccTuning::default(),
    )
}

/// Ensure the ICC profile is installed using gamma + luminance + advanced tuning.
pub fn ensure_profile_installed_with_gamma_luminance_and_tuning(
    profile_path: &Path,
    gamma: f64,
    luminance_cd_m2: f64,
    tuning: DynamicIccTuning,
) -> Result<bool, Box<dyn Error>> {
    ensure_profile_installed_with_gamma_luminance_tuning_identity_and_extra_tags(
        profile_path,
        gamma,
        luminance_cd_m2,
        tuning,
        None,
        &[],
    )
}

/// Ensure the ICC profile is installed with tuning + monitor identity + extra raw tags.
pub fn ensure_profile_installed_with_gamma_luminance_tuning_identity_and_extra_tags(
    profile_path: &Path,
    gamma: f64,
    luminance_cd_m2: f64,
    tuning: DynamicIccTuning,
    identity: Option<&DynamicMonitorIdentity>,
    extra_tags: &[ExtraRawTag],
) -> Result<bool, Box<dyn Error>> {
    let gamma = sanitize_dynamic_gamma(gamma);
    let luminance_cd_m2 = sanitize_dynamic_luminance_cd_m2(luminance_cd_m2);
    let generated = generate_dynamic_profile_bytes_with_luminance_tuning_identity_and_extra_tags(
        gamma,
        luminance_cd_m2,
        tuning,
        identity,
        extra_tags,
    )?;
    let validation = validate_icc_profile_bytes(&generated);
    if !validation.is_valid() {
        return Err(format!(
            "generated ICC failed validation: {}",
            validation.errors.join("; ")
        )
        .into());
    }
    if !validation.warnings.is_empty() {
        warn!(
            "generated ICC validation warnings: {}",
            validation.warnings.join("; ")
        );
    }

    // Check if it already exists with identical content
    if let Ok(existing) = std::fs::read(profile_path) {
        if existing == generated {
            info!("ICC profile already installed: {}", profile_path.display());
            // Even when the file exists, ensure it is registered with WCS.
            register_color_profile(profile_path)?;
            cleanup_legacy_profile_files(profile_path);
            if is_in_color_directory(profile_path) {
                if let Err(e) = export_profile_to_app_profiles_dir(profile_path) {
                    warn!(
                        "Could not export profile artifact to app profiles directory: {}",
                        e
                    );
                }
            }
            return Ok(false);
        }
    }

    // Ensure the parent directory exists
    if let Some(parent) = profile_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(profile_path, &generated)?;
    info!(
        "Dynamic ICC profile generated (gamma {:.3}, luminance {:.1} cd/m^2) and written to {}",
        gamma,
        luminance_cd_m2,
        profile_path.display()
    );

    // Register with WCS so WcsAssociateColorProfileWithDevice will succeed.
    register_color_profile(profile_path)?;

    // Remove legacy profile files so the old static profile does not linger.
    cleanup_legacy_profile_files(profile_path);
    if is_in_color_directory(profile_path) {
        if let Err(e) = export_profile_to_app_profiles_dir(profile_path) {
            warn!(
                "Could not export profile artifact to app profiles directory: {}",
                e
            );
        }
    }

    Ok(true)
}

/// Return the active generated profile path for the selected preset.
pub fn resolve_active_profile_path(
    color_dir: &Path,
    active_preset: &str,
    custom_profile_name: &str,
) -> PathBuf {
    let preset = parse_dynamic_icc_preset(active_preset);
    color_dir.join(preset.profile_name(custom_profile_name))
}

/// Return the active generated profile path for a specific monitor identity.
pub fn resolve_monitor_active_profile_path(
    color_dir: &Path,
    active_preset: &str,
    custom_profile_name: &str,
    identity: &DynamicMonitorIdentity,
) -> PathBuf {
    let preset = parse_dynamic_icc_preset(active_preset);
    resolve_monitor_scoped_profile_path(
        color_dir,
        &preset.profile_name(custom_profile_name),
        identity,
    )
}

/// Ensure specialized Gamma 2.2 and Gamma 2.4 profiles both exist.
pub fn ensure_specialized_profiles_installed(
    color_dir: &Path,
    luminance_cd_m2: f64,
) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    ensure_specialized_profiles_installed_tuned(
        color_dir,
        luminance_cd_m2,
        DynamicIccTuning::default(),
    )
}

/// Ensure specialized Gamma 2.2 and Gamma 2.4 profiles both exist with custom tuning.
pub fn ensure_specialized_profiles_installed_tuned(
    color_dir: &Path,
    luminance_cd_m2: f64,
    tuning: DynamicIccTuning,
) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let luminance_cd_m2 = sanitize_dynamic_luminance_cd_m2(luminance_cd_m2);
    let gamma22_path = color_dir.join(GAMMA22_PROFILE_NAME);
    let gamma24_path = color_dir.join(GAMMA24_PROFILE_NAME);
    let _ = ensure_profile_installed_with_gamma_luminance_and_tuning(
        &gamma22_path,
        PRESET_GAMMA_22,
        luminance_cd_m2,
        tuning,
    )?;
    let _ = ensure_profile_installed_with_gamma_luminance_and_tuning(
        &gamma24_path,
        PRESET_GAMMA_24,
        luminance_cd_m2,
        tuning,
    )?;
    Ok((gamma22_path, gamma24_path))
}

/// Ensure the active profile exists and return its full path.
///
/// - `active_preset`: `gamma22`, `gamma24`, `reader`, or any other value for custom mode.
/// - `custom_profile_name` + `custom_gamma`: used when preset resolves to custom mode.
/// - `install_specialized_profiles`: if true, always keeps both gamma22 and gamma24 files current.
pub fn ensure_active_profile_installed(
    color_dir: &Path,
    active_preset: &str,
    custom_profile_name: &str,
    custom_gamma: f64,
    luminance_cd_m2: f64,
    install_specialized_profiles: bool,
) -> Result<PathBuf, Box<dyn Error>> {
    ensure_active_profile_installed_tuned(
        color_dir,
        active_preset,
        custom_profile_name,
        custom_gamma,
        luminance_cd_m2,
        install_specialized_profiles,
        DynamicIccTuning::default(),
    )
}

/// Ensure the active profile exists with advanced tuning and return its full path.
pub fn ensure_active_profile_installed_tuned(
    color_dir: &Path,
    active_preset: &str,
    custom_profile_name: &str,
    custom_gamma: f64,
    luminance_cd_m2: f64,
    install_specialized_profiles: bool,
    tuning: DynamicIccTuning,
) -> Result<PathBuf, Box<dyn Error>> {
    let preset = parse_dynamic_icc_preset(active_preset);
    let luminance_cd_m2 = sanitize_dynamic_luminance_cd_m2(luminance_cd_m2);
    let effective_tuning = tuning;

    if install_specialized_profiles {
        let _ = ensure_specialized_profiles_installed_tuned(
            color_dir,
            luminance_cd_m2,
            effective_tuning,
        )?;
    }

    let active_path = color_dir.join(preset.profile_name(custom_profile_name));
    let gamma = preset.gamma(custom_gamma);
    let _ = ensure_profile_installed_with_gamma_luminance_and_tuning(
        &active_path,
        gamma,
        luminance_cd_m2,
        effective_tuning,
    )?;
    Ok(active_path)
}

/// Ensure a monitor-scoped active profile exists with advanced tuning and identity metadata.
#[allow(clippy::too_many_arguments)]
pub fn ensure_active_profile_installed_tuned_for_monitor(
    color_dir: &Path,
    active_preset: &str,
    custom_profile_name: &str,
    custom_gamma: f64,
    luminance_cd_m2: f64,
    install_specialized_profiles: bool,
    tuning: DynamicIccTuning,
    identity: &DynamicMonitorIdentity,
) -> Result<PathBuf, Box<dyn Error>> {
    let preset = parse_dynamic_icc_preset(active_preset);
    let luminance_cd_m2 = sanitize_dynamic_luminance_cd_m2(luminance_cd_m2);
    let effective_tuning = tuning;

    if install_specialized_profiles {
        let _ = ensure_specialized_profiles_installed_tuned(
            color_dir,
            luminance_cd_m2,
            effective_tuning,
        )?;
    }

    let base_name = preset.profile_name(custom_profile_name);
    let active_path = resolve_monitor_scoped_profile_path(color_dir, &base_name, identity);
    let gamma = preset.gamma(custom_gamma);
    let _ = ensure_profile_installed_with_gamma_luminance_tuning_identity_and_extra_tags(
        &active_path,
        gamma,
        luminance_cd_m2,
        effective_tuning,
        Some(identity),
        &[],
    )?;
    Ok(active_path)
}

/// Ensure both SDR and HDR mode profiles exist and return their full paths.
#[allow(clippy::too_many_arguments)]
pub fn ensure_mode_profiles_installed_tuned(
    color_dir: &Path,
    sdr_preset: &str,
    hdr_preset: &str,
    custom_profile_name: &str,
    custom_gamma: f64,
    luminance_cd_m2: f64,
    install_specialized_profiles: bool,
    tuning: DynamicIccTuning,
) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let sdr_path = ensure_active_profile_installed_tuned(
        color_dir,
        sdr_preset,
        custom_profile_name,
        custom_gamma,
        luminance_cd_m2,
        install_specialized_profiles,
        tuning,
    )?;
    let hdr_path = if sdr_preset.eq_ignore_ascii_case(hdr_preset) {
        sdr_path.clone()
    } else {
        ensure_active_profile_installed_tuned(
            color_dir,
            hdr_preset,
            custom_profile_name,
            custom_gamma,
            luminance_cd_m2,
            install_specialized_profiles,
            tuning,
        )?
    };
    Ok((sdr_path, hdr_path))
}

/// Ensure both SDR and HDR monitor-scoped mode profiles exist and return their full paths.
#[allow(clippy::too_many_arguments)]
pub fn ensure_mode_profiles_installed_tuned_for_monitor(
    color_dir: &Path,
    sdr_preset: &str,
    hdr_preset: &str,
    custom_profile_name: &str,
    custom_gamma: f64,
    luminance_cd_m2: f64,
    install_specialized_profiles: bool,
    tuning: DynamicIccTuning,
    identity: &DynamicMonitorIdentity,
) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let sdr_path = ensure_active_profile_installed_tuned_for_monitor(
        color_dir,
        sdr_preset,
        custom_profile_name,
        custom_gamma,
        luminance_cd_m2,
        install_specialized_profiles,
        tuning,
        identity,
    )?;
    let hdr_path = if sdr_preset.eq_ignore_ascii_case(hdr_preset) {
        sdr_path.clone()
    } else {
        ensure_active_profile_installed_tuned_for_monitor(
            color_dir,
            hdr_preset,
            custom_profile_name,
            custom_gamma,
            luminance_cd_m2,
            install_specialized_profiles,
            tuning,
            identity,
        )?
    };
    Ok((sdr_path, hdr_path))
}

/// Re-apply the currently-active profile while also refreshing SDR/HDR display associations.
pub fn reapply_profile_with_mode_associations(
    device_key: &str,
    active_profile_path: &Path,
    sdr_profile_path: &Path,
    hdr_profile_path: &Path,
    toggle_delay_ms: u64,
    _per_user: bool,
) -> Result<(), Box<dyn Error>> {
    if !active_profile_path.exists() {
        return Err(format!("Profile not found: {}", active_profile_path.display()).into());
    }
    if !sdr_profile_path.exists() {
        return Err(format!("SDR profile not found: {}", sdr_profile_path.display()).into());
    }
    if !hdr_profile_path.exists() {
        return Err(format!("HDR profile not found: {}", hdr_profile_path.display()).into());
    }

    register_color_profile(active_profile_path)?;
    register_color_profile(sdr_profile_path)?;
    if !hdr_profile_path.eq(sdr_profile_path) {
        register_color_profile(hdr_profile_path)?;
    }

    const MAX_APPLY_ATTEMPTS: usize = 3;
    let mut last_verification_error: Option<String> = None;

    for attempt in 1..=MAX_APPLY_ATTEMPTS {
        enable_per_user_monitor_profiles(device_key);

        // Always attempt both scopes; some systems only honor current-user scope
        // even when system-wide APIs report success.
        reapply_profile(device_key, active_profile_path, toggle_delay_ms, true)?;
        set_display_default_association(device_key, sdr_profile_path, true)?;
        add_hdr_display_association(device_key, hdr_profile_path, true)?;
        set_generic_default(device_key, sdr_profile_path, true)?;
        let icm_ok = match set_icm_profile_for_display_device(device_key, active_profile_path) {
            Ok(()) => true,
            Err(e) => {
                let msg = format!("SetICMProfileW apply failed for {}: {}", device_key, e);
                warn!("{}", msg);
                false
            }
        };
        let vcgt_ok = match apply_vcgt_gamma_ramp_from_profile(device_key, active_profile_path) {
            Ok(Some(())) => true,
            Ok(None) => {
                info!(
                    "No vcgt tag present in {}; skipping SetDeviceGammaRamp",
                    active_profile_path.display()
                );
                true
            }
            Err(e) => {
                let msg = format!("SetDeviceGammaRamp apply failed for {}: {}", device_key, e);
                warn!("{}", msg);
                false
            }
        };

        let verified_system = verify_wcs_default_profile_name(
            device_key,
            sdr_profile_path,
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
        )?;
        let verified_user = verify_wcs_default_profile_name(
            device_key,
            sdr_profile_path,
            WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
        )?;
        let verified = verified_system || verified_user;

        if verified && icm_ok && vcgt_ok {
            info!(
                "Profile verification passed for {} on attempt {}/{} (system_scope={} user_scope={} icm_ok={} vcgt_ok={})",
                device_key, attempt, MAX_APPLY_ATTEMPTS, verified_system, verified_user, icm_ok, vcgt_ok
            );
            return Ok(());
        }

        let note = format!(
            "{} '{}' for {} on attempt {}/{}",
            if !icm_ok {
                "SetICMProfileW did not confirm active profile"
            } else if !vcgt_ok {
                "SetDeviceGammaRamp did not apply vcgt"
            } else {
                "WCS did not confirm"
            },
            sdr_profile_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| sdr_profile_path.display().to_string()),
            device_key,
            attempt,
            MAX_APPLY_ATTEMPTS
        );
        warn!("{}", note);
        last_verification_error = Some(note);

        // Legacy fallback: explicitly associate the SDR/HDR profiles via the
        // older mscms API on stacks where modern default-association calls
        // are not enough.
        if let Err(e) = associate_profile_with_device_legacy(device_key, sdr_profile_path) {
            warn!("Legacy SDR association fallback failed: {}", e);
        }
        if !hdr_profile_path.eq(sdr_profile_path) {
            if let Err(e) = associate_profile_with_device_legacy(device_key, hdr_profile_path) {
                warn!("Legacy HDR association fallback failed: {}", e);
            }
        }

        if attempt < MAX_APPLY_ATTEMPTS {
            // Fallback refresh before retrying. Start with a soft, non-flicker
            // refresh and only escalate to a full mode-level refresh before
            // the final retry.
            let use_hard_refresh = attempt + 1 == MAX_APPLY_ATTEMPTS;
            if use_hard_refresh {
                warn!(
                    "Escalating to full display refresh before final retry for {}",
                    device_key
                );
                refresh_display(true, true, true);
            } else {
                refresh_display(false, true, false);
            }
            match run_calibration_loader_task() {
                Ok(()) => info!("Calibration Loader fallback triggered"),
                Err(e) => warn!("Calibration Loader fallback failed: {}", e),
            }

            // Give WCS a short window to settle before retrying.
            thread::sleep(Duration::from_millis(250 * attempt as u64));
        }
    }

    Err(last_verification_error
        .unwrap_or_else(|| {
            format!(
                "Profile apply could not be verified for {} after {} attempts",
                device_key, MAX_APPLY_ATTEMPTS
            )
        })
        .into())
}

/// Legacy fallback association method via `AssociateColorProfileWithDeviceW`.
///
/// Some display stacks respond better to this older API than modern WCS
/// default-association APIs. It is used only when verification fails.
fn associate_profile_with_device_legacy(
    device_key: &str,
    profile_path: &Path,
) -> Result<(), Box<dyn Error>> {
    if !profile_path.exists() {
        return Err(format!("Profile not found: {}", profile_path.display()).into());
    }

    let profile_wide: Vec<u16> = profile_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let device_wide = to_wide(device_key);

    unsafe {
        let result = AssociateColorProfileWithDeviceW(
            PCWSTR(ptr::null()),
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            return Err(format!(
                "AssociateColorProfileWithDeviceW failed for {} (Win32={})",
                device_key, err
            )
            .into());
        }
    }

    info!(
        "Legacy profile association set for {} using {}",
        device_key,
        profile_path.display()
    );
    Ok(())
}

/// Best-effort cleanup of legacy profile names from the Windows color store.
fn cleanup_legacy_profile_files(active_profile_path: &Path) {
    if !is_in_color_directory(active_profile_path) {
        return;
    }

    let Some(active_name) = active_profile_path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
    else {
        return;
    };

    let color_dir = color_directory();
    for legacy_name in LEGACY_PROFILE_NAMES {
        if active_name == *legacy_name {
            continue;
        }
        let legacy_path = color_dir.join(legacy_name);
        if !legacy_path.exists() {
            continue;
        }
        match std::fs::remove_file(&legacy_path) {
            Ok(()) => info!(
                "Removed legacy ICC profile from color store: {}",
                legacy_path.display()
            ),
            Err(e) => warn!(
                "Could not remove legacy ICC profile {}: {}",
                legacy_path.display(),
                e
            ),
        }
    }
}

/// Register an ICC profile with the Windows Color System via
/// `InstallColorProfileW` (mscms.dll).
///
/// This lets the WCS association/disassociation APIs find the profile.
/// Calling it on an already-registered profile is harmless.
///
/// **Important:** `InstallColorProfileW` copies the file into the system color
/// directory (`%WINDIR%\System32\spool\drivers\color`).  If the profile is
/// *not* already in that directory, calling this would create an unwanted copy
/// (e.g. from test paths).  To prevent that, this function is a no-op when the
/// profile path is outside the color directory.
pub fn register_color_profile(profile_path: &Path) -> Result<(), Box<dyn Error>> {
    if !is_in_color_directory(profile_path) {
        info!(
            "Skipping WCS registration (not in color directory): {}",
            profile_path.display()
        );
        return Ok(());
    }
    if !profile_path.exists() {
        return Err(format!("Profile not found: {}", profile_path.display()).into());
    }

    let path_wide: Vec<u16> = profile_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let install_ok =
        unsafe { InstallColorProfileW(PCWSTR(ptr::null()), PCWSTR(path_wide.as_ptr())) };
    if install_ok.as_bool() {
        info!("Profile registered with WCS: {}", profile_path.display());
        Ok(())
    } else {
        let code = io::Error::last_os_error();
        Err(format!(
            "InstallColorProfileW failed for {} ({})",
            profile_path.display(),
            code
        )
        .into())
    }
}

/// Return the Windows system color profile directory.
pub fn color_directory() -> PathBuf {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    PathBuf::from(windir)
        .join("System32")
        .join("spool")
        .join("drivers")
        .join("color")
}

/// Return the app-owned profile export directory.
///
/// This mirrors generated profiles for easy inspection/backups outside
/// the Windows color store.
pub fn app_profiles_directory() -> PathBuf {
    let program_data =
        std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    PathBuf::from(program_data)
        .join("LG-UltraGear-Monitor")
        .join("profiles")
}

fn export_profile_to_app_profiles_dir(profile_path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let file_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let dst_dir = app_profiles_directory();
    if !dst_dir.exists() {
        std::fs::create_dir_all(&dst_dir)?;
    }
    let dst = dst_dir.join(file_name);
    let src_bytes = std::fs::read(profile_path)?;
    if let Ok(existing) = std::fs::read(&dst) {
        if existing == src_bytes {
            return Ok(dst);
        }
    }
    std::fs::write(&dst, src_bytes)?;
    info!("Exported ICC profile artifact to {}", dst.display());
    Ok(dst)
}

/// Check whether `path` resides in the Windows color directory.
fn is_in_color_directory(path: &Path) -> bool {
    let color_dir = color_directory();
    match path.parent() {
        Some(parent) => {
            // Case-insensitive comparison for Windows paths.
            parent.to_string_lossy().to_lowercase() == color_dir.to_string_lossy().to_lowercase()
        }
        None => false,
    }
}

/// Remove stale/leftover ICM files from the system color directory.
///
/// Scans for files that do NOT match `expected_name` and whose names
/// match patterns known to come from test runs or previous versions of
/// this tool.  Returns a list of paths that were deleted.
pub fn cleanup_stale_profiles(expected_name: &str) -> Vec<PathBuf> {
    let color_dir = color_directory();
    let mut removed = Vec::new();

    let entries = match std::fs::read_dir(&color_dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Cannot read color directory {}: {}", color_dir.display(), e);
            return removed;
        }
    };

    // Known stale patterns from test runs and development.
    let stale_patterns: &[&str] = &[
        "lg-ultragear-full-cal.icm",
        "test-embedded.icm",
        "edge-test.icm",
        "wrong-size.icm",
        "nested.icm",
        "remove-test.icm",
        "check.icm",
        "test-extract.icm",
        "test-idempotent.icm",
        "test-roundtrip.icm",
        "test-re-extract.icm",
        "test-is-installed.icm",
        "test-content.icm",
        "test-overwrite.icm",
        "register-test.icm",
        "wrong.icm",
        "size-check.icm",
    ];

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip the expected profile
        if name_str.eq_ignore_ascii_case(expected_name) {
            continue;
        }

        // Only consider .icm files
        if !name_str.to_lowercase().ends_with(".icm") {
            continue;
        }

        let name_lower = name_str.to_lowercase();
        let is_monitor_scoped_generated = name_lower.starts_with("lg-ultragear-gamma22-cmx-")
            || name_lower.starts_with("lg-ultragear-gamma24-cmx-")
            || name_lower.starts_with("lg-ultragear-reader-cmx-")
            || name_lower.starts_with("lg-ultragear-dynamic-cmx-");
        if stale_patterns.iter().any(|p| name_lower == *p) || is_monitor_scoped_generated {
            let path = entry.path();
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    info!("Removed stale profile: {}", path.display());
                    removed.push(path);
                }
                Err(e) => {
                    warn!("Failed to remove stale profile {}: {}", path.display(), e);
                }
            }
        }
    }

    if !removed.is_empty() {
        info!("Cleaned up {} stale profile(s)", removed.len());
    }

    removed
}

// ============================================================================
// mscms.dll FFI — WCS color profile APIs
// ============================================================================

/// Check if the ICC profile is installed at the given path.
pub fn is_profile_installed(profile_path: &Path) -> bool {
    profile_path.exists()
}

/// Remove the ICC profile from the Windows color store.
///
/// Retries with exponential back-off if the file is locked (e.g. by the WCS
/// engine or the service process).  After all retries, schedules the file for
/// deletion on next reboot via `MoveFileExW(MOVEFILE_DELAY_UNTIL_REBOOT)`.
///
/// Returns `Ok(true)` if the file was removed (or scheduled for removal),
/// `Ok(false)` if it didn't exist.
pub fn remove_profile(profile_path: &Path) -> Result<bool, Box<dyn Error>> {
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};

    if !profile_path.exists() {
        info!("ICC profile not present: {}", profile_path.display());
        return Ok(false);
    }

    // Retry up to 5 times with increasing back-off (total ~3 s).
    let delays_ms: &[u64] = &[0, 200, 500, 1000, 1500];
    for (attempt, &ms) in delays_ms.iter().enumerate() {
        if ms > 0 {
            thread::sleep(Duration::from_millis(ms));
        }
        match std::fs::remove_file(profile_path) {
            Ok(()) => {
                info!(
                    "ICC profile removed: {} (attempt {})",
                    profile_path.display(),
                    attempt + 1
                );
                return Ok(true);
            }
            Err(e) if e.raw_os_error() == Some(32) => {
                // ERROR_SHARING_VIOLATION — file is locked, retry.
                info!(
                    "Profile locked (attempt {}): {} — retrying",
                    attempt + 1,
                    profile_path.display()
                );
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    // Last resort: schedule for deletion on next reboot.
    let wide: Vec<u16> = profile_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe { MoveFileExW(PCWSTR(wide.as_ptr()), None, MOVEFILE_DELAY_UNTIL_REBOOT) };
    match ok {
        Ok(()) => {
            warn!(
                "ICC profile locked — scheduled for deletion on reboot: {}",
                profile_path.display()
            );
            Ok(true)
        }
        Err(e) => Err(format!(
            "Could not remove or schedule {} for deletion: {}",
            profile_path.display(),
            e
        )
        .into()),
    }
}

fn enable_per_user_monitor_profiles(device_key: &str) {
    let device_wide = to_wide(device_key);
    let mut enabled = BOOL::from(false);
    let get_ok = unsafe {
        WcsGetUsePerUserProfiles(
            PCWSTR(device_wide.as_ptr()),
            CLASS_MONITOR_SIGNATURE,
            &mut enabled,
        )
    };
    if !get_ok.as_bool() {
        let err = io::Error::last_os_error();
        warn!(
            "WcsGetUsePerUserProfiles failed for {} (class='mntr', err={})",
            device_key, err
        );
        return;
    }

    if enabled.as_bool() {
        return;
    }

    let set_ok = unsafe {
        WcsSetUsePerUserProfiles(
            PCWSTR(device_wide.as_ptr()),
            CLASS_MONITOR_SIGNATURE,
            BOOL::from(true),
        )
    };
    if !set_ok.as_bool() {
        let err = io::Error::last_os_error();
        warn!(
            "WcsSetUsePerUserProfiles failed for {} (class='mntr', err={})",
            device_key, err
        );
    } else {
        info!("Enabled per-user monitor profile mode for {}", device_key);
    }
}

fn query_wcs_default_profile_name(
    device_key: &str,
    scope: WCS_PROFILE_MANAGEMENT_SCOPE,
) -> Result<Option<String>, Box<dyn Error>> {
    let device_wide = to_wide(device_key);
    let mut size_bytes = 0u32;
    let size_ok = unsafe {
        WcsGetDefaultColorProfileSize(
            scope,
            PCWSTR(device_wide.as_ptr()),
            CPT_ICC,
            CPST_NONE,
            0,
            &mut size_bytes,
        )
    };
    if !size_ok.as_bool() {
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(2) | Some(1168) => return Ok(None), // file/object not found
            _ => {
                return Err(format!(
                    "WcsGetDefaultColorProfileSize failed for {} (scope={}, err={})",
                    device_key, scope.0, err
                )
                .into());
            }
        }
    }
    if size_bytes == 0 {
        return Ok(None);
    }

    let mut buf = vec![0u16; (size_bytes as usize / 2).saturating_add(2)];
    let ok = unsafe {
        WcsGetDefaultColorProfile(
            scope,
            PCWSTR(device_wide.as_ptr()),
            CPT_ICC,
            CPST_NONE,
            0,
            size_bytes,
            PWSTR(buf.as_mut_ptr()),
        )
    };
    if !ok.as_bool() {
        let err = io::Error::last_os_error();
        return Err(format!(
            "WcsGetDefaultColorProfile failed for {} (scope={}, err={})",
            device_key, scope.0, err
        )
        .into());
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    if len == 0 {
        return Ok(None);
    }
    Ok(Some(String::from_utf16_lossy(&buf[..len])))
}

#[derive(Clone, Debug)]
struct DisplayColorTarget {
    adapter_id: windows::Win32::Foundation::LUID,
    source_id: u32,
    gdi_device_name: Option<String>,
}

fn query_active_display_paths_for_color() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>, Box<dyn Error>> {
    let (paths, _) = query_active_display_config()?;
    Ok(paths)
}

fn query_active_display_config(
) -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>), Box<dyn Error>> {
    const RETRIES: usize = 3;

    for _ in 0..RETRIES {
        let mut path_count = 0u32;
        let mut mode_count = 0u32;
        let size_status = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if size_status != ERROR_SUCCESS {
            return Err(format!("GetDisplayConfigBufferSizes failed: {}", size_status.0).into());
        }
        if path_count == 0 {
            return Ok((Vec::new(), Vec::new()));
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let mut queried_paths = path_count;
        let mut queried_modes = mode_count;
        let query_status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut queried_paths,
                paths.as_mut_ptr(),
                &mut queried_modes,
                modes.as_mut_ptr(),
                None,
            )
        };
        if query_status == ERROR_SUCCESS {
            paths.truncate(queried_paths as usize);
            modes.truncate(queried_modes as usize);
            return Ok((paths, modes));
        }
        if query_status != ERROR_INSUFFICIENT_BUFFER {
            return Err(format!("QueryDisplayConfig failed: {}", query_status.0).into());
        }
    }

    Err("QueryDisplayConfig repeatedly returned insufficient buffer".into())
}

fn utf16_nul_to_string(raw: &[u16]) -> String {
    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    if len == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&raw[..len]).trim().to_string()
}

fn normalize_monitor_device_key(value: &str) -> String {
    let mut out = value.trim().to_ascii_uppercase();
    if let Some(stripped) = out.strip_prefix(r"\\?\") {
        out = stripped.to_string();
    }
    if let Some(idx) = out.find("#{") {
        out = out[..idx].to_string();
    }
    out = out.replace('#', "\\");
    out = out.trim_end_matches('\\').to_string();
    out = out.trim_end_matches("_0").to_string();
    out
}

fn resolve_display_color_target(
    device_key: &str,
) -> Result<Option<DisplayColorTarget>, Box<dyn Error>> {
    let wanted = normalize_monitor_device_key(device_key);
    if wanted.is_empty() {
        return Ok(None);
    }
    let mut candidates: Vec<String> = Vec::new();

    for path in query_active_display_paths_for_color()? {
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
        source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
        source.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
        source.header.adapterId = path.sourceInfo.adapterId;
        source.header.id = path.sourceInfo.id;
        let source_status = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) };
        let gdi_name = if source_status == ERROR_SUCCESS.0 as i32 {
            let name = utf16_nul_to_string(&source.viewGdiDeviceName);
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        } else {
            None
        };

        let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        target.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
        target.header.adapterId = path.targetInfo.adapterId;
        target.header.id = path.targetInfo.id;

        let status = unsafe { DisplayConfigGetDeviceInfo(&mut target.header) };
        if status != ERROR_SUCCESS.0 as i32 {
            continue;
        }

        let target_path = utf16_nul_to_string(&target.monitorDevicePath);
        if target_path.is_empty() {
            continue;
        }
        let friendly = utf16_nul_to_string(&target.monitorFriendlyDeviceName);
        let normalized = normalize_monitor_device_key(&target_path);
        candidates.push(if friendly.is_empty() {
            normalized.clone()
        } else {
            format!("{} ({})", normalized, friendly)
        });
        if normalized.eq_ignore_ascii_case(&wanted)
            || normalized.contains(&wanted)
            || wanted.contains(&normalized)
        {
            return Ok(Some(DisplayColorTarget {
                adapter_id: path.targetInfo.adapterId,
                source_id: path.sourceInfo.id,
                gdi_device_name: gdi_name,
            }));
        }
    }

    if !candidates.is_empty() {
        warn!(
            "Could not map monitor device key '{}' to an active display path. Candidates: {}",
            wanted,
            candidates.join("; ")
        );
    } else {
        warn!(
            "Could not map monitor device key '{}' to an active display path (no active display targets reported)",
            wanted
        );
    }

    Ok(None)
}

fn canonical_profile_file_name(value: &str) -> String {
    let trimmed = value.trim().trim_matches('"');
    Path::new(trimmed)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| trimmed.to_string())
}

fn query_display_default_profile_name(
    device_key: &str,
    scope: WCS_PROFILE_MANAGEMENT_SCOPE,
) -> Result<Option<String>, Box<dyn Error>> {
    let Some(target) = resolve_display_color_target(device_key)? else {
        return Err(format!(
            "Could not resolve active display path for device key '{}'",
            device_key
        )
        .into());
    };

    let profile = unsafe {
        ColorProfileGetDisplayDefault(
            scope,
            target.adapter_id,
            target.source_id,
            CPT_ICC,
            CPST_STANDARD_DISPLAY_COLOR_MODE,
        )
    };
    let Ok(profile_ptr) = profile else {
        return Ok(None);
    };
    if profile_ptr.is_null() {
        return Ok(None);
    }

    let value = unsafe { PCWSTR(profile_ptr.0).to_string().unwrap_or_default() };
    unsafe {
        let _ = LocalFree(windows::Win32::Foundation::HLOCAL(profile_ptr.0 as *mut _));
    }
    let canonical = canonical_profile_file_name(&value);
    if canonical.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(canonical))
}

fn set_icm_profile_for_display_device(
    device_key: &str,
    profile_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let Some(target) = resolve_display_color_target(device_key)? else {
        return Err(format!(
            "Could not resolve active display path for device key '{}'",
            device_key
        )
        .into());
    };
    let Some(gdi_name) = target.gdi_device_name.as_ref() else {
        return Err(format!(
            "Could not resolve GDI display name for device key '{}'",
            device_key
        )
        .into());
    };

    let driver_wide = to_wide("DISPLAY");
    let gdi_wide = to_wide(gdi_name);
    let profile_wide: Vec<u16> = profile_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hdc = CreateDCW(
            PCWSTR(driver_wide.as_ptr()),
            PCWSTR(gdi_wide.as_ptr()),
            PCWSTR(ptr::null()),
            None,
        );
        if hdc.0.is_null() {
            let err = io::Error::last_os_error();
            return Err(format!("CreateDCW failed for {} ({})", gdi_name, err).into());
        }

        let set_ok = SetICMProfileW(hdc, PCWSTR(profile_wide.as_ptr()));

        // Best-effort readback verification from GDI.
        let mut size = 1024u32;
        let mut buf = vec![0u16; size as usize];
        let mut readback = String::new();
        if GetICMProfileW(hdc, &mut size, PWSTR(buf.as_mut_ptr())).as_bool() {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            if len > 0 {
                readback = canonical_profile_file_name(&String::from_utf16_lossy(&buf[..len]));
            }
        }

        let _ = DeleteDC(hdc);

        if !set_ok.as_bool() {
            let err = io::Error::last_os_error();
            return Err(format!("SetICMProfileW failed for {} ({})", gdi_name, err).into());
        }

        if !readback.is_empty() {
            let expected_name = profile_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| profile_path.display().to_string());
            if !readback.eq_ignore_ascii_case(&expected_name) {
                warn!(
                    "SetICMProfileW readback mismatch for {}: expected '{}' got '{}'",
                    gdi_name, expected_name, readback
                );
            }
        }
    }

    info!(
        "SetICMProfileW applied for device {} via {}",
        device_key, gdi_name
    );
    Ok(())
}

fn parse_vcgt_gamma_ramp(
    tag_payload: &[u8],
) -> Result<[u16; CURVE_TABLE_SIZE * 3], Box<dyn Error>> {
    let table_offset = if tag_payload.len() >= 8 && &tag_payload[0..4] == b"vcgt" {
        8usize // type signature + reserved
    } else {
        0usize
    };

    let read_u16 = |offset: usize| -> Result<u16, Box<dyn Error>> {
        let Some(slice) = tag_payload.get(offset..offset + 2) else {
            return Err(format!("vcgt payload too small at u16 offset {}", offset).into());
        };
        Ok(u16::from_be_bytes([slice[0], slice[1]]))
    };
    let read_u32 = |offset: usize| -> Result<u32, Box<dyn Error>> {
        let Some(slice) = tag_payload.get(offset..offset + 4) else {
            return Err(format!("vcgt payload too small at u32 offset {}", offset).into());
        };
        Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
    };

    let table_mode = read_u32(table_offset)?;
    if table_mode != 0 {
        return Err(format!(
            "unsupported vcgt mode {} (only table mode 0 is supported)",
            table_mode
        )
        .into());
    }
    let channels = read_u16(table_offset + 4)? as usize;
    let entries = read_u16(table_offset + 6)? as usize;
    let bytes_per_entry = read_u16(table_offset + 8)? as usize;

    if channels != 3 {
        return Err(format!("unsupported vcgt channel count {} (expected 3)", channels).into());
    }
    if entries == 0 {
        return Err("vcgt has zero entries".into());
    }
    if bytes_per_entry != 2 {
        return Err(format!(
            "unsupported vcgt entry size {} (expected 2)",
            bytes_per_entry
        )
        .into());
    }

    let mut channel_data: [Vec<u16>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut cursor = table_offset + 10;
    for channel in &mut channel_data {
        let mut values = Vec::with_capacity(entries);
        for _ in 0..entries {
            let Some(slice) = tag_payload.get(cursor..cursor + 2) else {
                return Err("vcgt payload truncated".into());
            };
            values.push(u16::from_be_bytes([slice[0], slice[1]]));
            cursor += 2;
        }
        *channel = values;
    }

    let sample = |values: &[u16], idx256: usize| -> u16 {
        if values.len() == 1 {
            return values[0];
        }
        let pos = (idx256 as f64) * ((values.len() - 1) as f64) / ((CURVE_TABLE_SIZE - 1) as f64);
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        if lo == hi {
            values[lo]
        } else {
            let t = pos - lo as f64;
            let a = values[lo] as f64;
            let b = values[hi] as f64;
            ((a + (b - a) * t).round() as i64).clamp(0, 65535) as u16
        }
    };

    let mut ramp = [0u16; CURVE_TABLE_SIZE * 3];
    for i in 0..CURVE_TABLE_SIZE {
        ramp[i] = sample(&channel_data[0], i);
        ramp[CURVE_TABLE_SIZE + i] = sample(&channel_data[1], i);
        ramp[(CURVE_TABLE_SIZE * 2) + i] = sample(&channel_data[2], i);
    }
    Ok(ramp)
}

fn apply_vcgt_gamma_ramp_from_profile(
    device_key: &str,
    profile_path: &Path,
) -> Result<Option<()>, Box<dyn Error>> {
    let bytes = std::fs::read(profile_path)?;
    let raw = RawProfile::from_bytes(&bytes)?;
    let Some(record) = raw.tags.get(&TagSignature::Vcgt) else {
        return Ok(None);
    };

    let ramp = parse_vcgt_gamma_ramp(record.tag.as_slice())?;

    let Some(target) = resolve_display_color_target(device_key)? else {
        return Err(format!(
            "Could not resolve active display path for device key '{}'",
            device_key
        )
        .into());
    };
    let Some(gdi_name) = target.gdi_device_name.as_ref() else {
        return Err(format!(
            "Could not resolve GDI display name for device key '{}'",
            device_key
        )
        .into());
    };

    let driver_wide = to_wide("DISPLAY");
    let gdi_wide = to_wide(gdi_name);
    unsafe {
        let hdc = CreateDCW(
            PCWSTR(driver_wide.as_ptr()),
            PCWSTR(gdi_wide.as_ptr()),
            PCWSTR(ptr::null()),
            None,
        );
        if hdc.0.is_null() {
            let err = io::Error::last_os_error();
            return Err(format!("CreateDCW failed for {} ({})", gdi_name, err).into());
        }

        let ok = SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const core::ffi::c_void);
        let _ = DeleteDC(hdc);
        if !ok.as_bool() {
            let err = io::Error::last_os_error();
            return Err(format!("SetDeviceGammaRamp failed for {} ({})", gdi_name, err).into());
        }
    }

    info!(
        "SetDeviceGammaRamp applied vcgt for device {} via {}",
        device_key, gdi_name
    );
    Ok(Some(()))
}

fn verify_wcs_default_profile_name(
    device_key: &str,
    expected_profile_path: &Path,
    scope: WCS_PROFILE_MANAGEMENT_SCOPE,
) -> Result<bool, Box<dyn Error>> {
    let expected_name = expected_profile_path
        .file_name()
        .ok_or_else(|| {
            format!(
                "Invalid expected profile path: {}",
                expected_profile_path.display()
            )
        })?
        .to_string_lossy()
        .to_string();

    if let Some(name) = query_display_default_profile_name(device_key, scope)? {
        if name.eq_ignore_ascii_case(&expected_name) {
            return Ok(true);
        }
        warn!(
            "Display default profile mismatch for {} (scope={}): expected '{}' got '{}'",
            device_key, scope.0, expected_name, name
        );
        return Ok(false);
    }

    let actual = query_wcs_default_profile_name(device_key, scope)?;
    match actual {
        Some(name) if name.eq_ignore_ascii_case(&expected_name) => Ok(true),
        Some(name) => {
            warn!(
                "WCS default profile mismatch for {} (scope={}): expected '{}' got '{}'",
                device_key, scope.0, expected_name, name
            );
            Ok(false)
        }
        None => {
            warn!(
                "WCS default profile check reported no default for {} (scope={}); expected '{}'",
                device_key, scope.0, expected_name
            );
            Ok(false)
        }
    }
}

/// Reapply the color profile for a single monitor device key using the toggle
/// approach: disassociate (reverts to default) → pause → reassociate (applies fix).
/// This forces Windows to actually reload the ICC profile.
///
/// # Arguments
/// * `device_key` — WMI device instance path (e.g. `DISPLAY\LGS\001`)
/// * `profile_path` — Full path to the ICC profile file
/// * `toggle_delay_ms` — Pause between disassociate and reassociate (ms)
/// * `per_user` — If true, also perform per-user scope operations
pub fn reapply_profile(
    device_key: &str,
    profile_path: &Path,
    toggle_delay_ms: u64,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    if !profile_path.exists() {
        return Err(format!("Profile not found: {}", profile_path.display()).into());
    }

    // WCS association APIs expect just the filename, not the full path.
    // The profile must already be registered via InstallColorProfileW.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let device_wide = to_wide(device_key);

    unsafe {
        // Step 1: Disassociate (reverts to default profile)
        // Failure here is non-fatal — the profile may not be currently associated.
        let result = WcsDisassociateColorProfileFromDevice(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            warn!(
                "WcsDisassociateColorProfileFromDevice failed for {} (Win32={}) (non-fatal)",
                device_key, err
            );
        }

        // Per-user disassociate (non-fatal)
        if per_user {
            let result = WcsDisassociateColorProfileFromDevice(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(profile_wide.as_ptr()),
                PCWSTR(device_wide.as_ptr()),
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "WcsDisassociateColorProfileFromDevice (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            }
        }

        // Step 2: Configurable pause to let Windows process the change
        thread::sleep(Duration::from_millis(toggle_delay_ms));

        // Step 3: Re-associate (applies the fix profile)
        // Failure here IS fatal — the profile was NOT applied.
        let result = WcsAssociateColorProfileWithDevice(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            PCWSTR(device_wide.as_ptr()),
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            return Err(format!(
                "WcsAssociateColorProfileWithDevice failed for {} (Win32={})",
                device_key, err
            )
            .into());
        }

        // Per-user associate
        if per_user {
            let result = WcsAssociateColorProfileWithDevice(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(profile_wide.as_ptr()),
                PCWSTR(device_wide.as_ptr()),
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "WcsAssociateColorProfileWithDevice (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            }
        }
    }

    info!("Profile toggled for device: {}", device_key);
    Ok(())
}

/// Set the profile as the generic default using the legacy `WcsSetDefaultColorProfile` API.
///
/// This is an optional operation — some systems or monitors benefit from having the
/// profile also registered as the generic ICC default, but it is NOT required for the
/// dimming fix to work.
///
/// # Arguments
/// * `device_key` — WMI device instance path
/// * `profile_path` — Full path to the ICC profile file
/// * `per_user` — If true, also set the per-user generic default
pub fn set_generic_default(
    device_key: &str,
    profile_path: &Path,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    // WCS APIs expect just the filename, not the full path.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let device_wide = to_wide(device_key);

    unsafe {
        // System-wide generic default
        let result = WcsSetDefaultColorProfile(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(device_wide.as_ptr()),
            CPT_ICC,
            CPST_NONE,
            0,
            PCWSTR(profile_wide.as_ptr()),
        );
        if !result.as_bool() {
            let err = io::Error::last_os_error();
            warn!(
                "WcsSetDefaultColorProfile (system) failed for {} (Win32={}) (non-fatal)",
                device_key, err
            );
        } else {
            info!("Generic default profile set (system) for {}", device_key);
        }

        // Per-user generic default
        if per_user {
            let result = WcsSetDefaultColorProfile(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(device_wide.as_ptr()),
                CPT_ICC,
                CPST_NONE,
                0,
                PCWSTR(profile_wide.as_ptr()),
            );
            if !result.as_bool() {
                let err = io::Error::last_os_error();
                warn!(
                    "WcsSetDefaultColorProfile (per-user) failed for {} (Win32={}) (non-fatal)",
                    device_key, err
                );
            } else {
                info!("Generic default profile set (per-user) for {}", device_key);
            }
        }
    }

    Ok(())
}

/// Set the SDR display-default association for a display device.
///
/// Calls `ColorProfileSetDisplayDefaultAssociation` (Win10+) which is the
/// modern API that the Color Management control panel uses.  This tells the
/// WCS display pipeline to actually apply the profile.
///
/// # Arguments
/// * `device_key` — WMI device instance path
/// * `profile_path` — Full path to the ICC profile file
/// * `per_user` — If true, also set the per-user association
pub fn set_display_default_association(
    device_key: &str,
    profile_path: &Path,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    // WCS APIs expect just the filename, not the full path.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let Some(target) = resolve_display_color_target(device_key)? else {
        warn!(
            "Could not map {} to an active display path for ColorProfileSetDisplayDefaultAssociation",
            device_key
        );
        return Ok(());
    };

    unsafe {
        let result = ColorProfileSetDisplayDefaultAssociation(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            CPT_ICC,
            CPST_STANDARD_DISPLAY_COLOR_MODE,
            target.adapter_id,
            target.source_id,
        );
        if let Err(err) = result {
            warn!(
                "ColorProfileSetDisplayDefaultAssociation (system) failed for {} ({}) (non-fatal)",
                device_key, err
            );
        } else {
            info!(
                "SDR display default association set (system) for {}",
                device_key
            );
        }

        if per_user {
            let result = ColorProfileSetDisplayDefaultAssociation(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(profile_wide.as_ptr()),
                CPT_ICC,
                CPST_STANDARD_DISPLAY_COLOR_MODE,
                target.adapter_id,
                target.source_id,
            );
            if let Err(err) = result {
                warn!(
                    "ColorProfileSetDisplayDefaultAssociation (per-user) failed for {} ({}) (non-fatal)",
                    device_key, err
                );
            } else {
                info!(
                    "SDR display default association set (per-user) for {}",
                    device_key
                );
            }
        }
    }

    Ok(())
}

/// Add the profile to the HDR/advanced-color association for a display device.
///
/// Calls `ColorProfileAddDisplayAssociation` (Win10+).
/// This is an opt-in operation for HDR displays.
///
/// # Arguments
/// * `device_key` — WMI device instance path
/// * `profile_path` — Full path to the ICC profile file
/// * `per_user` — If true, also add the per-user association
pub fn add_hdr_display_association(
    device_key: &str,
    profile_path: &Path,
    per_user: bool,
) -> Result<(), Box<dyn Error>> {
    // WCS APIs expect just the filename, not the full path.
    let profile_name = profile_path
        .file_name()
        .ok_or_else(|| format!("Invalid profile path: {}", profile_path.display()))?;
    let profile_wide: Vec<u16> = profile_name
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let Some(target) = resolve_display_color_target(device_key)? else {
        warn!(
            "Could not map {} to an active display path for ColorProfileAddDisplayAssociation",
            device_key
        );
        return Ok(());
    };

    unsafe {
        let result = ColorProfileAddDisplayAssociation(
            WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
            PCWSTR(profile_wide.as_ptr()),
            target.adapter_id,
            target.source_id,
            BOOL::from(true),
            BOOL::from(true),
        );
        if let Err(err) = result {
            warn!(
                "ColorProfileAddDisplayAssociation (system) failed for {} ({}) (non-fatal)",
                device_key, err
            );
        } else {
            info!("HDR display association added (system) for {}", device_key);
            if let Err(err) = ColorProfileSetDisplayDefaultAssociation(
                WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
                PCWSTR(profile_wide.as_ptr()),
                CPT_ICC,
                CPST_EXTENDED_DISPLAY_COLOR_MODE,
                target.adapter_id,
                target.source_id,
            ) {
                warn!(
                    "ColorProfileSetDisplayDefaultAssociation (HDR/system) failed for {} ({}) (non-fatal)",
                    device_key, err
                );
            }
        }

        if per_user {
            let result = ColorProfileAddDisplayAssociation(
                WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                PCWSTR(profile_wide.as_ptr()),
                target.adapter_id,
                target.source_id,
                BOOL::from(true),
                BOOL::from(true),
            );
            if let Err(err) = result {
                warn!(
                    "ColorProfileAddDisplayAssociation (per-user) failed for {} ({}) (non-fatal)",
                    device_key, err
                );
            } else {
                info!(
                    "HDR display association added (per-user) for {}",
                    device_key
                );
                if let Err(err) = ColorProfileSetDisplayDefaultAssociation(
                    WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER,
                    PCWSTR(profile_wide.as_ptr()),
                    CPT_ICC,
                    CPST_EXTENDED_DISPLAY_COLOR_MODE,
                    target.adapter_id,
                    target.source_id,
                ) {
                    warn!(
                        "ColorProfileSetDisplayDefaultAssociation (HDR/per-user) failed for {} ({}) (non-fatal)",
                        device_key, err
                    );
                }
            }
        }
    }

    Ok(())
}

fn reapply_display_topology_via_ccd() -> Result<&'static str, Box<dyn Error>> {
    let mut attempts: Vec<String> = Vec::new();

    let db_flags =
        SET_DISPLAY_CONFIG_FLAGS(SDC_APPLY.0 | SDC_USE_DATABASE_CURRENT.0 | SDC_NO_OPTIMIZATION.0);
    let db_status = unsafe { SetDisplayConfig(None, None, db_flags) };
    if db_status == ERROR_SUCCESS.0 as i32 {
        return Ok("database-current/no-optimization");
    }
    attempts.push(format!(
        "SetDisplayConfig(database-current/no-optimization)={}",
        db_status
    ));

    let db_allow_flags =
        SET_DISPLAY_CONFIG_FLAGS(SDC_APPLY.0 | SDC_USE_DATABASE_CURRENT.0 | SDC_ALLOW_CHANGES.0);
    let db_allow_status = unsafe { SetDisplayConfig(None, None, db_allow_flags) };
    if db_allow_status == ERROR_SUCCESS.0 as i32 {
        return Ok("database-current/allow-changes");
    }
    attempts.push(format!(
        "SetDisplayConfig(database-current/allow-changes)={}",
        db_allow_status
    ));

    let (paths, modes) = query_active_display_config()?;
    if paths.is_empty() {
        attempts.push("QueryDisplayConfig(active) returned zero paths".to_string());
        return Err(attempts.join("; ").into());
    }

    let supplied_flags = SET_DISPLAY_CONFIG_FLAGS(
        SDC_APPLY.0
            | SDC_USE_SUPPLIED_DISPLAY_CONFIG.0
            | SDC_ALLOW_CHANGES.0
            | SDC_NO_OPTIMIZATION.0,
    );
    let supplied_status = unsafe {
        SetDisplayConfig(
            Some(paths.as_slice()),
            Some(modes.as_slice()),
            supplied_flags,
        )
    };
    if supplied_status == ERROR_SUCCESS.0 as i32 {
        return Ok("supplied-config/no-optimization");
    }
    attempts.push(format!(
        "SetDisplayConfig(supplied-config/no-optimization)={}",
        supplied_status
    ));

    let supplied_allow_flags = SET_DISPLAY_CONFIG_FLAGS(
        SDC_APPLY.0 | SDC_USE_SUPPLIED_DISPLAY_CONFIG.0 | SDC_ALLOW_CHANGES.0,
    );
    let supplied_allow_status = unsafe {
        SetDisplayConfig(
            Some(paths.as_slice()),
            Some(modes.as_slice()),
            supplied_allow_flags,
        )
    };
    if supplied_allow_status == ERROR_SUCCESS.0 as i32 {
        return Ok("supplied-config/allow-changes");
    }
    attempts.push(format!(
        "SetDisplayConfig(supplied-config/allow-changes)={}",
        supplied_allow_status
    ));

    Err(attempts.join("; ").into())
}

fn no_flicker_test_mode_enabled() -> bool {
    TEST_NO_FLICKER_MODE.load(Ordering::Relaxed) || std::env::var_os(TEST_NO_FLICKER_ENV).is_some()
}

/// Test helper to disable all display-refresh side effects.
///
/// When enabled, `refresh_display` and `trigger_calibration_loader` become
/// no-ops. This is intended for test processes only.
pub fn set_test_no_flicker_mode(enabled: bool) {
    TEST_NO_FLICKER_MODE.store(enabled, Ordering::Relaxed);
}

/// Force display refresh using the specified Windows APIs.
///
/// # Arguments
/// * `display_settings` — Call `ChangeDisplaySettingsExW` (full display refresh)
/// * `broadcast_color` — Broadcast `WM_SETTINGCHANGE` with "Color" parameter
/// * `invalidate` — Call `InvalidateRect` to force window repaint
pub fn refresh_display(display_settings: bool, broadcast_color: bool, invalidate: bool) {
    if no_flicker_test_mode_enabled() {
        info!("No-flicker test mode: skipping display refresh operations");
        return;
    }

    if display_settings {
        match reapply_display_topology_via_ccd() {
            Ok(method) => info!("CCD display reapply succeeded via {}", method),
            Err(e) => warn!("CCD display reapply failed: {}", e),
        }
    }

    unsafe {
        // Method 1: ChangeDisplaySettingsEx with null — triggers full display mode refresh
        if display_settings {
            let _ = ChangeDisplaySettingsExW(
                PCWSTR(ptr::null()),
                None,
                HWND::default(),
                Default::default(),
                None,
            );
        }

        // Method 2: Broadcast WM_SETTINGCHANGE with "Color" parameter
        if broadcast_color {
            let color = HSTRING::from("Color");
            let mut _result = 0usize;
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(color.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                2000,
                Some(&mut _result),
            );
        }

        // Method 3: Invalidate all windows to force repaint
        if invalidate {
            let _ = InvalidateRect(HWND::default(), None, true);
        }
    }

    info!("Display refresh broadcast sent");
}

/// Trigger the built-in Windows Calibration Loader scheduled task.
///
/// Uses the COM Task Scheduler API directly (no external process spawning).
/// If `enabled` is false, returns immediately.
pub fn trigger_calibration_loader(enabled: bool) {
    if !enabled {
        return;
    }

    if no_flicker_test_mode_enabled() {
        info!("No-flicker test mode: skipping calibration loader trigger");
        return;
    }

    unsafe {
        if !WcsSetCalibrationManagementState(BOOL::from(true)).as_bool() {
            let err = io::Error::last_os_error();
            warn!("Could not enable calibration management state: {}", err);
        } else {
            info!("Calibration management state enabled");
        }
    }

    match run_calibration_loader_task() {
        Ok(()) => info!("Calibration Loader task triggered"),
        Err(e) => warn!("Calibration Loader task trigger failed: {}", e),
    }
}

/// Run the Windows Calibration Loader scheduled task via COM Task Scheduler API.
fn run_calibration_loader_task() -> Result<(), Box<dyn Error>> {
    // Initialize COM on this thread (balanced with CoUninitialize below).
    // ok() ignores S_FALSE (already initialized with same apartment model).
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED).ok();
    }

    let result = (|| -> Result<(), Box<dyn Error>> {
        let service: ITaskService =
            unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)? };

        // Connect to local Task Scheduler with current credentials
        let empty = windows::core::VARIANT::default();
        unsafe {
            service.Connect(&empty, &empty, &empty, &empty)?;
        }

        let folder =
            unsafe { service.GetFolder(&BSTR::from(r"\Microsoft\Windows\WindowsColorSystem"))? };

        let task = unsafe { folder.GetTask(&BSTR::from("Calibration Loader"))? };

        let _ = unsafe { task.Run(&windows::core::VARIANT::default())? };

        Ok(())
    })();

    unsafe {
        CoUninitialize();
    }

    result
}

/// Convert a Rust string to a null-terminated wide string (UTF-16).
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
#[path = "tests/profile_tests.rs"]
mod tests;
