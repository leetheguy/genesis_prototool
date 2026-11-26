//! Timers (spec 100, "Timers"): named in one registry, the app's from `timers.toml`, started and
//! stopped by name, on a clock the host (here, the test) controls.

use genesis_core::bus::Machine;
use genesis_core::config::{App, ConfigError, Timer};
use serde_json::{Value, json};

const TIMERS: &str =
    "# The app's timers.\n[poll]\nms = 100\nrepeat = true\n\n[save_timeout]\nms = 50\n";

fn load(app: &str, timers: &str) -> Result<App, Vec<ConfigError>> {
    App::from_files([
        (
            "app.toml",
            format!("[app]\nname = \"t\"\nmax_chain = 3\n{app}").as_str(),
        ),
        ("timers.toml", timers),
    ])
}

fn machine(app: &str) -> Machine {
    Machine::new(load(app, TIMERS).expect("app loads"))
}

fn trail(m: &Machine) -> Vec<Value> {
    let mut entries = m.get("sys.events").unwrap().as_array().unwrap().clone();
    entries.reverse();
    entries
}

fn names(m: &Machine) -> Vec<String> {
    trail(m)
        .iter()
        .filter_map(|e| e["name"].as_str().map(String::from))
        .collect()
}

fn count(m: &Machine, name: &str) -> usize {
    names(m).iter().filter(|n| *n == name).count()
}

