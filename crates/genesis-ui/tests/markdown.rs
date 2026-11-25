//! Drawing a `markdown` node (spec 040, "The atoms"): its HTML in a `div` with the `markdown` class
//! and its own style keys, templates filled first, and raw HTML shown as text.

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;

fn html(toml: &str) -> String {
    let toml = format!("[app]\nname = \"x\"\n[state]\nwho = \"Ada\"\n{toml}\n");
    Owner::new().with(|| app_view(Host::new(App::from_toml(&toml).unwrap())).to_html())
}

#[test]
fn markdown_is_drawn_as_html_in_one_styled_element() {
    let out = html(
        "[[body]]\ntype = \"markdown\"\ntext = \"## Hi {{ app.who }}\\n\\n- one\"\npad = 4\nwidth = \"1/2\"",
    );
    assert!(out.contains(r#"<div class="markdown p-4 w-1/2""#), "{out}");
    assert!(out.contains("<h2>Hi Ada</h2>"), "{out}");
    assert!(out.contains("<li>one</li>"), "{out}");
}

#[test]
fn raw_html_in_markdown_is_shown_as_text() {
    let out = html("[[body]]\ntype = \"markdown\"\ntext = \"<img src=x onerror=alert(1)>\"");
    assert!(!out.contains("<img"), "{out}");
    assert!(out.contains("&lt;img src=x onerror=alert(1)&gt;"), "{out}");
}
