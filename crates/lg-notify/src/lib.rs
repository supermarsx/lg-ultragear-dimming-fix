//! Windows toast notifications via WinRT APIs.
//!
//! Shows toast notifications using the Windows Runtime
//! `ToastNotificationManager` API directly — no external process spawning
//! (no PowerShell, no schtasks).
//!
//! In Session 0 (service context running as SYSTEM), the WinRT
//! notification infrastructure is unavailable — the attempt will
//! fail gracefully and the event is logged to the Windows Event Log.
//!
//! All functions take raw parameters (no Config dependency) so this crate
//! can be used independently.

use log::{info, warn};
use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

/// Show a Windows toast notification.
///
/// If `enabled` is false, returns immediately (useful for testing and
/// callers that want a single call site regardless of config).
///
/// Uses WinRT toast APIs directly. In Session 0 (service mode), the
/// notification infrastructure is unavailable and the call fails
/// gracefully — the event is still logged to the Windows Event Log.
///
/// # Arguments
/// * `enabled` — Whether to actually show the toast (false = no-op)
/// * `title` — Toast notification title
/// * `body` — Toast notification body text
/// * `verbose` — Log warnings on failure (otherwise fails silently)
pub fn show_reapply_toast(enabled: bool, title: &str, body: &str, verbose: bool) {
    if !enabled {
        return;
    }

    match show_toast_native(title, body) {
        Ok(()) => {
            info!("Toast notification shown");
        }
        Err(e) => {
            // In Session 0 (service mode) WinRT notifications are unavailable.
            // The profile reapply event is still logged to Windows Event Log.
            if verbose {
                warn!("Toast notification unavailable: {} (expected in Session 0)", e);
            }
        }
    }
}

/// Show a toast notification using the WinRT `ToastNotificationManager` API.
fn show_toast_native(title: &str, body: &str) -> Result<(), Box<dyn std::error::Error>> {
    let title_escaped = escape_xml(title);
    let body_escaped = escape_xml(body);

    let toast_xml = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        title_escaped, body_escaped
    );

    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(toast_xml.as_str()))?;

    let toast = ToastNotification::CreateToastNotification(&xml)?;

    // Use PowerShell's registered AppUserModelID — a common approach for
    // showing toasts without registering a custom application identity.
    let app_id = HSTRING::from(
        r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe",
    );
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&app_id)?;
    notifier.Show(&toast)?;

    Ok(())
}

/// Escape XML special characters for safe inclusion in toast XML.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
#[path = "tests/toast_tests.rs"]
mod tests;
