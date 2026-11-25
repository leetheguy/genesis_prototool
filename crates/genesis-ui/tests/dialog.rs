//! Drawing a `dialog` (spec 040, "The atoms"): a real `dialog` with daisyUI's modal classes, its
//! title and children inside the box, and a backdrop that closes it. Opening it with its variable
//! and closing it with Escape are checked in a real browser (`tests/deployed/dialog.mjs`).

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;

fn html(toml: &str) -> String {
    let toml = format!("[app]\nname = \"x\"\n[state]\nasking = true\n{toml}\n");
    Owner::new().with(|| app_view(Host::new(App::from_toml(&toml).unwrap())).to_html())
}

#[test]
fn a_dialog_is_a_modal_dialog_holding_its_title_and_children() {
    let out = html(
        "[[body]]\ntype = \"dialog\"\nopen = \"app.asking\"\ntitle = \"Sure, {{ app.asking }}?\"\n[[body.children]]\ntype = \"text\"\ntext = \"Inside\"",
    );
    assert!(out.contains(r#"<dialog class="modal""#), "{out}");
    assert!(
        out.contains(r#"<div class="modal-box"><h3 class="text-lg font-bold">Sure, true?</h3>"#),
        "{out}"
    );
    assert!(
        out.find("Inside").unwrap() < out.find("modal-backdrop").unwrap(),
        "{out}"
    );
    assert!(
        out.contains(r#"<form method="dialog" class="modal-backdrop">"#),
        "{out}"
    );
    // The browser opens it with showModal(), never with the attribute, which wouldn't be modal.
    assert!(!out.contains("<dialog class=\"modal\" open"), "{out}");
}

#[test]
fn a_dialog_without_a_title_has_no_heading() {
    let out = html("[[body]]\ntype = \"dialog\"\nopen = \"app.asking\"");
    assert!(!out.contains("<h3"), "{out}");
}
