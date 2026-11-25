//! Inside a component (spec 090): each instance's own `local.*`, events named under the `use`
//! node, and steps that read `props.*`, run against the app's state.

use genesis_core::bus::{Machine, Within};
use genesis_core::config::{App, ConfigError};
use serde_json::{Value, json};

const APP: &str = r#"
[app]
name = "Cards"

[state]
contacts = [{ name = "Ada" }, { name = "Lin" }]
opened = ""

[on]
"card.more.click" = [{ set = "app.opened", to = "{{ event.item.name }}" }]

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "use"
id = "card"
component = "contact_card"
props = { contact = "{{ item }}" }
"#;

const CARD: &str = r#"
[component]
props = ["contact"]

[state]
expanded = false
who = ""

[[body]]
type = "button"
id = "more"
label = "More"
on_click = [{ toggle = "local.expanded" }, { set = "local.who", to = "{{ props.contact.name }}" }]

[[body]]
type = "dialog"
id = "ask"
open = "local.expanded"
"#;

fn load(files: &[(&str, &str)]) -> Result<App, Vec<ConfigError>> {
    App::from_files(files.iter().copied())
}

fn machine() -> Machine {
    Machine::new(load(&[("app.toml", APP), ("contact_card.toml", CARD)]).unwrap())
}

