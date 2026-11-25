//! The load checks for wiring (spec 020, "Checks"): a step or a template that would only fail when
//! it ran is refused at load, naming its file, line, and column. A `go` to a page that isn't there
//! still loads, since a page can show up later (spec 100, Log).

use genesis_core::config::{App, ConfigError};

const API: &str = r#"[connections.crm]
base_url = "https://api.example.com"

[connections.crm.ops.ping]
method = "GET"
path = "/ping"

[sources.contacts]
call = "crm.ping"
"#;

const TIMERS: &str = "[poll]\nms = 100\n";

/// Loads `app` (after `[app]`, so its first line is line 3) with the API and the timers.
fn load(app: &str) -> Result<App, Vec<ConfigError>> {
    App::from_files([
        ("app.toml", format!("[app]\nname = \"t\"\n{app}").as_str()),
        ("crm/api.toml", API),
        ("timers.toml", TIMERS),
    ])
}

/// Every problem loading `app`, one per line.
fn problems(app: &str) -> Vec<String> {
    load(app)
        .expect_err("the app is refused")
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_step_naming_what_isnt_there_is_refused_on_its_line() {
    let cases = [
        (
            "\"nowhere\"",
            "there's no custom action named `nowhere`; define it as [actions.nowhere] with `do = [...]`",
        ),
        (
            "{ set = \"app.gone\", to = 1 }",
            "`app.gone` isn't declared; give it a starting value in [state]",
        ),
        (
            "{ step = \"app.gone\" }",
            "`app.gone` isn't declared; give it a starting value in [state]",
        ),
        (
            "{ call = \"crm.pong\" }",
            "there's no operation named `crm.pong`; the connections declare `crm.ping`",
        ),
        (
            "{ call = \"crm.ping\", into = { \"app.gone\" = \"\" } }",
            "`into`: `app.gone` isn't declared; give it a starting value in [state]",
        ),
        (
            "{ reload = [\"app.n\"] }",
            "`reload` takes sources, as \"app.NAME\" for each [sources.NAME], and \"app.n\" isn't one",
        ),
        (
            "{ start = \"pol\" }",
            "there's no timer named `pol`; a timer is a table in timers.toml, like [pol] with `ms`",
        ),
        (
            "{ stop = \"pol\" }",
            "there's no timer named `pol`; a timer is a table in timers.toml, like [pol] with `ms`",
        ),
    ];
    for (step, why) in cases {
        let app = format!("[state]\nn = 0\n[on]\n\"go\" = [{{ log = \"hi\" }}, {step}]\n");
        assert_eq!(problems(&app), [format!("app.toml:6:25: {why}")], "{step}");
    }
}

#[test]
fn a_step_writing_what_it_cant_is_refused() {
    for (var, why) in [
        (
            "sys.events",
            "`sys.events` can't be written; steps write `app.*`, `sys.theme`, and in a component its `local.*`",
        ),
        (
            "event.x",
            "`event.x` can't be written; steps write `app.*`, `sys.theme`, and in a component its `local.*`",
        ),
        (
            "local.x",
            "`local.x` is a component's own variable, so only its own steps write it",
        ),
    ] {
        let app = format!("[on]\n\"go\" = [{{ set = \"{var}\", to = 1 }}]\n");
        assert_eq!(problems(&app), [format!("app.toml:4:9: {why}")], "{var}");
    }
    // `sys.theme` is the one of Genesis's own that a step writes.
    load("[on]\n\"go\" = [{ set = \"sys.theme\", to = \"dark\" }]\n").unwrap();
}

#[test]
fn a_template_that_isnt_a_plain_lookup_or_reads_nothing_is_refused_where_its_written() {
    let cases = [
        (
            "Hi {{ app.n + 1 }}",
            "`{{ app.n + 1 }}` isn't a plain lookup like `{{ app.name }}`",
        ),
        ("Hi {{ app.n", "`{{ app.n` has no closing `}}`"),
        (
            "Hi {{ app.name }}",
            "`app.name` isn't declared; give it a starting value in [state], or make it a source",
        ),
        (
            "Hi {{ sys.now }}",
            "`sys.now` isn't one of Genesis's own variables: `sys.theme`, `sys.outlet`, `sys.errors`, `sys.events`, `sys.bus`, `sys.sources`, `sys.toasts`, `sys.asking`",
        ),
        (
            "Hi {{ event.name }}",
            "`event.name` reads the event being handled, so it works only in a step",
        ),
        (
            "Hi {{ item.name }}",
            "`item.name` reads the element of an `each`, so it works only inside one",
        ),
        (
            "Hi {{ form.name }}",
            "`form.name` reads a form's draft, so it works only inside a `form`",
        ),
        (
            "Hi {{ local.name }}",
            "`local.name` reads a component's `local.*`, so it works only inside one",
        ),
        (
            "Hi {{ creds.TOKEN }}",
            "`creds.TOKEN` works only in a connection, in [connections.NAME]",
        ),
        (
            "Hi {{ name }}",
            "`name` reads nothing: a template reads `app.*`, `sys.*`, `event.*` (in a step), `item.*` (in an `each`), `form.*` (in a form), or in a component its `local.*` and `props.*`",
        ),
    ];
    for (text, why) in cases {
        let app = format!("[state]\nn = 0\n\n[[body]]\ntype = \"text\"\ntext = \"{text}\"\n");
        assert_eq!(problems(&app), [format!("app.toml:8:8: {why}")], "{text}");
    }
}

#[test]
fn a_step_reads_its_row_and_form_through_the_event() {
    let app = r#"[state]
rows = []
[[body]]
type = "cell"
each = "app.rows"
[[body.children]]
type = "button"
label = "{{ item.name }}"
on_click = [{ log = "{{ item.name }}" }]
"#;
    assert_eq!(
        problems(app),
        ["app.toml:11:21: a step reads the row it fired in as `event.item`, not `item.name`"]
    );
    load(&app.replace(
        "log = \"{{ item.name }}\"",
        "log = \"{{ event.item.name }}\"",
    ))
    .unwrap();

    let app = r#"[state]
saved = ""
[[body]]
type = "form"
id = "f"
on_submit = [{ set = "app.saved", to = "{{ form.name }}" }]
[[body.children]]
type = "text"
text = "{{ form.name }}"
"#;
    assert_eq!(
        problems(app),
        ["app.toml:8:40: a step reads a submitted form as `event.form`, not `form.name`"]
    );
    load(&app.replace("to = \"{{ form.name }}\"", "to = \"{{ event.form.name }}\"")).unwrap();
}

#[test]
fn what_each_place_reads_loads() {
    load(
        r#"[state]
rows = []
page = 1
[sources.more]
call = "crm.ping"
params = { page = "{{ app.page }}" }
[[body]]
type = "cell"
each = "app.rows"
show = { var = "app.page", is = 1 }
[[body.children]]
type = "text"
text = "{{ item.name }} on {{ app.page }} in {{ sys.theme }}"
[[body]]
type = "button"
label = "Next"
on_click = [{ step = "app.page" }, { reload = ["app.contacts", "app.more"] }, { start = "poll" }, "save"]
[actions.save]
do = [{ call = "crm.ping", into = { "app.page" = "/page" } }, { toast = "Saved {{ event.name }}" }]
[on]
"crm.ping.failed" = [{ log = "{{ event.error }} after {{ sys.errors.0.message }}" }]
"#,
    )
    .unwrap();
}

#[test]
fn a_go_to_a_page_that_isnt_there_still_loads() {
    App::from_files([
        (
            "app.toml",
            "[app]\nname = \"t\"\n[[body]]\ntype = \"outlet\"\ndefault = \"home\"\n[on]\n\"go\" = [{ go = \"nowhere\" }]\n",
        ),
        ("home.toml", "[component]\n"),
    ])
    .unwrap();
}

#[test]
fn every_wiring_problem_in_every_file_is_reported_at_once() {
    let more = "[on]\n\"b\" = [\"nowhere\"]\n";
    let out: Vec<String> = App::from_files([
        (
            "app.toml",
            "[app]\nname = \"t\"\n[on]\n\"a\" = [{ set = \"app.x\", to = 1 }]\n",
        ),
        ("views/more.toml", more),
    ])
    .unwrap_err()
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(
        out,
        [
            "app.toml:4:8: `app.x` isn't declared; give it a starting value in [state]",
            "views/more.toml:2:8: there's no custom action named `nowhere`; define it as [actions.nowhere] with `do = [...]`",
        ]
    );
}

#[test]
fn a_components_own_steps_and_the_steps_passed_into_it_are_checked_where_written() {
    let card = r#"[component]
props = ["on_open"]

[state]
open = false

[[body]]
type = "cell"
each = "item"

[[body]]
type = "button"
label = "Open {{ app.n }}"
on_click = [{ toggle = "local.open" }, "{{ props.on_open }}"]
"#;
    let app = r#"[state]
n = 0
rows = []
[[body]]
type = "cell"
each = "app.rows"
[[body.children]]
type = "use"
component = "card"
props = { on_open = [{ set = "app.n", to = "{{ event.item.n }}" }, "nowhere"] }
"#;
    let out: Vec<String> = App::from_files([
        ("app.toml", format!("[app]\nname = \"t\"\n{app}").as_str()),
        ("card.toml", card),
    ])
    .unwrap_err()
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(
        out,
        [
            "app.toml:12:68: there's no custom action named `nowhere`; define it as [actions.nowhere] with `do = [...]`",
            "card.toml:9:8: `item` reads the element of an `each`, so it works only inside one",
        ]
    );
}
