//! The bus: one queue, handlers matched in file order, the trail, and sys.bus (spec 100).

use genesis_core::bus::Machine;
use genesis_core::config::App;
use serde_json::{Value, json};

fn machine(toml: &str) -> Machine {
    Machine::new(App::from_toml(&format!("[app]\nname = \"t\"\n{toml}")).expect("app loads"))
}

/// The trail's events, oldest first, as their names.
fn names(m: &Machine) -> Vec<String> {
    let trail = m.get("sys.events").unwrap();
    let mut names: Vec<String> = trail
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e.get("name").and_then(Value::as_str).map(String::from))
        .collect();
    names.reverse();
    names
}

/// The trail's log lines, oldest first.
fn logs(m: &Machine) -> Vec<String> {
    let trail = m.get("sys.events").unwrap();
    let mut lines: Vec<String> = trail
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e.get("log").and_then(Value::as_str).map(String::from))
        .collect();
    lines.reverse();
    lines
}

#[test]
fn an_event_nobody_listens_to_is_recorded_with_no_handlers() {
    let mut m = machine("");
    m.fire("nobody.home", json!({}));
    let newest = &m.get("sys.events").unwrap()[0];
    assert_eq!(newest["name"], "nobody.home");
    assert_eq!(newest["handlers"], json!([]));
}

#[test]
fn every_matching_handler_runs_in_file_order() {
    let mut m = machine(
        r#"
[on]
"a.*" = [{ log = "star" }]
"x.y" = [{ log = "never" }]
"a.b" = [{ log = "exact" }]
"#,
    );
    m.fire("a.b", json!({}));
    assert_eq!(logs(&m), ["star", "exact"]);
    assert_eq!(
        m.get("sys.events").unwrap()[2]["handlers"],
        json!(["a.*", "a.b"])
    );
}

#[test]
fn a_log_line_fills_its_templates_from_the_event() {
    let mut m = machine("[on]\n\"hi\" = [{ log = \"{{ event.name }} from {{ event.who }}\" }]\n");
    m.fire("hi", json!({ "who": "Ada" }));
    assert_eq!(logs(&m), ["hi from Ada"]);
}

#[test]
fn events_a_handler_fires_join_the_back_of_the_queue() {
    let mut m = machine(
        r#"
[on]
"start" = [{ fire = "one" }, { fire = "two" }]
"one" = [{ fire = "three" }]
"#,
    );
    m.fire("start", json!({}));
    assert_eq!(names(&m), ["start", "one", "two", "three"]);
}

#[test]
fn a_fired_event_is_one_deeper_and_names_what_fired_it() {
    let mut m = machine("[on]\n\"start\" = [{ fire = \"next\" }]\n");
    m.fire("start", json!({}));
    let trail = m.get("sys.events").unwrap();
    assert_eq!(
        (&trail[0]["name"], &trail[0]["depth"], &trail[0]["from"]),
        (&json!("next"), &json!(1), &json!("start"))
    );
    assert_eq!(
        (&trail[1]["depth"], &trail[1]["from"]),
        (&json!(0), &Value::Null)
    );
}

#[test]
fn a_fired_event_carries_its_with_table_filled_in() {
    let mut m = machine(
        r#"
[on]
"start" = [{ fire = "next", with = { n = "{{ event.n }}", fixed = "x" } }]
"next" = [{ log = "{{ event.n }} {{ event.fixed }}" }]
"#,
    );
    m.fire("start", json!({ "n": 7 }));
    assert_eq!(
        m.get("sys.events").unwrap()[1]["payload"],
        json!({ "n": 7, "fixed": "x" })
    );
    assert_eq!(logs(&m), ["7 x"]);
}

#[test]
fn the_trail_keeps_only_the_newest_entries() {
    let mut m = Machine::new(App::from_toml("[app]\nname = \"t\"\nevent_trail = 2\n").unwrap());
    for name in ["a", "b", "c"] {
        m.fire(name, json!({}));
    }
    assert_eq!(names(&m), ["b", "c"]);
}

#[test]
fn opening_the_app_fires_app_opened() {
    let mut m = machine("");
    m.open();
    assert_eq!(names(&m), ["app.opened"]);
}

#[test]
fn a_buttons_on_click_wires_its_click_event() {
    let mut m = machine(
        r#"
[[body]]
type = "button"
id = "go_btn"
label = "Go"
on_click = [{ log = "clicked" }]
"#,
    );
    m.fire("go_btn.click", json!({}));
    assert_eq!(logs(&m), ["clicked"]);
}

