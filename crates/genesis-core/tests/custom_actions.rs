//! Custom actions: named lists of steps, with `.done` and `.failed` (spec 100, "Actions").

use genesis_core::bus::Machine;
use genesis_core::config::App;
use serde_json::{Value, json};

fn machine(toml: &str) -> Machine {
    Machine::new(
        App::from_toml(&format!("[app]\nname = \"t\"\nmax_chain = 5\n{toml}")).expect("app loads"),
    )
}

/// The trail's event names, oldest first.
fn names(m: &Machine) -> Vec<String> {
    let mut names: Vec<String> = m
        .get("sys.events")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["name"].as_str().map(String::from))
        .collect();
    names.reverse();
    names
}

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
fn a_plain_string_step_runs_the_actions_steps() {
    let mut m = machine(
        "[state]\nn = 0\n[on]\n\"go\" = [\"bump\"]\n[actions.bump]\ndo = [{ step = \"app.n\" }, { step = \"app.n\" }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(2)));
}

#[test]
fn a_finished_action_fires_done() {
    let mut m = machine("[on]\n\"go\" = [\"cheer\"]\n[actions.cheer]\ndo = [{ log = \"yay\" }]\n");
    m.fire("go", json!({}));
    assert_eq!(names(&m), ["go", "cheer.done"]);
    assert_eq!(newest(&m, "cheer.done").unwrap()["from"], "go");
}

#[test]
fn an_actions_steps_see_the_event_that_ran_it() {
    let mut m = machine(
        "[state]\nsel = 0\n[on]\n\"pick\" = [\"choose\"]\n[actions.choose]\ndo = [{ set = \"app.sel\", to = \"{{ event.id }}\" }]\n",
    );
    m.fire("pick", json!({ "id": 9 }));
    assert_eq!(m.get("app.sel"), Some(json!(9)));
}

#[test]
fn a_failing_step_stops_the_action_and_fires_failed_with_why() {
    let mut m = machine(
        "[state]\nname = \"Ada\"\nn = 0\n[on]\n\"go\" = [\"broken\"]\n[actions.broken]\ndo = [{ step = \"app.name\" }, { set = \"app.n\", to = 1 }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(0)));
    assert_eq!(
        newest(&m, "broken.failed").unwrap()["payload"],
        json!({ "error": "app.name isn't a number" })
    );
    assert_eq!(newest(&m, "broken.done"), None);
}

#[test]
fn a_failed_action_stops_the_list_that_ran_it() {
    let mut m = machine(
        "[state]\nname = \"Ada\"\nn = 0\n[on]\n\"go\" = [\"broken\", { set = \"app.n\", to = 1 }]\n[actions.broken]\ndo = [{ toggle = \"app.name\" }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(0)));
}

#[test]
fn actions_run_other_actions_and_each_reports_when_done() {
    let mut m = machine(
        "[on]\n\"go\" = [\"outer\"]\n[actions.outer]\ndo = [\"inner\"]\n[actions.inner]\ndo = [{ log = \"in\" }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(names(&m), ["go", "inner.done", "outer.done"]);
}

#[test]
fn a_step_naming_no_action_is_a_failure() {
    let mut m =
        machine("[state]\nn = 0\n[on]\n\"go\" = [\"nowhere\", { set = \"app.n\", to = 1 }]\n");
    m.fire("go", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(0)));
}

#[test]
fn an_action_that_runs_itself_is_stopped_at_max_chain() {
    let mut m = machine("[on]\n\"go\" = [\"again\"]\n[actions.again]\ndo = [\"again\"]\n");
    m.fire("go", json!({}));
    let failed = newest(&m, "again.failed").expect("it fails instead of recursing forever");
    assert!(
        failed["payload"]["error"]
            .as_str()
            .unwrap()
            .contains("nested"),
        "{failed}"
    );
}
