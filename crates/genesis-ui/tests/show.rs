//! Display logic (spec 080): a node with `show` is drawn only while its condition holds, and a
//! button or control with `disabled` is drawn disabled while its condition holds. That the screen
//! follows without a redraw is checked in a real browser (`tests/deployed/show.mjs`).

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

/// The HTML drawn for `toml` (after `[app]`), before and after `then` does whatever it does.
fn before_and_after(toml: &str, then: impl FnOnce(&Host)) -> (String, String) {
    let toml = format!("[app]\nname = \"x\"\n{toml}\n");
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(&toml).unwrap());
        let before = app_view(host.clone()).to_html();
        then(&host);
        (before, app_view(host).to_html())
    })
}

#[test]
fn a_node_shows_only_while_its_condition_holds() {
    let (before, after) = before_and_after(
        r#"
[state]
view = "list"

[[body]]
type = "text"
text = "The list"
show = { var = "app.view", is = "list" }

[[body]]
type = "text"
text = "One contact"
show = { var = "app.view", is = "detail" }

[[body]]
type = "text"
text = "Always"

[on]
"open" = [{ set = "app.view", to = "detail" }]
"#,
        |host| host.fire("open", json!({})),
    );
    assert!(
        before.contains("The list") && !before.contains("One contact"),
        "{before}"
    );
    assert!(
        after.contains("One contact") && !after.contains("The list"),
        "{after}"
    );
    assert!(before.contains("Always") && after.contains("Always"));
}

#[test]
fn a_hidden_node_takes_no_place() {
    let (out, _) = before_and_after(
        r#"
[state]
flags = {}

[[body]]
type = "cell"
span = 6
show = { var = "app.flags.missing", empty = false }

[[body.children]]
type = "text"
text = "Hidden"
"#,
        |_| {},
    );
    assert!(
        !out.contains("Hidden") && !out.contains("col-span-6"),
        "{out}"
    );
}

#[test]
fn each_row_tests_its_own_item() {
    let (out, _) = before_and_after(
        r#"
[state]
contacts = [{ name = "Ada", vip = true }, { name = "Lin", vip = false }]

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "text"
text = "{{ item.name }} is a VIP"
show = { var = "item.vip", is = true }
"#,
        |_| {},
    );
    assert!(
        out.contains("Ada is a VIP") && !out.contains("Lin is a VIP"),
        "{out}"
    );
}

#[test]
fn a_button_is_disabled_while_its_condition_holds() {
    let (before, after) = before_and_after(
        r#"
[state]
count = 0

[[body]]
type = "button"
label = "Undo"
disabled = { var = "app.count", lte = 0 }

[[body]]
type = "button"
label = "Add"

[on]
"add.click" = [{ step = "app.count", by = 1 }]
"#,
        |host| host.fire("add.click", json!({})),
    );
    assert!(
        before.contains("disabled") && before.contains("Undo"),
        "{before}"
    );
    assert!(!after.contains("disabled"), "{after}");
}

#[test]
fn every_control_takes_disabled() {
    let (out, _) = before_and_after(
        r#"
[state]
locked = true
text = ""
on = false
size = 1

[[body]]
type = "input"
var = "app.text"
disabled = { var = "app.locked", is = true }

[[body]]
type = "textarea"
var = "app.text"
disabled = { var = "app.locked", is = true }

[[body]]
type = "checkbox"
var = "app.on"
disabled = { var = "app.locked", is = true }

[[body]]
type = "toggle"
var = "app.on"
disabled = { var = "app.locked", is = true }

[[body]]
type = "radio"
var = "app.size"
value = 1
disabled = { var = "app.locked", is = true }

[[body]]
type = "select"
var = "app.size"
options = [{ label = "Small", value = 1 }]
disabled = { var = "app.locked", is = true }
"#,
        |_| {},
    );
    for tag in [
        "<input type=\"text\"",
        "<textarea",
        "<input type=\"checkbox\"",
        "<input type=\"radio\"",
        "<select",
    ] {
        let at = out.find(tag).unwrap_or_else(|| panic!("{tag}: {out}"));
        let end = at + out[at..].find('>').unwrap();
        assert!(
            out[at..end].contains("disabled"),
            "{tag}: {}",
            &out[at..=end]
        );
    }
    assert_eq!(out.matches("disabled").count(), 6, "{out}");
}

#[test]
fn a_forms_button_reads_what_is_typed_into_it() {
    let (before, after) = before_and_after(
        r#"
[[body]]
type = "form"
id = "signup"

[[body.children]]
type = "input"
id = "email_box"
name = "email"

[[body.children]]
type = "button"
label = "Sign up"
submit = true
disabled = { not = { var = "form.email", matches = "^[^@ ]+@[^@ ]+$" } }
"#,
        |host| host.fire("email_box.change", json!({ "value": "ada@example.com" })),
    );
    let button = |out: &str| {
        let at = out.find("<button").unwrap();
        out[at..at + out[at..].find('>').unwrap()].to_string()
    };
    assert!(button(&before).contains("disabled"), "{before}");
    assert!(!button(&after).contains("disabled"), "{after}");
}

#[test]
fn a_form_reads_its_data_until_something_is_typed() {
    let (out, _) = before_and_after(
        r#"
[state]
contact = { name = "Ada" }

[[body]]
type = "form"
id = "edit"
data = "app.contact"

[[body.children]]
type = "text"
text = "Has a name"
show = { var = "form.name", empty = false }
"#,
        |_| {},
    );
    assert!(out.contains("Has a name"), "{out}");
}

#[test]
fn status_shows_while_a_source_loads() {
    let (before, after) = before_and_after(
        r#"
[connections.crm]
base_url = "https://example.com"

[connections.crm.crud.contacts]

[sources.contacts]
call = "crm.contacts.list"

[[body]]
type = "text"
text = "Loading"
show = { var = "app.contacts", status = "loading" }
"#,
        |host| {
            host.set_caller(|_| {});
            host.open();
        },
    );
    assert!(!before.contains("Loading"), "{before}");
    assert!(after.contains("Loading"), "{after}");
}
