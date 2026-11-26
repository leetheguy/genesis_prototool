//! `dialog` (spec 040, "The atoms"): what loads, what's refused, and how `.close` sets its variable.

use genesis_core::bus::Machine;
use genesis_core::config::{App, Node};
use serde_json::json;

fn app(toml: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"Dialogs\"\n{toml}"))
}

/// A dialog that asks before deleting, opened by a button and closed by another.
const ASK: &str = r#"
[state]
asking = false
said = "nothing"

[[body]]
type = "button"
label = "Delete"
on_click = [{ set = "app.asking", to = true }]

[[body]]
type = "dialog"
id = "ask"
open = "app.asking"
title = "Delete it?"
on_close = [{ set = "app.said", to = "never mind" }]

[[body.children]]
type = "button"
id = "yes_btn"
label = "Yes"
on_click = [{ set = "app.said", to = "deleted" }, { set = "app.asking", to = false }]
"#;

#[test]
fn a_dialog_loads_with_its_variable_title_and_children_and_survives_the_trip_to_the_browser() {
    let app = app(ASK).expect("loads");
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
    let Node::Dialog(dialog) = &app.body[1] else {
        panic!("not a dialog")
    };
    assert_eq!(dialog.open, "app.asking");
    assert_eq!(dialog.title.as_deref(), Some("Delete it?"));
    assert_eq!(dialog.children.len(), 1);
}

#[test]
fn a_dialog_needs_open_and_it_must_be_an_app_variable() {
    let out = format!("{:?}", app("[[body]]\ntype = \"dialog\"").unwrap_err());
    assert!(out.contains("open"), "{out}");
    let out = format!(
        "{:?}",
        app("[[body]]\ntype = \"dialog\"\nopen = \"asking\"").unwrap_err()
    );
    assert!(
        out.contains("a dialog's `open` is an app variable"),
        "{out}"
    );
}

#[test]
fn closing_sets_its_variable_false_then_runs_on_close() {
    let mut m = Machine::new(app(ASK).expect("loads"));
    m.fire("body.0.click", json!({}));
    assert_eq!(m.get("app.asking"), Some(json!(true)));
    m.fire("ask.close", json!({}));
    assert_eq!(m.get("app.asking"), Some(json!(false)));
    assert_eq!(m.get("app.said"), Some(json!("never mind")));
}

#[test]
fn a_button_inside_a_dialog_is_wired_like_any_other() {
    let mut m = Machine::new(app(ASK).expect("loads"));
    m.fire("body.0.click", json!({}));
    m.fire("yes_btn.click", json!({}));
    assert_eq!(m.get("app.said"), Some(json!("deleted")));
    assert_eq!(m.get("app.asking"), Some(json!(false)));
}

#[test]
fn a_dialog_without_an_id_closes_by_its_place() {
    let mut m = Machine::new(app(&ASK.replace("id = \"ask\"\n", "")).expect("loads"));
    m.fire("body.0.click", json!({}));
    m.fire("body.1.close", json!({}));
    assert_eq!(m.get("app.asking"), Some(json!(false)));
}

#[test]
fn sys_bus_lists_a_dialogs_close() {
    let m = Machine::new(app(ASK).expect("loads"));
    let bus = m.get("sys.bus").unwrap().to_string();
    assert!(bus.contains("ask.close"), "{bus}");
}
