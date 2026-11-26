//! The outlet and `go` (specs 040, 090, and 100, "Going"): `app.toml`'s one outlet draws a
//! component, its `default` at first, and `go` swaps in another, named under its own name.

use genesis_core::bus::{Machine, Within};
use genesis_core::config::{App, ConfigError};
use serde_json::{Value, json};

const APP: &str = r#"
[app]
name = "Pages"

[state]
heard = 0

[[body]]
type = "button"
id = "to_contacts"
label = "Contacts"
on_click = { go = "contacts" }

[[body]]
type = "cell"

[[body.children]]
type = "outlet"
default = "home"

[on]
"to_home.click" = [{ go = "home" }]
"to_card.click" = [{ go = "card" }]
"nowhere.click" = [{ go = "nowhere" }]
"contacts.add.click" = [{ step = "app.heard" }]
"#;

const HOME: &str = r#"
[component]

[[body]]
type = "text"
text = "Home"
"#;

const CONTACTS: &str = r#"
[component]

[state]
added = 0

[[body]]
type = "button"
id = "add"
label = "Add"
on_click = { step = "local.added" }
"#;

const CARD: &str = r#"
[component]
props = ["contact"]

[[body]]
type = "text"
text = "{{ props.contact }}"
"#;

fn load(files: &[(&str, &str)]) -> Result<App, Vec<ConfigError>> {
    App::from_files(files.iter().copied())
}

fn files(app: &str) -> Vec<(&str, &str)> {
    vec![
        ("app.toml", app),
        ("home.toml", HOME),
        ("contacts.toml", CONTACTS),
        ("card.toml", CARD),
    ]
}

fn machine() -> Machine {
    let mut m = Machine::new(load(&files(APP)).unwrap());
    m.open();
    m
}

fn problems(app: &str, more: &[(&str, &str)]) -> String {
    let mut all = files(app);
    all.extend_from_slice(more);
    load(&all)
        .expect_err("refused")
        .iter()
        .map(|p| format!("{}: {}", p.file.as_deref().unwrap_or("-"), p.message))
        .collect::<Vec<_>>()
        .join("\n")
}

fn names(m: &Machine) -> Vec<String> {
    let events = m.get("sys.events").unwrap();
    let mut names: Vec<String> = events
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["name"].as_str().map(String::from))
        .collect();
    names.reverse();
    names
}

fn errors(m: &Machine) -> String {
    m.get("sys.errors").unwrap().to_string()
}

/// Where the renderer says a node in the outlet's component fired from.
fn in_outlet(component: &str) -> Option<Within> {
    Some(Within {
        name: component.into(),
        row: None,
        props: Value::Null,
        outer: None,
    })
}

#[test]
fn the_outlet_starts_at_its_default_and_go_swaps_in_another() {
    let mut m = machine();
    assert_eq!(m.get("sys.outlet"), Some(json!("home")));
    m.fire("to_contacts.click", json!({}));
    assert_eq!(m.get("sys.outlet"), Some(json!("contacts")));
    let changed = m.get("sys.events").unwrap()[0].clone();
    assert_eq!(changed["name"], json!("sys.outlet.changed"));
    assert_eq!(
        changed["payload"],
        json!({ "value": "contacts", "was": "home" })
    );

    // Going where it already is changes nothing and fires nothing.
    let before = names(&m).len();
    m.fire("to_contacts.click", json!({}));
    assert_eq!(names(&m).len(), before + 1);
    assert_eq!(m.snapshot()["sys"]["outlet"], json!("contacts"));
}

#[test]
fn the_component_in_the_outlet_fires_under_its_own_name_and_keeps_its_locals_while_away() {
    let mut m = machine();
    m.fire("to_contacts.click", json!({}));
    m.fire_within("contacts.add.click", json!({}), None, in_outlet("contacts"));
    assert_eq!(m.local("contacts", None, "added"), Some(json!(1)));
    assert_eq!(m.get("app.heard"), Some(json!(1)));

    m.fire("to_home.click", json!({}));
    m.fire("to_contacts.click", json!({}));
    assert_eq!(m.local("contacts", None, "added"), Some(json!(1)));

    let fires = m.get("sys.bus.fires").unwrap();
    for name in ["contacts.add.click", "sys.outlet.changed"] {
        assert!(
            fires.as_array().unwrap().contains(&json!(name)),
            "{name} in {fires}"
        );
    }
}

#[test]
fn go_to_no_component_or_one_that_needs_props_fails_and_the_outlet_stays() {
    let mut m = machine();
    m.fire("nowhere.click", json!({}));
    assert!(
        errors(&m).contains("there's no component named \\\"nowhere\\\" to go to"),
        "{}",
        errors(&m)
    );
    m.fire("to_card.click", json!({}));
    assert!(
        errors(&m).contains("`card` needs the prop `contact`, and the outlet passes none"),
        "{}",
        errors(&m)
    );
    assert_eq!(m.get("sys.outlet"), Some(json!("home")));
}

#[test]
fn go_with_no_outlet_fails() {
    let app = r#"
[app]
name = "x"

[on]
"b.click" = [{ go = "home" }]
"#;
    let mut m = Machine::new(load(&files(app)).unwrap());
    assert_eq!(m.get("sys.outlet"), Some(Value::Null));
    m.fire("b.click", json!({}));
    assert!(
        errors(&m).contains("there's no outlet for `go` to put \\\"home\\\" in"),
        "{}",
        errors(&m)
    );
    let fires = m.get("sys.bus.fires").unwrap();
    assert!(
        !fires
            .as_array()
            .unwrap()
            .contains(&json!("sys.outlet.changed"))
    );
}

