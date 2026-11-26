//! `toast` and `confirm` (spec 100, "Toast and confirm"): a short message that goes away on its own,
//! and a question whose list waits for the answer.

use genesis_core::bus::{Effect, Machine};
use genesis_core::config::{App, Variant};
use serde_json::{Value, json};

const APP: &str = r#"
[app]
name = "Ask"
toast_ms = 1000

[state]
deleted = ""
other = 0

[actions.remove]
do = [
  { confirm = "Delete {{ event.who }}?", yes = "Delete", no = "Keep" },
  { set = "app.deleted", to = "{{ event.who }}" },
  { toast = "Deleted {{ event.who }}.", variant = "success" },
]

[on]
"delete" = ["remove"]
"ask" = [{ confirm = "Sure?" }, { toast = "Sure, then.", ms = 3000 }]
"say" = [{ toast = "Hi {{ event.nobody }}." }]
"other" = [{ step = "app.other" }]
"broken" = [{ toggle = "app.deleted" }]
"#;

fn machine() -> Machine {
    let mut m = Machine::new(App::from_files([("app.toml", APP)]).unwrap());
    m.tap();
    m
}

fn toasts(m: &Machine) -> Vec<Value> {
    m.get("sys.toasts").unwrap().as_array().unwrap().clone()
}

fn asking(m: &Machine) -> Vec<Value> {
    m.get("sys.asking").unwrap().as_array().unwrap().clone()
}

fn names(m: &mut Machine) -> Vec<String> {
    m.take_tapped()
        .iter()
        .filter_map(|e| e["name"].as_str().map(String::from))
        .collect()
}

#[test]
fn a_toast_shows_its_text_and_variant_then_goes_after_toast_ms() {
    let mut m = machine();
    let effects = m.fire("say", json!({}));
    assert_eq!(
        effects,
        [Effect::Toast {
            text: "Hi .".into(),
            variant: Variant::Info
        }]
    );
    let shown = toasts(&m);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0]["text"], "Hi .");
    assert_eq!(shown[0]["variant"], "info");
    m.advance(999);
    assert_eq!(toasts(&m).len(), 1);
    m.advance(1);
    assert!(toasts(&m).is_empty());
}

#[test]
fn a_toasts_own_ms_wins_and_a_tap_takes_it_away_sooner() {
    let mut m = machine();
    m.fire("ask", json!({}));
    let id = asking(&m)[0]["id"].as_u64().unwrap();
    m.answer(id, true);
    m.advance(2000);
    assert_eq!(toasts(&m)[0]["text"], "Sure, then.");
    let toast = toasts(&m)[0]["id"].as_u64().unwrap();
    m.dismiss(toast);
    assert!(toasts(&m).is_empty());
    m.dismiss(toast);
}

#[test]
fn a_failure_nobody_handles_is_toasted_as_an_error() {
    let mut m = machine();
    m.fire("broken", json!({}));
    let shown = toasts(&m);
    assert_eq!(shown[0]["variant"], "error");
    assert!(shown[0]["text"].as_str().unwrap().starts_with("broken: "));
}

#[test]
fn confirm_waits_while_other_events_flow_and_yes_carries_on() {
    let mut m = machine();
    let effects = m.fire("delete", json!({ "who": "Ada" }));
    let [Effect::Confirm { id, text }] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    assert_eq!(text, "Delete Ada?");
    assert_eq!(
        asking(&m),
        [json!({ "id": id, "text": "Delete Ada?", "yes": "Delete", "no": "Keep" })]
    );
    m.fire("other", json!({}));
    assert_eq!(m.get("app.other"), Some(json!(1)));
    assert_eq!(m.get("app.deleted"), Some(json!("")));
    names(&mut m);
    m.answer(*id, true);
    assert_eq!(m.get("app.deleted"), Some(json!("Ada")));
    assert_eq!(toasts(&m)[0]["text"], "Deleted Ada.");
    assert_eq!(toasts(&m)[0]["variant"], "success");
    assert!(names(&mut m).contains(&"remove.done".to_string()));
    assert!(asking(&m).is_empty());
}

#[test]
fn no_stops_the_list_quietly() {
    let mut m = machine();
    m.fire("delete", json!({ "who": "Ada" }));
    let id = asking(&m)[0]["id"].as_u64().unwrap();
    names(&mut m);
    let effects = m.answer(id, false);
    assert!(effects.is_empty(), "{effects:?}");
    assert_eq!(m.get("app.deleted"), Some(json!("")));
    assert!(
        names(&mut m).is_empty(),
        "nothing fires: no .done, no .failed"
    );
    assert!(m.get("sys.errors").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn questions_wait_their_turn_and_an_answer_to_nothing_fails_loud() {
    let mut m = machine();
    m.fire("delete", json!({ "who": "Ada" }));
    m.fire("delete", json!({ "who": "Lin" }));
    let waiting = asking(&m);
    assert_eq!(waiting[0]["text"], "Delete Ada?");
    assert_eq!(waiting[1]["text"], "Delete Lin?");
    let lin = waiting[1]["id"].as_u64().unwrap();
    m.answer(lin, true);
    assert_eq!(m.get("app.deleted"), Some(json!("Lin")));
    assert_eq!(asking(&m).len(), 1);
    m.answer(lin, true);
    let errors = m.get("sys.errors").unwrap().to_string();
    assert!(errors.contains("which isn't waiting"), "{errors}");
}

#[test]
fn confirms_buttons_default_to_yes_and_no() {
    let mut m = machine();
    m.fire("ask", json!({}));
    assert_eq!(asking(&m)[0]["yes"], "Yes");
    assert_eq!(asking(&m)[0]["no"], "No");
}

fn problems(files: &[(&str, &str)]) -> String {
    App::from_files(files.iter().copied())
        .expect_err("refused")
        .iter()
        .map(|p| p.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_bad_variant_or_a_zero_ms_is_a_config_error() {
    let app = "[app]\nname = \"x\"\n[on]\n\"a\" = [{ toast = \"hi\", variant = \"loud\" }]\n";
    assert!(problems(&[("app.toml", app)]).contains("loud"));
    let app = "[app]\nname = \"x\"\n[on]\n\"a\" = [{ toast = \"hi\", ms = 0 }]\n";
    assert!(problems(&[("app.toml", app)]).contains("`ms` is at least 1"));
}

#[test]
fn a_components_own_steps_cant_toast_or_confirm() {
    let card = r#"
[component]

[[body]]
type = "button"
id = "a"
label = "A"
on_click = [{ toast = "hi" }]

[[body]]
type = "button"
id = "b"
label = "B"
on_click = [{ confirm = "Sure?" }]
"#;
    let app = "[app]\nname = \"x\"\n[[body]]\ntype = \"use\"\ncomponent = \"card\"\n";
    let said = problems(&[("app.toml", app), ("card.toml", card)]);
    assert!(
        said.contains("which shows a toast, won't work here"),
        "{said}"
    );
    assert!(
        said.contains("which asks a question, won't work here"),
        "{said}"
    );
}
