//! Connections and their operations, crud included (spec 030).

use genesis_core::config::{App, Method};
use serde_json::json;

fn app(toml: &str) -> App {
    App::from_toml(&format!("[app]\nname = \"t\"\n{toml}")).expect("app loads")
}

const CRM: &str = r#"
[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }
timeout = 10

[connections.crm.ops.score_job]
method = "POST"
path = "/job-score"
headers = { "X-Extra" = "1" }
timeout = 0

[connections.crm.crud.contacts]
style = "post"

[connections.api]
base_url = "https://api.example.com/v1"

[connections.api.crud.people]
key = "person_id"

[connections.api.crud.orders]
path = "/shop/orders"

[connections.api.crud.orders.update]
method = "PUT"
path = "/shop/orders/{id}/replace"
"#;

#[test]
fn an_operation_joins_its_path_onto_the_connections_base_url() {
    let op = app(CRM).operation("crm.score_job").expect("declared");
    assert_eq!(
        (op.method, op.url.as_str()),
        (Method::Post, "https://purple.example.com/hooks/job-score")
    );
}

#[test]
fn an_operations_headers_merge_over_its_connections() {
    let op = app(CRM).operation("crm.score_job").unwrap();
    assert_eq!(
        op.headers.get("Authorization"),
        Some(&"Bearer {{ creds.PF_TOKEN }}".to_string())
    );
    assert_eq!(op.headers.get("X-Extra"), Some(&"1".to_string()));
}

#[test]
fn timeouts_default_to_thirty_seconds_and_an_operations_beats_its_connections() {
    let crm = app(CRM);
    assert_eq!(crm.operation("crm.score_job").unwrap().timeout, 0);
    assert_eq!(crm.operation("crm.contacts.list").unwrap().timeout, 10);
    assert_eq!(crm.operation("api.people.list").unwrap().timeout, 30);
}

#[test]
fn rest_crud_expands_to_the_five_rest_operations() {
    let api = app(CRM);
    let shape = |name: &str| api.operation(name).map(|op| (op.method, op.url));
    assert_eq!(
        shape("api.people.list"),
        Some((Method::Get, "https://api.example.com/v1/people".into()))
    );
    assert_eq!(
        shape("api.people.get"),
        Some((
            Method::Get,
            "https://api.example.com/v1/people/{person_id}".into()
        ))
    );
    assert_eq!(
        shape("api.people.create"),
        Some((Method::Post, "https://api.example.com/v1/people".into()))
    );
    assert_eq!(
        shape("api.people.update"),
        Some((
            Method::Patch,
            "https://api.example.com/v1/people/{person_id}".into()
        ))
    );
    assert_eq!(
        shape("api.people.delete"),
        Some((
            Method::Delete,
            "https://api.example.com/v1/people/{person_id}".into()
        ))
    );
}

#[test]
fn post_crud_expands_to_five_posts_named_after_the_resource() {
    let crm = app(CRM);
    for action in ["list", "get", "create", "update", "delete"] {
        let op = crm.operation(&format!("crm.contacts.{action}")).unwrap();
        assert_eq!(
            (op.method, op.url),
            (
                Method::Post,
                format!("https://purple.example.com/hooks/contacts-{action}")
            )
        );
    }
}

#[test]
fn crud_takes_a_path_and_a_per_operation_override() {
    let api = app(CRM);
    assert_eq!(
        api.operation("api.orders.list").unwrap().url,
        "https://api.example.com/v1/shop/orders"
    );
    let update = api.operation("api.orders.update").unwrap();
    assert_eq!(
        (update.method, update.url.as_str()),
        (
            Method::Put,
            "https://api.example.com/v1/shop/orders/{id}/replace"
        )
    );
}

#[test]
fn post_crud_takes_a_prefix() {
    let crm = app(
        "[connections.pf]\nbase_url = \"http://pf/hooks\"\n[connections.pf.crud.contacts]\nstyle = \"post\"\nprefix = \"people\"\n",
    );
    assert_eq!(
        crm.operation("pf.contacts.get").unwrap().url,
        "http://pf/hooks/people-get"
    );
}

#[test]
fn an_operation_nobody_declared_is_none() {
    assert!(app(CRM).operation("crm.nowhere").is_none());
    assert!(app(CRM).operation("nowhere.at_all").is_none());
}

#[test]
fn every_operation_is_listed_by_name() {
    let names = app(CRM).operation_names();
    assert!(
        names.contains(&"crm.score_job".to_string())
            && names.contains(&"api.orders.delete".to_string())
    );
    assert_eq!(names.len(), 16);
}

#[test]
fn a_method_is_any_http_method_in_any_case_and_nothing_else() {
    let ok = App::from_toml("[app]\nname = \"t\"\n[connections.a]\nbase_url = \"x\"\n[connections.a.ops.o]\nmethod = \"head\"\npath = \"/\"\n").unwrap();
    assert_eq!(ok.operation("a.o").unwrap().method, Method::Head);
    let bad = App::from_toml(
        "[app]\nname = \"t\"\n[connections.a]\nbase_url = \"x\"\n[connections.a.ops.o]\nmethod = \"FETCH\"\npath = \"/\"\n",
    );
    assert!(bad.unwrap_err()[0].message.contains("FETCH"));
}

#[test]
fn the_round_trip_through_json_keeps_connections() {
    let crm = app(CRM);
    let back: App = serde_json::from_value(serde_json::to_value(&crm).unwrap()).unwrap();
    assert_eq!(back, crm);
    assert_eq!(
        serde_json::to_value(&crm).unwrap()["connections"]["crm"]["crud"]["contacts"]["style"],
        json!("post")
    );
}

#[test]
fn a_crud_key_is_id_by_default() {
    assert_eq!(
        app(CRM).operation("api.orders.delete").unwrap().url,
        "https://api.example.com/v1/shop/orders/{id}"
    );
}
