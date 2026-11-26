//! Drawing a `use` node (spec 090): its component's body in its place, reading `props.*` filled
//! from where it's used, with names under the `use` node's.

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;

const CARD: &str = r#"
[component]
props = ["contact"]
optional = ["note"]

[[body]]
type = "text"
text = "{{ props.contact.name }} <{{ props.contact.email }}>{{ props.note }}"

[[body]]
type = "button"
label = "Open {{ props.contact.name }}"
"#;

fn html(app: &str, files: &[(&str, &str)]) -> String {
    let app = format!("[app]\nname = \"x\"\n{app}");
    let mut all = vec![("app.toml", app.as_str())];
    all.extend_from_slice(files);
    let app = App::from_files(all).unwrap();
    Owner::new().with(|| app_view(Host::new(app)).to_html())
}

#[test]
fn each_use_draws_its_component_with_its_own_props() {
    let out = html(
        r#"
[state]
contacts = [{ name = "Ada", email = "ada@example.com" }, { name = "Lin", email = "lin@example.com" }]

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "use"
id = "card"
component = "contact_card"
props = { contact = "{{ item }}" }
"#,
        &[("views/contact_card.toml", CARD)],
    );
    assert!(out.contains("Ada &lt;ada@example.com&gt;"), "{out}");
    assert!(out.contains("Lin &lt;lin@example.com&gt;"), "{out}");
    assert!(
        out.contains("Open Ada") && out.contains("Open Lin"),
        "{out}"
    );
}

#[test]
fn props_read_app_state_where_the_use_node_is_and_a_missing_one_shows_empty() {
    let out = html(
        r#"
[state]
me = { name = "Ada", email = "ada@example.com" }

[[body]]
type = "use"
component = "contact_card"
props = { contact = "{{ app.me }}", note = " (you)" }

[[body]]
type = "use"
component = "contact_card"
props = { contact = "{{ app.nobody }}" }
"#,
        &[("contact_card.toml", CARD)],
    );
    assert!(out.contains("Ada &lt;ada@example.com&gt; (you)"), "{out}");
    assert!(out.contains(" &lt;&gt;"), "{out}");
}

#[test]
fn a_use_node_sits_straight_in_its_parent_unless_it_is_placed() {
    let plain = html(
        "[state]\nme = { name = \"Ada\" }\n[[body]]\ntype = \"use\"\ncomponent = \"contact_card\"\nprops = { contact = \"{{ app.me }}\" }\n",
        &[("contact_card.toml", CARD)],
    );
    assert!(plain.contains(r#"<div class="contents">"#), "{plain}");
    let placed = html(
        "[state]\nme = { name = \"Ada\" }\n[[body]]\ntype = \"use\"\ncomponent = \"contact_card\"\nspan = 2\nprops = { contact = \"{{ app.me }}\" }\n",
        &[("contact_card.toml", CARD)],
    );
    assert!(placed.contains(r#"<div class="col-span-2">"#), "{placed}");
}

#[test]
fn components_use_components_passing_props_down() {
    let row = r#"
[component]
props = ["who"]

[[body]]
type = "use"
component = "contact_card"
props = { contact = "{{ props.who }}", note = "!" }
"#;
    let out = html(
        "[state]\nme = { name = \"Ada\", email = \"a@example.com\" }\n[[body]]\ntype = \"use\"\ncomponent = \"row\"\nprops = { who = \"{{ app.me }}\" }\n",
        &[("contact_card.toml", CARD), ("row.toml", row)],
    );
    assert!(out.contains("Ada &lt;a@example.com&gt;!"), "{out}");
}

#[test]
fn a_component_has_its_own_each_and_item_starts_over_inside() {
    let tags = r#"
[component]
props = ["tags"]

[[body]]
type = "cell"
each = "item"

[[body]]
type = "text"
text = "[{{ item }}]"
"#;
    let out = html(
        "[state]\nrows = [\"r\"]\n[[body]]\ntype = \"cell\"\neach = \"app.rows\"\n[[body.children]]\ntype = \"use\"\ncomponent = \"tags\"\nprops = { tags = \"{{ item }}\" }\n",
        &[("tags.toml", tags)],
    );
    // `item` outside its own `each` names nothing inside a component.
    assert!(out.contains("[]"), "{out}");
    assert!(!out.contains("[r]"), "{out}");
}

#[test]
fn a_whole_props_table_and_a_list_prop_are_filled() {
    let show = r#"
[component]
props = ["pair"]

[[body]]
type = "text"
text = "{{ props.pair.1 }} {{ props }}"
"#;
    let out = html(
        "[state]\na = \"x\"\n[[body]]\ntype = \"use\"\ncomponent = \"show\"\nprops = { pair = [\"{{ app.a }}\", { b = \"{{ app.a }}!\" }, 3] }\n",
        &[("show.toml", show)],
    );
    assert!(
        out.contains(r#"{"b":"x!"} {"pair":["x",{"b":"x!"},3]}"#),
        "{out}"
    );
}

#[test]
fn a_component_repeats_a_list_it_is_given() {
    let list = r#"
[component]
props = ["names"]

[[body]]
type = "cell"
each = "props.names"
empty = "No one."

[[body.children]]
type = "text"
text = "<{{ item }}>"
"#;
    let out = html(
        "[state]\nwho = [\"Ada\", \"Lin\"]\n[[body]]\ntype = \"use\"\ncomponent = \"list\"\nprops = { names = \"{{ app.who }}\" }\n[[body]]\ntype = \"use\"\ncomponent = \"list\"\nprops = { names = [] }\n",
        &[("list.toml", list)],
    );
    assert!(
        out.contains("&lt;Ada&gt;") && out.contains("&lt;Lin&gt;"),
        "{out}"
    );
    assert!(out.contains("No one."), "{out}");
}

const HOME: &str = r#"
[component]

[[body]]
type = "text"
text = "Welcome home"
"#;

const CONTACTS: &str = r#"
[component]

[state]
title = "All contacts"

[[body]]
type = "text"
text = "{{ local.title }}"
"#;

const PAGES: &str = r#"
[[body]]
type = "button"
id = "to_contacts"
label = "Contacts"
on_click = { go = "contacts" }

[[body]]
type = "outlet"
default = "home"
"#;

fn outlet_host() -> Host {
    let app = format!("[app]\nname = \"x\"\n{PAGES}");
    let app = App::from_files([
        ("app.toml", app.as_str()),
        ("home.toml", HOME),
        ("contacts.toml", CONTACTS),
    ])
    .unwrap();
    Host::new(app)
}

#[test]
fn the_outlet_draws_its_default_and_then_the_component_go_names() {
    Owner::new().with(|| {
        let host = outlet_host();
        let out = app_view(host.clone()).to_html();
        assert!(out.contains("Welcome home"), "{out}");
        assert!(!out.contains("All contacts"), "{out}");

        host.fire("to_contacts.click", serde_json::json!({}));
        let out = app_view(host).to_html();
        assert!(out.contains("All contacts"), "{out}");
        assert!(!out.contains("Welcome home"), "{out}");
    });
}
