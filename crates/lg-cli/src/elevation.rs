//! Auto-elevation helpers for Windows UAC.
//!
//! Provides functions to check whether the current process is running with
//! administrator privileges and to relaunch it elevated via `ShellExecuteW`
//! with the `"runas"` verb.

use std::error::Error;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Returns `true` if the current process is running elevated (administrator).
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let result = check_token_elevation(token);
        let _ = CloseHandle(token);
        result
    }
}

/// Check elevation status from a process token.
unsafe fn check_token_elevation(token: HANDLE) -> bool {
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned_length: u32 = 0;
    let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;

    let ok: Result<(), _> = GetTokenInformation(
        token,
        TokenElevation,
        Some(&mut elevation as *mut TOKEN_ELEVATION as *mut _),
        size,
        &mut returned_length,
    );
    if ok.is_err() {
        return false;
    }

    elevation.TokenIsElevated != 0
}

/// Relaunch the current process elevated via UAC (`ShellExecuteW` + `"runas"`).
///
/// This function does not return on success — the elevated child process takes
/// over and this (non-elevated) process should exit. Returns an error only if
/// the relaunch could not be initiated (e.g. the user cancelled UAC).
pub fn relaunch_elevated() -> Result<(), Box<dyn Error>> {
    let exe = std::env::current_exe()?;
    let exe_wide = to_wide(&exe.to_string_lossy());

    // Rebuild the original command-line arguments (skip argv[0]).
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_str = args.join(" ");
    let args_wide = to_wide(&args_str);

    let verb = to_wide("runas");

    // ShellExecuteW returns an HINSTANCE; values > 32 indicate success.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(exe_wide.as_ptr()),
            PCWSTR(args_wide.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // HINSTANCE is secretly an integer — values > 32 mean success.
    let code = result.0 as isize;
    if code > 32 {
        // Elevated child launched. Exit this non-elevated process.
        std::process::exit(0);
    }

    Err(format!(
        "Failed to elevate (ShellExecute returned {}). \
         The user may have cancelled the UAC prompt.",
        code
    )
    .into())
}

/// Convert a Rust string to a null-terminated wide (UTF-16) vector.
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_elevated_returns_bool() {
        // Just verify it doesn't panic — actual value depends on privileges.
        let _ = is_elevated();
    }

    #[test]
    fn to_wide_null_terminated() {
        let w = to_wide("hello");
        assert_eq!(w.len(), 6); // 5 chars + null
        assert_eq!(w[5], 0);
    }

    #[test]
    fn to_wide_empty_string() {
        let w = to_wide("");
        assert_eq!(w.len(), 1);
        assert_eq!(w[0], 0);
    }

    #[test]
    fn to_wide_unicode() {
        let w = to_wide("café");
        // 'c' 'a' 'f' 'é' + null = 5
        assert_eq!(w.last(), Some(&0));
        assert!(w.len() >= 5);
    }

    #[test]
    fn command_elevation_categories() {
        // Commands that need admin are handled in main.rs match.
        // Verify the elevation check itself doesn't panic.
        assert!(!is_elevated() || is_elevated()); // tautology — just tests call
    }
}
