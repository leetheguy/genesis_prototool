//! `form`, controls with `name`, and the `submit` button (spec 040, "Controls"): what loads, what's
//! refused, and how a form's draft fills, starts from `data`, goes out on `.submit`, and is kept.

use genesis_core::bus::Machine;
use genesis_core::config::{App, Node};
use serde_json::{Value, json};

fn app(toml: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"Forms\"\n{toml}"))
}

fn problem(toml: &str) -> String {
    app(toml)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the tests type into the password box: a made-up value, never a real one.
const TYPED: &str = "typed-into-the-box";

fn machine(toml: &str) -> Machine {
    Machine::new(app(toml).expect("app loads"))
}

/// A contact form: a name, an email, a password, and a submit button. Its `.submit` copies the
/// form into `app.saved`.
const CONTACT: &str = r#"
[state]
saved = {}
contact = { name = "Ada", email = "ada@example.com" }

[[body]]
type = "form"
id = "contact"
on_submit = [{ set = "app.saved", to = "{{ event.form }}" }]

[[body.children]]
type = "input"
id = "name_box"
name = "name"

[[body.children]]
type = "input"
id = "email_box"
name = "email"
kind = "email"

[[body.children]]
type = "input"
id = "pass_box"
name = "password"
kind = "password"

[[body.children]]
type = "button"
label = "Save"
submit = true
"#;

#[test]
fn a_form_loads_with_its_children_data_and_on_submit_and_survives_the_trip_to_the_browser() {
    let app = app(&CONTACT.replace(
        "id = \"contact\"",
        "id = \"contact\"\ndata = \"app.contact\"",
    ))
    .expect("loads");
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
    let Node::Form(form) = &app.body[0] else {
        panic!("not a form")
    };
    assert_eq!(form.data.as_deref(), Some("app.contact"));
    assert_eq!(form.children.len(), 4);
    assert!(form.on_submit.is_some());
    let Node::Button(button) = &form.children[3] else {
        panic!("not a button")
    };
    assert!(button.submit);
}

#[test]
fn a_control_with_both_name_and_var_is_a_config_error() {
    let out = problem(
        "[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"input\"\nname = \"q\"\nvar = \"app.q\"",
    );
    assert!(
        out.contains(
            "body.0.children.0: a control takes `name` (inside a form) or `var`, not both"
        ),
        "{out}"
    );
}

#[test]
fn a_control_with_name_outside_a_form_is_a_config_error() {
    let out = problem("[[body]]\ntype = \"checkbox\"\nname = \"agree\"");
    assert!(
        out.contains("body.0: `name` works inside a `form`; outside one, a control takes `var`"),
        "{out}"
    );
}

#[test]
fn a_control_with_neither_name_nor_var_is_a_config_error() {
    let out = problem("[[body]]\ntype = \"toggle\"\nlabel = \"Dark\"");
    assert!(
        out.contains("body.0: a control needs `var`, or `name` inside a form"),
        "{out}"
    );
}

#[test]
fn a_controls_name_is_a_plain_name() {
    let out =
        problem("[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"input\"\nname = \"a.b\"");
    assert!(
        out.contains("`name` is a plain name, like \"email\", not \"a.b\""),
        "{out}"
    );
}

#[test]
fn a_forms_data_is_a_variables_path() {
    let out = problem("[[body]]\ntype = \"form\"\ndata = \"contact\"");
    assert!(
        out.contains(
            "body.0: `data` is a variable's path, like \"app.contact\" or \"item\", not \"contact\""
        ),
        "{out}"
    );
}

#[test]
fn a_submit_button_outside_a_form_and_a_form_in_a_form_are_config_errors() {
    let out = problem("[[body]]\ntype = \"button\"\nlabel = \"Go\"\nsubmit = true");
    assert!(
        out.contains("body.0: `submit` works on a button inside a `form`"),
        "{out}"
    );
    let out = problem("[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"form\"");
    assert!(
        out.contains("body.0.children.0: a form can't hold another form"),
        "{out}"
    );
}

#[test]
fn controls_with_var_work_inside_a_form_and_named_ones_inside_its_cells() {
    let mut m = machine(
        "[state]\nq = \"\"\nsaved = {}\n[[body]]\ntype = \"form\"\nid = \"f\"\non_submit = [{ set = \"app.saved\", to = \"{{ event.form }}\" }]\n[[body.children]]\ntype = \"input\"\nid = \"q_box\"\nvar = \"app.q\"\n[[body.children]]\ntype = \"cell\"\n[[body.children.children]]\ntype = \"input\"\nid = \"n_box\"\nname = \"n\"",
    );
    m.fire("q_box.change", json!({ "value": "ada" }));
    m.fire("n_box.change", json!({ "value": "x" }));
    m.fire("f.submit", json!({}));
    assert_eq!(m.get("app.q"), Some(json!("ada")));
    assert_eq!(m.get("app.saved"), Some(json!({ "n": "x" })));
}

#[test]
fn a_named_controls_change_goes_into_its_forms_draft_not_into_app_state() {
    let mut m = machine(CONTACT);
    m.fire("name_box.change", json!({ "value": "Grace" }));
    assert_eq!(m.draft("contact", "name"), Some(json!("Grace")));
    assert_eq!(m.draft("contact", "email"), None);
    assert_eq!(m.get("app.saved"), Some(json!({})));
}

#[test]
fn submit_fires_with_each_named_control_in_form_and_clears_the_draft() {
    let mut m = machine(CONTACT);
    m.fire("name_box.change", json!({ "value": "Grace" }));
    m.fire("pass_box.change", json!({ "value": TYPED }));
    m.fire("contact.submit", json!({}));
    // A control nobody touched and no `data` fills goes out as null.
    assert_eq!(
        m.get("app.saved"),
        Some(json!({ "name": "Grace", "email": null, "password": TYPED }))
    );
    assert_eq!(m.draft("contact", "name"), None);
}

#[test]
fn a_form_starts_from_data_and_sends_the_draft_over_it() {
    let mut m = machine(&CONTACT.replace(
        "id = \"contact\"",
        "id = \"contact\"\ndata = \"app.contact\"",
    ));
    m.fire("email_box.change", json!({ "value": "grace@example.com" }));
    m.fire("contact.submit", json!({}));
    assert_eq!(
        m.get("app.saved"),
        Some(json!({ "name": "Ada", "email": "grace@example.com", "password": null }))
    );
}

#[test]
fn a_form_inside_an_each_starts_from_its_item() {
    let mut m = machine(
        "[state]\nsaved = {}\npeople = [{ name = \"Ada\" }]\n[[body]]\ntype = \"cell\"\neach = \"app.people\"\n[[body.children]]\ntype = \"form\"\nid = \"edit\"\ndata = \"item\"\non_submit = [{ set = \"app.saved\", to = \"{{ event.form }}\" }]\n[[body.children.children]]\ntype = \"input\"\nname = \"name\"",
    );
    m.fire("edit.submit", json!({ "item": { "name": "Ada" } }));
    assert_eq!(m.get("app.saved"), Some(json!({ "name": "Ada" })));
}

#[test]
fn radios_with_one_name_in_a_form_are_one_field() {
    let mut m = machine(
        "[state]\nsaved = {}\n[[body]]\ntype = \"form\"\nid = \"f\"\non_submit = [{ set = \"app.saved\", to = \"{{ event.form }}\" }]\n[[body.children]]\ntype = \"radio\"\nid = \"small\"\nname = \"size\"\nvalue = 1\n[[body.children]]\ntype = \"radio\"\nid = \"large\"\nname = \"size\"\nvalue = 3",
    );
    m.fire("small.change", json!({ "value": 1 }));
    m.fire("large.change", json!({ "value": 3 }));
    m.fire("f.submit", json!({}));
    assert_eq!(m.get("app.saved"), Some(json!({ "size": 3 })));
}

#[test]
fn a_form_without_an_id_submits_by_its_place() {
    let mut m = machine(
        "[state]\nsaved = {}\n[[body]]\ntype = \"form\"\non_submit = [{ set = \"app.saved\", to = \"{{ event.form }}\" }]\n[[body.children]]\ntype = \"checkbox\"\nname = \"agree\"",
    );
    m.fire("body.0.children.0.change", json!({ "value": true }));
    m.fire("body.0.submit", json!({}));
    assert_eq!(m.get("app.saved"), Some(json!({ "agree": true })));
}

#[test]
fn sys_bus_shows_a_forms_submit_and_its_on_submit_handler() {
    let m = machine(CONTACT);
    let bus = m.get("sys.bus").unwrap();
    let fires: Vec<&str> = bus["fires"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(fires.contains(&"contact.submit"), "{fires:?}");
    assert!(fires.contains(&"name_box.change"), "{fires:?}");
    assert!(
        bus["handlers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["pattern"] == "contact.submit"),
        "{bus}"
    );
}

#[test]
fn drafts_are_kept_and_come_back_but_password_inputs_never_are() {
    let mut m = machine(CONTACT);
    m.fire("name_box.change", json!({ "value": "Grace" }));
    m.fire("pass_box.change", json!({ "value": TYPED }));
    let kept = m.kept();
    assert_eq!(kept["drafts"], json!({ "contact": { "name": "Grace" } }));
    assert!(!kept.to_string().contains(TYPED), "{kept}");
    assert!(!m.snapshot().to_string().contains(TYPED));

    let mut again = machine(CONTACT);
    assert!(again.restore(&kept).is_empty());
    assert_eq!(again.draft("contact", "name"), Some(json!("Grace")));
}

#[test]
fn a_kept_draft_for_a_form_or_field_thats_gone_is_dropped_with_a_note() {
    let mut m = machine(CONTACT);
    let notes =
        m.restore(&json!({ "drafts": { "contact": { "nickname": "G" }, "old": { "a": 1 } } }));
    assert_eq!(m.draft("contact", "nickname"), None);
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("old")), "{notes:?}");
}

#[test]
fn the_event_trail_hides_what_a_password_input_holds() {
    let mut m = machine(CONTACT);
    m.fire("pass_box.change", json!({ "value": TYPED }));
    m.fire("contact.submit", json!({}));
    // What the form's own events carry is hidden on the trail. (This app's handler copies the
    // form into `app.saved`; that's the config's choice, and its `.changed` shows it.)
    let events = m.get("sys.events").unwrap();
    let own: Vec<&Value> = events
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["name"] == "pass_box.change" || e["name"] == "contact.submit")
        .collect();
    assert_eq!(own.len(), 2);
    assert!(!json!(own).to_string().contains(TYPED), "{own:?}");
    // The handler still got the real value.
    assert_eq!(m.get("app.saved.password"), Some(json!(TYPED)));
}

