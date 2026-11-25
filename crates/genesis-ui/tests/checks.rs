//! Checks on controls (spec 080, "Checks on input"): a control says what's wrong with its value
//! under itself, once the person has changed it or its form's submit failed a check, and stops
//! once the value is fixed. Typing is checked in a real browser (`tests/deployed/checks.mjs`).

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

/// The HTML drawn for `toml` (after `[app]`), after `then` has done whatever it does.
fn html(toml: &str, then: impl FnOnce(&Host)) -> String {
    let toml = format!("[app]\nname = \"x\"\n{toml}\n");
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(&toml).unwrap());
        then(&host);
        app_view(host).to_html()
    })
}

const SIGNUP: &str = r#"
[[body]]
type = "form"
id = "signup"

[[body.children]]
type = "input"
id = "email_box"
name = "email"
label = "Email"
required = true
checks = [{ matches = "^[^@ ]+@[^@ ]+$", message = "That doesn't look like an email." }]

[[body.children]]
type = "input"
id = "name_box"
name = "name"
required = "Who are you?"

[[body.children]]
type = "checkbox"
id = "terms_box"
name = "terms"
label = "I agree"
required = "Please agree to the terms."

[[body.children]]
type = "button"
label = "Sign up"
submit = true
"#;

#[test]
fn nothing_is_said_before_the_person_does_anything() {
    let out = html(SIGNUP, |_| {});
    assert!(
        !out.contains("Required.") && !out.contains("role=\"alert\""),
        "{out}"
    );
}

#[test]
fn a_control_says_whats_wrong_once_something_is_typed() {
    let out = html(SIGNUP, |host| {
        host.fire("email_box.change", json!({ "value": "ada" }));
    });
    assert!(out.contains("That doesn't look like an email."), "{out}");
    // Inside the label, after the box: the message takes no place of its own.
    let label = out.find("Email").unwrap();
    let message = out.find("That doesn't").unwrap();
    assert!(
        label < message && out[label..message].contains("<input"),
        "{out}"
    );
    assert!(
        !out.contains("Who are you?"),
        "a box nobody's touched stays quiet: {out}"
    );
}

#[test]
fn after_a_failed_submit_every_failing_control_says_so() {
    let out = html(SIGNUP, |host| host.fire("signup.submit", json!({})));
    for message in ["Required.", "Who are you?", "Please agree to the terms."] {
        assert!(out.contains(message), "{message}: {out}");
    }
    // A control with no label holds its message in a div with its placement.
    assert!(out.contains("<div class=\"\"><input"), "{out}");
}

#[test]
fn fixing_the_value_clears_its_message() {
    let out = html(SIGNUP, |host| {
        host.fire("signup.submit", json!({}));
        host.fire("email_box.change", json!({ "value": "ada@example.com" }));
        host.fire("terms_box.change", json!({ "value": true }));
    });
    assert!(
        !out.contains("Required.") && !out.contains("Please agree"),
        "{out}"
    );
    assert!(out.contains("Who are you?"), "{out}");
}

#[test]
fn a_good_submit_starts_the_form_over_quietly() {
    let out = html(SIGNUP, |host| {
        host.fire("signup.submit", json!({}));
        host.fire("email_box.change", json!({ "value": "ada@example.com" }));
        host.fire("name_box.change", json!({ "value": "Ada" }));
        host.fire("terms_box.change", json!({ "value": true }));
        host.fire("signup.submit", json!({}));
    });
    assert!(!out.contains("role=\"alert\""), "{out}");
}

#[test]
fn a_control_with_var_speaks_after_its_own_change() {
    let out = html(
        r#"
[state]
handle = ""

[[body]]
type = "input"
var = "app.handle"
label = "Handle"
checks = [{ max_len = 3, message = "Three letters at most." }]
"#,
        |host| host.fire("p0.change", json!({ "value": "abcdef" })),
    );
    // The state changed, but not through this box in the browser, so it hasn't spoken yet.
    assert!(!out.contains("Three letters"), "{out}");
}
