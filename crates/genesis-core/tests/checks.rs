//! Checks on controls (spec 080, "Checks on input"): what loads, what's refused, and how a form
//! with a failing check fires `.invalid`, keeps its draft, and submits once every check passes.

use genesis_core::bus::Machine;
use genesis_core::condition::{Check, Condition};
use genesis_core::config::{App, Node};
use serde_json::{Value, json};

fn app(toml: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"Checks\"\n{toml}"))
}

fn problem(toml: &str) -> String {
    app(toml)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn machine(toml: &str) -> Machine {
    Machine::new(app(toml).expect("app loads"))
}

/// A sign-up form: a required email that has to look like one, a seat count from 1 to 500, an
/// optional website, and terms to agree to. `.submit` copies the form into `app.saved`, and
/// `.invalid` copies what's wrong into `app.wrong`.
const SIGNUP: &str = r#"
[state]
saved = {}
wrong = {}

[[body]]
type = "form"
id = "signup"
on_submit = [{ set = "app.saved", to = "{{ event.form }}" }]

[[body.children]]
type = "input"
id = "email_box"
name = "email"
kind = "email"
required = true
checks = [{ matches = "^[^@ ]+@[^@ ]+$", message = "That doesn't look like an email." }]

[[body.children]]
type = "input"
id = "seats_box"
name = "seats"
kind = "number"
checks = [{ gte = 1, lte = 500, message = "1 to 500 seats." }]

[[body.children]]
type = "input"
id = "site_box"
name = "site"
checks = [{ matches = "^https://", message = "Starts with https://." }]

[[body.children]]
type = "checkbox"
id = "terms_box"
name = "terms"
required = "Please agree to the terms."

[[body.children]]
type = "button"
label = "Sign up"
submit = true

[on]
"signup.invalid" = [{ set = "app.wrong", to = "{{ event.invalid }}" }]
"#;

fn type_in(m: &mut Machine, values: &[(&str, Value)]) {
    for (control, value) in values {
        m.fire(&format!("{control}.change"), json!({ "value": value }));
    }
}

#[test]
fn checks_load_on_every_control_and_survive_the_trip_to_the_browser() {
    let loaded = app(SIGNUP).expect("loads");
    let back: App = serde_json::from_value(serde_json::to_value(&loaded).unwrap()).unwrap();
    assert_eq!(back, loaded);
    let Node::Form(form) = &loaded.body[0] else {
        panic!("not a form")
    };
    let messages =
        |node: &Node| -> Vec<String> { node.checks().into_iter().map(|c| c.message).collect() };
    assert_eq!(
        messages(&form.children[0]),
        ["Required.", "That doesn't look like an email."]
    );
    assert_eq!(messages(&form.children[3]), ["Please agree to the terms."]);
    for control in [
        "type = \"textarea\"\nvar = \"app.x\"",
        "type = \"select\"\nvar = \"app.x\"\noptions = []",
        "type = \"toggle\"\nvar = \"app.x\"",
        "type = \"radio\"\nvar = \"app.x\"\nvalue = 1",
    ] {
        let toml = format!(
            "[state]\nx = \"\"\n[[body]]\n{control}\nrequired = true\nchecks = [{{ max_len = 3, message = \"Short.\" }}]\n"
        );
        assert!(app(&toml).is_ok(), "{control}: {:?}", app(&toml));
    }
}

#[test]
fn a_form_with_a_failing_check_fires_invalid_not_submit_and_keeps_its_draft() {
    let mut m = machine(SIGNUP);
    type_in(
        &mut m,
        &[("email_box", json!("ada")), ("seats_box", json!(900))],
    );
    m.fire("signup.submit", json!({}));
    assert_eq!(m.get("app.saved"), Some(json!({})), "nothing was submitted");
    assert_eq!(
        m.get("app.wrong"),
        Some(json!({
            "email": "That doesn't look like an email.",
            "seats": "1 to 500 seats.",
            "terms": "Please agree to the terms.",
        }))
    );
    assert_eq!(m.draft("signup", "email"), Some(json!("ada")));
    assert!(m.tried("signup", None));
    // The trail shows `.invalid`, and what fired it.
    let events = m.get("sys.events").unwrap();
    assert!(
        events
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "signup.invalid"),
        "{events}"
    );
}

#[test]
fn once_every_check_passes_it_submits_and_starts_over() {
    let mut m = machine(SIGNUP);
    m.fire("signup.submit", json!({}));
    assert_eq!(m.get("app.wrong").unwrap()["email"], json!("Required."));
    type_in(
        &mut m,
        &[
            ("email_box", json!("ada@example.com")),
            ("seats_box", json!(12)),
            ("terms_box", json!(true)),
        ],
    );
    m.fire("signup.submit", json!({}));
    assert_eq!(
        m.get("app.saved"),
        Some(json!({ "email": "ada@example.com", "seats": 12, "site": null, "terms": true }))
    );
    assert!(!m.tried("signup", None));
    assert_eq!(m.draft("signup", "email"), None);
}

