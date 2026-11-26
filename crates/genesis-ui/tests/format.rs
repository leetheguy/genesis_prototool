//! Formats on `text` (spec 080, "Formats"): each value its templates fill in is shown in the format,
//! and the variable keeps its raw value. Outside a browser it's US English; the browser's own
//! language, and `[app] locale`, are checked in `tests/deployed/format.mjs`.

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

fn drawn(toml: &str) -> (String, Host) {
    let toml = format!("[app]\nname = \"x\"\n{toml}\n");
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(&toml).unwrap());
        (app_view(host.clone()).to_html(), host)
    })
}

#[test]
fn each_value_in_the_text_is_shown_in_its_format_and_the_variable_keeps_its_own() {
    let (out, host) = drawn(
        r#"
[state]
paid = 1234.5
owed = 20

[[body]]
type = "text"
text = "Paid {{ app.paid }} of {{ app.owed }}"
format = { currency = "USD" }
"#,
    );
    assert!(out.contains("Paid $1,234.50 of $20.00"), "{out}");
    assert_eq!(host.get("app.paid"), Some(json!(1234.5)));
}

#[test]
fn a_value_the_format_cant_take_is_shown_as_it_is() {
    let (out, _) = drawn(
        "[state]\nprice = \"call us\"\n[[body]]\ntype = \"text\"\ntext = \"{{ app.price }}\"\nformat = { currency = \"USD\" }\n",
    );
    assert!(out.contains("call us"), "{out}");
}

#[test]
fn each_row_maps_its_own_value() {
    let (out, _) = drawn(
        r#"
[state]
deals = [{ stage = "won" }, { stage = "lost" }, { stage = "new" }]

[[body]]
type = "cell"
each = "app.deals"

[[body.children]]
type = "text"
text = "{{ item.stage }}"
format = { map = { won = "Won!", lost = "Lost" } }
"#,
    );
    assert!(
        out.contains("Won!") && out.contains("Lost") && out.contains(">new<"),
        "{out}"
    );
}

#[test]
fn a_heading_takes_a_format_too() {
    let (out, _) = drawn(
        "[state]\nname = \"ada lovelace\"\n[[body]]\ntype = \"text\"\nlevel = 1\ntext = \"{{ app.name }}\"\nformat = { case = \"title\" }\n",
    );
    assert!(out.contains(">Ada Lovelace</h1>"), "{out}");
}
