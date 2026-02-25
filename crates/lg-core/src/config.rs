//! Configuration management — TOML file with sensible defaults.
//!
//! Config file location: `%ProgramData%\LG-UltraGear-Monitor\config.toml`
//! Falls back to compiled-in defaults if the file is missing or malformed.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Path to the config directory.
pub fn config_dir() -> PathBuf {
    let program_data =
        std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    PathBuf::from(program_data).join("LG-UltraGear-Monitor")
}

/// Full path to the config file.
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Full path to the installed service binary.
pub fn install_path() -> PathBuf {
    config_dir().join("lg-ultragear-dimming-fix.exe")
}

/// Service configuration with defaults for every field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Display name pattern to match (case-insensitive contains).
    pub monitor_match: String,

    /// Use regex matching for `monitor_match` instead of substring.
    pub monitor_match_regex: bool,

    /// ICC profile filename (looked up in Windows color store).
    pub profile_name: String,

    /// Gamma value used to generate the dynamic ICC transfer curves.
    /// Lower values brighten shadows/midtones; higher values darken them.
    pub icc_gamma: f64,

    /// Active ICC preset: `gamma22`, `gamma24`, `reader`, or `custom`.
    /// `custom` uses `profile_name` + `icc_gamma`.
    pub icc_active_preset: String,

    /// Keep Gamma 2.2 and 2.4 specialized profiles generated and up to date.
    pub icc_generate_specialized_profiles: bool,

    /// Target white luminance (cd/m^2) used for TRC shaping and encoded in ICC tags.
    pub icc_luminance_cd_m2: f64,

    /// Anti-dimming tuning preset.
    /// Supported values: `manual`, `anti_dim_soft`, `anti_dim_balanced`,
    /// `anti_dim_aggressive`, `anti_dim_night`, `reader_balanced`.
    pub icc_tuning_preset: String,

    /// When true, manually configured ICC tuning fields override a selected
    /// tuning preset (only for fields changed from defaults).
    pub icc_tuning_overlay_manual: bool,

    /// Lift near-black output to counter aggressive dimming.
    pub icc_black_lift: f64,

    /// Midtone shaping amount (positive boosts mids, negative dips mids).
    pub icc_midtone_boost: f64,

    /// Highlight compression amount to reduce clipping.
    pub icc_white_compression: f64,

    /// Per-channel gamma multipliers.
    pub icc_gamma_r: f64,
    pub icc_gamma_g: f64,
    pub icc_gamma_b: f64,

    /// Add a VCGT tag with LUT-based tuning.
    pub icc_vcgt_enabled: bool,

    /// VCGT blend amount from identity to generated LUT (0..1).
    pub icc_vcgt_strength: f64,

    /// Target black floor in cd/m^2 used while shaping curves.
    pub icc_target_black_cd_m2: f64,

    /// Include media black point tag (`bkpt`) based on target black luminance.
    pub icc_include_media_black_point: bool,

    /// Include device manufacturer/model description tags.
    pub icc_include_device_descriptions: bool,

    /// Include characterization target tag (`targ`).
    pub icc_include_characterization_target: bool,

    /// Include viewing condition description tag (`vued`).
    pub icc_include_viewing_cond_desc: bool,

    /// Optional Technology tag signature (4-char ICC signature, empty disables).
    pub icc_technology_signature: String,

    /// Optional Colorimetric Intent Image State tag signature (empty disables).
    pub icc_ciis_signature: String,

    /// Include CICP tag and payload.
    pub icc_cicp_enabled: bool,
    pub icc_cicp_primaries: u8,
    pub icc_cicp_transfer: u8,
    pub icc_cicp_matrix: u8,
    pub icc_cicp_full_range: bool,

    /// Include metadata tag (`meta`) with an empty dictionary payload.
    pub icc_metadata_enabled: bool,

    /// Include calibration timestamp tag (`calt`).
    pub icc_include_calibration_datetime: bool,

    /// Include chromatic adaptation matrix tag (`chad`).
    pub icc_include_chromatic_adaptation: bool,

    /// Include chromaticity primaries tag (`chrm`).
    pub icc_include_chromaticity: bool,

    /// Include measurement condition tag (`meas`).
    pub icc_include_measurement: bool,

    /// Include viewing conditions tag (`view`).
    pub icc_include_viewing_conditions: bool,

    /// Include spectral scaffolding tags (`sdin`, `swpt`, `svcn`) using data payloads.
    pub icc_include_spectral_scaffold: bool,

    /// Generate separate ICC files per matched monitor and embed identity metadata.
    pub icc_per_monitor_profiles: bool,

    /// Preferred preset when HDR/advanced color mode is active.
    pub icc_hdr_preset: String,

    /// Preferred preset when SDR mode is active.
    pub icc_sdr_preset: String,

    /// Optional day preset (used when both day and night presets are set).
    pub icc_schedule_day_preset: String,

    /// Optional night preset (used when both day and night presets are set).
    pub icc_schedule_night_preset: String,

    /// Automatically regenerate and apply optimized ICC after each ICC Studio
    /// parameter edit. Intended for rapid testing/tuning workflows.
    pub icc_auto_apply_on_change: bool,

    /// Show a Windows toast notification after each successful reapply.
    pub toast_enabled: bool,

    /// Toast title text.
    pub toast_title: String,

    /// Toast body text.
    pub toast_body: String,

    /// Milliseconds to wait after a display/session event before reapplying.
    /// Gives the display time to stabilize after connect/wake.
    pub stabilize_delay_ms: u64,

    /// Milliseconds to pause between disassociate and reassociate.
    /// Gives Windows time to process the profile removal.
    pub toggle_delay_ms: u64,

    /// Milliseconds to wait after the event storm settles before reapplying.
    /// This gives the display time to fully initialize (backlight ramp,
    /// scaler sync, color pipeline). Default 12000 (12 seconds).
    pub reapply_delay_ms: u64,

    /// Whether to call `ChangeDisplaySettingsExW` as part of the refresh.
    pub refresh_display_settings: bool,

    /// Whether to broadcast `WM_SETTINGCHANGE` with "Color" parameter.
    pub refresh_broadcast_color: bool,

    /// Whether to call `InvalidateRect` to force window repaint.
    pub refresh_invalidate: bool,

    /// Whether to trigger the Windows Calibration Loader scheduled task.
    pub refresh_calibration_loader: bool,

    /// Automatically set DDC/CI brightness after each profile reapply.
    pub ddc_brightness_on_reapply: bool,

    /// DDC/CI brightness value (0–100) to set when `ddc_brightness_on_reapply`
    /// is enabled.  Also used by the TUI "Set DDC Brightness" maintenance action.
    pub ddc_brightness_value: u32,

    /// Enable logging of every event (useful for debugging).
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor_match: "LG ULTRAGEAR".to_string(),
            monitor_match_regex: false,
            profile_name: "lg-ultragear-dynamic-cmx.icm".to_string(),
            icc_gamma: 2.05,
            icc_active_preset: "gamma22".to_string(),
            icc_generate_specialized_profiles: true,
            icc_luminance_cd_m2: 120.0,
            icc_tuning_preset: "anti_dim_balanced".to_string(),
            icc_tuning_overlay_manual: true,
            icc_black_lift: 0.0,
            icc_midtone_boost: 0.0,
            icc_white_compression: 0.0,
            icc_gamma_r: 1.0,
            icc_gamma_g: 1.0,
            icc_gamma_b: 1.0,
            icc_vcgt_enabled: false,
            icc_vcgt_strength: 0.0,
            icc_target_black_cd_m2: 0.2,
            icc_include_media_black_point: true,
            icc_include_device_descriptions: true,
            icc_include_characterization_target: true,
            icc_include_viewing_cond_desc: true,
            icc_technology_signature: "vidm".to_string(),
            icc_ciis_signature: "".to_string(),
            icc_cicp_enabled: false,
            icc_cicp_primaries: 1,
            icc_cicp_transfer: 13,
            icc_cicp_matrix: 0,
            icc_cicp_full_range: true,
            icc_metadata_enabled: false,
            icc_include_calibration_datetime: true,
            icc_include_chromatic_adaptation: true,
            icc_include_chromaticity: true,
            icc_include_measurement: true,
            icc_include_viewing_conditions: true,
            icc_include_spectral_scaffold: false,
            icc_per_monitor_profiles: true,
            icc_hdr_preset: "gamma22".to_string(),
            icc_sdr_preset: "gamma22".to_string(),
            icc_schedule_day_preset: "".to_string(),
            icc_schedule_night_preset: "".to_string(),
            icc_auto_apply_on_change: false,
            toast_enabled: true,
            toast_title: "LG UltraGear".to_string(),
            toast_body: "Color profile reapplied ✓".to_string(),
            stabilize_delay_ms: 1500,
            toggle_delay_ms: 100,
            reapply_delay_ms: 12000,
            refresh_display_settings: false,
            refresh_broadcast_color: true,
            refresh_invalidate: false,
            refresh_calibration_loader: true,
            ddc_brightness_on_reapply: false,
            ddc_brightness_value: 50,
            verbose: false,
        }
    }
}

