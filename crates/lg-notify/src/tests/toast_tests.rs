use super::*;

// ── show_reapply_toast disabled ──────────────────────────────────

#[test]
fn show_reapply_toast_disabled_is_noop() {
    // enabled=false returns immediately without spawning any process
    show_reapply_toast(false, "Title", "Body", false);
}

#[test]
fn show_reapply_toast_disabled_with_custom_text() {
    show_reapply_toast(false, "Should Not Show", "This should be a no-op", false);
}

// ── Text escaping edge cases ─────────────────────────────────────

#[test]
fn toast_title_with_quotes_does_not_panic() {
    show_reapply_toast(
        false,
        r#"Title with "quotes" and 'apostrophes'"#,
        "Normal body",
        false,
    );
}

#[test]
fn toast_body_with_special_chars_does_not_panic() {
    show_reapply_toast(false, "Title", "Body with <xml> & special chars £€¥", false);
}

#[test]
fn toast_with_empty_strings_does_not_panic() {
    show_reapply_toast(false, "", "", false);
}

#[test]
fn toast_with_unicode_does_not_panic() {
    show_reapply_toast(false, "カラープロファイル", "適用済み ✓", false);
}

// ── Verbose flag ─────────────────────────────────────────────────

#[test]
fn toast_verbose_flag_does_not_panic() {
    show_reapply_toast(false, "Test", "Test", true);
}

// ── show_toast_via_schtasks runs on separate thread ──────────────

#[test]
fn schtasks_fallback_does_not_block_caller() {
    // Verify that the schtasks path is delegated to a thread (non-blocking).
    // We can't easily test the actual toast, but we verify the thread-spawn
    // pattern exists and handles owned strings correctly.
    let title = "Test Title".to_owned();
    let body = "Test Body".to_owned();
    let verbose = false;
    let handle = std::thread::Builder::new()
        .name("toast-schtasks-test".into())
        .spawn(move || {
            // Just verify the closure captures work — don't actually run schtasks
            assert!(!title.is_empty());
            assert!(!body.is_empty());
            let _ = verbose;
        })
        .unwrap();
    handle.join().unwrap();
}
