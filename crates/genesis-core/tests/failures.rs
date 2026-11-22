//! Chain depth, and failures always logged (spec 100, "The bus").

use genesis_core::bus::{Effect, Machine};
use genesis_core::config::App;
use serde_json::{Value, json};

fn machine(toml: &str) -> Machine {
    Machine::new(
        App::from_toml(&format!("[app]\nname = \"t\"\nmax_chain = 3\n{toml}")).expect("app loads"),
    )
}

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

fn errors(m: &Machine) -> Vec<Value> {
    m.get("sys.errors").unwrap().as_array().unwrap().clone()
}

fn toasts(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .map(|Effect::Toast { text }| text.clone())
        .collect()
}

#[test]
fn a_chain_past_max_chain_is_dropped() {
    let mut m = machine("[on]\n\"ping\" = [{ fire = \"ping\" }]\n");
    m.fire("ping", json!({}));
    assert_eq!(names(&m), ["ping", "ping", "ping", "ping"]);
}

#[test]
fn a_dropped_chain_fails_loud_with_its_names() {
    let mut m = machine("[on]\n\"a\" = [{ fire = \"b\" }]\n\"b\" = [{ fire = \"a\" }]\n");
    let effects = m.fire("a", json!({}));
    let error = &errors(&m)[0];
    assert_eq!(error["name"], "chain.too_deep");
    assert_eq!(error["chain"], json!(["a", "b", "a", "b", "a"]));
    assert_eq!(toasts(&effects).len(), 1);
}

#[test]
fn a_chain_at_exactly_max_chain_still_runs() {
    let mut m = machine(
        "[on]\n\"a\" = [{ fire = \"b\" }]\n\"b\" = [{ fire = \"c\" }]\n\"c\" = [{ fire = \"d\" }]\n",
    );
    m.fire("a", json!({}));
    assert_eq!(names(&m), ["a", "b", "c", "d"]);
    assert!(errors(&m).is_empty());
}

#[test]
fn a_failed_event_is_in_sys_errors_before_any_handler_runs() {
    let mut m = machine(
        "[state]\nseen = \"\"\n[on]\n\"go\" = [\"broken\"]\n\"*.failed\" = [{ set = \"app.seen\", to = \"{{ sys.errors.0.name }}\" }]\n[actions.broken]\ndo = [{ step = \"app.seen\" }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!(m.get("app.seen"), Some(json!("broken.failed")));
}

#[test]
fn a_failure_nobody_handles_shows_a_toast_saying_what_failed() {
    let mut m = machine(
        "[state]\nname = \"Ada\"\n[on]\n\"go\" = [\"broken\"]\n[actions.broken]\ndo = [{ step = \"app.name\" }]\n",
    );
    let effects = m.fire("go", json!({}));
    assert_eq!(toasts(&effects), ["broken.failed: app.name isn't a number"]);
    assert_eq!(errors(&m)[0]["message"], "app.name isn't a number");
}

#[test]
fn a_handled_failure_shows_no_toast_but_is_still_logged() {
    let mut m = machine(
        "[state]\nname = \"Ada\"\n[on]\n\"go\" = [\"broken\"]\n\"broken.failed\" = [{ log = \"handled\" }]\n[actions.broken]\ndo = [{ step = \"app.name\" }]\n",
    );
    let effects = m.fire("go", json!({}));
    assert!(toasts(&effects).is_empty());
    assert_eq!(errors(&m)[0]["name"], "broken.failed");
}

#[test]
fn any_event_ending_in_failed_is_logged() {
    let mut m =
        machine("[on]\n\"go\" = [{ fire = \"mine.failed\", with = { error = \"nope\" } }]\n");
    m.fire("go", json!({}));
    assert_eq!(errors(&m)[0]["name"], "mine.failed");
    assert_eq!(errors(&m)[0]["message"], "nope");
}

#[test]
fn a_step_failing_in_a_handlers_own_list_is_logged_and_toasted() {
    let mut m = machine("[state]\nname = \"Ada\"\n[on]\n\"go\" = [{ toggle = \"app.name\" }]\n");
    let effects = m.fire("go", json!({}));
    assert_eq!(errors(&m)[0]["name"], "go");
    assert_eq!(errors(&m)[0]["message"], "app.name isn't true or false");
    assert_eq!(toasts(&effects), ["go: app.name isn't true or false"]);
}

#[test]
fn sys_errors_keeps_only_the_newest_entries() {
    let mut m = Machine::new(App::from_toml("[app]\nname = \"t\"\nevent_trail = 2\n").unwrap());
    for name in ["a.failed", "b.failed", "c.failed"] {
        m.fire(name, json!({}));
    }
    let names: Vec<Value> = errors(&m).iter().map(|e| e["name"].clone()).collect();
    assert_eq!(names, [json!("c.failed"), json!("b.failed")]);
}
