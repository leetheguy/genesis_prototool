//! Sources: app variables an API fills, as a shortcut for handlers (spec 050, "Sources").

use genesis_core::bus::{Call, Effect, Machine, Reply};
use genesis_core::config::App;
use serde_json::{Value, json};

const BASE: &str = r#"
[connections.crm]
base_url = "https://pf.example.com/hooks"
[connections.crm.crud.contacts]
style = "post"

[state]
page = 1
"#;

fn machine(toml: &str) -> Machine {
    Machine::new(App::from_toml(&format!("[app]\nname = \"t\"\n{BASE}{toml}")).expect("app loads"))
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

const CONTACTS: &str = r#"
[sources.contacts]
call = "crm.contacts.list"
params = { page = "{{ app.page }}" }
select = "/rows"
"#;

#[test]
fn a_source_starts_empty_and_idle() {
    let m = machine(CONTACTS);
    assert_eq!(m.get("app.contacts"), Some(Value::Null));
    assert_eq!(m.get("sys.sources.contacts"), Some(json!("idle")));
}

#[test]
fn a_source_calls_when_the_app_opens_and_fills_its_variable_from_select() {
    let mut m = machine(CONTACTS);
    let call = calls(&m.open())[0].clone();
    assert_eq!(
        (call.op.as_str(), call.params.clone()),
        ("crm.contacts.list", json!({ "page": 1 }))
    );
    assert_eq!(m.get("sys.sources.contacts"), Some(json!("loading")));
    m.reply(
        call.id,
        Reply::Response {
            status: 200,
            body: json!({ "rows": [{ "name": "Ada" }] }),
        },
    );
    assert_eq!(m.get("app.contacts"), Some(json!([{ "name": "Ada" }])));
    assert_eq!(m.get("sys.sources.contacts"), Some(json!("ok")));
}

#[test]
fn without_select_a_source_takes_the_whole_body() {
    let mut m = machine("[sources.all]\ncall = \"crm.contacts.list\"\n");
    let call = calls(&m.open())[0].clone();
    m.reply(
        call.id,
        Reply::Response {
            status: 200,
            body: json!([1, 2]),
        },
    );
    assert_eq!(m.get("app.all"), Some(json!([1, 2])));
}

#[test]
fn a_source_calls_again_when_a_variable_in_its_params_changes() {
    let mut m = machine(&format!(
        "{CONTACTS}[on]\n\"next\" = [{{ step = \"app.page\" }}]\n"
    ));
    let first = calls(&m.open())[0].clone();
    m.reply(
        first.id,
        Reply::Response {
            status: 200,
            body: json!({ "rows": [] }),
        },
    );
    let again = calls(&m.fire("next", json!({})))[0].clone();
    assert_eq!(again.params, json!({ "page": 2 }));
}

#[test]
fn load_on_replaces_app_opened() {
    let mut m = machine(&format!("{CONTACTS}load_on = [\"auth.in\"]\n"));
    assert!(calls(&m.open()).is_empty());
    assert_eq!(calls(&m.fire("auth.in", json!({}))).len(), 1);
}

#[test]
fn an_empty_load_on_waits_for_a_reload() {
    let mut m = machine(&format!(
        "{CONTACTS}load_on = []\n[on]\n\"refresh\" = [{{ reload = [\"app.contacts\"] }}]\n"
    ));
    assert!(calls(&m.open()).is_empty());
    assert_eq!(calls(&m.fire("refresh", json!({}))).len(), 1);
}

#[test]
fn reloading_something_that_isnt_a_source_is_a_failure() {
    let text =
        format!("[app]\nname = \"t\"\n{BASE}[on]\n\"refresh\" = [{{ reload = [\"app.page\"] }}]\n");
    // Caught at load...
    let problems = format!("{:?}", App::from_toml(&text).unwrap_err());
    assert!(
        problems.contains("`reload` takes sources, as \\\"app.NAME\\\" for each [sources.NAME], and \\\"app.page\\\" isn't one"),
        "{problems}"
    );
    // ...and by the bus, for an app read without the load checks.
    let mut m = Machine::new(toml::from_str(&text).unwrap());
    m.fire("refresh", json!({}));
    assert_eq!(
        m.get("sys.errors.0.message"),
        Some(json!("app.page isn't a source"))
    );
}

#[test]
fn a_source_that_fails_says_so_loudly() {
    let mut m = machine(CONTACTS);
    let call = calls(&m.open())[0].clone();
    let effects = m.reply(call.id, Reply::Failed("refused".into()));
    assert_eq!(m.get("sys.sources.contacts"), Some(json!("failed")));
    assert_eq!(m.get("app.contacts"), Some(Value::Null));
    assert!(effects.iter().any(|e| matches!(e, Effect::Toast { .. })));
}

#[test]
fn a_source_and_an_app_variable_cant_share_a_name() {
    let toml = format!("[app]\nname = \"t\"\n{BASE}[sources.page]\ncall = \"crm.contacts.list\"\n");
    let errors = App::from_toml(&toml).unwrap_err();
    assert!(errors[0].message.contains("page"), "{errors:?}");
}

#[test]
fn a_new_config_keeps_a_sources_value_and_status() {
    let mut m = machine(CONTACTS);
    let call = calls(&m.open())[0].clone();
    m.reply(
        call.id,
        Reply::Response {
            status: 200,
            body: json!({ "rows": [1] }),
        },
    );
    m.reload(App::from_toml(&format!("[app]\nname = \"t\"\n{BASE}{CONTACTS}[sources.more]\ncall = \"crm.contacts.list\"\nload_on = []\n")).unwrap());
    assert_eq!(
        (m.get("app.contacts"), m.get("sys.sources.contacts")),
        (Some(json!([1])), Some(json!("ok")))
    );
    assert_eq!(
        (m.get("app.more"), m.get("sys.sources.more")),
        (Some(Value::Null), Some(json!("idle")))
    );
}