#[test]
fn go_takes_a_name_written_out_and_a_component_cant_go() {
    let templated = APP.replace("{ go = \"home\" }", "{ go = \"{{ app.heard }}\" }");
    let found = problems(&templated, &[]);
    assert!(
        found.contains("`go` takes a component's name, written out, not a template"),
        "{found}"
    );

    let going = r#"
[component]

[[body]]
type = "button"
id = "b"
label = "Go"
on_click = { go = "home" }
"#;
    let found = problems(APP, &[("going.toml", going)]);
    assert!(
        found.contains("going.toml: body.0: a component's own steps only fire events, so this step, which puts `home` in the outlet, won't work here"),
        "{found}"
    );
}

#[test]
fn an_outlet_is_refused_in_a_component_a_form_or_an_each_and_twice() {
    let holder = r#"
[component]

[[body]]
type = "outlet"
default = "home"
"#;
    let found = problems(APP, &[("holder.toml", holder)]);
    assert!(
        found.contains("holder.toml: body.0: an `outlet` goes only in app.toml's body; a component can't hold one"),
        "{found}"
    );

    let in_form = APP.replace("type = \"cell\"\n", "type = \"form\"\nid = \"f\"\n");
    let found = problems(&in_form, &[]);
    assert!(
        found.contains("app.toml: body.1.children.0: an `outlet` can't go inside a `form`"),
        "{found}"
    );

    let in_each = APP.replace(
        "type = \"cell\"\n",
        "type = \"cell\"\neach = \"app.heard\"\n",
    );
    let found = problems(&in_each, &[]);
    assert!(
        found.contains("app.toml: body.1.children.0: an `outlet` can't go inside an `each`"),
        "{found}"
    );

    let twice = format!("{APP}\n[[body]]\ntype = \"outlet\"\ndefault = \"home\"\n");
    let found = problems(&twice, &[]);
    assert!(
        found.contains(
            "app.toml: there's one outlet, and app.toml's body has 2: body.1.children.0, body.2; keep one"
        ),
        "{found}"
    );
}

#[test]
fn the_default_must_be_a_component_that_needs_no_props() {
    let found = problems(
        &APP.replace("default = \"home\"", "default = \"gone\""),
        &[],
    );
    assert!(
        found.contains("body.1.children.0: there's no component named \"gone\""),
        "{found}"
    );
    let found = problems(
        &APP.replace("default = \"home\"", "default = \"card\""),
        &[],
    );
    assert!(
        found.contains("body.1.children.0: `card` needs the prop `contact`"),
        "{found}"
    );
    let found = problems(&APP.replace("default = \"home\"", "kind = \"home\""), &[]);
    assert!(found.contains("default"), "{found}");
}

#[test]
fn sys_outlet_is_kept_and_the_app_reopens_on_it_while_go_can_still_put_it_there() {
    let mut m = machine();
    m.fire("to_contacts.click", json!({}));
    let kept = m.kept();
    assert_eq!(kept["sys"]["outlet"], json!("contacts"));
    let mut back = Machine::new(load(&files(APP)).unwrap());
    let notes = back.restore(&kept);
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(back.get("sys.outlet"), Some(json!("contacts")));

    // Once no `go` names it, it can't be in the outlet: the app opens on the default, with a note.
    let without = APP.replace("on_click = { go = \"contacts\" }", "");
    let mut back = Machine::new(load(&files(&without)).unwrap());
    let notes = back.restore(&kept);
    assert_eq!(
        notes,
        ["the outlet can't hold contacts now; opening on home"]
    );
    assert_eq!(back.get("sys.outlet"), Some(json!("home")));

    // An app without an outlet ignores a kept one.
    let none = "[app]\nname = \"x\"\n";
    let mut back = Machine::new(load(&files(none)).unwrap());
    let notes = back.restore(&kept);
    assert!(!notes.iter().any(|n| n.contains("outlet")), "{notes:?}");
    assert_eq!(back.get("sys.outlet"), Some(Value::Null));
}

#[test]
fn over_a_config_reload_the_outlet_keeps_its_component_while_go_can_still_put_it_there() {
    let mut m = machine();
    m.fire("to_contacts.click", json!({}));
    m.reload(load(&files(APP)).unwrap());
    assert_eq!(m.get("sys.outlet"), Some(json!("contacts")));

    // No `go` names `contacts` now, so it can't be in the outlet: it's back to the default.
    let without = APP.replace("on_click = { go = \"contacts\" }", "");
    m.reload(load(&files(&without)).unwrap());
    assert_eq!(m.get("sys.outlet"), Some(json!("home")));

    // With no outlet at all, there's nothing in it.
    let none = r#"
[app]
name = "x"
"#;
    m.reload(load(&files(none)).unwrap());
    assert_eq!(m.get("sys.outlet"), Some(Value::Null));
}

#[test]
fn a_go_passed_as_a_prop_wires_its_component() {
    let nav = r#"
[component]
props = ["on_pick"]

[[body]]
type = "button"
id = "pick"
label = "Pick"
on_click = "{{ props.on_pick }}"
"#;
    let app = r#"
[app]
name = "x"

[[body]]
type = "use"
id = "nav"
component = "nav"
props = { on_pick = { go = "contacts" } }

[[body]]
type = "outlet"
default = "home"
"#;
    let mut all = files(app);
    all.push(("nav.toml", nav));
    let mut m = Machine::new(load(&all).unwrap());
    m.fire_within("nav.pick.click", json!({}), None, in_outlet("nav"));
    assert_eq!(m.get("sys.outlet"), Some(json!("contacts")));
    m.fire_within("contacts.add.click", json!({}), None, in_outlet("contacts"));
    assert_eq!(m.local("contacts", None, "added"), Some(json!(1)));
}
