//! Props that carry steps (spec 090): a caller passes a list of steps, the component runs it with
//! `"{{ props.NAME }}"`, and the steps run where the component is used, against the app's state.

use genesis_core::bus::{Machine, Within};
use genesis_core::config::{App, ConfigError};
use serde_json::{Value, json};

const APP: &str = r#"
[app]
name = "Cards"

[state]
contacts = [{ name = "Ada" }, { name = "Lin" }]
opened = ""
view = "main"

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "use"
id = "card"
component = "contact_card"
props = { contact = "{{ item }}", on_open = [{ set = "app.opened", to = "{{ event.item.name }}" }, { set = "app.view", to = "contact" }] }
"#;

const CARD: &str = r#"
[component]
props = ["contact", "on_open"]
optional = ["on_close"]

[state]
opens = 0

[[body]]
type = "button"
id = "open"
label = "Open"
on_click = "{{ props.on_open }}"

[[body]]
type = "button"
id = "both"
label = "Count and open"
on_click = [{ step = "local.opens" }, "{{ props.on_open }}", { step = "local.opens" }]

[[body]]
type = "button"
id = "close"
label = "Close"
on_click = "{{ props.on_close }}"
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

fn machine() -> Machine {
    Machine::new(load(&[("app.toml", APP), ("contact_card.toml", CARD)]).unwrap())
}

fn at(name: &str, row: Option<&str>, outer: Option<Within>) -> Option<Within> {
    Some(Within {
        name: name.into(),
        row: row.map(String::from),
        props: Value::Null,
        outer: outer.map(Box::new),
    })
}

#[test]
fn passed_steps_run_in_the_app_with_the_event_inside() {
    let mut m = machine();
    m.fire_within(
        "card.open.click",
        json!({ "item": { "name": "Lin" } }),
        Some("1"),
        at("card", Some("1"), None),
    );
    assert_eq!(m.get("app.opened"), Some(json!("Lin")));
    assert_eq!(m.get("app.view"), Some(json!("contact")));
}

#[test]
fn a_prop_step_in_a_list_runs_in_its_place_and_the_list_goes_back_to_the_component() {
    let mut m = machine();
    m.fire_within(
        "card.both.click",
        json!({ "item": { "name": "Ada" } }),
        Some("0"),
        at("card", Some("0"), None),
    );
    assert_eq!(m.get("app.opened"), Some(json!("Ada")));
    assert_eq!(m.local("card", Some("0"), "opens"), Some(json!(2)));
}

#[test]
fn an_optional_steps_prop_left_out_runs_nothing() {
    let mut m = machine();
    m.fire_within(
        "card.close.click",
        json!({}),
        Some("0"),
        at("card", Some("0"), None),
    );
    assert_eq!(m.get("sys.errors"), Some(json!([])));
}

#[test]
fn passed_steps_run_where_they_were_written_through_every_component_between() {
    // `list` passes its own `on_pick` on to each `card`; `on_pick` was written in the app, so it
    // runs there. `list`'s own `on_card` runs in `list`'s instance, writing its local.
    let card = r#"
[component]
props = ["on_open"]

[[body]]
type = "button"
id = "open"
label = "Open"
on_click = "{{ props.on_open }}"
"#;
    let list = r#"
[component]
props = ["on_pick"]

[state]
picks = 0

[[body]]
type = "use"
id = "first"
component = "card"
props = { on_open = "{{ props.on_pick }}" }

[[body]]
type = "use"
id = "second"
component = "card"
props = { on_open = [{ step = "local.picks" }] }
"#;
    let app = r#"
[app]
name = "x"

[state]
picked = 0

[[body]]
type = "use"
id = "list"
component = "list"
props = { on_pick = [{ step = "app.picked" }] }
"#;
    let mut m =
        Machine::new(load(&[("app.toml", app), ("card.toml", card), ("list.toml", list)]).unwrap());
    m.fire_within(
        "list.first.open.click",
        json!({}),
        None,
        at("list.first", None, at("list", None, None)),
    );
    assert_eq!(m.get("app.picked"), Some(json!(1)));
    m.fire_within(
        "list.second.open.click",
        json!({}),
        None,
        at("list.second", None, at("list", None, None)),
    );
    assert_eq!(m.local("list", None, "picks"), Some(json!(1)));
    assert_eq!(m.get("app.picked"), Some(json!(1)));
}

