//! The call step: a call effect out, a reply in, and its events (specs 030 and 100).

use genesis_core::bus::{Call, Effect, Machine, Reply};
use genesis_core::config::App;
use serde_json::{Value, json};

const API: &str = r#"
[connections.crm]
base_url = "https://pf.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}", "X-User" = "{{ app.user }}" }

[connections.crm.crud.contacts]
style = "post"
"#;

fn machine(toml: &str) -> Machine {
    Machine::new(
        App::from_toml(&format!(
            "[app]\nname = \"t\"\n[state]\nuser = \"ada\"\nn = 0\nrows = []\n{API}{toml}"
        ))
        .expect("app loads"),
    )
}

fn calls(effects: &[Effect]) -> Vec<Call> {
    effects
        .iter()
        .filter_map(|e| {
            if let Effect::Call(call) = e {
                Some(call.clone())
            } else {
                None
            }
        })
        .collect()
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

fn ok(body: Value) -> Reply {
    Reply::Response { status: 200, body }
}

#[test]
fn a_call_hands_the_host_the_operation_its_params_and_its_headers() {
    let mut m = machine(
        "[on]\n\"go\" = [{ call = \"crm.contacts.get\", params = { id = \"{{ event.id }}\" } }]\n",
    );
    let call = calls(&m.fire("go", json!({ "id": 7 })))[0].clone();
    assert_eq!(
        (call.op.as_str(), call.params.clone()),
        ("crm.contacts.get", json!({ "id": 7 }))
    );
    assert_eq!(call.headers.get("X-User").map(String::as_str), Some("ada"));
    assert_eq!(
        call.headers.get("Authorization").map(String::as_str),
        Some("Bearer {{ creds.PF_TOKEN }}"),
        "creds are serve's to fill"
    );
}

#[test]
fn the_list_waits_for_the_reply_then_carries_on() {
    let mut m =
        machine("[on]\n\"go\" = [{ call = \"crm.contacts.list\" }, { step = \"app.n\" }]\n");
    let call = calls(&m.fire("go", json!({})))[0].clone();
    assert_eq!(m.get("app.n"), Some(json!(0)));
    m.reply(call.id, ok(json!({ "rows": [] })));
    assert_eq!(m.get("app.n"), Some(json!(1)));
}

#[test]
fn a_good_reply_fires_ok_and_its_status_with_the_body() {
    let mut m = machine("[on]\n\"go\" = [{ call = \"crm.contacts.list\" }]\n");
    let call = calls(&m.fire("go", json!({})))[0].clone();
    m.reply(
        call.id,
        Reply::Response {
            status: 201,
            body: json!({ "made": true }),
        },
    );
    assert_eq!(
        names(&m),
        ["go", "crm.contacts.list.ok", "crm.contacts.list.201"]
    );
    assert_eq!(
        m.get("sys.events.0.payload"),
        Some(json!({ "status": 201, "body": { "made": true } }))
    );
}

#[test]
fn into_fills_variables_from_the_reply() {
    let mut m = machine(
        "[on]\n\"go\" = [{ call = \"crm.contacts.list\", into = { \"app.rows\" = \"/rows\", \"app.n\" = \"/count\" } }]\n",
    );
    let call = calls(&m.fire("go", json!({})))[0].clone();
    m.reply(
        call.id,
        ok(json!({ "rows": [{ "name": "Ada" }], "count": 1 })),
    );
    assert_eq!(
        (m.get("app.rows"), m.get("app.n")),
        (Some(json!([{ "name": "Ada" }])), Some(json!(1)))
    );
}

#[test]
fn an_empty_pointer_takes_the_whole_body() {
    let mut m = machine(
        "[on]\n\"go\" = [{ call = \"crm.contacts.list\", into = { \"app.rows\" = \"\" } }]\n",
    );
    let call = calls(&m.fire("go", json!({})))[0].clone();
    m.reply(call.id, ok(json!([1, 2])));
    assert_eq!(m.get("app.rows"), Some(json!([1, 2])));
}

#[test]
fn into_finding_nothing_is_a_failure() {
    let mut m = machine(
        "[on]\n\"go\" = [{ call = \"crm.contacts.list\", into = { \"app.rows\" = \"/nope\" } }, { step = \"app.n\" }]\n",
    );
    let call = calls(&m.fire("go", json!({})))[0].clone();
    m.reply(call.id, ok(json!({ "rows": [] })));
    assert_eq!(m.get("app.n"), Some(json!(0)));
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!("nothing at /nope in the reply to crm.contacts.list"))
    );
}

