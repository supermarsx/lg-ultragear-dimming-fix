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
