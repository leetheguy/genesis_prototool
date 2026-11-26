//! The actions that change variables, and their `.changed` events (spec 100, "Actions").

use genesis_core::bus::Machine;
use genesis_core::config::App;
use serde_json::{Value, json};

fn machine(toml: &str) -> Machine {
    Machine::new(App::from_toml(&format!("[app]\nname = \"t\"\n{toml}")).expect("app loads"))
}

/// The newest trail entry with this event name.
fn newest(m: &Machine, name: &str) -> Option<Value> {
    m.get("sys.events")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == name)
        .cloned()
}

#[test]
fn set_gives_a_variable_its_value_keeping_the_type() {
    let mut m = machine(
        "[state]\nsel = 0\n[on]\n\"pick\" = [{ set = \"app.sel\", to = \"{{ event.id }}\" }]\n",
    );
    m.fire("pick", json!({ "id": 42 }));
    assert_eq!(m.get("app.sel"), Some(json!(42)));
}

#[test]
fn a_change_fires_changed_with_the_value_and_what_it_was() {
    let mut m = machine("[state]\nsel = 1\n[on]\n\"pick\" = [{ set = \"app.sel\", to = 2 }]\n");
    m.fire("pick", json!({}));
    let changed = newest(&m, "app.sel.changed").expect("changed fired");
    assert_eq!(changed["payload"], json!({ "value": 2, "was": 1 }));
    assert_eq!(changed["from"], "pick");
}

#[test]
fn setting_the_same_value_fires_nothing() {
    let mut m = machine("[state]\nsel = 1\n[on]\n\"pick\" = [{ set = \"app.sel\", to = 1 }]\n");
    m.fire("pick", json!({}));
    assert_eq!(newest(&m, "app.sel.changed"), None);
}

#[test]
fn step_adds_one_by_default_and_keeps_whole_numbers_whole() {
    let mut m = machine("[state]\npage = 1\n[on]\n\"next\" = [{ step = \"app.page\" }]\n");
    m.fire("next", json!({}));
    assert_eq!(m.get("app.page"), Some(json!(2)));
}

#[test]
fn step_adds_by_any_amount() {
    let mut m = machine("[state]\nx = 1.5\n[on]\n\"go\" = [{ step = \"app.x\", by = -0.25 }]\n");
    m.fire("go", json!({}));
    assert_eq!(m.get("app.x"), Some(json!(1.25)));
}

#[test]
fn step_stays_within_min_and_max() {
    let mut m = machine(
        "[state]\npage = 2\n[on]\n\"up\" = [{ step = \"app.page\", by = 5, max = 4 }]\n\"down\" = [{ step = \"app.page\", by = -9, min = 1 }]\n",
    );
    m.fire("up", json!({}));
    assert_eq!(m.get("app.page"), Some(json!(4)));
    m.fire("down", json!({}));
    assert_eq!(m.get("app.page"), Some(json!(1)));
}

#[test]
fn toggle_flips_a_boolean() {
    let mut m = machine("[state]\nopen = false\n[on]\n\"flip\" = [{ toggle = \"app.open\" }]\n");
    m.fire("flip", json!({}));
    assert_eq!(m.get("app.open"), Some(json!(true)));
    m.fire("flip", json!({}));
    assert_eq!(m.get("app.open"), Some(json!(false)));
}

#[test]
fn set_can_change_the_theme() {
    let mut m = machine("[on]\n\"dark\" = [{ set = \"sys.theme\", to = \"dark\" }]\n");
    m.fire("dark", json!({}));
    assert_eq!(m.get("sys.theme"), Some(json!("dark")));
}

#[test]
fn step_on_something_that_isnt_a_number_stops_the_list() {
    let mut m = machine(
        "[state]\nname = \"Ada\"\nn = 0\n[on]\n\"go\" = [{ step = \"app.name\" }, { set = \"app.n\", to = 1 }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(0)));
}

#[test]
fn toggle_on_something_that_isnt_a_boolean_stops_the_list() {
    let mut m = machine(
        "[state]\nname = \"Ada\"\nn = 0\n[on]\n\"go\" = [{ toggle = \"app.name\" }, { set = \"app.n\", to = 1 }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(0)));
}

#[test]
fn writing_a_variable_that_cant_be_written_stops_the_list() {
    let text = "[app]\nname = \"t\"\n[state]\nn = 0\n[on]\n\"a\" = [{ set = \"sys.events\", to = 1 }, { set = \"app.n\", to = 1 }]\n\"b\" = [{ set = \"app.undeclared\", to = 1 }, { set = \"app.n\", to = 2 }]\n";
    // Caught at load...
    let problems = format!("{:?}", App::from_toml(text).unwrap_err());
    assert!(
        problems.contains("`sys.events` can't be written"),
        "{problems}"
    );
    assert!(
        problems.contains("`app.undeclared` isn't declared"),
        "{problems}"
    );
    // ...and by the bus, for an app read without the load checks.
    let mut m = Machine::new(toml::from_str(text).unwrap());
    m.fire("a", json!({}));
    m.fire("b", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(0)));
    assert_eq!(m.get("app.undeclared"), None);
}

#[test]
fn a_missing_value_in_set_stops_the_list() {
    let mut m = machine(
        "[state]\nn = 0\nsel = 0\n[on]\n\"go\" = [{ set = \"app.sel\", to = \"{{ event.gone }}\" }, { set = \"app.n\", to = 1 }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(
        (m.get("app.sel"), m.get("app.n")),
        (Some(json!(0)), Some(json!(0)))
    );
}

#[test]
fn step_past_what_an_integer_holds_stays_a_number() {
    let mut m = machine("[state]\nbig = 1e300\n[on]\n\"go\" = [{ step = \"app.big\" }]\n");
    m.fire("go", json!({}));
    assert_eq!(m.get("app.big"), Some(json!(1e300)));
}
