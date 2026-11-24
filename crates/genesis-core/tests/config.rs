//! Loading an app's TOML into config types (specs 020 and 100).

use genesis_core::config::{App, Node, Step};
use serde_json::json;

const HELLO: &str = r#"
[app]
name = "Hello"
theme = "dark"
start = "home"

[state]
clicks = 0
greeting = "hi"

[[body]]
type = "cell"

[[body.children]]
type = "text"
text = "Hello"
level = 2

[[body.children]]
type = "button"
id = "hello_btn"
label = "Click me"

[on]
"hello_btn.click" = [{ step = "app.clicks", by = 1 }, "cheer"]
"*.failed" = [{ log = "failed: {{ event.name }}" }]

[actions.cheer]
do = [{ log = "clicked" }, { fire = "cheered", with = { n = 1 } }]
"#;

fn hello() -> App {
    App::from_toml(HELLO).expect("hello loads")
}

#[test]
fn the_app_table_loads_its_values() {
    let app = hello();
    assert_eq!(app.app.name, "Hello");
    assert_eq!(app.app.theme, "dark");
    assert_eq!(app.app.start, "home");
}

#[test]
fn the_app_table_fills_in_defaults() {
    let app = App::from_toml("[app]\nname = \"Bare\"\n").expect("loads");
    assert_eq!(app.app.theme, "light");
    assert_eq!(app.app.start, "main");
    assert_eq!(app.app.max_chain, 100);
    assert_eq!(app.app.event_trail, 200);
    assert!(
        app.state.is_empty() && app.on.is_empty() && app.actions.is_empty() && app.body.is_empty()
    );
}

#[test]
fn state_keeps_starting_values_with_their_types() {
    let app = hello();
    assert_eq!(app.state.get("clicks"), Some(&json!(0)));
    assert_eq!(app.state.get("greeting"), Some(&json!("hi")));
}

#[test]
fn nodes_nest_and_keep_their_ids() {
    let app = hello();
    let Node::Cell(cell) = &app.body[0] else {
        panic!("expected a cell")
    };
    assert_eq!(cell.children.len(), 2);
    let Node::Text(heading) = &cell.children[0] else {
        panic!("expected a text")
    };
    assert_eq!((heading.text.as_str(), heading.level), ("Hello", Some(2)));
    let Node::Button(button) = &cell.children[1] else {
        panic!("expected a button")
    };
    assert_eq!(button.id.as_deref(), Some("hello_btn"));
    assert_eq!(button.label, "Click me");
}

#[test]
fn handlers_keep_the_order_the_file_lists_them() {
    let app = hello();
    let patterns: Vec<&str> = app.on.iter().map(|h| h.pattern.as_str()).collect();
    assert_eq!(patterns, ["hello_btn.click", "*.failed"]);
}

#[test]
fn a_plain_string_step_runs_that_action() {
    assert_eq!(hello().on[0].steps[1], Step::Run("cheer".into()));
}

#[test]
fn a_step_step_adds_one_by_default() {
    let app =
        App::from_toml("[app]\nname = \"x\"\n[on]\n\"a.b\" = [{ step = \"app.n\" }]\n").unwrap();
    assert_eq!(
        app.on[0].steps[0],
        Step::Step {
            step: "app.n".into(),
            by: 1.0,
            min: None,
            max: None
        }
    );
}

#[test]
fn every_built_in_step_loads() {
    let toml = r#"
[app]
name = "x"
[on]
"a.b" = [
  { set = "app.x", to = 3 },
  { toggle = "app.open" },
  { go = "home" },
  { fire = "x.y" },
  { log = "hi" },
  { delay = 500, id = "t" },
  { every = 1000, fire = "tick", id = "ticker" },
  { cancel = "ticker" },
]
"#;
    let app = App::from_toml(toml).unwrap();
    let steps = &app.on[0].steps;
    assert_eq!(
        steps[0],
        Step::Set {
            set: "app.x".into(),
            to: json!(3)
        }
    );
    assert_eq!(
        steps[1],
        Step::Toggle {
            toggle: "app.open".into()
        }
    );
    assert_eq!(steps[2], Step::Go { go: "home".into() });
    assert_eq!(
        steps[3],
        Step::Fire {
            fire: "x.y".into(),
            with: Default::default()
        }
    );
    assert_eq!(steps[4], Step::Log { log: "hi".into() });
    assert_eq!(
        steps[5],
        Step::Delay {
            delay: 500,
            id: Some("t".into())
        }
    );
    assert_eq!(
        steps[6],
        Step::Every {
            every: 1000,
            fire: "tick".into(),
            id: Some("ticker".into()),
            with: Default::default()
        }
    );
    assert_eq!(
        steps[7],
        Step::Cancel {
            cancel: "ticker".into()
        }
    );
}

