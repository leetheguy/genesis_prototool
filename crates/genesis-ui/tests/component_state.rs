//! Drawing what's inside a component (spec 090): each instance reads its own `local.*`, and a
//! form inside one shows its own draft. Clicking and typing are checked in a real browser
//! (`tests/deployed/components.mjs`); here, events go in the way the browser sends them.

use genesis_core::bus::Within;
use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

const APP: &str = r#"
[app]
name = "x"

[state]
contacts = [{ name = "Ada" }, { name = "Lin" }]

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "use"
id = "card"
component = "card"
props = { contact = "{{ item }}" }
"#;

const CARD: &str = r#"
[component]
props = ["contact"]

[state]
clicks = 0

[[body]]
type = "text"
text = "{{ props.contact.name }}: {{ local.clicks }}"

[[body]]
type = "button"
id = "more"
label = "More"
on_click = [{ step = "local.clicks" }]

[[body]]
type = "input"
id = "box"
var = "local.clicks"
"#;

fn html(before: impl FnOnce(&Host)) -> String {
    Owner::new().with(|| {
        let app = App::from_files([("app.toml", APP), ("card.toml", CARD)]).unwrap();
        let host = Host::new(app);
        before(&host);
        app_view(host).to_html()
    })
}

#[test]
fn each_instance_draws_its_own_local_state() {
    let out = html(|host| {
        let within = Within {
            name: "card".into(),
            row: Some("1".into()),
            props: json!({ "contact": { "name": "Lin" } }),
            outer: None,
        };
        host.fire_within(
            "card.more.click",
            json!({}),
            Some("1"),
            Some(within.clone()),
        );
        host.fire_within("card.more.click", json!({}), Some("1"), Some(within));
    });
    assert!(out.contains("Ada: 0"), "{out}");
    assert!(out.contains("Lin: 2"), "{out}");
    // The control bound to it shows it too.
    assert!(out.contains(r#"value="2""#), "{out}");
}
