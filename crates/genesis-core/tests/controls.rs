//! Controls with `var` (spec 040, "Controls"): what each loads with, what it refuses, and how it
//! writes its variable through the bus.

use genesis_core::bus::Machine;
use genesis_core::config::{App, Node};
use serde_json::json;

fn app(toml: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"Controls\"\n{toml}"))
}

fn problem(toml: &str) -> String {
    app(toml)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Loads one node, checks it survives the trip to the browser (JSON), and hands it back.
fn round_trip(node: &str) -> Node {
    let app = app(&format!("[[body]]\n{node}\n")).unwrap_or_else(|e| panic!("{node}: {e:?}"));
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app, "{node}");
    app.body[0].clone()
}

fn machine(toml: &str) -> Machine {
    Machine::new(app(toml).expect("app loads"))
}

#[test]
fn an_input_loads_with_var_kind_label_and_placeholder() {
    let Node::Input(input) = round_trip(
        "type = \"input\"\nid = \"search\"\nvar = \"app.query\"\nkind = \"search\"\nlabel = \"Search\"\nplaceholder = \"Name or email\"\nwidth = \"1/2\"",
    ) else {
        panic!("not an input")
    };
    assert_eq!(input.var.as_deref(), Some("app.query"));
    assert_eq!(input.placeholder.as_deref(), Some("Name or email"));
}

#[test]
fn an_input_takes_each_kind_and_nothing_else() {
    for kind in ["text", "email", "password", "number", "date", "search"] {
        round_trip(&format!(
            "type = \"input\"\nvar = \"app.x\"\nkind = \"{kind}\""
        ));
    }
    assert!(
        problem("[[body]]\ntype = \"input\"\nvar = \"app.x\"\nkind = \"colour\"")
            .contains("unknown variant `colour`")
    );
}

#[test]
fn every_control_loads_with_its_own_keys() {
    round_trip("type = \"textarea\"\nvar = \"app.note\"\nlabel = \"Note\"\nrows = 5");
    round_trip("type = \"checkbox\"\nvar = \"app.agree\"\nlabel = \"I agree\"");
    round_trip("type = \"toggle\"\nvar = \"app.dark\"\nlabel = \"Dark\"");
    round_trip("type = \"radio\"\nvar = \"app.size\"\nvalue = 3\nlabel = \"Three\"");
    round_trip(
        "type = \"select\"\nvar = \"sys.theme\"\noptions = [{ label = \"Light\", value = \"light\" }, { label = \"Dark\", value = \"dark\" }]",
    );
    round_trip(
        "type = \"select\"\nvar = \"app.company\"\noptions = \"app.companies\"\noption_label = \"/name\"\noption_value = \"/id\"",
    );
}

#[test]
fn a_radio_needs_a_value() {
    assert!(
        problem("[[body]]\ntype = \"radio\"\nvar = \"app.size\"\nlabel = \"Three\"")
            .contains("missing field `value`")
    );
}

#[test]
fn a_control_needs_a_var() {
    assert!(
        problem("[[body]]\ntype = \"checkbox\"\nlabel = \"I agree\"")
            .contains("body.0: a control needs `var`, or `name` inside a form")
    );
}

#[test]
fn a_controls_var_must_be_a_variable_actions_can_write() {
    for var in ["item.name", "app", "sys.errors", "theme", "app.a.b"] {
        let out = problem(&format!("[[body]]\ntype = \"input\"\nvar = \"{var}\""));
        assert!(
            out.contains("body.0: `var` is an app variable, like \"app.query\""),
            "{var}: {out}"
        );
    }
    for var in ["app.query", "sys.view", "sys.theme"] {
        round_trip(&format!("type = \"input\"\nvar = \"{var}\""));
    }
}

