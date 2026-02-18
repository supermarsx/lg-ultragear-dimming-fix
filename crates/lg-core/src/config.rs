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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Display name pattern to match (case-insensitive contains).
    pub monitor_match: String,

    /// ICC profile filename (looked up in Windows color store).
    pub profile_name: String,

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

    /// Enable logging of every event (useful for debugging).
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor_match: "LG ULTRAGEAR".to_string(),
            profile_name: "lg-ultragear-full-cal.icm".to_string(),
            toast_enabled: true,
            toast_title: "LG UltraGear".to_string(),
            toast_body: "Color profile reapplied ✓".to_string(),
            stabilize_delay_ms: 1500,
            toggle_delay_ms: 100,
            reapply_delay_ms: 12000,
            refresh_display_settings: true,
            refresh_broadcast_color: true,
            refresh_invalidate: true,
            refresh_calibration_loader: true,
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
# Case-insensitive substring match against monitor friendly names.
monitor_match = "{monitor_match}"

# ICC profile filename (must be in %WINDIR%\System32\spool\drivers\color\).
profile_name = "{profile_name}"

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
# All enabled by default for maximum reliability.
refresh_display_settings = {refresh_display_settings}    # ChangeDisplaySettingsExW (full display refresh)
refresh_broadcast_color = {refresh_broadcast_color}     # WM_SETTINGCHANGE "Color" broadcast
refresh_invalidate = {refresh_invalidate}          # InvalidateRect (force repaint)
refresh_calibration_loader = {refresh_calibration_loader} # Trigger Calibration Loader task (ICC reload)

# ─── Debug ───────────────────────────────────────────────────────────
# Log every event and action (useful for troubleshooting).
verbose = {verbose}
"##,
            monitor_match = escape_toml_string(&cfg.monitor_match),
            profile_name = escape_toml_string(&cfg.profile_name),
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