#[test]
fn a_failing_passed_step_stops_the_list_and_is_logged() {
    let app = APP.replace(
        "{ set = \"app.view\", to = \"contact\" }",
        "\"no_such_action\"",
    );
    let mut m = Machine::new(load(&[("app.toml", &app), ("contact_card.toml", CARD)]).unwrap());
    m.fire_within(
        "card.both.click",
        json!({ "item": { "name": "Ada" } }),
        Some("0"),
        at("card", Some("0"), None),
    );
    assert_eq!(m.get("app.opened"), Some(json!("Ada")));
    // The step before the failure ran; the one after it didn't.
    assert_eq!(m.local("card", Some("0"), "opens"), Some(json!(1)));
    let errors = m.get("sys.errors").unwrap();
    assert!(
        errors
            .to_string()
            .contains("there's no action named no_such_action"),
        "{errors}"
    );
}

#[test]
fn a_prop_step_outside_a_component_is_refused_on_load_and_fails_loud_from_on() {
    let out = problems(&[(
        "app.toml",
        "[app]\nname = \"x\"\n[[body]]\ntype = \"button\"\nlabel = \"x\"\non_click = \"{{ props.go }}\"\n",
    )]);
    assert!(out.contains("body.0: a step \"{{ props.go }}\" runs steps passed into a component, so it works only inside one"), "{out}");
    let mut m = Machine::new(
        load(&[(
            "app.toml",
            "[app]\nname = \"x\"\n[on]\n\"go\" = [\"{{ props.go }}\"]\n",
        )])
        .unwrap(),
    );
    m.fire("go", json!({}));
    let errors = m.get("sys.errors").unwrap();
    assert!(
        errors.to_string().contains("runs only inside a component"),
        "{errors}"
    );
}

#[test]
fn a_step_template_that_isnt_a_prop_and_a_prop_that_isnt_steps_are_config_errors() {
    let card = CARD.replace(
        "on_click = \"{{ props.on_close }}\"",
        "on_click = \"{{ local.opens }}\"",
    );
    let out = problems(&[("app.toml", APP), ("contact_card.toml", &card)]);
    assert!(
        out.contains("body.2: a step can run a prop's steps as \"{{ props.NAME }}\""),
        "{out}"
    );

    for bad in ["\"{{ item }}\"", "[{ sett = \"app.x\" }]", "3"] {
        let app = APP.replace(
            "on_open = [{ set = \"app.opened\", to = \"{{ event.item.name }}\" }, { set = \"app.view\", to = \"contact\" }]",
            &format!("on_open = {bad}"),
        );
        let out = problems(&[("app.toml", &app), ("contact_card.toml", CARD)]);
        assert!(
            out.contains("`contact_card` runs `on_open` as steps, so pass a list of steps"),
            "{bad}: {out}"
        );
    }
    // One step alone, or a custom action's name, is fine.
    let app = APP.replace(
        "on_open = [{ set = \"app.opened\", to = \"{{ event.item.name }}\" }, { set = \"app.view\", to = \"contact\" }]",
        "on_open = { set = \"app.view\", to = \"contact\" }",
    );
    assert!(load(&[("app.toml", &app), ("contact_card.toml", CARD)]).is_ok());
}

