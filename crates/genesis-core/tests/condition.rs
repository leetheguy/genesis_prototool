//! Conditions (spec 080): each test, the combinators, a wrong type being false, and what a
//! condition's `var` may read where it's written.

use genesis_core::condition::Condition;
use genesis_core::config::App;
use serde_json::{Value, json};

fn condition(value: Value) -> Condition {
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{value}: {e}"))
}

fn refused(value: Value) -> String {
    serde_json::from_value::<Condition>(value)
        .unwrap_err()
        .to_string()
}

/// Whether the condition holds with `app.x` set to `x` (`None`: missing).
fn holds(c: &Condition, x: Option<Value>) -> bool {
    c.holds(&|path: &str| if path == "app.x" { x.clone() } else { None })
}

#[test]
fn each_test_passes_and_fails() {
    let cases = [
        (json!({ "var": "app.x", "is": "a" }), json!("a"), json!("b")),
        (json!({ "var": "app.x", "is": 1 }), json!(1.0), json!(2)),
        (
            json!({ "var": "app.x", "in": ["a", "b"] }),
            json!("b"),
            json!("c"),
        ),
        (
            json!({ "var": "app.x", "empty": true }),
            json!(""),
            json!("a"),
        ),
        (
            json!({ "var": "app.x", "empty": false }),
            json!([1]),
            json!([]),
        ),
        (json!({ "var": "app.x", "gt": 2 }), json!(3), json!(2)),
        (json!({ "var": "app.x", "gte": 2 }), json!(2), json!(1)),
        (json!({ "var": "app.x", "lt": 2 }), json!(1.5), json!(2)),
        (json!({ "var": "app.x", "lte": 2 }), json!(2), json!(3)),
        (
            json!({ "var": "app.x", "matches": "^[^@]+@[^@]+$" }),
            json!("a@b"),
            json!("ab"),
        ),
    ];
    for (written, pass, fail) in cases {
        let c = condition(written.clone());
        assert!(holds(&c, Some(pass.clone())), "{written} on {pass}");
        assert!(!holds(&c, Some(fail.clone())), "{written} on {fail}");
        // It's written back as it was read, so it survives the trip to the browser.
        assert_eq!(condition(Value::from(c.clone())), c);
    }
}

#[test]
fn missing_and_null_are_empty() {
    let c = condition(json!({ "var": "app.x", "empty": true }));
    assert!(holds(&c, None) && holds(&c, Some(Value::Null)) && holds(&c, Some(json!({}))));
}

#[test]
fn a_test_on_the_wrong_type_is_false_never_an_error() {
    let not_numbers = [
        json!("3"),
        json!(true),
        json!([1]),
        json!({ "a": 1 }),
        Value::Null,
    ];
    for written in ["gt", "gte", "lt", "lte"].map(|t| json!({ "var": "app.x", t: 0 })) {
        let c = condition(written.clone());
        for value in &not_numbers {
            assert!(!holds(&c, Some(value.clone())), "{written} on {value}");
        }
        assert!(!holds(&c, None), "{written} on nothing");
    }
    let c = condition(json!({ "var": "app.x", "matches": "." }));
    for value in [json!(1), json!(true), json!(["a"]), Value::Null] {
        assert!(!holds(&c, Some(value.clone())), "matches on {value}");
    }
    // "1" isn't 1.
    assert!(!holds(
        &condition(json!({ "var": "app.x", "is": "1" })),
        Some(json!(1))
    ));
}

#[test]
fn all_any_and_not_combine() {
    let c = condition(json!({ "all": [
        { "var": "app.x", "gt": 0 },
        { "any": [{ "var": "app.x", "lt": 5 }, { "var": "app.x", "is": 10 }] },
        { "not": { "var": "app.x", "is": 3 } },
    ] }));
    let at = |n: i64| holds(&c, Some(json!(n)));
    assert!(at(1) && at(4) && at(10));
    assert!(!at(0) && !at(3) && !at(7));
    assert_eq!(c.tests().len(), 4);
}

#[test]
fn status_reads_the_sources_status() {
    let c = condition(json!({ "var": "app.contacts", "status": "loading" }));
    let now = |status: &'static str| {
        move |path: &str| (path == "sys.sources.contacts").then(|| json!(status))
    };
    assert!(c.holds(&now("loading")));
    assert!(!c.holds(&now("ok")));
}

