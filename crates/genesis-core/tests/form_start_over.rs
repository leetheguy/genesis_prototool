//! A form's draft starts over when the variable its `data` reads changes (spec 040, "Controls"):
//! editing one record, closing, then editing another shows the second, not what was typed over the
//! first.

use genesis_core::bus::{Machine, Within};
use genesis_core::config::App;
use serde_json::json;

/// An edit form over `app.contact`, with a required name, and a note box that's no form's.
const APP: &str = r#"
[app]
name = "Edit"

[state]
contact = { name = "Ada", email = "ada@example.com" }
note = ""

[on]
"pick.click" = [{ set = "app.contact", to = "{{ event.contact }}" }]
"jot.click" = [{ set = "app.note", to = "{{ event.note }}" }]

[[body]]
type = "button"
id = "pick"
label = "Pick"

[[body]]
type = "button"
id = "jot"
label = "Jot"

[[body]]
type = "form"
id = "edit"
data = "app.contact"

[[body.children]]
type = "input"
id = "name_box"
name = "name"
required = "A name, please."

[[body.children]]
type = "input"
id = "email_box"
name = "email"

[[body.children]]
type = "button"
label = "Save"
submit = true

[[body]]
type = "form"
id = "blank"

[[body.children]]
type = "input"
id = "blank_box"
name = "anything"
"#;

fn machine() -> Machine {
    Machine::new(App::from_toml(APP).expect("app loads"))
}

#[test]
fn a_new_record_in_data_drops_the_draft_and_the_failed_try() {
    let mut m = machine();
    m.fire("email_box.change", json!({ "value": "typed@example.com" }));
    m.fire("name_box.change", json!({ "value": "" }));
    m.fire("edit.submit", json!({}));
    assert!(m.tried("edit", None));
    m.fire("blank_box.change", json!({ "value": "kept" }));

    m.fire(
        "pick.click",
        json!({ "contact": { "name": "Lin", "email": "lin@example.com" } }),
    );
    assert_eq!(m.draft("edit", "email"), None);
    assert_eq!(m.draft("edit", "name"), None);
    assert!(!m.tried("edit", None));
    // A form whose data didn't change keeps what was typed in it.
    assert_eq!(m.draft("blank", "anything"), Some(json!("kept")));
}

#[test]
fn the_same_record_again_or_another_variable_leaves_the_draft() {
    let mut m = machine();
    m.fire("email_box.change", json!({ "value": "typed@example.com" }));
    m.fire(
        "pick.click",
        json!({ "contact": { "name": "Ada", "email": "ada@example.com" } }),
    );
    m.fire("jot.click", json!({ "note": "call back" }));
    assert_eq!(m.draft("edit", "email"), Some(json!("typed@example.com")));
}

/// A card per contact, each with its own `local.draft_of` and a form over it.
const LIST: &str = r#"
[app]
name = "Cards"

[state]
contacts = [{ name = "Ada" }, { name = "Lin" }]

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "use"
id = "card"
component = "card"
props = { contact = "{{ item }}" }
"#;

const CARD: &str = r#"
[component]
props = ["contact"]

[state]
draft_of = { name = "" }

[[body]]
type = "button"
id = "load"
label = "Load"
on_click = [{ set = "local.draft_of", to = "{{ props.contact }}" }]

[[body]]
type = "form"
id = "form"
data = "local.draft_of"

[[body.children]]
type = "input"
id = "name_box"
name = "name"
"#;

fn card(row: &str, name: &str) -> Option<Within> {
    Some(Within {
        name: "card".into(),
        row: Some(row.into()),
        props: json!({ "contact": { "name": name } }),
        outer: None,
    })
}

#[test]
fn a_local_in_one_instance_starts_over_only_that_instances_form() {
    let mut m = Machine::new(App::from_files([("app.toml", LIST), ("card.toml", CARD)]).unwrap());
    for (row, name) in [("0", "Ada"), ("1", "Lin")] {
        m.fire_within(
            "card.name_box.change",
            json!({ "value": "typed" }),
            Some(row),
            card(row, name),
        );
    }
    m.fire_within("card.load.click", json!({}), Some("1"), card("1", "Lin"));
    assert_eq!(
        m.local("card", Some("1"), "draft_of"),
        Some(json!({ "name": "Lin" }))
    );
    assert_eq!(m.draft_at("card.form", Some("1"), "name"), None);
    assert_eq!(
        m.draft_at("card.form", Some("0"), "name"),
        Some(json!("typed"))
    );
}
