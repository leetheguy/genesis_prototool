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
    let missing = "[app]\nname = \"x\"\n[state]\nbox = {}\n[[body]]\ntype = \"cell\"\neach = \"app.box.nothing\"\n";
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

#[test]
fn a_list_loading_again_keeps_its_rows_until_the_new_ones_come() {
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(SOURCED).unwrap());
        let calls: Arc<Mutex<Vec<Call>>> = Arc::default();
        let into = calls.clone();
        host.set_caller(move |call| into.lock().unwrap().push(call));
        host.open();
        let id = calls.lock().unwrap()[0].id;
        host.reply(
            id,
            Reply::Response {
                status: 200,
                body: json!([{ "name": "Ada" }]),
            },
        );

        host.fire("app.contacts.reload", json!({}));
        assert_eq!(host.get("sys.sources.contacts"), Some(json!("loading")));
        let out = draw(&host);
        assert!(out.contains(">Ada<"), "the rows it had: {out}");
        assert!(!out.contains("loading-spinner"), "{out}");
    });
}

/// People with ids, each row a form editing its person, keyed by id, and a button that takes Bob
/// (the middle row) out.
const KEYED: &str = r#"
[app]
name = "x"

[state]
people = [{ id = "a", name = "Ada" }, { id = "b", name = "Bob" }, { id = "c", name = "Cy" }]

[[body]]
type = "button"
id = "drop_bob"
label = "Drop Bob"
on_click = { set = "app.people", to = [{ id = "a", name = "Ada" }, { id = "c", name = "Cy" }] }

[[body]]
type = "cell"
each = "app.people"
key = "item.id"

[[body.children]]
type = "form"
id = "edit"
data = "item"

[[body.children.children]]
type = "input"
id = "name_box"
name = "name"
"#;

#[test]
fn a_keyed_row_is_its_records_so_a_draft_stays_with_it_when_a_row_above_goes() {
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(KEYED).unwrap());
        // Typing in Cy's row: its row is Cy's key, not its place.
        host.fire_at("name_box.change", json!({ "value": "Cyrus" }), Some("c"));
        let out = draw(&host);
        assert!(out.contains(r#"value="Cyrus""#), "{out}");
        host.fire("drop_bob.click", json!({}));
        let out = draw(&host);
        assert!(!out.contains(r#"value="Bob""#), "{out}");
        assert!(out.contains(r#"value="Ada""#), "{out}");
        assert!(
            out.contains(r#"value="Cyrus""#),
            "Cy's draft followed Cy: {out}"
        );
        assert_eq!(
            host.draft_at("edit", Some("c"), "name"),
            Some(json!("Cyrus"))
        );
    });
}

#[test]
fn a_row_without_a_key_or_sharing_one_is_a_loud_failure_in_its_place() {
    let missing = KEYED.replace(r#"{ id = "b", name = "Bob" }"#, r#"{ name = "Bob" }"#);
    let out = html(&missing);
    assert!(out.contains("alert-error"), "{out}");
    assert!(
        out.contains("each row needs a key at item.id, and row 2 has none"),
        "{out}"
    );
    let shared = KEYED.replace(
        r#"{ id = "c", name = "Cy" }"#,
        r#"{ id = "a", name = "Cy" }"#,
    );
    let out = html(&shared);
    assert!(out.contains("rows 1 and 3 share the key"), "{out}");
    assert!(out.contains("at item.id"), "{out}");
}

#[test]
fn a_list_of_text_can_be_keyed_by_the_element_itself() {
    let out = html(
        "[app]\nname = \"x\"\n[state]\ntags = [\"vip\", \"new\"]\n[[body]]\ntype = \"cell\"\neach = \"app.tags\"\nkey = \"item\"\n[[body.children]]\ntype = \"text\"\ntext = \"tag {{ item }}\"\n",
    );
    assert!(out.contains("tag vip") && out.contains("tag new"), "{out}");
}
