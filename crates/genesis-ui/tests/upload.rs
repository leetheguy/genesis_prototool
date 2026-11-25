//! Drawing a `file` (spec 040, "Controls"): a real file picker with daisyUI's class, its label,
//! what it accepts, and whether it takes several. Picking files, the size check, and sending them
//! are checked in a real browser (`tests/deployed/file.mjs`).

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;

fn html(toml: &str) -> String {
    let toml = format!("[app]\nname = \"x\"\n[[body]]\ntype = \"form\"\nid = \"up\"\n{toml}\n");
    Owner::new().with(|| app_view(Host::new(App::from_toml(&toml).unwrap())).to_html())
}

#[test]
fn a_file_is_a_file_input_inside_its_label() {
    let out = html(
        "width = \"1/2\"\n[[body.children]]\ntype = \"file\"\nname = \"doc\"\nlabel = \"Document\"\naccept = \".pdf\"\nwidth = \"1/2\"",
    );
    assert!(out.contains(r#"<label class="fieldset w-1/2">"#), "{out}");
    assert!(out.contains("Document"), "{out}");
    assert!(out.contains(r#"type="file""#), "{out}");
    assert!(out.contains(r#"class="file-input""#), "{out}");
    assert!(out.contains(r#"accept=".pdf""#), "{out}");
    assert!(!out.contains("multiple"), "{out}");
}

#[test]
fn a_file_without_a_label_takes_its_placement_on_one_element_and_can_take_several() {
    let out = html(
        "[[body.children]]\ntype = \"file\"\nname = \"docs\"\nmultiple = true\nwidth = \"1/3\"",
    );
    assert!(
        out.contains(r#"<div class="w-1/3"><input type="file""#),
        "{out}"
    );
    assert!(out.contains("multiple"), "{out}");
}
