//! The CRM sample's wiring (spec 130): `samples/crm/configs` loads, uses every page and component it
//! has, and each thing the person does makes the calls it should, with canned answers standing in for
//! the API. The real API and a real browser are `tests/deployed/crm.mjs`.

use genesis_core::bus::{Call, Effect, Machine, Reply};
use genesis_core::config::{App, Variant, outlet_components};
use serde_json::{Value, json};
use std::path::Path;

/// Every `.toml` file under `dir`, by its path from `dir`, with its text.
fn files(dir: &Path, under: &str, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let at = if under.is_empty() {
            name
        } else {
            format!("{under}/{name}")
        };
        if path.is_dir() {
            files(&path, &at, out);
        } else if at.ends_with(".toml") {
            out.push((at, std::fs::read_to_string(&path).unwrap()));
        }
    }
}

fn app() -> App {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/crm/configs");
    let mut found = Vec::new();
    files(&dir, "", &mut found);
    App::from_files(found).unwrap_or_else(|problems| {
        panic!(
            "{}",
            problems
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

fn calls(effects: &[Effect]) -> Vec<Call> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Call(call) => Some(call.clone()),
            _ => None,
        })
        .collect()
}

/// Each call by its operation and params.
fn asked(effects: &[Effect]) -> Vec<(String, Value)> {
    calls(effects)
        .into_iter()
        .map(|c| (c.op, c.params))
        .collect()
}

fn toasts(effects: &[Effect]) -> Vec<(String, Variant)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Toast { text, variant } => Some((text.clone(), *variant)),
            _ => None,
        })
        .collect()
}

const ACME: &str =
    r#"{ "id": 1, "name": "Acme", "website": "https://acme.example.com", "contacts": 2 }"#;

/// What the API would answer each list with.
fn answer(call: &Call) -> Value {
    let acme: Value = serde_json::from_str(ACME).unwrap();
    match call.op.as_str() {
        "crm.companies.list" => {
            json!({ "rows": [acme, { "id": 2, "name": "Globex", "website": "", "contacts": 1 }] })
        }
        "crm.contacts.list" => {
            json!({ "rows": [{ "id": 1, "name": "Ada Lovelace", "email": "ada@acme.example.com", "company_id": 1, "company": "Acme" }] })
        }
        _ => json!({}),
    }
}

/// Answers every call in `effects`, and any calls those answers lead to, with a 2xx; hands back
/// every effect along the way.
fn settle(m: &mut Machine, effects: Vec<Effect>) -> Vec<Effect> {
    let mut all = effects.clone();
    let mut waiting = calls(&effects);
    while let Some(call) = waiting.pop() {
        let more = m.reply(
            call.id,
            Reply::Response {
                status: 200,
                body: answer(&call),
            },
        );
        waiting.extend(calls(&more));
        all.extend(more);
    }
    all
}

/// The machine, opened, with its first lists in.
fn opened() -> Machine {
    let mut m = Machine::new(app());
    let effects = m.open();
    settle(&mut m, effects);
    m
}

const RELOADS: [&str; 4] = [
    "crm.companies.list",
    "crm.contacts.list",
    "crm.companies.list",
    "crm.contacts.list",
];

#[test]
fn it_loads_and_uses_every_page_and_component() {
    let app = app();
    let gone_to = outlet_components(&app);
    for page in ["companies", "company", "contacts"] {
        assert!(gone_to.contains(page), "nothing goes to {page}");
    }
    let mut used = Vec::new();
    files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/crm/configs"),
        "",
        &mut used,
    );
    let text: String = used.iter().map(|(_, t)| t.as_str()).collect();
    for name in app.components.keys() {
        assert!(
            gone_to.contains(name) || text.contains(&format!("component = \"{name}\"")),
            "{name} isn't used"
        );
    }
    assert_eq!(app.components.len(), 5);
}

#[test]
fn opening_fetches_the_lists_and_searching_asks_the_api() {
    let mut m = Machine::new(app());
    let mut first: Vec<String> = asked(&m.open()).into_iter().map(|(op, _)| op).collect();
    first.sort();
    assert_eq!(
        first,
        [
            "crm.companies.list",
            "crm.companies.list",
            "crm.contacts.list",
            "crm.contacts.list"
        ]
    );
    assert_eq!(
        asked(&m.fire("companies.search.change", json!({ "value": "glo" }))),
        [("crm.companies.list".to_string(), json!({ "q": "glo" }))]
    );
    assert_eq!(
        asked(&m.fire("contacts.search.change", json!({ "value": "acme" }))),
        [("crm.contacts.list".to_string(), json!({ "q": "acme" }))]
    );
}

#[test]
fn opening_a_company_shows_its_page_and_fetches_its_contacts() {
    let mut m = opened();
    let acme: Value = serde_json::from_str(ACME).unwrap();
    let effects = m.fire("companies.open.click", json!({ "item": acme }));
    assert_eq!(m.get("sys.outlet"), Some(json!("company")));
    assert_eq!(m.get("app.company.name"), Some(json!("Acme")));
    assert_eq!(
        asked(&effects),
        [("crm.contacts.list".to_string(), json!({ "company_id": 1 }))]
    );
    m.fire("company.back.click", json!({}));
    assert_eq!(m.get("sys.outlet"), Some(json!("companies")));
}