#[test]
fn a_draft_stays_across_a_reload_while_its_form_and_field_do() {
    let mut m = machine(CONTACT);
    m.fire("name_box.change", json!({ "value": "Grace" }));
    m.fire("email_box.change", json!({ "value": "g@example.com" }));
    let without_email = CONTACT.replace(
        "[[body.children]]\ntype = \"input\"\nid = \"email_box\"\nname = \"email\"\nkind = \"email\"\n",
        "",
    );
    m.reload(app(&without_email).unwrap());
    assert_eq!(m.draft("contact", "name"), Some(json!("Grace")));
    assert_eq!(m.draft("contact", "email"), None);
}

/// A list of people, each with its own edit form starting from its row's `item`.
const ROWS: &str = r#"
[state]
saved = {}
people = [{ name = "Ada" }, { name = "Grace" }]

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
id = "name_box"
name = "name"
"#;

#[test]
fn a_form_in_each_row_of_an_each_has_its_own_draft() {
    let mut m = machine(ROWS);
    m.fire_at(
        "name_box.change",
        json!({ "value": "Ada L.", "item": { "name": "Ada" } }),
        Some("0"),
    );
    m.fire_at(
        "name_box.change",
        json!({ "value": "Grace H.", "item": { "name": "Grace" } }),
        Some("1"),
    );
    assert_eq!(m.draft_at("edit", Some("0"), "name"), Some(json!("Ada L.")));
    assert_eq!(
        m.draft_at("edit", Some("1"), "name"),
        Some(json!("Grace H."))
    );
    assert_eq!(m.draft("edit", "name"), None);

    m.fire_at(
        "edit.submit",
        json!({ "item": { "name": "Grace" } }),
        Some("1"),
    );
    assert_eq!(m.get("app.saved"), Some(json!({ "name": "Grace H." })));
    // Submitting row 1 leaves row 0's draft as it was.
    assert_eq!(m.draft_at("edit", Some("1"), "name"), None);
    assert_eq!(m.draft_at("edit", Some("0"), "name"), Some(json!("Ada L.")));
}

#[test]
fn each_rows_draft_is_kept_and_comes_back_by_its_row() {
    let mut m = machine(ROWS);
    m.fire_at("name_box.change", json!({ "value": "Ada L." }), Some("0"));
    let kept = m.kept();
    assert_eq!(kept["drafts"], json!({ "edit#0": { "name": "Ada L." } }));

    let mut again = machine(ROWS);
    assert!(again.restore(&kept).is_empty());
    assert_eq!(
        again.draft_at("edit", Some("0"), "name"),
        Some(json!("Ada L."))
    );
    assert_eq!(again.draft_at("edit", Some("1"), "name"), None);
}