#[test]
fn event_names_a_branch_from_the_reply() {
    let mut m = machine("[on]\n\"go\" = [{ call = \"crm.contacts.get\", event = \"/outcome\" }]\n");
    let call = calls(&m.fire("go", json!({})))[0].clone();
    m.reply(call.id, ok(json!({ "outcome": "needs_review" })));
    assert!(
        names(&m).contains(&"crm.contacts.get.needs_review".to_string()),
        "{:?}",
        names(&m)
    );
}

#[test]
fn a_bad_status_fires_failed_and_its_status_and_stops_the_list() {
    let mut m = machine("[on]\n\"go\" = [{ call = \"crm.contacts.get\" }, { step = \"app.n\" }]\n");
    let call = calls(&m.fire("go", json!({})))[0].clone();
    let effects = m.reply(
        call.id,
        Reply::Response {
            status: 404,
            body: json!({ "error": "no such contact" }),
        },
    );
    assert_eq!(
        names(&m),
        ["go", "crm.contacts.get.failed", "crm.contacts.get.404"]
    );
    assert_eq!(m.get("app.n"), Some(json!(0)));
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!(r#"HTTP 404: {"error":"no such contact"}"#))
    );
    assert_eq!(effects.len(), 1, "an unhandled failure toasts once");
}

#[test]
fn a_call_that_never_got_a_response_fails_with_why() {
    let mut m = machine("[on]\n\"go\" = [{ call = \"crm.contacts.get\" }]\n");
    let call = calls(&m.fire("go", json!({})))[0].clone();
    m.reply(call.id, Reply::Failed("timed out after 30 seconds".into()));
    assert_eq!(names(&m), ["go", "crm.contacts.get.failed"]);
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!("timed out after 30 seconds"))
    );
}

#[test]
fn a_failed_call_inside_an_action_fails_the_action_without_logging_twice() {
    let mut m = machine(
        "[on]\n\"go\" = [\"load\"]\n[actions.load]\ndo = [{ call = \"crm.contacts.get\" }]\n",
    );
    let call = calls(&m.fire("go", json!({})))[0].clone();
    m.reply(call.id, Reply::Failed("refused".into()));
    assert!(names(&m).contains(&"load.failed".to_string()));
    let errors: Vec<Value> = m
        .get("sys.errors")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].clone())
        .collect();
    assert_eq!(
        errors,
        [json!("load.failed"), json!("crm.contacts.get.failed")]
    );
}

#[test]
fn a_call_to_an_operation_nobody_declared_fails_and_calls_nothing() {
    let mut m = machine("[on]\n\"go\" = [{ call = \"crm.nowhere\" }]\n");
    let effects = m.fire("go", json!({}));
    assert!(calls(&effects).is_empty());
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!("there's no operation named crm.nowhere"))
    );
}

#[test]
fn a_missing_param_value_fails_and_calls_nothing() {
    let mut m = machine(
        "[on]\n\"go\" = [{ call = \"crm.contacts.get\", params = { id = \"{{ event.id }}\" } }]\n",
    );
    assert!(calls(&m.fire("go", json!({}))).is_empty());
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!("event.id isn't set"))
    );
}

#[test]
fn a_reply_to_no_call_is_logged() {
    let mut m = machine("");
    m.reply(99, ok(json!({})));
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!("a reply came for call 99, which isn't waiting"))
    );
}

#[test]
fn each_call_gets_its_own_id() {
    let mut m = machine("[on]\n\"go\" = [{ call = \"crm.contacts.list\" }]\n");
    let first = calls(&m.fire("go", json!({})))[0].id;
    let second = calls(&m.fire("go", json!({})))[0].id;
    assert_ne!(first, second);
}

#[test]
fn every_2xx_status_is_a_success_and_nothing_else_is() {
    for (status, outcome) in [(199, "failed"), (200, "ok"), (299, "ok"), (300, "failed")] {
        let mut m = machine("[on]\n\"go\" = [{ call = \"crm.contacts.list\" }]\n");
        let call = calls(&m.fire("go", json!({})))[0].clone();
        m.reply(
            call.id,
            Reply::Response {
                status,
                body: Value::Null,
            },
        );
        assert!(
            names(&m).contains(&format!("crm.contacts.list.{outcome}")),
            "{status}: {:?}",
            names(&m)
        );
    }
}
