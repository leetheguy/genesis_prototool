//! Drawing the controls with `var` (spec 040, "Controls"): each as its own HTML element, showing
//! its variable's current value. Typing and picking are checked in a real browser
//! (`tests/deployed/controls.mjs`); here, a change goes in the way the browser sends it, as
//! `NAME.change` with its `value`.

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

/// The HTML drawn for `body` (TOML `[[body]]` tables), with `[state]` lines too, after `before`
/// has done whatever it does to the host.
fn html(state: &str, body: &str, before: impl FnOnce(&Host)) -> String {
    let toml = format!("[app]\nname = \"x\"\n[state]\n{state}\n{body}\n");
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(&toml).unwrap());
        before(&host);
        app_view(host).to_html()
    })
}

#[test]
fn an_input_is_a_daisyui_input_of_its_kind_showing_its_variable() {
    let out = html(
        "query = \"ada\"",
        "[[body]]\ntype = \"input\"\nvar = \"app.query\"\nkind = \"search\"\nplaceholder = \"Name\"",
        |_| {},
    );
    assert!(out.contains(r#"<input type="search""#), "{out}");
    assert!(out.contains(r#"class="input""#), "{out}");
    assert!(out.contains(r#"placeholder="Name""#), "{out}");
    assert!(out.contains(r#"value="ada""#), "{out}");
}

#[test]
fn an_input_follows_its_variable_after_a_change() {
    let out = html(
        "qty = 1",
        "[[body]]\ntype = \"input\"\nid = \"qty_box\"\nvar = \"app.qty\"\nkind = \"number\"",
        |host| host.fire("qty_box.change", json!({ "value": 12 })),
    );
    assert!(out.contains(r#"value="12""#), "{out}");
}

#[test]
fn an_input_shows_null_as_empty() {
    let out = html(
        "qty = 1",
        "[[body]]\ntype = \"input\"\nid = \"qty_box\"\nvar = \"app.qty\"\nkind = \"number\"",
        |host| host.fire("qty_box.change", json!({ "value": null })),
    );
    assert!(out.contains(r#"value="""#), "{out}");
}

#[test]
fn a_label_wraps_its_control_so_screen_readers_name_it() {
    let out = html(
        "who = \"Ada\"\nnote = \"\"",
        "[[body]]\ntype = \"textarea\"\nvar = \"app.note\"\nlabel = \"Note for {{ app.who }}\"\nrows = 4\nspan = 2",
        |_| {},
    );
    assert!(
        out.contains(r#"<label class="fieldset col-span-2"><span class="fieldset-legend">Note for Ada</span><textarea"#),
        "{out}"
    );
    assert!(out.contains(r#"class="textarea""#), "{out}");
    assert!(out.contains(r#"rows="4""#), "{out}");
}

#[test]
fn a_control_without_a_label_takes_its_placement_itself() {
    let out = html(
        "q = \"\"",
        "[[body]]\ntype = \"input\"\nvar = \"app.q\"\nwidth = \"1/2\"",
        |_| {},
    );
    assert!(out.contains(r#"class="input w-1/2""#), "{out}");
    assert!(!out.contains("<label"), "{out}");
}

#[test]
fn a_textarea_shows_its_variable() {
    let out = html(
        "note = \"hello\"",
        "[[body]]\ntype = \"textarea\"\nvar = \"app.note\"",
        |_| {},
    );
    assert!(out.contains(">hello</textarea>"), "{out}");
}

#[test]
fn a_checkbox_and_a_toggle_are_checked_while_their_variable_is_true() {
    let body = "[[body]]\ntype = \"checkbox\"\nvar = \"app.a\"\nlabel = \"Agree\"\n[[body]]\ntype = \"toggle\"\nvar = \"app.b\"";
    let out = html("a = true\nb = false", body, |_| {});
    assert!(
        out.contains(r#"<label class="label"><input type="checkbox" checked class="checkbox">"#),
        "{out}"
    );
    assert!(out.contains("<span>Agree</span>"), "{out}");
    assert!(
        out.contains(r#"<input type="checkbox" class="toggle">"#),
        "{out}"
    );
}

#[test]
fn radios_on_one_var_are_one_group_and_the_one_matching_it_is_picked() {
    let body = "[[body]]\ntype = \"radio\"\nvar = \"app.size\"\nvalue = 1\nlabel = \"One\"\n[[body]]\ntype = \"radio\"\nid = \"three\"\nvar = \"app.size\"\nvalue = 3\nlabel = \"Three\"";
    let out = html("size = 1", body, |host| {
        host.fire("three.change", json!({ "value": 3 }))
    });
    assert_eq!(out.matches(r#"name="app.size""#).count(), 2, "{out}");
    assert!(
        out.contains(r#"name="app.size" checked class="radio""#)
            && out.matches("checked").count() == 1,
        "{out}"
    );
    let picked = out.find("checked").unwrap();
    assert!(
        out[picked..].contains("Three") && !out[picked..].contains("One"),
        "{out}"
    );
}

#[test]
fn a_radio_matches_its_value_by_type_too() {
    let body = "[[body]]\ntype = \"radio\"\nvar = \"app.size\"\nvalue = 3";
    assert!(!html("size = \"3\"", body, |_| {}).contains("checked"));
    assert!(html("size = 3", body, |_| {}).contains("checked"));
}

#[test]
fn a_select_lists_its_options_and_picks_the_one_matching_its_variable() {
    let body = "[[body]]\ntype = \"select\"\nvar = \"app.size\"\noptions = [{ label = \"Small\", value = 1 }, { label = \"Large\", value = 3 }]";
    let out = html("size = 3", body, |_| {});
    assert!(out.contains(r#"<select class="select""#), "{out}");
    assert!(out.contains(r#"<option value="0">Small</option>"#), "{out}");
    assert!(
        out.contains(r#"<option value="1" selected>Large</option>"#),
        "{out}"
    );
}

#[test]
fn a_select_shows_its_placeholder_while_the_value_matches_no_option() {
    let body = "[[body]]\ntype = \"select\"\nvar = \"app.size\"\nplaceholder = \"Pick a size\"\noptions = [{ label = \"Small\", value = 1 }]";
    let out = html("size = 9", body, |_| {});
    assert!(
        out.contains(r#"<option value="" disabled selected>Pick a size</option>"#),
        "{out}"
    );
    let out = html("size = 1", body, |_| {});
    assert!(
        out.contains(r#"<option value="" disabled>Pick a size</option>"#),
        "{out}"
    );
}

#[test]
fn a_select_takes_its_options_from_a_variable_by_pointers() {
    let body = "[[body]]\ntype = \"select\"\nvar = \"app.company\"\noptions = \"app.companies\"\noption_label = \"/name\"\noption_value = \"/id\"";
    let state =
        "company = 2\ncompanies = [{ name = \"Acme\", id = 1 }, { name = \"Globex\", id = 2 }]";
    let out = html(state, body, |_| {});
    assert!(out.contains(r#"<option value="0">Acme</option>"#), "{out}");
    assert!(
        out.contains(r#"<option value="1" selected>Globex</option>"#),
        "{out}"
    );
}

#[test]
fn a_select_on_sys_theme_changes_the_screens_theme() {
    let body = "[[body]]\ntype = \"select\"\nid = \"theme\"\nvar = \"sys.theme\"\noptions = [{ label = \"Light\", value = \"light\" }, { label = \"Dark\", value = \"dark\" }]";
    let out = html("", body, |host| {
        host.fire("theme.change", json!({ "value": "dark" }))
    });
    assert!(out.contains(r#"data-theme="dark""#), "{out}");
    assert!(
        out.contains(r#"<option value="1" selected>Dark</option>"#),
        "{out}"
    );
}

#[test]
fn a_selects_options_variable_that_isnt_a_list_shows_a_loud_failure() {
    let body = "[[body]]\ntype = \"select\"\nvar = \"app.company\"\noptions = \"app.companies\"";
    let out = html("company = 1\ncompanies = \"oops\"", body, |_| {});
    assert!(out.contains(r#"role="alert""#), "{out}");
    assert!(
        out.contains("a select's options need a list, and app.companies isn't one"),
        "{out}"
    );
    assert!(!out.contains("<select"), "{out}");
}

#[test]
fn a_select_with_no_options_yet_is_an_empty_select() {
    let body = "[[body]]\ntype = \"select\"\nvar = \"app.company\"\noptions = \"app.companies\"";
    let out = html("company = 1\ncompanies = []", body, |_| {});
    assert!(out.contains(r#"<select class="select">"#), "{out}");
    assert!(!out.contains("<option"), "{out}");
}
