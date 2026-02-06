//! Windows toast notifications via PowerShell.
//!
//! Since the service runs as SYSTEM/LocalSystem, we shell out to PowerShell
//! running as the interactive user to show the toast. This avoids the Session 0
//! isolation problem where SYSTEM can't display UI in the user's session.
//!
//! Falls back silently if notifications can't be shown.

use crate::config::Config;
use log::{info, warn};
use std::os::windows::process::CommandExt;

/// Show a Windows toast notification for the current config.
/// Runs PowerShell in a hidden window to create the toast.
pub fn show_reapply_toast(config: &Config) {
    if !config.toast_enabled {
        return;
    }

    let title = &config.toast_title;
    let body = &config.toast_body;

    // PowerShell script to show a toast notification.
    // Uses the Windows Runtime ToastNotification API via PowerShell interop.
    let ps_script = format!(
        r#"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$xml = [Windows.Data.Xml.Dom.XmlDocument]::new()
$xml.LoadXml('<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text></binding></visual></toast>')
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
$appId = '{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\WindowsPowerShell\v1.0\powershell.exe'
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
"#,
        title = title.replace('\'', "''").replace('"', "&quot;"),
        body = body.replace('\'', "''").replace('"', "&quot;"),
    );

    // Try to find an interactive session to run the toast in.
    // We use schtasks to run a quick one-off PowerShell as the logged-in user.
    let result = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NoLogo",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &ps_script,
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    match result {
        Ok(output) if output.status.success() => {
            info!("Toast notification shown");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // This is expected when running as SYSTEM in Session 0
            if config.verbose {
                warn!(
                    "Toast notification failed (expected in Session 0): {}",
                    stderr.trim()
                );
            }
            // Fallback: try via schtasks to run in user's session
            show_toast_via_schtasks(title, body, config.verbose);
        }
        Err(e) => {
            if config.verbose {
                warn!("Failed to launch PowerShell for toast: {}", e);
            }
        }
    }
}

/// Fallback: create a temporary scheduled task that runs as the interactive user
/// to show the toast notification, then clean it up.
fn show_toast_via_schtasks(title: &str, body: &str, verbose: bool) {
    let ps_command = format!(
        r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null; $x = [Windows.Data.Xml.Dom.XmlDocument]::new(); $x.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>{title}</text><text>{body}</text></binding></visual></toast>'); [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\WindowsPowerShell\v1.0\powershell.exe').Show([Windows.UI.Notifications.ToastNotification]::new($x))"#,
        title = title.replace('"', "&quot;"),
        body = body.replace('"', "&quot;"),
    );

    let task_name = "LG-UltraGear-Toast-Temp";

    // Create a one-off task that runs immediately as the BUILTIN\Users group
    let create_result = std::process::Command::new("schtasks.exe")
        .args([
            "/Create",
            "/TN", task_name,
            "/TR", &format!("powershell.exe -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -Command \"{}\"", ps_command),
            "/SC", "ONCE",
            "/ST", "00:00",
            "/F",
            "/RL", "LIMITED",
            "/IT", // Interactive only
        ])
        .creation_flags(0x08000000)
        .output();

    if let Ok(output) = create_result {
        if output.status.success() {
            // Run the task
            let _ = std::process::Command::new("schtasks.exe")
                .args(["/Run", "/TN", task_name])
                .creation_flags(0x08000000)
                .output();

            // Small delay then clean up
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = std::process::Command::new("schtasks.exe")
                .args(["/Delete", "/TN", task_name, "/F"])
                .creation_flags(0x08000000)
                .output();

            if verbose {
                info!("Toast shown via temporary scheduled task");
            }
        } else if verbose {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to create toast task: {}", stderr.trim());
        }
    }
}

#[cfg(test)]
#[path = "tests/toast_tests.rs"]
mod tests;
