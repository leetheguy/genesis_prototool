//! Templates: `{{ name.path }}` lookups (spec 020, "Templates").

use genesis_core::template;
use serde_json::{Value, json};

fn look(path: &str) -> Option<Value> {
    let vars = json!({
        "app": { "n": 3, "name": "Ada", "list": [1, 2], "none": null, "card": { "title": "Hi" } },
        "event": { "name": "hello_btn.click" }
    });
    path.split('.')
        .try_fold(&vars, |at, key| at.get(key))
        .cloned()
}

#[test]
fn text_fills_in_each_template() {
    assert_eq!(
        template::text("{{ app.name }} has {{app.n}} cards", look),
        "Ada has 3 cards"
    );
}

#[test]
fn text_shows_values_as_plain_text() {
    assert_eq!(
        template::text("[{{ app.list }}|{{ app.none }}|{{ app.card }}]", look),
        r#"[[1,2]||{"title":"Hi"}]"#
    );
}

#[test]
fn text_shows_a_missing_value_as_empty() {
    assert_eq!(template::text("a{{ app.gone }}b", look), "ab");
}

#[test]
fn text_without_templates_is_unchanged() {
    assert_eq!(template::text("just {braces}", look), "just {braces}");
}

#[test]
fn a_string_that_is_only_a_template_keeps_the_values_type() {
    assert_eq!(
        template::value(&json!("{{ app.list }}"), look),
        Ok(json!([1, 2]))
    );
    assert_eq!(template::value(&json!("{{ app.n }}"), look), Ok(json!(3)));
}

#[test]
fn a_template_inside_a_longer_string_becomes_text() {
    assert_eq!(
        template::value(&json!("n={{ app.n }}"), look),
        Ok(json!("n=3"))
    );
}

#[test]
fn values_inside_tables_and_lists_are_filled_in() {
    let filled = template::value(
        &json!({ "id": "{{ app.n }}", "tags": ["{{ app.name }}", 7] }),
        look,
    );
    assert_eq!(filled, Ok(json!({ "id": 3, "tags": ["Ada", 7] })));
}

#[test]
fn a_missing_value_is_an_error_naming_it() {
    assert_eq!(
        template::value(&json!({ "id": "{{ app.gone }}" }), look),
        Err("app.gone isn't set".to_string())
    );
}

#[test]
fn check_accepts_plain_lookups_only() {
    assert_eq!(
        template::check("{{ app.contact.name }} and {{ event.row.0 }}"),
        Ok(())
    );
    assert!(template::check("{{ app.n | upper }}").is_err());
    assert!(template::check("{{ app.n + 1 }}").is_err());
    assert!(template::check("{{ }}").is_err());
    assert!(template::check("{{ app.n").is_err());
}

#[test]
fn check_finds_a_bad_template_right_after_a_good_one() {
    assert!(template::check("{{ a }}{{ b + c }}").is_err());
}