impl Config {
    /// Load config from the TOML file, falling back to defaults.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(cfg) => {
                    info!("Config loaded from {}", path.display());
                    cfg
                }
                Err(e) => {
                    warn!(
                        "Config parse error in {}: {} — using defaults",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => {
                info!("No config file at {} — using defaults", path.display());
                Self::default()
            }
        }
    }

    /// Write the default config to disk (creates directory if needed).
    /// Used by `install` to bootstrap the config file.
    pub fn write_default() -> Result<(), Box<dyn std::error::Error>> {
        let dir = config_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }

        let path = config_path();
        let cfg = Self::default();
        let toml_str = Self::to_toml_commented(&cfg);
        std::fs::write(&path, toml_str)?;
        info!("Default config written to {}", path.display());
        Ok(())
    }

    /// Write a specific config to disk.
    pub fn write_config(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
        let dir = config_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }

        let path = config_path();
        let toml_str = Self::to_toml_commented(cfg);
        std::fs::write(&path, toml_str)?;
        info!("Config written to {}", path.display());
        Ok(())
    }

    /// Serialize config to a TOML string with helpful comments.
    fn to_toml_commented(cfg: &Config) -> String {
        format!(
            r##"# LG UltraGear Color Profile Tool — Configuration
# Location: %ProgramData%\LG-UltraGear-Monitor\config.toml
# Changes take effect on next service restart (or next event trigger).

# ─── Monitor Detection ───────────────────────────────────────────────
# Match against monitor friendly names.
# - monitor_match_regex = false: case-insensitive substring
# - monitor_match_regex = true: case-insensitive regex
monitor_match = "{monitor_match}"
monitor_match_regex = {monitor_match_regex}

# ICC profile filename (must be in %WINDIR%\System32\spool\drivers\color\).
profile_name = "{profile_name}"

# Dynamic ICC gamma tuning (recommended range: 1.2–3.0).
# Lower gamma can offset aggressive dimming by lifting shadows.
icc_gamma = {icc_gamma}

# Active ICC preset:
# - "gamma22": generated Gamma 2.2 profile
# - "gamma24": generated Gamma 2.4 profile
# - "reader": generated reader profile (unyellowing tuned curve)
# - "custom": uses profile_name + icc_gamma
icc_active_preset = "{icc_active_preset}"

# Keep both specialized Gamma 2.2 and 2.4 profiles generated in the color store.
icc_generate_specialized_profiles = {icc_generate_specialized_profiles}

# Target white luminance used for curve shaping and encoded in ICC tags (cd/m^2).
icc_luminance_cd_m2 = {icc_luminance_cd_m2}

# Anti-dimming tuning preset:
# - "manual": use raw icc_* tuning fields below exactly
# - "anti_dim_soft": light shadow lift
# - "anti_dim_balanced": balanced anti-dim uplift (recommended)
# - "anti_dim_aggressive": strongest anti-dim lift
# - "anti_dim_night": aggressive lift tuned for dark-room use
# - "reader_balanced": unyellow + brighter preset for reading/text-heavy use
icc_tuning_preset = "{icc_tuning_preset}"

# If true, explicitly changed manual icc_* tuning fields override the preset.
icc_tuning_overlay_manual = {icc_tuning_overlay_manual}

# Near-black lift amount (0.0–0.25 recommended).
icc_black_lift = {icc_black_lift}

# Midtone boost amount (-0.5..0.5, positive boosts).
icc_midtone_boost = {icc_midtone_boost}

# Highlight compression amount (0.0–1.0).
icc_white_compression = {icc_white_compression}

# Per-channel gamma multipliers (0.5–1.5).
icc_gamma_r = {icc_gamma_r}
icc_gamma_g = {icc_gamma_g}
icc_gamma_b = {icc_gamma_b}

# Optional VCGT LUT generation.
icc_vcgt_enabled = {icc_vcgt_enabled}
icc_vcgt_strength = {icc_vcgt_strength}

# Target black floor in cd/m^2.
icc_target_black_cd_m2 = {icc_target_black_cd_m2}

# Extended tag options.
icc_include_media_black_point = {icc_include_media_black_point}
icc_include_device_descriptions = {icc_include_device_descriptions}
icc_include_characterization_target = {icc_include_characterization_target}
icc_include_viewing_cond_desc = {icc_include_viewing_cond_desc}
icc_technology_signature = "{icc_technology_signature}"
icc_ciis_signature = "{icc_ciis_signature}"
icc_cicp_enabled = {icc_cicp_enabled}
icc_cicp_primaries = {icc_cicp_primaries}
icc_cicp_transfer = {icc_cicp_transfer}
icc_cicp_matrix = {icc_cicp_matrix}
icc_cicp_full_range = {icc_cicp_full_range}
icc_metadata_enabled = {icc_metadata_enabled}
icc_include_calibration_datetime = {icc_include_calibration_datetime}
icc_include_chromatic_adaptation = {icc_include_chromatic_adaptation}
icc_include_chromaticity = {icc_include_chromaticity}
icc_include_measurement = {icc_include_measurement}
icc_include_viewing_conditions = {icc_include_viewing_conditions}
icc_include_spectral_scaffold = {icc_include_spectral_scaffold}

# Generate and use monitor-scoped ICC files with serial/device identity in tags.
icc_per_monitor_profiles = {icc_per_monitor_profiles}

# HDR/SDR preferred presets.
icc_hdr_preset = "{icc_hdr_preset}"
icc_sdr_preset = "{icc_sdr_preset}"

# Optional schedule presets (used when both are non-empty).
icc_schedule_day_preset = "{icc_schedule_day_preset}"
icc_schedule_night_preset = "{icc_schedule_night_preset}"

# ICC Studio behavior.
# When enabled, every parameter edit auto-runs optimized ICC generate+apply.
icc_auto_apply_on_change = {icc_auto_apply_on_change}

# ─── Toast Notifications ─────────────────────────────────────────────
# Show a Windows notification after each successful profile reapply.
toast_enabled = {toast_enabled}
toast_title = "{toast_title}"
toast_body = "{toast_body}"

# ─── Timing ──────────────────────────────────────────────────────────
# Delay after display/session event before reapplying (ms).
# Increase if the profile isn't sticking on slow displays.
stabilize_delay_ms = {stabilize_delay_ms}

# Pause between disassociate and reassociate (ms).
# The "toggle" forces Windows to actually reload the ICC data.
toggle_delay_ms = {toggle_delay_ms}

# Delay after events settle before reapplying the profile (ms).
# Lets the display fully power on (backlight, scaler, color pipeline).
# 12000 = 12 seconds. Increase to 15000 for slow-wake monitors.
reapply_delay_ms = {reapply_delay_ms}

# ─── Refresh Methods ─────────────────────────────────────────────────
# Which display refresh methods to use after toggling the profile.
# Defaults favor no-flicker apply (soft refresh).
# Enable refresh_display_settings only if your driver ignores soft refresh.
refresh_display_settings = {refresh_display_settings}    # ChangeDisplaySettingsExW + CCD reapply (can flicker)
refresh_broadcast_color = {refresh_broadcast_color}     # WM_SETTINGCHANGE "Color" broadcast (soft)
refresh_invalidate = {refresh_invalidate}          # InvalidateRect repaint nudge (soft)
refresh_calibration_loader = {refresh_calibration_loader} # Trigger Calibration Loader task (ICC reload)

# ─── DDC/CI Brightness ───────────────────────────────────────────────
# Automatically set monitor brightness via DDC/CI after each profile reapply.
# Requires DDC/CI support on your monitor (most LG UltraGears support it).
ddc_brightness_on_reapply = {ddc_brightness_on_reapply}

# Brightness level (0–100) to set via DDC/CI.
# Only used when ddc_brightness_on_reapply is enabled.
            ddc_brightness_value = {ddc_brightness_value}

# ─── Debug ───────────────────────────────────────────────────────────
# Log every event and action (useful for troubleshooting).
verbose = {verbose}
"##,
            monitor_match = escape_toml_string(&cfg.monitor_match),
            monitor_match_regex = cfg.monitor_match_regex,
            profile_name = escape_toml_string(&cfg.profile_name),
            icc_gamma = cfg.icc_gamma,
            icc_active_preset = escape_toml_string(&cfg.icc_active_preset),
            icc_generate_specialized_profiles = cfg.icc_generate_specialized_profiles,
            icc_luminance_cd_m2 = cfg.icc_luminance_cd_m2,
            icc_tuning_preset = escape_toml_string(&cfg.icc_tuning_preset),
            icc_tuning_overlay_manual = cfg.icc_tuning_overlay_manual,
            icc_black_lift = cfg.icc_black_lift,
            icc_midtone_boost = cfg.icc_midtone_boost,
            icc_white_compression = cfg.icc_white_compression,
            icc_gamma_r = cfg.icc_gamma_r,
            icc_gamma_g = cfg.icc_gamma_g,
            icc_gamma_b = cfg.icc_gamma_b,
            icc_vcgt_enabled = cfg.icc_vcgt_enabled,
            icc_vcgt_strength = cfg.icc_vcgt_strength,
            icc_target_black_cd_m2 = cfg.icc_target_black_cd_m2,
            icc_include_media_black_point = cfg.icc_include_media_black_point,
            icc_include_device_descriptions = cfg.icc_include_device_descriptions,
            icc_include_characterization_target = cfg.icc_include_characterization_target,
            icc_include_viewing_cond_desc = cfg.icc_include_viewing_cond_desc,
            icc_technology_signature = escape_toml_string(&cfg.icc_technology_signature),
            icc_ciis_signature = escape_toml_string(&cfg.icc_ciis_signature),
            icc_cicp_enabled = cfg.icc_cicp_enabled,
            icc_cicp_primaries = cfg.icc_cicp_primaries,
            icc_cicp_transfer = cfg.icc_cicp_transfer,
            icc_cicp_matrix = cfg.icc_cicp_matrix,
            icc_cicp_full_range = cfg.icc_cicp_full_range,
            icc_metadata_enabled = cfg.icc_metadata_enabled,
            icc_include_calibration_datetime = cfg.icc_include_calibration_datetime,
            icc_include_chromatic_adaptation = cfg.icc_include_chromatic_adaptation,
            icc_include_chromaticity = cfg.icc_include_chromaticity,
            icc_include_measurement = cfg.icc_include_measurement,
            icc_include_viewing_conditions = cfg.icc_include_viewing_conditions,
            icc_include_spectral_scaffold = cfg.icc_include_spectral_scaffold,
            icc_per_monitor_profiles = cfg.icc_per_monitor_profiles,
            icc_hdr_preset = escape_toml_string(&cfg.icc_hdr_preset),
            icc_sdr_preset = escape_toml_string(&cfg.icc_sdr_preset),
            icc_schedule_day_preset = escape_toml_string(&cfg.icc_schedule_day_preset),
            icc_schedule_night_preset = escape_toml_string(&cfg.icc_schedule_night_preset),
            icc_auto_apply_on_change = cfg.icc_auto_apply_on_change,
            toast_enabled = cfg.toast_enabled,
            toast_title = escape_toml_string(&cfg.toast_title),
            toast_body = escape_toml_string(&cfg.toast_body),
            stabilize_delay_ms = cfg.stabilize_delay_ms,
            toggle_delay_ms = cfg.toggle_delay_ms,
            reapply_delay_ms = cfg.reapply_delay_ms,
            refresh_display_settings = cfg.refresh_display_settings,
            refresh_broadcast_color = cfg.refresh_broadcast_color,
            refresh_invalidate = cfg.refresh_invalidate,
            refresh_calibration_loader = cfg.refresh_calibration_loader,
            ddc_brightness_on_reapply = cfg.ddc_brightness_on_reapply,
            ddc_brightness_value = cfg.ddc_brightness_value,
            verbose = cfg.verbose,
        )
    }

    /// Get the full path to the ICC profile in the Windows color store.
    pub fn profile_path(&self) -> PathBuf {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
        PathBuf::from(windir)
            .join("System32")
            .join("spool")
            .join("drivers")
            .join("color")
            .join(&self.profile_name)
    }
}

/// Escape a string for safe inclusion inside a TOML basic string (`"..."`).
///
/// Handles backslashes, double-quotes, and common control characters that
/// would otherwise break the TOML output from `to_toml_commented()`.
fn escape_toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "tests/config_tests.rs"]
mod tests;
