//! Carrying the state across a new config (spec 050, "Across a config reload"; spec 060).

use genesis_core::bus::Machine;
use genesis_core::config::App;
use serde_json::json;

fn app(toml: &str) -> App {
    App::from_toml(&format!("[app]\nname = \"t\"\n{toml}")).expect("app loads")
}

#[test]
fn variables_still_declared_keep_their_values() {
    let mut m = Machine::new(app(
        "[state]\nn = 0\n[on]\n\"go\" = [{ step = \"app.n\" }]\n",
    ));
    m.fire("go", json!({}));
    m.reload(app("[state]\nn = 0\n"));
    assert_eq!(m.get("app.n"), Some(json!(1)));
}

#[test]
fn new_variables_start_at_their_starting_values_and_removed_ones_go() {
    let mut m = Machine::new(app("[state]\nold = 1\n"));
    m.reload(app("[state]\nfresh = \"hi\"\n"));
    assert_eq!(
        (m.get("app.fresh"), m.get("app.old")),
        (Some(json!("hi")), None)
    );
}

#[test]
fn the_new_handlers_are_the_ones_that_run() {
    let mut m = Machine::new(app(
        "[state]\nn = 0\n[on]\n\"go\" = [{ step = \"app.n\" }]\n",
    ));
    m.reload(app(
        "[state]\nn = 0\n[on]\n\"go\" = [{ step = \"app.n\", by = 10 }]\n",
    ));
    m.fire("go", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(10)));
    assert_eq!(m.get("sys.bus.handlers.0.steps.0.by"), Some(json!(10.0)));
}

#[test]
fn the_theme_trail_and_errors_carry_over() {
    let mut m = Machine::new(app(
        "[on]\n\"go\" = [{ set = \"sys.theme\", to = \"dark\" }]\n",
    ));
    m.fire("go", json!({}));
    m.fire("x.failed", json!({ "error": "kept" }));
    m.reload(app(""));
    assert_eq!(m.get("sys.theme"), Some(json!("dark")));
    assert_eq!(m.get("sys.errors.0.message"), Some(json!("kept")));
    assert!(
        m.get("sys.events")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "go")
    );
}

#[test]
fn a_reload_fires_config_loaded() {
    let mut m = Machine::new(app(""));
    m.reload(app(
        "[state]\nseen = 0\n[on]\n\"config.loaded\" = [{ step = \"app.seen\" }]\n",
    ));
    assert_eq!(m.get("app.seen"), Some(json!(1)));
}

#[test]
fn a_reload_hands_back_what_the_host_must_do() {
    let mut m = Machine::new(app(""));
    let effects = m.reload(app(
        "[on]\n\"config.loaded\" = [{ fire = \"oops.failed\" }]\n",
    ));
    assert_eq!(effects.len(), 1);
}