#[test]
fn a_selects_option_pointers_go_with_options_from_a_variable() {
    let out = problem(
        "[[body]]\ntype = \"select\"\nvar = \"app.c\"\noptions = [{ label = \"A\", value = 1 }]\noption_label = \"/name\"",
    );
    assert!(
        out.contains("body.0: `option_label` goes with `options` from a variable"),
        "{out}"
    );
    let out = problem("[[body]]\ntype = \"select\"\nvar = \"app.c\"\noptions = \"companies\"");
    assert!(
        out.contains("body.0: `options` is a list, or a variable's path like \"app.companies\""),
        "{out}"
    );
}

#[test]
fn a_controls_change_sets_its_variable_to_the_value_with_its_type() {
    let mut m = machine(
        "[state]\nqty = 1\n[[body]]\ntype = \"input\"\nid = \"qty_box\"\nvar = \"app.qty\"\nkind = \"number\"",
    );
    m.fire("qty_box.change", json!({ "value": 7 }));
    assert_eq!(m.get("app.qty"), Some(json!(7)));
    m.fire("qty_box.change", json!({ "value": null }));
    assert_eq!(m.get("app.qty"), Some(json!(null)));
}

#[test]
fn a_change_fires_the_variables_changed_event_so_sources_and_handlers_follow() {
    let mut m = machine(
        "[state]\nquery = \"\"\n[[body]]\ntype = \"input\"\nid = \"search\"\nvar = \"app.query\"\n[on]\n\"app.query.changed\" = [{ log = \"now {{ event.value }}\" }]",
    );
    m.fire("search.change", json!({ "value": "ada" }));
    let trail = m.get("sys.events").unwrap();
    assert_eq!(trail[0]["log"], "now ada");
}

#[test]
fn a_select_on_sys_theme_changes_the_theme() {
    let mut m = machine(
        "[[body]]\ntype = \"select\"\nid = \"theme\"\nvar = \"sys.theme\"\noptions = [{ label = \"Dark\", value = \"dark\" }]",
    );
    m.fire("theme.change", json!({ "value": "dark" }));
    assert_eq!(m.get("sys.theme"), Some(json!("dark")));
}

#[test]
fn a_control_without_an_id_still_writes_its_variable_by_its_place() {
    let mut m = machine(
        "[state]\nagree = false\n[[body]]\ntype = \"cell\"\n[[body.children]]\ntype = \"checkbox\"\nvar = \"app.agree\"",
    );
    m.fire("body.0.children.0.change", json!({ "value": true }));
    assert_eq!(m.get("app.agree"), Some(json!(true)));
}

#[test]
fn radios_on_one_var_set_the_picked_ones_value() {
    let mut m = machine(
        "[state]\nsize = 1\n[[body]]\ntype = \"radio\"\nvar = \"app.size\"\nvalue = 1\n[[body]]\ntype = \"radio\"\nid = \"big\"\nvar = \"app.size\"\nvalue = 3",
    );
    m.fire("big.change", json!({ "value": 3 }));
    assert_eq!(m.get("app.size"), Some(json!(3)));
}

#[test]
fn sys_bus_shows_a_controls_change_and_the_set_it_runs() {
    let m =
        machine("[state]\nq = \"\"\n[[body]]\ntype = \"input\"\nid = \"q_box\"\nvar = \"app.q\"");
    let bus = m.get("sys.bus").unwrap();
    assert!(
        bus["handlers"]
            .as_array()
            .unwrap()
            .contains(&json!({ "pattern": "q_box.change", "steps": [{ "set": "app.q", "to": "{{ event.value }}" }] })),
        "{bus}"
    );
    assert!(
        bus["fires"]
            .as_array()
            .unwrap()
            .contains(&json!("q_box.change"))
    );
}

#[test]
fn a_change_to_an_undeclared_variable_fails_loudly() {
    let mut m = machine("[[body]]\ntype = \"input\"\nid = \"q_box\"\nvar = \"app.q\"");
    m.fire("q_box.change", json!({ "value": "x" }));
    let errors = m.get("sys.errors").unwrap();
    assert!(
        errors[0]["message"]
            .as_str()
            .unwrap()
            .contains("app.q isn't declared in [state]"),
        "{errors}"
    );
}