#[test]
fn a_badly_written_condition_says_why() {
    let cases = [
        (json!("app.x"), "a condition is a table"),
        (json!({ "var": "app.x" }), "makes one test"),
        (
            json!({ "var": "app.x", "is": 1, "gt": 0 }),
            "combine them with `all`",
        ),
        (
            json!({ "var": "app.x", "near": 1 }),
            "there's no test `near`",
        ),
        (json!({ "var": "app.x", "gt": "2" }), "`gt` takes a number"),
        (json!({ "var": "app.x", "in": "a" }), "`in` takes a list"),
        (
            json!({ "var": "app.x", "empty": 1 }),
            "`empty` takes true or false",
        ),
        (json!({ "var": "app.x", "matches": "(" }), "isn't one"),
        (
            json!({ "var": "app.x", "status": "busy" }),
            "`status` is one of",
        ),
        (json!({ "all": [] }), "one or more conditions"),
        (
            json!({ "not": { "var": "app.x", "is": 1 }, "var": "app.y" }),
            "stands alone",
        ),
        (json!({ "is": 1 }), "names a variable with `var`"),
    ];
    for (written, why) in cases {
        let said = refused(written.clone());
        assert!(said.contains(why), "{written}: {said}");
    }
}

fn problem(files: &[(&str, &str)]) -> String {
    App::from_files(files.iter().copied())
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

const HEAD: &str = "[app]\nname = \"x\"\n\n[state]\nview = \"a\"\n";

#[test]
fn show_and_disabled_load_where_they_belong() {
    let app = format!(
        r#"{HEAD}
[connections.crm]
base_url = "https://example.com"

[connections.crm.crud.contacts]

[sources.contacts]
call = "crm.contacts.list"

[[body]]
type = "text"
text = "Loading"
show = {{ var = "app.contacts", status = "loading" }}

[[body]]
type = "form"
id = "f"

[[body.children]]
type = "input"
name = "email"

[[body.children]]
type = "button"
label = "Save"
submit = true
disabled = {{ any = [{{ var = "form.email", empty = true }}, {{ var = "sys.theme", is = "dark" }}] }}
"#
    );
    let loaded = App::from_toml(&app);
    assert!(loaded.is_ok(), "{loaded:?}");
}

#[test]
fn a_condition_reads_only_what_it_can_see() {
    let cases = [
        (
            "show = { var = \"view\", is = \"a\" }",
            "isn't a variable's path",
        ),
        (
            "show = { var = \"app.\", is = \"a\" }",
            "isn't a variable's path",
        ),
        (
            "show = { var = \"event.x\", is = 1 }",
            "a condition reads `app.*`",
        ),
        (
            "show = { var = \"local.open\", is = true }",
            "works only inside one",
        ),
        (
            "show = { var = \"props.x\", is = true }",
            "works only inside one",
        ),
        (
            "show = { var = \"form.email\", empty = true }",
            "works only inside a `form`",
        ),
        (
            "show = { var = \"app.view\", status = \"ok\" }",
            "tests a source",
        ),
    ];
    for (line, why) in cases {
        let app = format!("{HEAD}\n[[body]]\ntype = \"text\"\ntext = \"hi\"\n{line}\n");
        let said = problem(&[("app.toml", &app)]);
        assert!(said.contains(why), "{line}: {said}");
    }
}

#[test]
fn disabled_goes_only_on_buttons_and_controls() {
    let app = format!(
        "{HEAD}\n[[body]]\ntype = \"text\"\ntext = \"hi\"\ndisabled = {{ var = \"app.view\", is = \"a\" }}\n"
    );
    assert!(problem(&[("app.toml", &app)]).contains("disabled"));
}

#[test]
fn a_component_reads_its_own_locals_and_props_in_a_condition() {
    let card = r#"
[component]
props = ["contact"]

[state]
open = false

[[body]]
type = "text"
text = "{{ props.contact.name }}"
show = { all = [{ var = "local.open", is = true }, { var = "props.contact.name", empty = false }] }
"#;
    let app = format!(
        "{HEAD}\n[[body]]\ntype = \"use\"\ncomponent = \"card\"\nprops = {{ contact = {{ name = \"Ada\" }} }}\n"
    );
    let loaded = App::from_files([("app.toml", app.as_str()), ("card.toml", card)]);
    assert!(loaded.is_ok(), "{loaded:?}");
}

#[test]
fn a_component_whose_condition_reads_app_loads_where_a_use_node_draws_it() {
    let card = r#"
[component]

[[body]]
type = "text"
text = "hi"
show = { var = "app.view", is = "a" }
"#;
    let app = format!("{HEAD}\n[[body]]\ntype = \"use\"\ncomponent = \"card\"\n");
    let loaded = App::from_files([("app.toml", app.as_str()), ("card.toml", card)]);
    assert!(loaded.is_ok(), "{loaded:?}");
}
