//! A cell with `each` (spec 040, "Repeating: `each`"): what it loads with, and what it refuses.

use genesis_core::config::{App, Node};

fn app(body: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!(
        "[app]\nname = \"x\"\n\n[state]\nrows = []\npage = {{ rows = [] }}\nmy_rows = []\n\n{body}"
    ))
}

fn problems(body: &str) -> Vec<String> {
    app(body)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_cell_loads_with_each_empty_and_error_and_survives_json() {
    let app = app("[[body]]\ntype = \"cell\"\neach = \"app.rows\"\nempty = \"No rows yet.\"\nerror = \"Couldn't load the rows.\"\n\n[[body.children]]\ntype = \"text\"\ntext = \"{{ item.name }}\"\n").unwrap();
    let Node::Cell(cell) = &app.body[0] else {
        panic!("not a cell")
    };
    assert_eq!(cell.each.as_deref(), Some("app.rows"));
    assert_eq!(cell.empty.as_deref(), Some("No rows yet."));
    assert_eq!(cell.error.as_deref(), Some("Couldn't load the rows."));
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
}

#[test]
fn each_takes_an_app_or_item_path_and_nothing_else() {
    for path in [
        "app.rows",
        "app.page.rows",
        "item.tags",
        "item",
        "app.my_rows",
        "item.tag-list.0",
    ] {
        // Inside a row, so `item` has one to read.
        app(&format!("[[body]]\ntype = \"cell\"\neach = \"app.rows\"\n[[body.children]]\ntype = \"cell\"\neach = \"{path}\"\n"))
            .unwrap_or_else(|e| panic!("{path}: {e:?}"));
    }
    for path in [
        "{{ app.rows }}",
        "rows",
        "sys.errors.0",
        "app.",
        "app rows",
        "event.rows",
    ] {
        assert_eq!(
            problems(&format!("[[body]]\ntype = \"cell\"\neach = \"{path}\"\n")),
            [format!(
                "app.toml: body.0: `each` is a variable's path, like \"app.contacts\" or \"item.tags\", not {path:?}"
            )],
        );
    }
}

#[test]
fn empty_or_error_without_each_is_a_problem_naming_the_cell() {
    assert_eq!(
        problems(
            "[[body]]\ntype = \"cell\"\n\n[[body.children]]\ntype = \"cell\"\nempty = \"None.\"\nerror = \"Oops.\"\n"
        ),
        [
            "app.toml: body.0.children.0: `empty` goes with `each`",
            "app.toml: body.0.children.0: `error` goes with `each`",
        ]
    );
}
