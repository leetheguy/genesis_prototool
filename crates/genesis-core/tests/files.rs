//! An app from many files (spec 020, "Top level" and "Checks"; spec 120, "One app").

use genesis_core::config::{App, ConfigError};
use serde_json::json;

const APP: &str = r#"[app]
name = "CRM"

[state]
page = 1

[[body]]
type = "text"
text = "Contacts"

[on]
"app.opened" = [{ log = "from app.toml" }]
"#;

const API: &str = r#"[connections.crm]
base_url = "https://api.example.com"

[connections.crm.ops.ping]
method = "GET"
path = "/ping"

[sources.contacts]
call = "crm.ping"
"#;

const MORE: &str = r#"[state]
selected = ""

[actions.refresh]
do = [{ reload = ["app.contacts"] }]

[on]
"app.opened" = [{ log = "from more.toml" }]
"#;

fn messages(problems: &[ConfigError]) -> Vec<String> {
    problems.iter().map(ToString::to_string).collect()
}

#[test]
fn an_app_from_several_files_in_several_folders_loads_as_one() {
    let app = App::from_files([
        ("app.toml", APP),
        ("crm/api.toml", API),
        ("views/deep/more.toml", MORE),
    ])
    .unwrap();
    assert_eq!(app.app.name, "CRM");
    assert_eq!(app.state.get("page"), Some(&json!(1)));
    assert_eq!(app.state.get("selected"), Some(&json!("")));
    assert_eq!(app.body.len(), 1);
    assert!(app.connections.contains_key("crm"));
    assert!(app.sources.contains_key("contacts"));
    assert!(app.actions.contains_key("refresh"));
}

#[test]
fn moving_a_file_to_another_folder_changes_nothing() {
    let here = App::from_files([("app.toml", APP), ("crm/api.toml", API)]).unwrap();
    let there = App::from_files([("app.toml", APP), ("elsewhere/deeper/api.toml", API)]).unwrap();
    assert_eq!(here, there);
}

#[test]
fn handlers_for_one_event_in_two_files_both_run_app_tomls_first() {
    let app = App::from_files([("views/deep/more.toml", MORE), ("app.toml", APP)]).unwrap();
    let patterns: Vec<&str> = app.on.iter().map(|h| h.pattern.as_str()).collect();
    assert_eq!(patterns, ["app.opened", "app.opened"]);
    assert_eq!(
        serde_json::to_value(&app.on[0].steps).unwrap(),
        json!([{ "log": "from app.toml" }])
    );
}

