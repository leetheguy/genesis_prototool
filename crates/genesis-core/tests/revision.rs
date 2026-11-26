//! The machine's revision (spec 010, "The state machine"): it goes up when something the
//! screen shows may have changed, and stays put when the clock moves on with nothing due, so a
//! host redraws nothing for an idle tick.

use genesis_core::bus::Machine;
use genesis_core::config::App;
use serde_json::json;

const APP: &str = r#"
[app]
name = "Ticks"
toast_ms = 1000

[state]
count = 0

[on]
"bump.click" = [{ step = "app.count" }]
"say.click" = [{ toast = "Hi." }]
"wait.click" = [{ start = "later" }]
"later.complete" = [{ step = "app.count" }]
"#;

fn machine() -> Machine {
    Machine::new(
        App::from_files([("app.toml", APP), ("timers.toml", "[later]\nms = 300\n")]).unwrap(),
    )
}

#[test]
fn an_idle_tick_leaves_the_revision_and_an_event_moves_it() {
    let mut m = machine();
    let start = m.revision();
    m.advance(50);
    m.advance(50);
    assert_eq!(m.revision(), start, "nothing was due");
    m.fire("bump.click", json!({}));
    assert!(m.revision() > start);
}

#[test]
fn a_timer_falling_due_and_a_toast_going_move_it() {
    let mut m = machine();
    m.fire("wait.click", json!({}));
    let started = m.revision();
    m.advance(100);
    assert_eq!(m.revision(), started, "the timer isn't due yet");
    m.advance(250);
    assert!(m.revision() > started, "the timer ran");
    assert_eq!(m.get("app.count"), Some(json!(1)));

    m.fire("say.click", json!({}));
    let shown = m.revision();
    m.advance(500);
    assert_eq!(m.revision(), shown, "the toast is still up");
    m.advance(600);
    assert!(m.revision() > shown, "the toast went");
}

#[test]
fn a_tap_on_a_toast_and_a_reload_move_it() {
    let mut m = machine();
    m.fire("say.click", json!({}));
    let id = m.get("sys.toasts.0.id").and_then(|v| v.as_u64()).unwrap();
    let before = m.revision();
    m.dismiss(id);
    assert!(m.revision() > before);
    let before = m.revision();
    m.reload(App::from_files([("app.toml", APP), ("timers.toml", "[later]\nms = 300\n")]).unwrap());
    assert!(m.revision() > before);
}
