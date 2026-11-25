//! A cell with `each` (spec 040, "Repeating: `each`"): its children once per element, and the faces
//! the `Wired` wrapper draws while its list is loading, failed, or empty.

use genesis_core::bus::{Call, Reply};
use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;
use std::sync::{Arc, Mutex};

const ROWS: &str = r#"
[app]
name = "x"

[state]
rows = [
  { name = "Ada", company = "Acme", tags = ["vip", "new"] },
  { name = "Bob", company = "Bix", tags = [] },
]

[[body]]
type = "cell"
each = "app.rows"

[[body.children]]
type = "text"
text = "{{ item.name }} at {{ item.company }}"

[[body.children]]
type = "cell"
each = "item.tags"
empty = "no tags"

[[body.children.children]]
type = "text"
text = "tag {{ item }}"
"#;

fn draw(host: &Host) -> String {
    app_view(host.clone()).to_html()
}

fn html(toml: &str) -> String {
    Owner::new().with(|| draw(&Host::new(App::from_toml(toml).unwrap())))
}

#[test]
fn each_draws_its_children_once_per_element_each_seeing_its_own_item() {
    let out = html(ROWS);
    let order: Vec<usize> = ["Ada at Acme", "tag vip", "tag new", "Bob at Bix", "no tags"]
        .iter()
        .map(|text| out.find(text).unwrap_or_else(|| panic!("{text}: {out}")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{out}");
}

#[test]
fn an_empty_or_missing_list_shows_empty_or_its_default() {
    let none = ROWS.replace(
        "rows = [\n  { name = \"Ada\", company = \"Acme\", tags = [\"vip\", \"new\"] },\n  { name = \"Bob\", company = \"Bix\", tags = [] },\n]",
        "rows = []",
    );
    assert!(html(&none).contains("Nothing here."), "{}", html(&none));
    let custom = none.replace(
        "each = \"app.rows\"",
        "each = \"app.rows\"\nempty = \"No contacts yet.\"",
    );
    assert!(html(&custom).contains("No contacts yet."));
    let missing = "[app]\nname = \"x\"\n[[body]]\ntype = \"cell\"\neach = \"app.nothing\"\n";
    assert!(html(missing).contains("Nothing here."));
}

#[test]
fn a_value_that_isnt_a_list_is_a_loud_failure_in_its_place() {
    let out = html(
        "[app]\nname = \"x\"\n[state]\nrows = { a = 1 }\n[[body]]\ntype = \"cell\"\neach = \"app.rows\"\n",
    );
    assert!(out.contains("alert-error"), "{out}");
    assert!(
        out.contains("each needs a list, and app.rows is a table"),
        "{out}"
    );
}

const SOURCED: &str = r#"
[app]
name = "x"

[connections.crm]
base_url = "http://api"

[connections.crm.ops.list]
method = "GET"
path = "/contacts"

[sources.contacts]
call = "crm.list"

[[body]]
type = "cell"
each = "app.contacts"
error = "Couldn't load the contacts."

[[body.children]]
type = "text"
text = "{{ item.name }}"
"#;

#[test]
fn a_cell_follows_its_source_through_loading_failed_and_ok() {
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(SOURCED).unwrap());
        let calls: Arc<Mutex<Vec<Call>>> = Arc::default();
        let into = calls.clone();
        host.set_caller(move |call| into.lock().unwrap().push(call));
        host.open();

        let out = draw(&host);
        assert!(out.contains("loading loading-spinner"), "loading: {out}");

        let id = calls.lock().unwrap()[0].id;
        host.reply(
            id,
            Reply::Response {
                status: 503,
                body: json!("down for upkeep"),
            },
        );
        let out = draw(&host);
        assert!(out.contains("alert-error"), "failed: {out}");
        assert!(
            out.contains("Couldn't load the contacts."),
            "the cell's own line: {out}"
        );
        assert!(
            out.contains("crm.list failed: HTTP 503: down for upkeep"),
            "the detail too: {out}"
        );

        host.fire("app.contacts.reload", json!({}));
        let id = calls.lock().unwrap()[1].id;
        host.reply(
            id,
            Reply::Response {
                status: 200,
                body: json!([{ "name": "Ada" }]),
            },
        );
        let out = draw(&host);
        assert!(out.contains(">Ada<"), "ok: {out}");
        // The earlier failure's toast may still be up; the cell itself has moved on.
        let cell = &out[..out.find("<div class=\"toast").unwrap()];
        assert!(
            !cell.contains("alert-error") && !cell.contains("loading-spinner"),
            "{cell}"
        );
    });
}

#[test]
fn item_paths_reach_into_tables_and_lists() {
    let toml = ROWS.replace("text = \"{{ item.name }} at {{ item.company }}\"", "text = \"first tag [{{ item.tags.0 }}], none [{{ item.tags.9 }}], app [{{ app.rows.1.name }}]\"");
    let out = html(&toml);
    assert!(out.contains("first tag [vip], none [], app [Bob]"), "{out}");
}