#[test]
fn files_other_than_app_toml_add_their_handlers_in_path_order() {
    let a = r#"[on]
"x" = [{ log = "a" }]
"#;
    let b = r#"[on]
"x" = [{ log = "b" }]
"#;
    let app =
        App::from_files([("b/second.toml", b), ("app.toml", APP), ("a/first.toml", a)]).unwrap();
    let logs: Vec<String> = app
        .on
        .iter()
        .skip(1)
        .map(|h| serde_json::to_value(&h.steps).unwrap()[0]["log"].to_string())
        .collect();
    assert_eq!(logs, [r#""a""#, r#""b""#]);
}

#[test]
fn two_files_with_one_name_in_different_folders_are_a_problem_naming_both_paths() {
    let problems = App::from_files([
        ("app.toml", APP),
        ("views/review.toml", "[state]\na = 1\n"),
        ("old/review.toml", "[state]\nb = 1\n"),
    ])
    .unwrap_err();
    assert_eq!(
        messages(&problems),
        [
            "views/review.toml: the file name review.toml is also old/review.toml; give one of them another name"
        ]
    );
}

#[test]
fn the_same_definition_in_two_files_is_a_problem_naming_both_places() {
    // Each place is where the name itself is written: a [state] key, or the name in a table's header.
    let cases = [
        (
            "[state]\npage = 2\n",
            "z/twice.toml:2:1",
            "app.page",
            "app.toml:5:1",
        ),
        (
            "[connections.crm]\nbase_url = \"http://x\"\n",
            "z/twice.toml:1:14",
            "connection crm",
            "crm/api.toml:1:14",
        ),
        (
            "[sources.contacts]\ncall = \"crm.ping\"\n",
            "z/twice.toml:1:10",
            "source contacts",
            "crm/api.toml:8:10",
        ),
        (
            "[actions.refresh]\ndo = []\n",
            "z/twice.toml:1:10",
            "action refresh",
            "views/deep/more.toml:4:10",
        ),
    ];
    for (twice, second, what, first) in cases {
        let problems = App::from_files([
            ("app.toml", APP),
            ("crm/api.toml", API),
            ("views/deep/more.toml", MORE),
            ("z/twice.toml", twice),
        ])
        .unwrap_err();
        assert_eq!(
            messages(&problems),
            [format!(
                "{second}: {what} is defined twice; it's also at {first}"
            )],
            "{what}"
        );
    }
}

#[test]
fn a_state_variable_and_a_source_with_one_name_in_two_files_are_a_problem() {
    let problems = App::from_files([
        ("app.toml", APP),
        ("crm/api.toml", API),
        ("more.toml", "[state]\ncontacts = []\n"),
    ])
    .unwrap_err();
    assert_eq!(
        messages(&problems),
        [
            "more.toml:2:1: app.contacts is both a [state] variable and a source (at crm/api.toml:8:10); give one another name"
        ]
    );
}

#[test]
fn app_outside_app_toml_is_a_problem_where_it_is() {
    let problems = App::from_files([
        ("app.toml", APP),
        (
            "views/extra.toml",
            "[state]\nx = 1\n\n[app]\nname = \"Two\"\n",
        ),
    ])
    .unwrap_err();
    assert_eq!(
        messages(&problems),
        ["views/extra.toml:4:1: [app] goes only in app.toml, at the top of the configs folder"]
    );
}

#[test]
fn body_outside_app_toml_is_a_problem_where_it_is() {
    let problems = App::from_files([
        ("app.toml", APP),
        (
            "views/extra.toml",
            "[[body]]\ntype = \"text\"\ntext = \"hi\"\n",
        ),
    ])
    .unwrap_err();
    assert_eq!(
        messages(&problems),
        ["views/extra.toml:1:1: [[body]] goes only in app.toml, at the top of the configs folder"]
    );
}

#[test]
fn no_app_toml_at_the_top_is_a_problem() {
    let problems = App::from_files([("views/more.toml", MORE)]).unwrap_err();
    assert_eq!(
        messages(&problems),
        ["app.toml not found at the top of the configs folder"]
    );
}

#[test]
fn an_unknown_key_in_another_file_names_the_file_line_and_column() {
    let problems = App::from_files([
        ("app.toml", APP),
        ("crm/api.toml", "[state]\nx = 1\n\n[colour]\nred = 1\n"),
    ])
    .unwrap_err();
    let text = messages(&problems).join("\n");
    assert!(
        // Column 2: where the key itself is written, inside the brackets.
        text.starts_with("crm/api.toml:4:2: ") && text.contains("colour"),
        "{text}"
    );
}

#[test]
fn problems_in_several_files_are_all_reported_at_once() {
    let problems = App::from_files([
        ("app.toml", "[app]\nname = \"x\"\ncolour = 1\n"),
        ("a.toml", "[bogus]\n"),
        ("b.toml", "[state]\nfine = 1\n"),
    ])
    .unwrap_err();
    let files: Vec<Option<&str>> = problems.iter().map(|p| p.file.as_deref()).collect();
    assert_eq!(files, [Some("app.toml"), Some("a.toml")]);
}

#[test]
fn a_problem_reads_as_file_line_column_and_message() {
    let problem = ConfigError {
        file: Some("crm/api.toml".into()),
        message: "bad".into(),
        line: Some(3),
        column: Some(7),
    };
    assert_eq!(problem.to_string(), "crm/api.toml:3:7: bad");
}

#[test]
fn app_and_body_subtables_outside_app_toml_are_problems_where_their_headers_are() {
    let problems = App::from_files([
        ("app.toml", APP),
        ("a.toml", "[state]\nx = 1\n\n  [app.auth]\nlogin = \"x\"\n"),
        (
            "b.toml",
            "[[body.children]]\ntype = \"text\"\ntext = \"hi\"\n",
        ),
    ])
    .unwrap_err();
    assert_eq!(
        messages(&problems),
        [
            "a.toml:4:3: [app] goes only in app.toml, at the top of the configs folder",
            "b.toml:1:1: [[body]] goes only in app.toml, at the top of the configs folder",
        ]
    );
}
