//! Components (spec 090): files with a `[component]` table, used by name with a `use` node, and
//! the checks made when they load.

use genesis_core::config::{App, ConfigError, Node};
use serde_json::json;

const APP: &str = r#"
[app]
name = "Contacts"

[state]
contacts = [{ name = "Ada", email = "ada@example.com" }]
user = "ada"

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "use"
id = "card"
component = "contact_card"
props = { contact = "{{ item }}" }
"#;

const CARD: &str = r#"
[component]
props = ["contact"]
optional = ["compact"]

[state]
expanded = false

[[body]]
type = "text"
text = "{{ props.contact.name }}"
"#;

fn load(files: &[(&str, &str)]) -> Result<App, Vec<ConfigError>> {
    App::from_files(files.iter().copied())
}

fn problems(files: &[(&str, &str)]) -> String {
    load(files)
        .expect_err("refused")
        .iter()
        .map(|p| format!("{}: {}", p.file.as_deref().unwrap_or("-"), p.message))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A component file with `head` as its `[component]` table's body and `body` after it.
fn component(head: &str, body: &str) -> String {
    format!("[component]\n{head}\n{body}")
}

#[test]
fn a_component_in_any_folder_loads_by_its_file_name_with_its_props_state_and_body() {
    let app = load(&[("app.toml", APP), ("views/people/contact_card.toml", CARD)]).unwrap();
    let card = &app.components["contact_card"];
    assert_eq!(card.props, ["contact"]);
    assert_eq!(card.optional, ["compact"]);
    assert_eq!(card.state["expanded"], json!(false));
    assert!(matches!(card.body[0], Node::Text(_)));
    assert!(card.takes("compact") && !card.takes("other"));
    // Its local state is its own, not the app's.
    assert!(!app.state.contains_key("expanded"));
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
}

#[test]
fn a_use_node_loads_its_component_name_props_and_placement() {
    let app = load(&[("app.toml", APP), ("contact_card.toml", CARD)]).unwrap();
    let Node::Use(used) = &app.body[0].children()[0] else {
        panic!("not a use node")
    };
    assert_eq!(used.component, "contact_card");
    assert_eq!(used.props["contact"], json!("{{ item }}"));
    assert_eq!(app.body[0].children()[0].name("x"), "card");
}

#[test]
fn an_unknown_component_is_a_config_error() {
    let out = problems(&[("app.toml", APP)]);
    assert!(
        out.contains("app.toml: body.0.children.0: there's no component named \"contact_card\""),
        "{out}"
    );
}

#[test]
fn a_missing_required_prop_and_an_unknown_prop_are_config_errors() {
    let app = APP.replace(
        "props = { contact = \"{{ item }}\" }",
        "props = { contakt = \"{{ item }}\" }",
    );
    let out = problems(&[("app.toml", &app), ("contact_card.toml", CARD)]);
    assert!(
        out.contains("`contact_card` needs the prop `contact`"),
        "{out}"
    );
    assert!(
        out.contains("`contact_card` takes no prop `contakt`"),
        "{out}"
    );
}

#[test]
fn an_optional_prop_may_be_left_out_or_passed() {
    let app = APP.replace(
        "props = { contact = \"{{ item }}\" }",
        "props = { contact = \"{{ item }}\", compact = true }",
    );
    assert!(load(&[("app.toml", &app), ("contact_card.toml", CARD)]).is_ok());
}

#[test]
fn a_loop_of_components_is_a_config_error_once() {
    let uses = |other: &str| {
        component(
            "",
            &format!("[[body]]\ntype = \"use\"\ncomponent = \"{other}\"\n"),
        )
    };
    let (a, b) = (uses("b"), uses("a"));
    let app = "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\ncomponent = \"a\"\n";
    let out = problems(&[("app.toml", app), ("a.toml", &a), ("b.toml", &b)]);
    assert!(
        out.contains("a.toml: components use each other in a loop: a uses b uses a"),
        "{out}"
    );
    assert_eq!(out.matches("loop").count(), 1, "{out}");
    // One that uses itself, too.
    let me = uses("me");
    let out = problems(&[
        ("app.toml", app.replace("\"a\"", "\"me\"").as_str()),
        ("me.toml", &me),
    ]);
    assert!(
        out.contains("me.toml: components use each other in a loop: me uses me"),
        "{out}"
    );
}

#[test]
fn a_component_reading_app_loads_wherever_its_drawn() {
    // Read in a template and as an `each`: drawn by app.toml it gets `app.*`; drawn inside
    // another component it reads nothing there. Neither is a load problem.
    let card = component(
        "props = [\"contact\"]",
        r#"
[[body]]
type = "text"
text = "Hi {{ app.user }}"

[[body]]
type = "cell"
each = "app.contacts"
"#,
    );
    let list = component(
        "",
        "[[body]]\ntype = \"use\"\ncomponent = \"contact_card\"\nprops = { contact = { name = \"Ada\" } }\n",
    );
    let app = format!("{APP}\n[[body]]\ntype = \"use\"\ncomponent = \"list\"\n");
    let loaded = load(&[
        ("app.toml", &app),
        ("contact_card.toml", &card),
        ("list.toml", &list),
    ]);
    assert!(loaded.is_ok(), "{loaded:?}");
}

#[test]
fn a_component_never_writes_app() {
    let card = component(
        "props = [\"contact\"]",
        r#"
[[body]]
type = "button"
label = "Go"
on_click = [{ set = "app.page", to = 2 }]

[[body]]
type = "input"
var = "app.query"
"#,
    );
    let out = problems(&[("app.toml", APP), ("contact_card.toml", &card)]);
    for place in ["body.0", "body.1"] {
        assert!(
            out.contains(&format!(
                "contact_card.toml: {place}: a component reads `app.*` but never writes it"
            )),
            "{out}"
        );
    }
}

#[test]
fn sys_is_visible_inside_a_component() {
    let card = component(
        "props = [\"contact\"]",
        "[[body]]\ntype = \"text\"\ntext = \"{{ sys.theme }}\"\n",
    );
    assert!(load(&[("app.toml", APP), ("contact_card.toml", &card)]).is_ok());
}

#[test]
fn writing_props_is_a_config_error() {
    let card = component(
        "props = [\"contact\"]",
        r#"
[[body]]
type = "input"
var = "props.contact"

[[body]]
type = "button"
label = "Rename"
on_click = [{ set = "props.contact", to = "x" }]

[[body]]
type = "dialog"
open = "props.contact"
"#,
    );
    let out = problems(&[("app.toml", APP), ("contact_card.toml", &card)]);
    for place in ["body.0", "body.1", "body.2"] {
        assert!(
            out.contains(&format!(
                "contact_card.toml: {place}: `props.*` is read-only"
            )),
            "{place}: {out}"
        );
    }
    assert!(!out.contains("is an app variable"), "{out}");
}

#[test]
fn a_component_checks_its_own_body_and_props() {
    let card = component(
        "props = [\"contact\", \"contact\", \"no good\"]",
        "[[body]]\ntype = \"cell\"\nflow = \"row\"\ngrid = 2\n",
    );
    let out = problems(&[("app.toml", APP), ("contact_card.toml", &card)]);
    assert!(out.contains("the prop `contact` is named twice"), "{out}");
    assert!(out.contains("a prop is a plain name"), "{out}");
    assert!(
        out.contains("contact_card.toml: body.0: a cell lays out by `flow` or by `grid`"),
        "{out}"
    );
}

#[test]
fn a_component_file_that_does_not_read_is_one_problem_not_two() {
    let card = component(
        "props = [\"contact\"]",
        "[[body]]\ntype = \"button\"\nlabel = \"x\"\nvar = \"app.x\"\n",
    );
    let out = problems(&[("app.toml", APP), ("contact_card.toml", &card)]);
    assert!(
        out.contains("contact_card.toml: unknown field `var`"),
        "{out}"
    );
    assert!(!out.contains("no component named"), "{out}");
}

#[test]
fn a_component_with_named_controls_fills_the_form_it_is_used_in() {
    let fields = component(
        "",
        r#"
[[body]]
type = "input"
name = "email"

[[body]]
type = "button"
label = "Send"
submit = true
"#,
    );
    let inside = "[app]\nname = \"x\"\n[[body]]\ntype = \"form\"\nid = \"signup\"\n[[body.children]]\ntype = \"use\"\ncomponent = \"fields\"\n";
    assert!(load(&[("app.toml", inside), ("fields.toml", &fields)]).is_ok());
    let outside = "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\ncomponent = \"fields\"\n";
    let out = problems(&[("app.toml", outside), ("fields.toml", &fields)]);
    assert!(
        out.contains("app.toml: body.0: `fields` holds a control with `name`"),
        "{out}"
    );
    // Through another component, too.
    let wrap = component("", "[[body]]\ntype = \"use\"\ncomponent = \"fields\"\n");
    let out = problems(&[
        ("app.toml", &outside.replace("\"fields\"", "\"wrap\"")),
        ("fields.toml", &fields),
        ("wrap.toml", &wrap),
    ]);
    assert!(out.contains("`wrap` holds a control with `name`"), "{out}");
}

#[test]
fn a_component_holding_a_form_cant_be_used_in_a_form() {
    let signup = component(
        "",
        "[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"input\"\nname = \"email\"\n",
    );
    let outside = "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\ncomponent = \"signup\"\n";
    assert!(load(&[("app.toml", outside), ("signup.toml", &signup)]).is_ok());
    let inside = "[app]\nname = \"x\"\n[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"use\"\ncomponent = \"signup\"\n";
    let out = problems(&[("app.toml", inside), ("signup.toml", &signup)]);
    assert!(
        out.contains("`signup` holds a form, and a form can't hold another form"),
        "{out}"
    );
}

#[test]
fn components_go_in_files_of_their_own_and_hold_only_their_three_tables() {
    let out = problems(&[(
        "app.toml",
        "[app]\nname = \"x\"\n[components.card]\nbody = []\n",
    )]);
    assert!(out.contains("a component is a file of its own"), "{out}");
    let out = problems(&[
        ("app.toml", APP),
        (
            "contact_card.toml",
            &format!("{CARD}\n[app]\nname = \"y\"\n"),
        ),
    ]);
    assert!(
        out.contains("contact_card.toml: [app] goes only in app.toml"),
        "{out}"
    );
    let out = problems(&[
        ("app.toml", APP),
        ("contact_card.toml", &format!("{CARD}\n[actions]\n")),
    ]);
    assert!(
        out.contains("contact_card.toml: unknown field `actions`"),
        "{out}"
    );
    // A [[body]] in a file that isn't a component is still refused.
    let out = problems(&[
        ("app.toml", APP),
        ("contact_card.toml", CARD),
        ("more.toml", "[[body]]\ntype = \"divider\"\n"),
    ]);
    assert!(out.contains("more.toml: [[body]] goes only in app.toml, at the top of the configs folder, or in a component"), "{out}");
}