#[test]
fn a_button_without_an_id_is_named_by_its_place_in_the_tree() {
    let mut m = machine(
        r#"
[[body]]
type = "cell"
[[body.children]]
type = "text"
text = "hi"
[[body.children]]
type = "button"
label = "Go"
on_click = [{ log = "clicked" }]
"#,
    );
    m.fire("body.0.children.1.click", json!({}));
    assert_eq!(logs(&m), ["clicked"]);
}

#[test]
fn sys_bus_lists_each_pattern_with_its_handlers() {
    let m = machine("[on]\n\"a.*\" = [{ log = \"x\" }, { fire = \"b\" }]\n");
    let bus = m.get("sys.bus").unwrap();
    assert_eq!(
        bus["handlers"],
        json!([{ "pattern": "a.*", "steps": [{ "log": "x" }, { "fire": "b" }] }])
    );
}

#[test]
fn sys_bus_lists_every_event_the_config_can_fire() {
    let m = machine(
        r#"
[state]
n = 0
[[body]]
type = "button"
id = "go_btn"
label = "Go"
[on]
"go_btn.click" = [{ fire = "went" }]
[actions.cheer]
do = [{ every = 10, fire = "tick" }]
"#,
    );
    let fires = m.get("sys.bus").unwrap()["fires"].clone();
    for name in [
        "app.opened",
        "go_btn.click",
        "app.n.changed",
        "sys.view.changed",
        "sys.theme.changed",
        "went",
        "tick",
        "cheer.done",
        "cheer.failed",
    ] {
        assert!(
            fires.as_array().unwrap().contains(&json!(name)),
            "{name} missing from {fires}"
        );
    }
}

#[test]
fn app_variables_start_at_their_starting_values() {
    let m = machine("[state]\nclicks = 0\nname = \"Ada\"\n");
    assert_eq!(m.get("app.clicks"), Some(json!(0)));
    assert_eq!(m.get("app.name"), Some(json!("Ada")));
    assert_eq!(m.get("app.gone"), None);
}

#[test]
fn sys_view_and_theme_start_at_the_apps_settings() {
    let m = Machine::new(
        App::from_toml("[app]\nname = \"t\"\nstart = \"home\"\ntheme = \"dark\"\n").unwrap(),
    );
    assert_eq!(m.get("sys.view"), Some(json!("home")));
    assert_eq!(m.get("sys.theme"), Some(json!("dark")));
}

#[test]
fn a_path_walks_into_tables_and_lists() {
    let m = machine("[state]\ncard = { title = \"Hi\", tags = [\"a\", \"b\"] }\n");
    assert_eq!(m.get("app.card.title"), Some(json!("Hi")));
    assert_eq!(m.get("app.card.tags.1"), Some(json!("b")));
    assert_eq!(m.get("app.card.tags.2"), None);
    assert_eq!(m.get("app.card.title.more"), None);
}

#[test]
fn a_tap_hands_over_every_entry_once_oldest_first_even_past_the_trail_cap() {
    let mut m = Machine::new(
        App::from_toml("[app]\nname = \"t\"\nevent_trail = 1\n[on]\n\"a\" = [{ log = \"x\" }]\n")
            .unwrap(),
    );
    m.tap();
    m.fire("a", json!({}));
    let taken: Vec<Value> = m
        .take_tapped()
        .iter()
        .map(|e| e.get("name").or(e.get("log")).cloned().unwrap())
        .collect();
    assert_eq!(taken, [json!("a"), json!("x")]);
    assert!(m.take_tapped().is_empty());
}

#[test]
fn without_a_tap_nothing_is_kept_for_taking() {
    let mut m = machine("");
    m.fire("a", json!({}));
    assert!(m.take_tapped().is_empty());
}

#[test]
fn a_snapshot_holds_the_variables_view_theme_and_errors() {
    let mut m =
        machine("[state]\nn = 1\n[on]\n\"go\" = [{ step = \"app.n\" }, { toggle = \"app.n\" }]\n");
    m.fire("go", json!({}));
    let snapshot = m.snapshot();
    assert_eq!(snapshot["app"], json!({ "n": 2 }));
    assert_eq!(
        (&snapshot["sys"]["view"], &snapshot["sys"]["theme"]),
        (&json!("main"), &json!("light"))
    );
    assert_eq!(
        snapshot["sys"]["errors"][0]["message"],
        "app.n isn't true or false"
    );
}
