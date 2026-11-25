//! Drawing a `form` and its named controls (spec 040, "Controls"): a real `form` element, its
//! controls starting from `data` and showing what's typed over it, and its `submit` button.
//! Typing and submitting are checked in a real browser (`tests/deployed/form.mjs`); here, events
//! go in the way the browser sends them.

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

/// The HTML drawn for `toml` (after `[app]`), after `before` has done whatever it does to the host.
fn html(toml: &str, before: impl FnOnce(&Host)) -> String {
    let toml = format!("[app]\nname = \"x\"\n{toml}\n");
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(&toml).unwrap());
        before(&host);
        app_view(host).to_html()
    })
}

const CONTACT: &str = r#"
[state]
contact = { name = "Ada", agree = true }

[[body]]
type = "form"
id = "contact"
data = "app.contact"
width = "1/2"

[[body.children]]
type = "input"
id = "name_box"
name = "name"
label = "Name"

[[body.children]]
type = "checkbox"
id = "agree_box"
name = "agree"
label = "Agree"

[[body.children]]
type = "button"
label = "Save"
submit = true

[[body.children]]
type = "button"
label = "Cancel"
"#;

#[test]
fn a_form_is_a_form_element_holding_its_children_with_its_placement() {
    let out = html(CONTACT, |_| {});
    assert!(out.contains(r#"<form class="w-1/2""#), "{out}");
    assert!(
        out.find("<form").unwrap() < out.find("Name").unwrap(),
        "{out}"
    );
    assert!(out.contains("</form>"), "{out}");
}

#[test]
fn a_submit_button_submits_and_any_other_button_in_a_form_doesnt() {
    let out = html(CONTACT, |_| {});
    assert!(out.contains(r#"<button type="submit""#), "{out}");
    assert!(out.contains(r#"<button type="button""#), "{out}");
}

#[test]
fn named_controls_start_from_the_forms_data() {
    let out = html(CONTACT, |_| {});
    assert!(out.contains(r#"value="Ada""#), "{out}");
    assert!(out.contains(r#"<input type="checkbox" checked"#), "{out}");
}

#[test]
fn a_named_control_shows_whats_typed_over_its_data_until_submit() {
    let out = html(CONTACT, |host| {
        host.fire("name_box.change", json!({ "value": "Grace" }));
        host.fire("agree_box.change", json!({ "value": false }));
    });
    assert!(out.contains(r#"value="Grace""#), "{out}");
    assert!(!out.contains(r#"checked"#), "{out}");

    let out = html(CONTACT, |host| {
        host.fire("name_box.change", json!({ "value": "Grace" }));
        host.fire("contact.submit", json!({}));
    });
    assert!(out.contains(r#"value="Ada""#), "{out}");
}

#[test]
fn a_named_control_in_a_form_without_data_starts_empty() {
    let out = html(
        "[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"input\"\nname = \"q\"",
        |_| {},
    );
    assert!(out.contains(r#"value="""#), "{out}");
}

#[test]
fn radios_in_a_form_are_grouped_by_the_form_and_their_name() {
    let out = html(
        "[state]\nsizes = { size = 3 }\n[[body]]\ntype = \"form\"\nid = \"f\"\ndata = \"app.sizes\"\n[[body.children]]\ntype = \"radio\"\nname = \"size\"\nvalue = 1\n[[body.children]]\ntype = \"radio\"\nname = \"size\"\nvalue = 3",
        |_| {},
    );
    assert_eq!(out.matches(r#"name="f.size""#).count(), 2, "{out}");
    assert_eq!(out.matches("checked").count(), 1, "{out}");
}

#[test]
fn a_form_inside_an_each_starts_each_one_from_its_item() {
    let out = html(
        "[state]\npeople = [{ name = \"Ada\" }, { name = \"Grace\" }]\n[[body]]\ntype = \"cell\"\neach = \"app.people\"\n[[body.children]]\ntype = \"form\"\ndata = \"item\"\n[[body.children.children]]\ntype = \"input\"\nname = \"name\"",
        |_| {},
    );
    assert!(out.contains(r#"value="Ada""#), "{out}");
    assert!(out.contains(r#"value="Grace""#), "{out}");
}
