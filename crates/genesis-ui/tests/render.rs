//! Drawing an app's nodes from its state (spec 040). Rendered to HTML natively; clicks are
//! checked in a real browser against the served app (spec 010, the hello deployed check).

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

const HELLO: &str = r#"
[app]
name = "Hello"
theme = "dark"

[state]
clicks = 0

[[body]]
type = "stack"

[[body.children]]
type = "heading"
text = "Hello"
level = 2

[[body.children]]
type = "text"
text = "Clicked {{ app.clicks }} times"

[[body.children]]
type = "button"
id = "hello_btn"
label = "Click me"

[on]
"hello_btn.click" = [{ step = "app.clicks" }]
"dark_off" = [{ set = "sys.theme", to = "light" }]
"#;

/// Draws `toml` as HTML, after `before` has done whatever it does to the host.
fn html(toml: &str, before: impl FnOnce(&Host)) -> String {
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(toml).expect("app loads"));
        before(&host);
        app_view(host).to_html()
    })
}

#[test]
fn a_heading_draws_at_its_level() {
    let out = html(HELLO, |_| {});
    assert!(out.contains("<h2"), "{out}");
    assert!(out.contains(">Hello</h2>"), "{out}");
}

#[test]
fn a_text_fills_its_templates_from_the_current_state() {
    assert!(html(HELLO, |_| {}).contains("Clicked 0 times"));
    let out = html(HELLO, |host| {
        host.fire("hello_btn.click", json!({}));
        host.fire("hello_btn.click", json!({}));
    });
    assert!(out.contains("Clicked 2 times"), "{out}");
}

#[test]
fn a_button_draws_as_a_daisyui_button_with_its_label() {
    let out = html(HELLO, |_| {});
    assert!(out.contains(r#"class="btn"#), "{out}");
    assert!(out.contains(">Click me</button>"), "{out}");
}

#[test]
fn a_stack_draws_its_children_in_order() {
    let out = html(HELLO, |_| {});
    let (h, t, b) = (
        out.find("<h2").unwrap(),
        out.find("Clicked").unwrap(),
        out.find("<button").unwrap(),
    );
    assert!(h < t && t < b, "{out}");
}

#[test]
fn the_screen_wears_the_theme_and_follows_it() {
    assert!(html(HELLO, |_| {}).contains(r#"data-theme="dark""#));
    let out = html(HELLO, |host| {
        host.fire("dark_off", json!({}));
    });
    assert!(out.contains(r#"data-theme="light""#), "{out}");
}

#[test]
fn a_failure_nobody_handles_shows_as_a_toast() {
    let out = html(HELLO, |host| {
        host.fire("oops.failed", json!({ "error": "nobody caught me" }));
    });
    assert!(out.contains("oops.failed: nobody caught me"), "{out}");
    assert!(out.contains("alert-error"), "{out}");
}

#[test]
fn opening_the_app_fires_app_opened() {
    let opened = Owner::new().with(|| {
        let host = Host::new(App::from_toml("[app]\nname = \"x\"\n[state]\nn = 0\n[on]\n\"app.opened\" = [{ step = \"app.n\" }]\n").unwrap());
        host.open();
        host.get("app.n")
    });
    assert_eq!(opened, Some(json!(1)));
}

#[test]
fn headings_draw_at_levels_one_and_three_too() {
    let out = html(
        "[app]\nname = \"x\"\n[[body]]\ntype = \"heading\"\ntext = \"Big\"\n[[body]]\ntype = \"heading\"\ntext = \"Small\"\nlevel = 3\n",
        |_| {},
    );
    assert!(
        out.contains(">Big</h1>") && out.contains(">Small</h3>"),
        "{out}"
    );
}

#[test]
fn advancing_the_clock_runs_timers_and_redraws() {
    let toml = "[app]\nname = \"x\"\n[state]\nn = 0\n[[body]]\ntype = \"text\"\ntext = \"n={{ app.n }}\"\n[on]\n\"app.opened\" = [{ every = 100, fire = \"tick\" }]\n\"tick\" = [{ step = \"app.n\" }]\n";
    let out = html(toml, |host| {
        host.open();
        host.advance(250);
    });
    assert!(out.contains("n=2"), "{out}");
}

#[test]
fn a_new_config_redraws_the_screen_and_keeps_the_state() {
    let out = html(HELLO, |host| {
        host.fire("hello_btn.click", json!({}));
        let changed = HELLO.replace("text = \"Hello\"", "text = \"Hello again\"");
        host.reload(App::from_toml(&changed).unwrap());
    });
    assert!(out.contains(">Hello again</h2>"), "{out}");
    assert!(
        out.contains("Clicked 1 times"),
        "the state carried over: {out}"
    );
}

#[test]
fn a_bad_config_shows_its_problems_over_the_last_good_screen() {
    let out = html(HELLO, |host| {
        host.show_problems("app.toml:3:1: unknown field `colour`")
    });
    assert!(
        out.contains("app.toml:3:1: unknown field `colour`"),
        "{out}"
    );
    assert!(
        out.contains("Click me"),
        "the last good screen stays: {out}"
    );
}

#[test]
fn a_good_config_clears_the_problems() {
    let out = html(HELLO, |host| {
        host.show_problems("app.toml:3:1: unknown field `colour`");
        host.reload(App::from_toml(HELLO).unwrap());
    });
    assert!(!out.contains("unknown field"), "{out}");
}