#[test]
fn an_empty_optional_field_passes_its_checks() {
    let mut m = machine(SIGNUP);
    type_in(
        &mut m,
        &[
            ("email_box", json!("ada@example.com")),
            ("terms_box", json!(true)),
            ("site_box", json!("")),
        ],
    );
    m.fire("signup.submit", json!({}));
    assert_eq!(m.get("app.saved").unwrap()["site"], json!(""));
    type_in(
        &mut m,
        &[
            ("email_box", json!("ada@example.com")),
            ("terms_box", json!(true)),
            ("site_box", json!("example.com")),
        ],
    );
    m.fire("signup.submit", json!({}));
    assert_eq!(
        m.get("app.wrong").unwrap(),
        json!({ "site": "Starts with https://." })
    );
}

#[test]
fn checks_read_the_forms_data_where_nothing_is_typed() {
    let mut m = machine(
        &SIGNUP
            .replace("id = \"signup\"", "id = \"signup\"\ndata = \"app.person\"")
            .replace(
                "saved = {}",
                "saved = {}\nperson = { email = \"ada@example.com\", terms = true }",
            ),
    );
    m.fire("signup.submit", json!({}));
    assert_eq!(
        m.get("app.saved").unwrap()["email"],
        json!("ada@example.com")
    );
}

#[test]
fn each_rows_form_is_checked_against_its_own_item() {
    let mut m = machine(
        r#"
[state]
saved = {}
people = [{ name = "" }, { name = "Grace" }]

[[body]]
type = "cell"
each = "app.people"

[[body.children]]
type = "form"
id = "edit"
data = "item"
on_submit = [{ set = "app.saved", to = "{{ event.form }}" }]

[[body.children.children]]
type = "input"
name = "name"
required = true
"#,
    );
    m.fire_at("edit.submit", json!({ "item": { "name": "" } }), Some("0"));
    assert_eq!(m.get("app.saved"), Some(json!({})));
    assert!(m.tried("edit", Some("0")) && !m.tried("edit", Some("1")));
    m.fire_at(
        "edit.submit",
        json!({ "item": { "name": "Grace" } }),
        Some("1"),
    );
    assert_eq!(m.get("app.saved"), Some(json!({ "name": "Grace" })));
}

#[test]
fn a_password_stays_hidden_on_the_trail_of_an_invalid_submit() {
    let mut m = machine(
        r#"
[[body]]
type = "form"
id = "login"

[[body.children]]
type = "input"
id = "pass_box"
name = "password"
kind = "password"
checks = [{ min_len = 40, message = "Longer." }]
"#,
    );
    m.fire(
        "pass_box.change",
        json!({ "value": "made-up-for-the-test" }),
    );
    m.fire("login.submit", json!({}));
    let events = m.get("sys.events").unwrap().to_string();
    assert!(
        events.contains("login.invalid") && !events.contains("made-up-for-the-test"),
        "{events}"
    );
}

#[test]
fn a_badly_written_check_says_why() {
    let cases = [
        (
            "checks = [{ matches = \"@\" }]",
            "a check says what's wrong with `message`",
        ),
        ("checks = [{ message = \"No.\" }]", "at least one test"),
        (
            "checks = [{ var = \"app.x\", is = 1, message = \"No.\" }]",
            "takes no `var`",
        ),
        (
            "checks = [{ status = \"ok\", message = \"No.\" }]",
            "can't be a check",
        ),
        (
            "checks = [{ min_len = -1, message = \"No.\" }]",
            "takes a whole number",
        ),
        (
            "checks = [{ near = 1, message = \"No.\" }]",
            "there's no test `near`",
        ),
        ("required = 3", "required"),
    ];
    for (line, why) in cases {
        let said = problem(&format!(
            "[state]\nx = \"\"\n[[body]]\ntype = \"input\"\nvar = \"app.x\"\n{line}\n"
        ));
        assert!(said.contains(why), "{line}: {said}");
    }
}

#[test]
fn a_check_with_several_tests_needs_them_all() {
    let check: Check =
        serde_json::from_value(json!({ "gte": 1, "lte": 500, "message": "1 to 500." })).unwrap();
    assert!(check.passes(Some(&json!(1))) && check.passes(Some(&json!(500))));
    assert!(!check.passes(Some(&json!(0))) && !check.passes(Some(&json!(501))));
    assert!(!check.passes(Some(&json!("12"))), "text isn't a number");
    assert!(check.passes(None), "empty is for required to catch");
}

#[test]
fn min_len_and_max_len_count_characters_and_items_and_work_in_conditions_too() {
    let short: Condition = serde_json::from_value(json!({ "var": "app.x", "max_len": 3 })).unwrap();
    let long: Condition = serde_json::from_value(json!({ "var": "app.x", "min_len": 2 })).unwrap();
    let at = |c: &Condition, v: Value| c.holds(&move |_: &str| Some(v.clone()));
    assert!(
        at(&short, json!("añb")),
        "three characters, though four bytes"
    );
    assert!(!at(&short, json!("abcd")));
    assert!(at(&long, json!([1, 2])) && !at(&long, json!([1])));
    assert!(!at(&long, json!(12)), "a number has no length");
}
