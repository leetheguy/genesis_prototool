//! `file` (spec 040, "Controls"): what loads, what's refused, file references, and how a picked
//! file goes into a form's draft and out with a call.

use genesis_core::bus::{Effect, Machine};
use genesis_core::config::{App, Node};
use genesis_core::file::{MAX_SIZE, reference, references, size_text, too_big};
use serde_json::{Map, Value, json};

fn app(toml: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"Files\"\n{toml}"))
}

fn problem(toml: &str) -> String {
    app(toml)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A reference as the browser makes one, to a made-up file.
fn picked(id: &str, name: &str) -> Value {
    json!({ "file": id, "name": name, "type": "text/plain", "size": 12 })
}

/// A form with a note and a file, whose `.submit` sends both to an upload operation.
const UPLOAD: &str = r#"
[connections.docs]
base_url = "https://pf.example.com/hooks"

[connections.docs.ops.upload]
method = "POST"
path = "/upload"

[[body]]
type = "form"
id = "upload"
on_submit = [{ call = "docs.upload", params = { note = "{{ event.form.note }}", doc = "{{ event.form.doc }}" } }]

[[body.children]]
type = "input"
id = "note_box"
name = "note"

[[body.children]]
type = "file"
id = "doc_box"
name = "doc"
label = "Document"
accept = ".txt,.pdf"
max_size = 5_000_000

[[body.children]]
type = "button"
label = "Send"
submit = true
"#;

#[test]
fn a_file_loads_with_its_options_and_survives_the_trip_to_the_browser() {
    let app = app(UPLOAD).expect("loads");
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
    let Node::Form(form) = &app.body[0] else {
        panic!("not a form")
    };
    let Node::File(file) = &form.children[1] else {
        panic!("not a file")
    };
    assert_eq!(file.name.as_deref(), Some("doc"));
    assert_eq!(file.accept.as_deref(), Some(".txt,.pdf"));
    assert_eq!(file.max_size, Some(5_000_000));
    assert!(!file.multiple);
}

#[test]
fn a_file_with_var_or_outside_a_form_is_a_config_error() {
    let inside = "[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"file\"\nname = \"doc\"\n";
    let out = problem(&format!("{inside}var = \"app.doc\""));
    assert!(out.contains("a `file` works only inside a `form`"), "{out}");
    let out = problem("[[body]]\ntype = \"file\"\nname = \"doc\"");
    assert!(out.contains("a `file` works only inside a `form`"), "{out}");
}

#[test]
fn a_file_in_a_form_needs_a_name() {
    let out =
        problem("[[body]]\ntype = \"form\"\n[[body.children]]\ntype = \"file\"\nlabel = \"Doc\"");
    assert!(out.contains("name"), "{out}");
}

#[test]
fn a_reference_is_exactly_file_name_type_and_size() {
    assert_eq!(reference(&picked("id-1", "notes.txt")), Some("id-1"));
    let mut extra = picked("id-1", "notes.txt");
    extra["more"] = json!(1);
    assert_eq!(reference(&extra), None);
    assert_eq!(
        reference(&json!({ "file": 7, "name": "a", "type": "b", "size": 1 })),
        None
    );
    assert_eq!(
        reference(&json!({ "file": "x", "name": "a", "kind": "b", "size": 1 })),
        None
    );
    assert_eq!(reference(&json!("id-1")), None);
}

#[test]
fn references_finds_a_param_that_is_one_and_each_in_a_list() {
    let params: Map<String, Value> = serde_json::from_value(json!({
        "note": "hi",
        "doc": picked("one", "a.txt"),
        "more": [picked("two", "b.txt"), "not one", picked("three", "c.txt")],
    }))
    .unwrap();
    let mut found = references(&params);
    found.sort();
    assert_eq!(found, ["one", "three", "two"]);
    assert!(references(&Map::new()).is_empty());
}

#[test]
fn too_big_names_both_sizes_and_zero_means_no_limit() {
    assert_eq!(MAX_SIZE, 100_000_000);
    assert_eq!(too_big("a.txt", 10, 10), None);
    assert_eq!(
        too_big("big.mov", 12_300_000, 5_000_000).as_deref(),
        Some("big.mov is 12.3 MB; this takes at most 5 MB.")
    );
    assert_eq!(too_big("huge.iso", u64::MAX, 0), None);
}

#[test]
fn size_text_says_bytes_kb_or_mb() {
    assert_eq!(size_text(512), "512 bytes");
    assert_eq!(size_text(800_000), "800 KB");
    assert_eq!(size_text(1_500), "1.5 KB");
    assert_eq!(size_text(12_340_000), "12.3 MB");
}

#[test]
fn a_picked_file_goes_into_the_draft_and_out_with_the_call_as_its_reference() {
    let mut m = Machine::new(app(UPLOAD).expect("loads"));
    m.fire("note_box.change", json!({ "value": "for the record" }));
    m.fire(
        "doc_box.change",
        json!({ "value": picked("id-1", "notes.txt") }),
    );
    assert_eq!(m.draft("upload", "doc"), Some(picked("id-1", "notes.txt")));
    let effects = m.fire("upload.submit", json!({}));
    let call = effects
        .iter()
        .find_map(|e| match e {
            Effect::Call(call) => Some(call.clone()),
            _ => None,
        })
        .expect("a call");
    assert_eq!(call.op, "docs.upload");
    assert_eq!(
        call.params,
        json!({ "note": "for the record", "doc": picked("id-1", "notes.txt") })
    );
    assert_eq!(m.draft("upload", "doc"), None);
}

#[test]
fn several_files_go_out_as_a_list() {
    let mut m = Machine::new(
        app(&UPLOAD.replace("max_size = 5_000_000", "multiple = true")).expect("loads"),
    );
    let both = json!([picked("a", "a.txt"), picked("b", "b.txt")]);
    m.fire("doc_box.change", json!({ "value": both }));
    assert_eq!(m.draft("upload", "doc"), Some(both));
}
