//! `markdown` (spec 040, "The atoms"): Markdown becomes HTML, and nothing in it can run or reach
//! outside the page on its own.

use genesis_core::config::{App, Node};
use genesis_core::markdown::{html, safe};

#[test]
fn markdown_becomes_html_with_tables_strikethrough_and_task_lists() {
    let out = html(
        "# Notes\n\nSome **bold** and ~~gone~~.\n\n- [x] done\n- one\n\n| a | b |\n|---|---|\n| 1 | 2 |\n",
    );
    assert!(out.contains("<h1>Notes</h1>"), "{out}");
    assert!(out.contains("<strong>bold</strong>"), "{out}");
    assert!(out.contains("<del>gone</del>"), "{out}");
    assert!(out.contains(r#"type="checkbox""#), "{out}");
    assert!(out.contains("<td>1</td>"), "{out}");
}

#[test]
fn raw_html_shows_as_text() {
    let out = html("Hi <script>alert(1)</script> there\n\n<div onclick=\"x()\">block</div>\n");
    assert!(!out.contains("<script"), "{out}");
    assert!(out.contains("&lt;script&gt;"), "{out}");
    assert!(!out.contains("<div"), "{out}");
    assert!(out.contains("&lt;div onclick="), "{out}");
}

#[test]
fn a_link_or_image_to_an_unsafe_address_keeps_only_its_words() {
    let out = html("[click](javascript:alert(1)) and [ok](https://example.com) and [here](/page)");
    assert!(!out.contains("javascript"), "{out}");
    assert!(out.contains("click and"), "{out}");
    assert!(
        out.contains(r#"<a href="https://example.com">ok</a>"#),
        "{out}"
    );
    assert!(out.contains(r#"<a href="/page">here</a>"#), "{out}");
    let out = html("![a cat](data:image/svg+xml,x) ![a dog](https://example.com/dog.png)");
    assert!(!out.contains("data:"), "{out}");
    assert!(out.contains("a cat"), "{out}");
    assert!(
        out.contains(r#"<img src="https://example.com/dog.png" alt="a dog""#),
        "{out}"
    );
}

#[test]
fn safe_takes_a_few_schemes_and_relative_addresses() {
    for url in [
        "https://x.example",
        "http://x.example",
        "mailto:a@example.com",
        "tel:+15550100",
        "/a",
        "a/b",
        "#top",
        "?q=1",
    ] {
        assert!(safe(url), "{url}");
    }
    for url in [
        "javascript:x",
        " JavaScript:x",
        "java\tscript:x",
        "vbscript:x",
        "data:text/html,x",
        "file:///etc",
    ] {
        assert!(!safe(url), "{url}");
    }
    // A colon after the path starts is part of a relative address, not a scheme.
    assert!(safe("a/b:c"));
}

#[test]
fn a_markdown_node_loads_and_survives_the_trip_to_the_browser() {
    let app = App::from_toml(
        "[app]\nname = \"x\"\n[[body]]\ntype = \"markdown\"\ntext = \"# Hi\"\npad = 4\n",
    )
    .expect("loads");
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
    let Node::Markdown(md) = &app.body[0] else {
        panic!("not markdown")
    };
    assert_eq!(md.text, "# Hi");
}