#[test]
fn adding_a_company_saves_closes_reloads_and_toasts() {
    let mut m = opened();
    m.fire("companies.add.click", json!({}));
    assert_eq!(m.get("app.adding_company"), Some(json!(true)));
    let effects = m.fire(
        "new_company.submit",
        json!({ "form": { "name": "Umbrella", "website": null } }),
    );
    assert_eq!(
        asked(&effects),
        [(
            "crm.companies.create".to_string(),
            json!({ "name": "Umbrella", "website": null })
        )]
    );
    let saved = settle(&mut m, effects);
    assert_eq!(m.get("app.adding_company"), Some(json!(false)));
    let ops: Vec<String> = asked(&saved)
        .into_iter()
        .skip(1)
        .map(|(op, _)| op)
        .collect();
    assert_eq!(ops.len(), RELOADS.len(), "{ops:?}");
    assert_eq!(
        toasts(&saved),
        [("Added Umbrella.".to_string(), Variant::Success)]
    );
}

#[test]
fn editing_a_company_sends_its_id_and_keeps_the_saved_row() {
    let mut m = opened();
    let acme: Value = serde_json::from_str(ACME).unwrap();
    m.fire("companies.open.click", json!({ "item": acme }));
    m.fire("company.edit.click", json!({}));
    assert_eq!(m.get("app.editing_company"), Some(json!(true)));
    let effects = m.fire(
        "company_changes.submit",
        json!({ "form": { "name": "Acme Corp", "website": "https://acme.example.com" } }),
    );
    let call = calls(&effects)[0].clone();
    assert_eq!(call.params["id"], 1);
    let after = m.reply(
        call.id,
        Reply::Response {
            status: 200,
            body: json!({ "id": 1, "name": "Acme Corp", "website": "", "contacts": 2 }),
        },
    );
    settle(&mut m, after);
    assert_eq!(m.get("app.company.name"), Some(json!("Acme Corp")));
    assert_eq!(m.get("app.editing_company"), Some(json!(false)));
}

#[test]
fn a_contact_added_from_a_company_starts_at_that_company() {
    let mut m = opened();
    let acme: Value = serde_json::from_str(ACME).unwrap();
    m.fire("companies.open.click", json!({ "item": acme }));
    m.fire("company.add_contact.click", json!({}));
    assert_eq!(m.get("app.new_contact"), Some(json!({ "company_id": 1 })));
    assert_eq!(m.get("app.adding_contact"), Some(json!(true)));
    // From the contacts page, it starts with no company.
    m.fire("contacts.add.click", json!({}));
    assert_eq!(m.get("app.new_contact"), Some(json!({})));
    let form =
        json!({ "name": "Bo", "email": "bo@acme.example.com", "phone": null, "company_id": 1 });
    let effects = m.fire("new_contact.submit", json!({ "form": form }));
    assert_eq!(asked(&effects), [("crm.contacts.create".to_string(), form)]);
}

#[test]
fn editing_a_contact_from_either_list_sends_its_id() {
    let ada = json!({ "id": 7, "name": "Ada Lovelace", "email": "ada@acme.example.com", "company_id": 1 });
    for click in ["contacts.edit.click", "company.edit_contact.click"] {
        let mut m = opened();
        m.fire(click, json!({ "item": ada }));
        assert_eq!(m.get("app.editing_contact"), Some(json!(true)), "{click}");
        let effects = m.fire("contact_changes.submit", json!({ "form": { "name": "Ada King", "email": "ada@acme.example.com", "phone": "", "company_id": 1 } }));
        let call = calls(&effects)[0].clone();
        assert_eq!(
            (call.op.as_str(), &call.params["id"]),
            ("crm.contacts.update", &json!(7)),
            "{click}"
        );
        let saved = settle(&mut m, effects);
        assert_eq!(
            toasts(&saved),
            [("Saved Ada King.".to_string(), Variant::Success)],
            "{click}"
        );
    }
}

#[test]
fn deleting_asks_first_and_a_refusal_is_loud() {
    let mut m = opened();
    let ada = json!({ "id": 7, "name": "Ada Lovelace" });
    for click in ["contacts.delete.click", "company.delete_contact.click"] {
        let effects = m.fire(click, json!({ "item": ada }));
        let [Effect::Confirm { id, text }] = effects.as_slice() else {
            panic!("{click} didn't ask: {effects:?}")
        };
        assert_eq!(text, "Delete Ada Lovelace?");
        assert!(
            calls(&m.answer(*id, false)).is_empty(),
            "Keep deleted something"
        );
    }
    let effects = m.fire("contacts.delete.click", json!({ "item": ada }));
    let Effect::Confirm { id, .. } = effects[0] else {
        panic!()
    };
    let effects = m.answer(id, true);
    assert_eq!(
        asked(&effects),
        [("crm.contacts.delete".to_string(), json!({ "id": 7 }))]
    );
    let done = settle(&mut m, effects);
    assert_eq!(
        toasts(&done),
        [("Deleted Ada Lovelace.".to_string(), Variant::Success)]
    );

    // The API refuses to delete a company that has contacts: its sentence is toasted, and nothing more runs.
    let acme: Value = serde_json::from_str(ACME).unwrap();
    let effects = m.fire("companies.delete.click", json!({ "item": acme }));
    let Effect::Confirm { id, .. } = effects[0] else {
        panic!()
    };
    let call = calls(&m.answer(id, true))[0].clone();
    let refused = m.reply(
        call.id,
        Reply::Response {
            status: 409,
            body: json!("Acme still has 2 contacts."),
        },
    );
    assert!(calls(&refused).is_empty(), "a refusal still reloaded");
    let shown = toasts(&refused);
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0].1, Variant::Error);
    assert!(
        shown[0].0.contains("HTTP 409: Acme still has 2 contacts."),
        "{shown:?}"
    );
}
