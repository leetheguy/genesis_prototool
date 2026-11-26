//! The guide's examples work (spec 110, "The guide"). `docs/authoring.md` shows whole apps: a ```toml
//! block whose first line is `# app.toml` starts one, and each block after it whose first line names a
//! file (`# crm.toml: ...`) is another of its files. Every one of those apps loads, and the patterns
//! the guide leans on hardest do what it says they do.

use genesis_core::bus::{Effect, Machine, Reply};
use genesis_core::config::App;
use serde_json::{Value, json};

const GUIDE: &str = include_str!("../../../docs/authoring.md");

/// Each example app in the guide: its `[app] name`, and its files as (path, text).
fn examples() -> Vec<Vec<(String, String)>> {
    let mut apps: Vec<Vec<(String, String)>> = Vec::new();
    let mut rest = GUIDE;
    while let Some(start) = rest.find("```toml\n") {
        let body = &rest[start + "```toml\n".len()..];
        let end = body.find("```").expect("every ```toml block is closed");
        let block = &body[..end];
        rest = &body[end + 3..];
        let Some(path) = file_named(block) else {
            continue;
        };
        if path == "app.toml" {
            apps.push(Vec::new());
        }
        if let Some(app) = apps.last_mut() {
            assert!(
                app.iter().all(|(p, _)| *p != path),
                "{path} twice in one example"
            );
            app.push((path, block.to_string()));
        }
    }
    apps
}

/// The file a block's first line names: `# views/home.toml: the first page.` names `views/home.toml`.
fn file_named(block: &str) -> Option<String> {
    let line = block.lines().next()?.strip_prefix("# ")?;
    let name: String = line
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-'))
        .collect();
    name.ends_with(".toml").then_some(name)
}

fn example(name: &str) -> App {
    let files = examples()
        .into_iter()
        .find(|files| files[0].1.contains(&format!("name = \"{name}\"")))
        .unwrap_or_else(|| panic!("the guide has an example app called {name}"));
    App::from_files(files).expect("it loads")
}

fn calls(effects: &[Effect]) -> Vec<(u64, String, Value)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Call(call) => Some((call.id, call.op.clone(), call.params.clone())),
            _ => None,
        })
        .collect()
}

fn answer(m: &mut Machine, id: u64, status: u16, body: Value) -> Vec<Effect> {
    m.reply(id, Reply::Response { status, body })
}

#[test]
fn every_example_app_in_the_guide_loads() {
    let apps = examples();
    assert!(apps.len() >= 6, "found {} example apps", apps.len());
    for files in apps {
        let names: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        if let Err(problems) = App::from_files(files.clone()) {
            let problems: Vec<String> = problems
                .iter()
                .map(|p| format!("{}: {}", p.file.as_deref().unwrap_or("-"), p.message))
                .collect();
            panic!("{names:?} doesn't load:\n{}", problems.join("\n"));
        }
    }
}

#[test]
fn picking_a_contact_fetches_it() {
    let mut m = Machine::new(example("Contacts"));
    let list = calls(&m.open());
    answer(
        &mut m,
        list[0].0,
        200,
        json!({ "rows": [{ "id": 7, "name": "Ada" }] }),
    );
    let get = calls(&m.fire("contact.picked", json!({ "id": 7 })));
    assert_eq!(
        (get[0].1.as_str(), &get[0].2),
        ("crm.contacts.get", &json!({ "id": 7 }))
    );
    answer(&mut m, get[0].0, 200, json!({ "id": 7, "name": "Ada" }));
    assert_eq!(m.get("app.contact.name"), Some(json!("Ada")));
}

#[test]
fn a_failed_save_says_why_and_a_good_one_says_so() {
    let form = json!({ "form": { "name": "Ada", "email": "ada@example.com" } });
    let mut m = Machine::new(example("New contact"));
    let call = calls(&m.fire("new_contact.submit", form.clone()));
    answer(&mut m, call[0].0, 409, json!({ "error": "taken" }));
    assert_eq!(
        (m.get("app.problem"), m.get("app.added")),
        (
            Some(json!("Someone already has that email.")),
            Some(json!(""))
        )
    );
    let call = calls(&m.fire("new_contact.submit", form));
    answer(&mut m, call[0].0, 201, json!({}));
    assert_eq!(
        (m.get("app.problem"), m.get("app.added")),
        (Some(json!("")), Some(json!("Added Ada.")))
    );
}

#[test]
fn the_api_picks_the_branch() {
    let mut m = Machine::new(example("Review"));
    let call = calls(&m.fire("score.click", json!({})));
    answer(&mut m, call[0].0, 200, json!({ "outcome": "needs_review" }));
    assert_eq!(m.get("app.reviewing"), Some(json!(true)));
    let call = calls(&m.fire("approve.click", json!({})));
    assert_eq!(call[0].2, json!({ "verdict": "approve" }));
    answer(&mut m, call[0].0, 200, json!({}));
    assert_eq!(m.get("app.reviewing"), Some(json!(false)));
}

#[test]
fn pages_go_by_the_apps_handlers() {
    let mut m = Machine::new(example("Pages"));
    m.open();
    assert_eq!(m.get("sys.outlet"), Some(json!("home")));
    m.fire("nav.new_contact", json!({}));
    assert_eq!(m.get("sys.outlet"), Some(json!("new_contact")));
    let call = calls(&m.fire(
        "new_contact.form.submit",
        json!({ "form": { "name": "Ada" } }),
    ));
    answer(&mut m, call[0].0, 200, json!({}));
    assert_eq!(m.get("sys.outlet"), Some(json!("home")));
}

#[test]
fn search_waits_for_a_pause_in_typing() {
    let mut m = Machine::new(example("Timers"));
    m.open();
    assert!(calls(&m.fire("search.change", json!({ "value": "a" }))).is_empty());
    m.advance(100);
    m.fire("search.change", json!({ "value": "ad" }));
    assert!(calls(&m.advance(299)).is_empty());
    let search = calls(&m.advance(1));
    assert_eq!(
        (search[0].1.as_str(), &search[0].2),
        ("crm.search", &json!({ "q": "ad" }))
    );
}

#[test]
fn the_inbox_is_fetched_again_every_thirty_seconds() {
    let mut m = Machine::new(example("Timers"));
    let first = calls(&m.open());
    assert_eq!(first[0].1, "crm.inbox");
    answer(&mut m, first[0].0, 200, json!({ "messages": [] }));
    assert!(calls(&m.advance(29_999)).is_empty());
    assert_eq!(calls(&m.advance(1))[0].1, "crm.inbox");
}

#[test]
fn a_slow_save_says_so_and_saved_goes_away_on_its_own() {
    let mut m = Machine::new(example("Timers"));
    m.open();
    let save = calls(&m.fire("save.click", json!({})));
    m.advance(5000);
    assert_eq!(
        m.get("app.slow"),
        Some(json!("Still saving. The server is slow right now."))
    );
    answer(&mut m, save[0].0, 200, json!({}));
    assert_eq!(
        (m.get("app.slow"), m.get("app.saved")),
        (Some(json!("")), Some(json!(true)))
    );
    m.advance(2000);
    assert_eq!(m.get("app.saved"), Some(json!(false)));
}

#[test]
fn a_push_opens_the_review() {
    let mut m = Machine::new(example("Inbox"));
    m.fire(
        "push.review_ready",
        json!({ "contact": { "id": 42, "name": "Ada" }, "note": "Ready for review" }),
    );
    assert_eq!(
        (m.get("app.contact.name"), m.get("app.reviewing")),
        (Some(json!("Ada")), Some(json!(true)))
    );
}