/// Where the renderer says a node in row `row`'s card fired from.
fn card(row: &str, name: &str) -> Option<Within> {
    Some(Within {
        name: "card".into(),
        row: Some(row.into()),
        props: json!({ "contact": { "name": name } }),
        outer: None,
    })
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

#[test]
fn each_instance_has_its_own_local_state_starting_from_the_components() {
    let mut m = machine();
    assert_eq!(m.local("card", Some("0"), "expanded"), Some(json!(false)));
    m.fire_within(
        "card.more.click",
        json!({ "item": { "name": "Lin" } }),
        Some("1"),
        card("1", "Lin"),
    );
    assert_eq!(m.local("card", Some("1"), "expanded"), Some(json!(true)));
    assert_eq!(m.local("card", Some("0"), "expanded"), Some(json!(false)));
    assert_eq!(m.local("card", Some("1"), "who"), Some(json!("Lin")));
    assert_eq!(m.local("card", Some("1"), "nothing"), None);
    assert_eq!(m.local("nobody", None, "expanded"), None);
}

#[test]
fn a_node_inside_fires_under_the_use_node_and_the_app_hears_it_with_its_item() {
    let mut m = machine();
    m.fire_within(
        "card.more.click",
        json!({ "item": { "name": "Ada" } }),
        Some("0"),
        card("0", "Ada"),
    );
    assert_eq!(m.get("app.opened"), Some(json!("Ada")));
    let names = names(&m);
    assert!(
        names.contains(&"card.local.expanded.changed".to_string()),
        "{names:?}"
    );
    let bus = m.get("sys.bus.fires").unwrap();
    for name in ["card.more.click", "card.ask.close"] {
        assert!(bus.as_array().unwrap().contains(&json!(name)), "{bus}");
    }
}

#[test]
fn a_dialog_inside_closes_through_its_own_instances_local() {
    let mut m = machine();
    m.fire_within("card.more.click", json!({}), Some("0"), card("0", "Ada"));
    m.fire_within("card.more.click", json!({}), Some("1"), card("1", "Lin"));
    m.fire_within("card.ask.close", json!({}), Some("1"), card("1", "Lin"));
    assert_eq!(m.local("card", Some("1"), "expanded"), Some(json!(false)));
    assert_eq!(m.local("card", Some("0"), "expanded"), Some(json!(true)));
}

#[test]
fn an_event_pushed_in_without_where_it_came_from_is_its_rows_instance_with_no_props() {
    let mut m = machine();
    m.fire_at("card.more.click", json!({}), Some("1"));
    assert_eq!(m.local("card", Some("1"), "expanded"), Some(json!(true)));
    // `props.contact.name` names nothing, so the second step fails loud and changes nothing.
    assert_eq!(m.local("card", Some("1"), "who"), Some(json!("")));
    assert!(!m.get("sys.errors").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn locals_are_kept_brought_back_and_kept_across_a_config_reload() {
    let mut m = machine();
    m.fire_within("card.more.click", json!({}), Some("0"), card("0", "Ada"));
    let kept = m.kept();
    assert_eq!(kept["locals"]["card#0"]["expanded"], json!(true));
    assert_eq!(m.snapshot()["locals"], kept["locals"]);

    let mut back = machine();
    let notes = back.restore(&kept);
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(back.local("card", Some("0"), "expanded"), Some(json!(true)));

    // A local the component no longer declares, or one of another kind, starts fresh.
    let changed = CARD
        .replace("expanded = false", "expanded = 0")
        .replace("who = \"\"", "");
    let changed = changed.replace(
        ", { set = \"local.who\", to = \"{{ props.contact.name }}\" }",
        "",
    );
    let app = load(&[("app.toml", APP), ("contact_card.toml", &changed)]).unwrap();
    back.reload(app.clone());
    assert_eq!(back.local("card", Some("0"), "expanded"), Some(json!(0)));
    let notes = Machine::new(app).restore(&kept);
    assert!(
        notes.iter().any(|n| n.contains("local.expanded")),
        "{notes:?}"
    );
}

#[test]
fn locals_of_a_component_gone_from_the_config_are_dropped() {
    let mut m = machine();
    m.fire_within("card.more.click", json!({}), Some("0"), card("0", "Ada"));
    let kept = m.kept();
    let bare = "[app]\nname = \"Cards\"\n";
    let mut other = Machine::new(load(&[("app.toml", bare)]).unwrap());
    let notes = other.restore(&kept);
    assert!(
        notes
            .iter()
            .any(|n| n.contains("the component card is gone")),
        "{notes:?}"
    );
    m.reload(load(&[("app.toml", bare)]).unwrap());
    assert_eq!(m.kept()["locals"], json!({}));
}

#[test]
fn named_controls_inside_a_component_fill_the_form_it_is_used_in() {
    let app = r#"
[app]
name = "Sign up"

[state]
sent = {}

[[body]]
type = "form"
id = "signup"
on_submit = [{ set = "app.sent", to = "{{ event.form }}" }]

[[body.children]]
type = "use"
id = "fields"
component = "fields"
"#;
    let fields = r#"
[component]

[[body]]
type = "input"
id = "email_box"
name = "email"
"#;
    let mut m = Machine::new(load(&[("app.toml", app), ("fields.toml", fields)]).unwrap());
    let within = Some(Within {
        name: "fields".into(),
        row: None,
        props: json!({}),
        outer: None,
    });
    m.fire_within(
        "fields.email_box.change",
        json!({ "value": "ada@example.com" }),
        None,
        within,
    );
    m.fire("signup.submit", json!({}));
    assert_eq!(
        m.get("app.sent"),
        Some(json!({ "email": "ada@example.com" }))
    );
}

#[test]
fn a_form_inside_a_component_starts_from_its_props_or_locals() {
    let app = r#"
[app]
name = "Edit"

[state]
saved = {}

[on]
"editor.edit.submit" = [{ set = "app.saved", to = "{{ event.form }}" }]

[[body]]
type = "use"
id = "editor"
component = "editor"
props = { contact = { name = "Ada" } }
"#;
    let editor = |data: &str| {
        format!(
            "[component]\nprops = [\"contact\"]\n[state]\nmine = {{ name = \"Lin\" }}\n[[body]]\ntype = \"form\"\nid = \"edit\"\ndata = \"{data}\"\n[[body.children]]\ntype = \"input\"\nname = \"name\"\n"
        )
    };
    for (data, name) in [("props.contact", "Ada"), ("local.mine", "Lin")] {
        let mut m =
            Machine::new(load(&[("app.toml", app), ("editor.toml", &editor(data))]).unwrap());
        let within = Some(Within {
            name: "editor".into(),
            row: None,
            props: json!({ "contact": { "name": "Ada" } }),
            outer: None,
        });
        m.fire_within("editor.edit.submit", json!({}), None, within);
        assert_eq!(m.get("app.saved"), Some(json!({ "name": name })), "{data}");
    }
}

#[test]
fn a_control_writes_its_instances_local() {
    let toggles = "[component]\n[state]\non = false\n[[body]]\ntype = \"toggle\"\nid = \"sw\"\nvar = \"local.on\"\n";
    let app = "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\nid = \"a\"\ncomponent = \"toggles\"\n[[body]]\ntype = \"use\"\ncomponent = \"toggles\"\n";
    let mut m = Machine::new(load(&[("app.toml", app), ("toggles.toml", toggles)]).unwrap());
    let at = |name: &str| {
        Some(Within {
            name: name.into(),
            row: None,
            props: Value::Null,
            outer: None,
        })
    };
    m.fire_within("a.sw.change", json!({ "value": true }), None, at("a"));
    // The second has no id, so it's named by its place.
    m.fire_within(
        "body.1.sw.change",
        json!({ "value": false }),
        None,
        at("body.1"),
    );
    assert_eq!(m.local("a", None, "on"), Some(json!(true)));
    assert_eq!(m.local("body.1", None, "on"), Some(json!(false)));
}

#[test]
fn components_inside_components_have_their_own_instances() {
    let inner = "[component]\n[state]\nn = 0\n[[body]]\ntype = \"button\"\nid = \"up\"\nlabel = \"+\"\non_click = [{ step = \"local.n\" }]\n";
    let outer = "[component]\n[[body]]\ntype = \"use\"\nid = \"counter\"\ncomponent = \"inner\"\n";
    let app =
        "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\nid = \"box\"\ncomponent = \"outer\"\n";
    let mut m = Machine::new(
        load(&[
            ("app.toml", app),
            ("inner.toml", inner),
            ("outer.toml", outer),
        ])
        .unwrap(),
    );
    let within = Some(Within {
        name: "box.counter".into(),
        row: None,
        props: Value::Null,
        outer: None,
    });
    m.fire_within("box.counter.up.click", json!({}), None, within);
    assert_eq!(m.local("box.counter", None, "n"), Some(json!(1)));
    let names = names(&m);
    assert!(
        names.contains(&"box.counter.local.n.changed".to_string()),
        "{names:?}"
    );
}

#[test]
fn local_outside_a_component_cant_be_written() {
    let mut m = Machine::new(
        load(&[(
            "app.toml",
            "[app]\nname = \"x\"\n[on]\n\"go\" = [{ set = \"local.x\", to = 1 }]\n",
        )])
        .unwrap(),
    );
    m.fire("go", json!({}));
    let errors = m.get("sys.errors").unwrap();
    assert!(
        errors
            .to_string()
            .contains("local.x is only inside a component"),
        "{errors}"
    );
}

#[test]
fn an_undeclared_local_or_prop_inside_a_component_is_a_config_error() {
    let card = r#"
[component]
props = ["contact"]

[state]
open = false

[[body]]
type = "text"
text = "{{ local.nope }} {{ props.other }}"

[[body]]
type = "button"
label = "x"
on_click = [{ toggle = "local.closed" }]

[[body]]
type = "input"
var = "local.open"
"#;
    let problems = load(&[("app.toml", APP), ("contact_card.toml", card)]).unwrap_err();
    let out: Vec<String> = problems.iter().map(|p| p.message.clone()).collect();
    let out = out.join("\n");
    assert!(
        out.contains("body.0: local.nope isn't declared in this component's [state]"),
        "{out}"
    );
    assert!(
        out.contains("body.0: this component has no prop `other`"),
        "{out}"
    );
    assert!(out.contains("body.1: local.closed isn't declared"), "{out}");
    assert!(!out.contains("body.2"), "{out}");
    // Outside a component, a control can't take `local.*`.
    let outside = "[app]\nname = \"x\"\n[[body]]\ntype = \"input\"\nvar = \"local.open\"\n";
    assert!(load(&[("app.toml", outside)]).is_err());
}