fn problems(app: &str, timers: &str) -> String {
    load(app, timers)
        .expect_err("refused")
        .iter()
        .map(|p| format!("{}: {}", p.file.as_deref().unwrap_or("-"), p.message))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn timers_toml_fills_the_registry() {
    let app = load("", TIMERS).unwrap();
    assert_eq!(
        app.timers["poll"],
        Timer {
            ms: 100,
            repeat: true
        }
    );
    assert!(!app.timers["save_timeout"].repeat);
    assert_eq!(genesis_core::config::timers(&app).len(), 2);
}

#[test]
fn a_timer_fires_start_then_complete_when_its_time_is_up() {
    let mut m = machine("[on]\n\"save\" = [{ start = \"save_timeout\" }]\n");
    m.fire("save", json!({}));
    assert_eq!(names(&m), ["save", "save_timeout.start"]);
    m.advance(49);
    assert_eq!(count(&m, "save_timeout.complete"), 0);
    m.advance(1);
    assert_eq!(count(&m, "save_timeout.complete"), 1);
    m.advance(500);
    assert_eq!(count(&m, "save_timeout.complete"), 1);
}

#[test]
fn a_repeating_timer_completes_each_time_round_at_depth_zero_until_stopped() {
    let mut m =
        machine("[on]\n\"go\" = [{ start = \"poll\" }]\n\"halt\" = [{ stop = \"poll\" }]\n");
    m.fire("go", json!({}));
    m.advance(350);
    let done: Vec<Value> = trail(&m)
        .into_iter()
        .filter(|e| e["name"] == "poll.complete")
        .collect();
    let at: Vec<&Value> = done.iter().map(|e| &e["at"]).collect();
    assert_eq!(at, [&json!(100), &json!(200), &json!(300)]);
    assert!(done.iter().all(|e| e["depth"] == 0));
    m.fire("halt", json!({}));
    assert_eq!(count(&m, "poll.stop"), 1);
    m.advance(500);
    assert_eq!(count(&m, "poll.complete"), 3);
}

#[test]
fn a_repeating_timer_runs_past_max_chain() {
    let mut m = machine(
        "[on]\n\"go\" = [{ start = \"poll\" }]\n\"poll.complete\" = [{ fire = \"a\" }]\n\"a\" = [{ fire = \"b\" }]\n",
    );
    m.fire("go", json!({}));
    m.advance(1000);
    assert_eq!(count(&m, "b"), 10);
    assert!(m.get("sys.errors").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn starting_a_running_timer_starts_it_over() {
    let mut m = machine("[on]\n\"save\" = [{ start = \"save_timeout\" }]\n");
    m.fire("save", json!({}));
    m.advance(40);
    m.fire("save", json!({}));
    m.advance(40);
    assert_eq!(count(&m, "save_timeout.complete"), 0);
    m.advance(10);
    assert_eq!(count(&m, "save_timeout.complete"), 1);
    assert_eq!(count(&m, "save_timeout.start"), 2);
}

#[test]
fn stopping_a_timer_that_isnt_running_fires_nothing() {
    let mut m = machine("[on]\n\"halt\" = [{ stop = \"poll\" }]\n");
    m.fire("halt", json!({}));
    assert_eq!(names(&m), ["halt"]);
}

#[test]
fn other_events_flow_while_a_timer_runs() {
    let mut m = machine(
        "[on]\n\"save\" = [{ start = \"save_timeout\" }, { log = \"after\" }]\n\"other\" = [{ log = \"now\" }]\n",
    );
    m.fire("save", json!({}));
    m.fire("other", json!({}));
    m.advance(50);
    let logs: Vec<Value> = trail(&m)
        .iter()
        .map(|e| e["log"].clone())
        .filter(|l| !l.is_null())
        .collect();
    assert_eq!(logs, [json!("after"), json!("now")]);
}

#[test]
fn timers_due_at_the_same_moment_complete_in_the_order_they_were_started() {
    let timers = "[first]\nms = 10\n[second]\nms = 10\n";
    let mut m = Machine::new(
        load(
            "[on]\n\"go\" = [{ start = \"second\" }, { start = \"first\" }]\n",
            timers,
        )
        .unwrap(),
    );
    m.fire("go", json!({}));
    m.advance(10);
    let done: Vec<String> = names(&m)
        .into_iter()
        .filter(|n| n.ends_with(".complete"))
        .collect();
    assert_eq!(done, ["second.complete", "first.complete"]);
}

#[test]
fn starting_or_stopping_a_timer_nobody_declared_is_a_failure() {
    let mut m =
        machine("[on]\n\"go\" = [{ start = \"nope\" }]\n\"halt\" = [{ stop = \"nope\" }]\n");
    m.fire("go", json!({}));
    m.fire("halt", json!({}));
    assert_eq!(
        m.get("sys.errors.1.message"),
        Some(json!("there's no timer named nope"))
    );
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!("there's no timer named nope"))
    );
}

#[test]
fn sys_bus_lists_each_timers_events() {
    let m = machine("");
    let fires = m.get("sys.bus").unwrap()["fires"].clone();
    for name in [
        "poll.start",
        "poll.stop",
        "poll.complete",
        "save_timeout.complete",
    ] {
        assert!(fires.as_array().unwrap().contains(&json!(name)), "{name}");
    }
}

#[test]
fn over_a_reload_a_timer_keeps_running_while_it_is_still_declared() {
    let mut m = machine("[on]\n\"go\" = [{ start = \"poll\" }, { start = \"save_timeout\" }]\n");
    m.fire("go", json!({}));
    m.reload(load("", "[poll]\nms = 100\nrepeat = true\n").unwrap());
    m.advance(100);
    assert_eq!(count(&m, "poll.complete"), 1);
    assert_eq!(count(&m, "save_timeout.complete"), 0);
}

#[test]
fn a_timer_needs_ms_of_at_least_one_a_plain_name_and_only_its_keys() {
    let out = problems("", "[poll]\nms = 0\n");
    assert!(
        out.contains("timers.toml: the timer `poll` needs `ms` of at least 1"),
        "{out}"
    );
    let out = problems("", "[\"a.b\"]\nms = 5\n");
    assert!(out.contains("a timer's name is a plain name"), "{out}");
    let out = problems("", "[poll]\nms = 5\nevery = 3\n");
    assert!(
        out.contains("timers.toml") && out.contains("every"),
        "{out}"
    );
}

#[test]
fn timers_go_only_in_timers_toml() {
    let out = problems("[timers.poll]\nms = 5\n", "");
    assert!(
        out.contains("app.toml: timers go in a file of their own, timers.toml"),
        "{out}"
    );
}

#[test]
fn delay_every_and_cancel_are_gone() {
    for step in [
        "{ delay = 10 }",
        "{ every = 10, fire = \"t\" }",
        "{ cancel = \"t\" }",
    ] {
        let out = problems(&format!("[on]\n\"go\" = [{step}]\n"), "");
        assert!(
            out.contains("step: a timer is named in timers.toml, and `start` and `stop` run it"),
            "{step}: {out}"
        );
    }
}

#[test]
fn the_trail_is_stamped_with_the_clock() {
    let mut m = machine("");
    m.advance(250);
    m.fire("later", json!({}));
    assert_eq!(m.get("sys.events.0.at"), Some(json!(250)));
}
