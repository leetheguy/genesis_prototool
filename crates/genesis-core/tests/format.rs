//! Formats (spec 080, "Formats"): each one turns its example into what's shown, a value it can't
//! take is an error (and shown as it is), and a badly written one is a config error. These run in
//! US English and UTC; the browser's own language is checked in `tests/deployed/format.mjs`.

use genesis_core::config::{App, Node};
use genesis_core::format::{English, Format, check_locale};
use serde_json::{Value, json};

/// 2026-11-21T12:30:00Z, in milliseconds since 1970.
const NOW: i64 = 1_795_264_200_000;

fn format(written: Value) -> Format {
    serde_json::from_value(written.clone()).unwrap_or_else(|e| panic!("{written}: {e}"))
}

fn shown(written: Value, value: Value) -> Result<String, String> {
    format(written).apply(&value, &English::at(NOW))
}

#[test]
fn now_is_what_the_tests_say_it_is() {
    assert_eq!(
        shown(
            json!({ "date": "%Y-%m-%dT%H:%M:%S" }),
            json!("2026-11-21T12:30:00Z")
        ),
        Ok("2026-11-21T12:30:00".into())
    );
    assert_eq!(
        shown(json!({ "ago": true }), json!("2026-11-21T12:30:00Z")),
        Ok("now".into())
    );
}

#[test]
fn each_format_turns_its_example_into_what_the_spec_says() {
    let cases = [
        (json!({ "number": 2 }), json!(1234.5), "1,234.50"),
        (json!({ "number": 0 }), json!(-1234567.8), "-1,234,568"),
        (json!({ "currency": "USD" }), json!(1234.5), "$1,234.50"),
        (json!({ "currency": "USD" }), json!(-5), "-$5.00"),
        (json!({ "currency": "JPY" }), json!(1234.5), "¥1,235"),
        (json!({ "currency": "CHF" }), json!(1234.5), "CHF 1,234.50"),
        (json!({ "percent": 1 }), json!(0.1234), "12.3%"),
        (json!({ "percent": 0 }), json!(1), "100%"),
        (
            json!({ "date": "%b %d, %Y" }),
            json!("2026-11-21T09:30:00Z"),
            "Nov 21, 2026",
        ),
        (
            json!({ "date": "%A %-d %B, %-I:%M %p" }),
            json!("2026-11-21T21:05:00Z"),
            "Saturday 21 November, 9:05 PM",
        ),
        (
            json!({ "date": "%a %e %y %H:%M:%S %%" }),
            json!("2026-03-01T00:00:09Z"),
            "Sun 1 26 00:00:09 %",
        ),
        (
            json!({ "ago": true }),
            json!("2026-11-21T09:30:00Z"),
            "3 hours ago",
        ),
        (
            json!({ "ago": true }),
            json!("2026-11-21T12:29:00Z"),
            "1 minute ago",
        ),
        (
            json!({ "ago": true }),
            json!("2026-11-23T13:00:00Z"),
            "in 2 days",
        ),
        (
            json!({ "ago": true }),
            json!("2024-11-21T12:30:00Z"),
            "2 years ago",
        ),
        (json!({ "bytes": true }), json!(1234.5), "1.2 KB"),
        (json!({ "bytes": true }), json!(999), "999 B"),
        (json!({ "bytes": true }), json!(1000), "1 KB"),
        (json!({ "bytes": true }), json!(1_500_000), "1.5 MB"),
        (
            json!({ "case": "upper" }),
            json!("hello world"),
            "HELLO WORLD",
        ),
        (
            json!({ "case": "lower" }),
            json!("Hello World"),
            "hello world",
        ),
        (
            json!({ "case": "title" }),
            json!("hello wORLD"),
            "Hello World",
        ),
        (
            json!({ "truncate": 10 }),
            json!("Hello there world"),
            "Hello the…",
        ),
        (json!({ "truncate": 10 }), json!("Short"), "Short"),
        (
            json!({ "map": { "active": "Active", "1": "One", "true": "Yes" } }),
            json!("active"),
            "Active",
        ),
        (
            json!({ "map": { "active": "Active", "1": "One", "true": "Yes" } }),
            json!(1),
            "One",
        ),
        (
            json!({ "map": { "active": "Active", "1": "One", "true": "Yes" } }),
            json!(true),
            "Yes",
        ),
        (
            json!({ "map": { "active": "Active" } }),
            json!("lost"),
            "lost",
        ),
    ];
    for (written, value, want) in cases {
        assert_eq!(
            shown(written.clone(), value.clone()),
            Ok(want.into()),
            "{written} on {value}"
        );
    }
}