#[test]
fn a_custom_action_holds_its_steps() {
    let app = hello();
    let cheer = &app.actions["cheer"];
    assert_eq!(
        cheer.steps[0],
        Step::Log {
            log: "clicked".into()
        }
    );
}

#[test]
fn an_unknown_key_is_an_error_naming_the_key_and_its_line() {
    let toml =
        "[app]\nname = \"x\"\n\n[[body]]\ntype = \"button\"\nlabel = \"Go\"\ncolour = \"red\"\n";
    let errors = App::from_toml(toml).unwrap_err();
    assert!(errors[0].message.contains("colour"), "{:?}", errors);
    assert_eq!(errors[0].line, Some(7));
}

#[test]
fn an_unknown_node_type_is_an_error_naming_it() {
    let errors =
        App::from_toml("[app]\nname = \"x\"\n[[body]]\ntype = \"carousel\"\n").unwrap_err();
    assert!(errors[0].message.contains("carousel"), "{:?}", errors);
}

#[test]
fn an_unknown_step_is_an_error_naming_its_keys() {
    let errors =
        App::from_toml("[app]\nname = \"x\"\n[on]\n\"a.b\" = [{ explode = true }]\n").unwrap_err();
    assert!(errors[0].message.contains("explode"), "{:?}", errors);
}

#[test]
fn a_step_with_an_extra_key_is_an_error() {
    let errors =
        App::from_toml("[app]\nname = \"x\"\n[on]\n\"a.b\" = [{ go = \"home\", to = 1 }]\n")
            .unwrap_err();
    assert!(errors[0].message.contains("to"), "{:?}", errors);
}

#[test]
fn the_same_app_loads_from_toml_and_from_its_json_form() {
    let app = hello();
    let json = serde_json::to_string(&app).unwrap();
    let back: App = serde_json::from_str(&json).unwrap();
    assert_eq!(back, app);
}

#[test]
fn an_unknown_key_error_gives_the_keys_column() {
    let toml =
        "[app]\nname = \"x\"\n\n[[body]]\ntype = \"button\"\nlabel = \"Go\"\n   colour = \"red\"\n";
    let errors = App::from_toml(toml).unwrap_err();
    assert_eq!((errors[0].line, errors[0].column), (Some(7), Some(4)));
}

#[test]
fn an_on_that_isnt_a_table_says_what_it_should_be() {
    let errors = App::from_toml("on = 5\n[app]\nname = \"x\"\n").unwrap_err();
    assert!(
        errors[0].message.contains("a table of event patterns"),
        "{:?}",
        errors
    );
}

#[test]
fn a_text_without_a_level_is_a_paragraph() {
    let app =
        App::from_toml("[app]\nname = \"x\"\n[[body]]\ntype = \"text\"\ntext = \"Hi\"\n").unwrap();
    let Node::Text(text) = &app.body[0] else {
        panic!("expected a text")
    };
    assert_eq!(text.level, None);
}

#[test]
fn a_level_outside_one_to_three_is_a_problem_where_it_is() {
    let problems = App::from_toml(
        "[app]\nname = \"x\"\n[[body]]\ntype = \"text\"\ntext = \"Hi\"\nlevel = 4\n",
    )
    .unwrap_err();
    assert_eq!(
        problems[0].to_string(),
        "app.toml:6:1: `level` is 1, 2, or 3, not 4"
    );
}

#[test]
fn stack_and_heading_arent_types() {
    for kind in ["stack", "heading"] {
        let problems = App::from_toml(&format!(
            "[app]\nname = \"x\"\n[[body]]\ntype = \"{kind}\"\ntext = \"Hi\"\n"
        ))
        .unwrap_err();
        assert!(problems[0].message.contains(kind), "{problems:?}");
    }
}

#[test]
fn a_node_is_named_by_its_id() {
    let app = hello();
    let Node::Cell(cell) = &app.body[0] else {
        panic!("expected a cell")
    };
    assert_eq!(cell.children[1].name("body.0.children.1"), "hello_btn");
}

#[test]
fn a_node_without_an_id_is_named_by_its_place() {
    let app = hello();
    assert_eq!(
        app.body[0].name(&genesis_core::config::body_place(0)),
        "body.0"
    );
    assert_eq!(
        genesis_core::config::child_place("body.0", 1),
        "body.0.children.1"
    );
}
