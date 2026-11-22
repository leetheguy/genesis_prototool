//! What a session keeps across a reload of the browser, and what it fetches fresh (spec 050, "Persisting").

use genesis_core::bus::{Call, Effect, Machine, Reply};
use genesis_core::config::App;
use serde_json::{Value, json};

const APP: &str = r#"
[app]
name = "t"
forget = ["scratch"]

[state]
clicks = 0
picked = ""
scratch = 0

[connections.api]
base_url = "http://api"
[connections.api.ops.get]
method = "GET"
path = "/x"

[sources.rows]
call = "api.get"
load_on = []

[on]
"click" = [{ step = "app.clicks" }, { step = "app.scratch" }]
"pick" = [{ call = "api.get", into = { "app.picked" = "/name" } }]
"pick_by_hand" = [{ set = "app.picked", to = "Grace" }]
"elsewhere" = [{ go = "other" }, { set = "sys.theme", to = "dark" }]
"#;

fn machine() -> Machine {
    Machine::new(App::from_toml(APP).unwrap())
}

fn first_call(effects: &[Effect]) -> Call {
    effects
        .iter()
        .find_map(|e| {
            if let Effect::Call(c) = e {
                Some(c.clone())
            } else {
                None
            }
        })
        .unwrap()
}

#[test]
fn what_the_user_did_is_kept() {
    let mut m = machine();
    m.fire("click", json!({}));
    m.fire("elsewhere", json!({}));
    let kept = m.kept();
    assert_eq!(kept["app"]["clicks"], 1);
    assert_eq!(
        (&kept["sys"]["view"], &kept["sys"]["theme"]),
        (&json!("other"), &json!("dark"))
    );
    assert!(
        kept["sys"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "click")
    );
}

#[test]
fn sources_and_forgotten_variables_arent_kept() {
    let mut m = machine();
    m.fire("click", json!({}));
    let kept = m.kept();
    assert_eq!(kept["app"].get("rows"), None);
    assert_eq!(kept["app"].get("scratch"), None);
}

#[test]
fn what_a_reply_filled_isnt_kept_until_the_user_sets_it() {
    let mut m = machine();
    let call = first_call(&m.fire("pick", json!({})));
    m.reply(
        call.id,
        Reply::Response {
            status: 200,
            body: json!({ "name": "Ada" }),
        },
    );
    assert_eq!(m.kept()["app"].get("picked"), None);
    m.fire("pick_by_hand", json!({}));
    assert_eq!(m.kept()["app"]["picked"], "Grace");
}

#[test]
fn restoring_brings_it_all_back() {
    let mut first = machine();
    first.fire("click", json!({}));
    first.fire("elsewhere", json!({}));
    first.fire("x.failed", json!({ "error": "kept" }));
    let mut again = machine();
    again.restore(&first.kept());
    assert_eq!(again.get("app.clicks"), Some(json!(1)));
    assert_eq!(
        (again.get("sys.view"), again.get("sys.theme")),
        (Some(json!("other")), Some(json!("dark")))
    );
    assert_eq!(again.get("sys.errors.0.message"), Some(json!("kept")));
    assert!(
        again
            .get("sys.events")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "click")
    );
}

#[test]
fn restoring_drops_what_the_config_no_longer_has_or_now_types_differently() {
    let mut m = machine();
    let notes =
        m.restore(&json!({ "app": { "clicks": "three", "gone": 1, "picked": "Ada" }, "sys": {} }));
    assert_eq!(m.get("app.clicks"), Some(json!(0)));
    assert_eq!(m.get("app.gone"), None);
    assert_eq!(m.get("app.picked"), Some(json!("Ada")));
    assert_eq!(notes.len(), 2, "{notes:?}");
}

#[test]
fn restoring_never_touches_sources_or_forgotten_variables() {
    let mut m = machine();
    m.restore(&json!({ "app": { "rows": [1], "scratch": 9 } }));
    assert_eq!(
        (m.get("app.rows"), m.get("app.scratch")),
        (Some(Value::Null), Some(json!(0)))
    );
}

#[test]
fn restoring_something_that_isnt_kept_state_is_a_note_not_a_crash() {
    let mut m = machine();
    let notes = m.restore(&json!("garbage"));
    assert_eq!(notes.len(), 1);
    assert_eq!(m.get("app.clicks"), Some(json!(0)));
}