#[test]
fn a_date_with_no_zone_is_shown_as_written_and_one_with_a_zone_is_moved_to_local_time() {
    let day = json!({ "date": "%Y-%m-%d %H:%M" });
    assert_eq!(
        shown(day.clone(), json!("2026-11-21")),
        Ok("2026-11-21 00:00".into())
    );
    assert_eq!(
        shown(day.clone(), json!("2026-11-21T09:30")),
        Ok("2026-11-21 09:30".into())
    );
    // English is UTC: an hour ahead of UTC moves back an hour.
    assert_eq!(
        shown(day.clone(), json!("2026-11-21T00:30:00.250+01:00")),
        Ok("2026-11-20 23:30".into())
    );
    assert_eq!(
        shown(day, json!("2024-02-29 23:59:59-0500")),
        Ok("2024-03-01 04:59".into())
    );
}

#[test]
fn a_value_a_format_cant_take_is_an_error() {
    let cases = [
        (json!({ "currency": "USD" }), json!("12.50")),
        (json!({ "number": 2 }), json!(null)),
        (json!({ "percent": 1 }), json!([1])),
        (json!({ "bytes": true }), json!("1 KB")),
        (json!({ "date": "%Y" }), json!(1_763_728_200)),
        (json!({ "date": "%Y" }), json!("2026-02-30")),
        (json!({ "date": "%Y" }), json!("21/11/2026")),
        (json!({ "ago": true }), json!("yesterday")),
        (json!({ "case": "upper" }), json!(3)),
        (json!({ "truncate": 5 }), json!(false)),
    ];
    for (written, value) in cases {
        assert!(
            shown(written.clone(), value.clone()).is_err(),
            "{written} on {value}"
        );
    }
}

#[test]
fn a_badly_written_format_says_why() {
    let cases = [
        (json!("currency"), "a format is a table"),
        (json!({}), "a format has one key"),
        (
            json!({ "number": 2, "currency": "USD" }),
            "a format has one key",
        ),
        (json!({ "round": 2 }), "there's no format `round`"),
        (json!({ "number": 21 }), "0 to 20"),
        (json!({ "percent": -1 }), "0 to 20"),
        (json!({ "currency": "usd" }), "three-letter currency code"),
        (json!({ "date": "%Q" }), "isn't one of them"),
        (json!({ "date": "%-B" }), "isn't one of them"),
        (json!({ "date": "ends in %" }), "isn't one of them"),
        (json!({ "ago": false }), "`ago` takes true"),
        (json!({ "bytes": "yes" }), "`bytes` takes true"),
        (
            json!({ "case": "shout" }),
            "\"upper\", \"lower\", or \"title\"",
        ),
        (json!({ "truncate": 1 }), "2 or more"),
        (json!({ "map": { "a": 1 } }), "not text"),
        (json!({ "map": "a" }), "a table of labels"),
    ];
    for (written, why) in cases {
        let said = serde_json::from_value::<Format>(written.clone())
            .unwrap_err()
            .to_string();
        assert!(said.contains(why), "{written}: {said}");
    }
}

#[test]
fn a_format_survives_the_trip_to_the_browser() {
    for written in [
        json!({ "number": 2 }),
        json!({ "date": "%b %-d, %Y" }),
        json!({ "ago": true }),
        json!({ "map": { "a": "A" } }),
    ] {
        assert_eq!(Value::from(format(written.clone())), written);
    }
}

#[test]
fn text_takes_a_format_and_the_app_a_locale() {
    let app = App::from_toml(
        "[app]\nname = \"x\"\nlocale = \"en-GB\"\n[state]\ntotal = 12.5\n[[body]]\ntype = \"text\"\ntext = \"Total {{ app.total }}\"\nformat = { currency = \"GBP\" }\n",
    )
    .expect("loads");
    assert_eq!(app.app.locale.as_deref(), Some("en-GB"));
    let Node::Text(text) = &app.body[0] else {
        panic!("not text")
    };
    assert_eq!(text.format, Some(format(json!({ "currency": "GBP" }))));
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
}

#[test]
fn format_goes_only_on_text_and_locale_is_a_language_tag() {
    let refused = |toml: &str| {
        App::from_toml(&format!("[app]\nname = \"x\"\n{toml}"))
            .unwrap_err()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(
        refused("[[body]]\ntype = \"button\"\nlabel = \"x\"\nformat = { number = 2 }\n")
            .contains("format")
    );
    assert!(refused("locale = \"English\"\n").contains("language tag"));
    for tag in ["en", "en-GB", "pt-BR", "zh-Hant-TW", "es-419"] {
        assert_eq!(check_locale(tag), None, "{tag}");
    }
    for tag in ["", "EN", "en_GB", "e", "en-", "english"] {
        assert!(check_locale(tag).is_some(), "{tag}");
    }
}
