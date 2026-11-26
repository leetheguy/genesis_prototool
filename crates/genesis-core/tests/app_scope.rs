//! Who reads `app.*` (spec 090): `app.toml`, and any component it draws, by a `use` node in its
//! body or in its outlet. A component drawn inside another reads none of it, unless that one
//! passes it down as props. Either way, any component loads anywhere.

use genesis_core::bus::{Machine, Within};
use genesis_core::config::App;
use serde_json::{Value, json};

const APP: &str = r#"
[app]
name = "Scope"

[state]
user = "Ada"

[[body]]
type = "use"
id = "top"
component = "tag"

[[body]]
type = "use"
id = "box"
component = "frame"
"#;

/// Copies what it reads of `app.user` into its own `seen`, and what it's handed into `given`.
const TAG: &str = r#"
[component]
optional = ["user"]

[state]
seen = "unset"
given = "unset"

[[body]]
type = "button"
id = "look"
label = "Look"
on_click = [{ set = "local.seen", to = "{{ app.user }}" }, { set = "local.given", to = "{{ props.user }}" }]
"#;

/// Draws a `tag` inside itself, handing it `app.user` as a prop.
const FRAME: &str = r#"
[component]

[[body]]
type = "use"
id = "inner"
component = "tag"
props = { user = "{{ app.user }}" }
"#;

fn machine() -> Machine {
    Machine::new(
        App::from_files([("app.toml", APP), ("tag.toml", TAG), ("frame.toml", FRAME)]).unwrap(),
    )
}

fn at(name: &str, props: Value, outer: Option<Within>) -> Option<Within> {
    Some(Within {
        name: name.into(),
        row: None,
        props,
        outer: outer.map(Box::new),
    })
}

#[test]
fn a_component_app_toml_draws_reads_app() {
    let mut m = machine();
    m.fire_within(
        "top.look.click",
        json!({}),
        None,
        at("top", json!({}), None),
    );
    assert_eq!(m.local("top", None, "seen"), Some(json!("Ada")));
}

#[test]
fn a_component_drawn_inside_another_reads_no_app_and_says_so() {
    let mut m = machine();
    let inner = at(
        "box.inner",
        json!({ "user": "Ada" }),
        at("box", json!({}), None),
    );
    m.fire_within("box.inner.look.click", json!({}), None, inner);
    // It fails loud, saying why, and the rest of its steps don't run.
    let errors = m.get("sys.errors").unwrap().to_string();
    assert!(
        errors.contains("app.user can't be read here: this component is drawn inside another"),
        "{errors}"
    );
    assert_eq!(m.local("box.inner", None, "seen"), Some(json!("unset")));
}

#[test]
fn a_component_drawn_inside_another_gets_app_values_it_is_handed_as_props() {
    let tag = TAG.replace("{ set = \"local.seen\", to = \"{{ app.user }}\" }, ", "");
    let mut m = Machine::new(
        App::from_files([("app.toml", APP), ("tag.toml", &tag), ("frame.toml", FRAME)]).unwrap(),
    );
    let inner = at(
        "box.inner",
        json!({ "user": "Ada" }),
        at("box", json!({}), None),
    );
    m.fire_within("box.inner.look.click", json!({}), None, inner);
    assert_eq!(m.local("box.inner", None, "given"), Some(json!("Ada")));
}