#[test]
fn a_prop_passed_on_is_checked_where_it_is_first_written() {
    let card = "[component]\nprops = [\"on_open\"]\n[[body]]\ntype = \"button\"\nlabel = \"x\"\non_click = \"{{ props.on_open }}\"\n";
    let list = "[component]\nprops = [\"on_pick\"]\n[[body]]\ntype = \"use\"\ncomponent = \"card\"\nprops = { on_open = \"{{ props.on_pick }}\" }\n";
    let app = "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\ncomponent = \"list\"\nprops = { on_pick = \"{{ app.x }}\" }\n";
    let out = problems(&[("app.toml", app), ("card.toml", card), ("list.toml", list)]);
    assert!(out.contains("`list` runs `on_pick` as steps"), "{out}");
    // Passing a prop on from the app isn't passing it on: there's nothing to pass.
    let direct = "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\ncomponent = \"card\"\nprops = { on_open = \"{{ props.on_pick }}\" }\n";
    let out = problems(&[("app.toml", direct), ("card.toml", card)]);
    assert!(out.contains("`card` runs `on_open` as steps"), "{out}");
}

#[test]
fn one_step_alone_works_as_any_shortcut() {
    let app = "[app]\nname = \"x\"\n[state]\nn = 0\n[actions.up]\ndo = [{ step = \"app.n\" }]\n[[body]]\ntype = \"button\"\nid = \"b\"\nlabel = \"+\"\non_click = \"up\"\n[[body]]\ntype = \"form\"\nid = \"f\"\non_submit = { step = \"app.n\" }\n";
    let mut m = Machine::new(load(&[("app.toml", app)]).unwrap());
    m.fire("b.click", json!({}));
    m.fire("f.submit", json!({}));
    assert_eq!(m.get("app.n"), Some(json!(2)));
}

#[test]
fn a_components_own_steps_that_reach_past_it_are_config_errors() {
    let card = r#"
[component]
props = ["contact", "on_open"]

[[body]]
type = "button"
label = "Cheer"
on_click = "cheer"

[[body]]
type = "form"
on_submit = [{ call = "api.contacts.create" }]

[[body.children]]
type = "button"
label = "Again"
on_click = [{ reload = ["app.contacts"] }]

[[body]]
type = "button"
label = "Time"
on_click = [{ start = "poll" }, { stop = "poll" }, { log = "hi" }]
"#;
    let out = problems(&[("app.toml", APP), ("contact_card.toml", card)]);
    assert!(
        out.contains("contact_card.toml: body.0: a component's own steps only fire events, so this step, which runs the custom action `cheer`"),
        "{out}"
    );
    assert!(out.contains("body.1: a component's own steps only fire events, so this step, which calls `api.contacts.create`"), "{out}");
    assert!(out.contains("body.1.children.0: a component's own steps only fire events, so this step, which reloads a source"), "{out}");
    for what in [
        "starts the timer `poll`",
        "stops the timer `poll`",
        "writes to the log",
    ] {
        assert!(
            out.contains(&format!(
                "body.2: a component's own steps only fire events, so this step, which {what}"
            )),
            "{out}"
        );
    }
}

#[test]
fn a_component_fires_any_event_and_the_apps_handlers_do_the_rest() {
    let app = APP.replace(
        "view = \"main\"",
        "view = \"main\"\n\n[on]\n\"contact.picked\" = [{ set = \"app.opened\", to = \"{{ event.who }}\" }, { set = \"app.view\", to = \"contact\" }]",
    );
    let card = r#"
[component]
props = ["contact", "on_open"]

[[body]]
type = "button"
id = "pick"
label = "Pick"
on_click = [{ fire = "contact.picked", with = { who = "{{ props.contact.name }}" } }]
"#;
    let mut m = Machine::new(load(&[("app.toml", &app), ("contact_card.toml", card)]).unwrap());
    m.fire_within(
        "card.pick.click",
        json!({ "item": { "name": "Lin" } }),
        Some("1"),
        Some(Within {
            name: "card".into(),
            row: Some("1".into()),
            props: json!({ "contact": { "name": "Lin" } }),
            outer: None,
        }),
    );
    assert_eq!(m.get("app.opened"), Some(json!("Lin")));
    assert_eq!(m.get("app.view"), Some(json!("contact")));
}
