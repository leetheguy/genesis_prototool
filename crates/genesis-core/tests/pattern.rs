//! Matching event names against handler patterns (spec 100, "Handlers").

use genesis_core::pattern::matches;

#[test]
fn a_plain_name_matches_only_itself() {
    assert!(matches("hello_btn.click", "hello_btn.click"));
    assert!(!matches("hello_btn.click", "hello_btn.clicks"));
    assert!(!matches("hello_btn.click", "hello_btn"));
}

#[test]
fn star_matches_any_run_of_characters_including_dots() {
    assert!(matches("*.failed", "auth.logout.failed"));
    assert!(matches("*.failed", "x.failed"));
    assert!(!matches("*.failed", "auth.failed.later"));
}

#[test]
fn star_matches_nothing_too() {
    assert!(matches("*", ""));
    assert!(matches("*", "anything.at.all"));
    assert!(matches("app.*changed", "app.changed"));
}

#[test]
fn question_mark_matches_exactly_one_character() {
    assert!(matches("contacts.4??", "contacts.404"));
    assert!(!matches("contacts.4??", "contacts.40"));
    assert!(!matches("contacts.4??", "contacts.4045"));
    assert!(matches("a?b", "a.b"));
}

#[test]
fn several_stars_backtrack_to_find_a_match() {
    assert!(matches("a*b*c", "a_b_b_c"));
    assert!(matches("*.ok*", "crm.contacts.ok.later"));
    assert!(!matches("a*b*c", "a_c_b"));
}

#[test]
fn matching_is_case_sensitive() {
    assert!(!matches("Hello.click", "hello.click"));
}

#[test]
fn characters_beyond_ascii_match_one_at_a_time() {
    assert!(matches("caf?.open", "café.open"));
}
