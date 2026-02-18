use super::*;

// ── show_reapply_toast disabled ──────────────────────────────────

#[test]
fn show_reapply_toast_disabled_is_noop() {
    // enabled=false returns immediately without calling any WinRT API
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

// ── escape_xml ───────────────────────────────────────────────────

#[test]
fn escape_xml_plain_text_unchanged() {
    assert_eq!(escape_xml("hello world"), "hello world");
}

#[test]
fn escape_xml_escapes_ampersand() {
    assert_eq!(escape_xml("a & b"), "a &amp; b");
}

#[test]
fn escape_xml_escapes_angle_brackets() {
    assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
}

#[test]
fn escape_xml_escapes_quotes() {
    assert_eq!(escape_xml(r#"say "hi""#), "say &quot;hi&quot;");
}

#[test]
fn escape_xml_escapes_apostrophe() {
    assert_eq!(escape_xml("it's"), "it&apos;s");
}

#[test]
fn escape_xml_empty_string() {
    assert_eq!(escape_xml(""), "");
}

#[test]
fn escape_xml_combined() {
    assert_eq!(
        escape_xml(r#"<a & "b" > 'c'"#),
        "&lt;a &amp; &quot;b&quot; &gt; &apos;c&apos;"
    );
}

#[test]
fn escape_xml_preserves_unicode() {
    assert_eq!(escape_xml("Color profile ✓"), "Color profile ✓");
}
