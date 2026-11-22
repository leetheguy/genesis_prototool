//! delay, every, and cancel, on a clock the host (here, the test) controls (spec 100, "The bus").

use genesis_core::bus::Machine;
use genesis_core::config::App;
use serde_json::{Value, json};

fn machine(toml: &str) -> Machine {
    Machine::new(
        App::from_toml(&format!("[app]\nname = \"t\"\nmax_chain = 3\n{toml}")).expect("app loads"),
    )
}

fn trail(m: &Machine) -> Vec<Value> {
    let mut entries = m.get("sys.events").unwrap().as_array().unwrap().clone();
    entries.reverse();
    entries
}

fn logs(m: &Machine) -> Vec<String> {
    trail(m)
        .iter()
        .filter_map(|e| e["log"].as_str().map(String::from))
        .collect()
}

fn count(m: &Machine, name: &str) -> usize {
    trail(m).iter().filter(|e| e["name"] == name).count()
}

#[test]
fn delay_waits_then_carries_on_with_the_list() {
    let mut m = machine("[on]\n\"go\" = [{ log = \"a\" }, { delay = 100 }, { log = \"b\" }]\n");
    m.fire("go", json!({}));
    assert_eq!(logs(&m), ["a"]);
    m.advance(99);
    assert_eq!(logs(&m), ["a"]);
    m.advance(1);
    assert_eq!(logs(&m), ["a", "b"]);
}

#[test]
fn other_events_flow_while_a_list_waits() {
    let mut m = machine(
        "[on]\n\"go\" = [{ delay = 100 }, { log = \"late\" }]\n\"other\" = [{ log = \"now\" }]\n",
    );
    m.fire("go", json!({}));
    m.fire("other", json!({}));
    m.advance(100);
    assert_eq!(logs(&m), ["now", "late"]);
}

#[test]
fn a_resumed_list_still_sees_the_event_that_started_it() {
    let mut m = machine("[on]\n\"go\" = [{ delay = 10 }, { log = \"n={{ event.n }}\" }]\n");
    m.fire("go", json!({ "n": 5 }));
    m.advance(10);
    assert_eq!(logs(&m), ["n=5"]);
}

#[test]
fn a_delay_inside_an_action_pauses_the_list_that_ran_it_too() {
    let mut m = machine(
        "[on]\n\"go\" = [\"slow\", { log = \"after\" }]\n[actions.slow]\ndo = [{ delay = 50 }, { log = \"inside\" }]\n",
    );
    m.fire("go", json!({}));
    assert_eq!((logs(&m).len(), count(&m, "slow.done")), (0, 0));
    m.advance(50);
    assert_eq!(logs(&m), ["inside", "after"]);
    assert_eq!(count(&m, "slow.done"), 1);
}

#[test]
fn a_delay_loop_runs_past_max_chain() {
    let mut m = machine("[on]\n\"tick\" = [{ delay = 10 }, { fire = \"tick\" }]\n");
    m.fire("tick", json!({}));
    m.advance(100);
    assert_eq!(count(&m, "tick"), 11);
    assert!(m.get("sys.errors").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn every_fires_on_a_repeating_timer_at_depth_zero() {
    let mut m = machine(
        "[on]\n\"start\" = [{ every = 100, fire = \"tick\", id = \"t\", with = { n = 1 } }]\n",
    );
    m.fire("start", json!({}));
    m.advance(350);
    let ticks: Vec<Value> = trail(&m)
        .into_iter()
        .filter(|e| e["name"] == "tick")
        .collect();
    let at: Vec<&Value> = ticks.iter().map(|e| &e["at"]).collect();
    assert_eq!(at, [&json!(100), &json!(200), &json!(300)]);
    assert!(
        ticks
            .iter()
            .all(|e| e["depth"] == 0 && e["payload"] == json!({ "n": 1 }))
    );
}

#[test]
fn cancel_stops_an_every() {
    let mut m = machine(
        "[on]\n\"start\" = [{ every = 100, fire = \"tick\", id = \"t\" }]\n\"stop\" = [{ cancel = \"t\" }]\n",
    );
    m.fire("start", json!({}));
    m.advance(150);
    m.fire("stop", json!({}));
    m.advance(300);
    assert_eq!(count(&m, "tick"), 1);
}

#[test]
fn cancel_stops_a_waiting_delay() {
    let mut m = machine(
        "[on]\n\"go\" = [{ delay = 100, id = \"d\" }, { log = \"late\" }]\n\"stop\" = [{ cancel = \"d\" }]\n",
    );
    m.fire("go", json!({}));
    m.fire("stop", json!({}));
    m.advance(200);
    assert!(logs(&m).is_empty());
}

#[test]
fn timers_due_at_the_same_moment_run_in_the_order_they_were_set() {
    let mut m = machine(
        "[on]\n\"go\" = [{ every = 10, fire = \"first\" }, { every = 10, fire = \"second\" }]\n",
    );
    m.fire("go", json!({}));
    m.advance(10);
    let names: Vec<Value> = trail(&m).iter().map(|e| e["name"].clone()).collect();
    assert_eq!(names, [json!("go"), json!("first"), json!("second")]);
}

#[test]
fn an_every_of_zero_is_a_failure() {
    let mut m = machine("[on]\n\"go\" = [{ every = 0, fire = \"tick\" }]\n");
    m.fire("go", json!({}));
    assert_eq!(m.get("sys.errors.0.name"), Some(json!("go")));
}

#[test]
fn the_trail_is_stamped_with_the_clock() {
    let mut m = machine("");
    m.advance(250);
    m.fire("later", json!({}));
    assert_eq!(m.get("sys.events.0.at"), Some(json!(250)));
}
