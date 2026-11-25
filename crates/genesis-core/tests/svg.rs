//! `svg` and `divider` (spec 040, "Media and embeds"): an svg is checked at load, and what's drawn is
//! Genesis's own copy of what it checked.

use genesis_core::config::{App, Node};
use genesis_core::svg::clean;

const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor">
  <!-- a check mark -->
  <title>Done</title>
  <path d="M5 13l4 4L19 7" stroke-width="2"/>
  <use href="#dot"/>
  <defs><linearGradient id="g"><stop offset="0" stop-color="red"/></linearGradient></defs>
  <rect width="2" height="2" style="fill: url(#g)"/>
</svg>"##;

#[test]
fn a_clean_svg_is_drawn_from_genesiss_own_copy() {
    assert_eq!(
        clean(ICON).unwrap(),
        concat!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" width="1em" height="1em">"##,
            "\n  \n  <title>Done</title>\n  ",
            r##"<path d="M5 13l4 4L19 7" stroke-width="2"></path>"##,
            "\n  ",
            r##"<use href="#dot"></use>"##,
            "\n  ",
            r##"<defs><linearGradient id="g"><stop offset="0" stop-color="red"></stop></linearGradient></defs>"##,
            "\n  ",
            r##"<rect width="2" height="2" style="fill: url(#g)"></rect>"##,
            "\n</svg>"
        )
    );
}

#[test]
fn cleaning_is_the_same_the_second_time() {
    let once = clean(ICON).unwrap();
    assert_eq!(clean(&once).unwrap(), once);
}

#[test]
fn an_svg_with_its_own_size_keeps_it_and_text_stays_escaped() {
    assert_eq!(
        clean(r#"<svg width="48" height="20"><text x="0" y="15">a &lt; b &amp; "c"</text></svg>"#)
            .unwrap(),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="20"><text x="0" y="15">a &lt; b &amp; "c"</text></svg>"#
    );
}

#[test]
fn an_svg_with_anything_that_could_run_or_reach_out_is_a_problem() {
    let cases = [
        (
            "<svg><script>alert(1)</script></svg>",
            "`svg` can't hold <script>; it draws shapes, text, and paint only",
        ),
        (
            "<svg><foreignObject><div/></foreignObject></svg>",
            "`svg` can't hold <foreignObject>; it draws shapes, text, and paint only",
        ),
        (
            "<svg><a href=\"#x\"><path d=\"M0 0\"/></a></svg>",
            "`svg` can't hold <a>; it draws shapes, text, and paint only",
        ),
        (
            "<svg><set attributeName=\"href\" to=\"x\"/></svg>",
            "`svg` can't hold <set>; it draws shapes, text, and paint only",
        ),
        (
            "<svg onload=\"alert(1)\"/>",
            "`svg` can't hold onload on <svg>: no event attributes",
        ),
        (
            "<svg><path ONCLICK=\"x\" d=\"M0 0\"/></svg>",
            "`svg` can't hold ONCLICK on <path>: no event attributes",
        ),
        (
            "<svg><use href=\"https://evil.example/a.svg#x\"/></svg>",
            "`svg` can't link outside itself: href=\"https://evil.example/a.svg#x\" on <use>",
        ),
        (
            "<svg xmlns:xlink=\"http://www.w3.org/1999/xlink\"><use xlink:href=\"data:x\"/></svg>",
            "`svg` can't link outside itself: href=\"data:x\" on <use>",
        ),
        (
            "<svg><rect style=\"fill: url(https://evil.example/x)\"/></svg>",
            "`svg` can't link outside itself: style=\"fill: url(https://evil.example/x)\" on <rect>",
        ),
        (
            "<svg><rect fill=\"java&#10;script:x\"/></svg>",
            "`svg` can't hold a javascript: URL (fill on <rect>)",
        ),
        (
            "<svg xmlns:i=\"urn:x\"><rect i:label=\"x\"/></svg>",
            "`svg` can't hold i:label on <rect>: only SVG's own attributes",
        ),
        (
            "<svg xmlns:i=\"urn:x\"><i:thing/></svg>",
            "`svg` can't hold <i:thing>; it draws shapes, text, and paint only",
        ),
    ];
    for (markup, message) in cases {
        assert_eq!(clean(markup).unwrap_err(), message, "{markup}");
    }
}

#[test]
fn markup_that_isnt_one_svg_element_is_a_problem() {
    assert_eq!(
        clean("<div/>").unwrap_err(),
        "`svg` is one <svg> element, not <div>"
    );
    let bad = clean("<svg><path></svg>").unwrap_err();
    assert!(bad.starts_with("`svg` isn't well-formed XML: "), "{bad}");
    let two = clean("<svg/><svg/>").unwrap_err();
    assert!(two.starts_with("`svg` isn't well-formed XML: "), "{two}");
}

fn app(node: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"x\"\n\n[[body]]\n{node}\n"))
}

#[test]
fn an_svg_node_loads_cleaned_with_a_label_and_type_and_placement_keys_and_survives_json() {
    let app = app("type = \"svg\"\nlabel = \"Done\"\ncolor = \"success\"\nsize = \"2xl\"\nspan = 1\nsvg = '''\n<svg viewBox=\"0 0 1 1\"><circle r=\"1\"/></svg>'''").unwrap();
    let Node::Svg(svg) = &app.body[0] else {
        panic!("not an svg")
    };
    assert_eq!(
        svg.svg.as_str(),
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1" width="1em" height="1em"><circle r="1"></circle></svg>"#
    );
    assert_eq!(svg.label.as_deref(), Some("Done"));
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app);
}

#[test]
fn a_bad_svg_is_a_problem_on_its_keys_line() {
    let problems = app("type = \"svg\"\nid = \"x\"\nsvg = \"<svg><script/></svg>\"").unwrap_err();
    assert_eq!(
        problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["app.toml:7:1: `svg` can't hold <script>; it draws shapes, text, and paint only"]
    );
}

#[test]
fn a_divider_loads_with_a_theme_color_and_the_box_and_placement_keys() {
    let app =
        app("type = \"divider\"\ncolor = \"primary\"\nmargin_y = 4\nspan = \"full\"").unwrap();
    assert!(matches!(app.body[0], Node::Divider(_)));
    assert!(app.body[0].placed().span.is_some());
    let problems = app_problem("type = \"divider\"\ncolor = \"muted\"");
    assert!(
        problems.starts_with("app.toml:6:1: `color` is \"base-100\""),
        "{problems}"
    );
}

fn app_problem(node: &str) -> String {
    app(node).unwrap_err()[0].to_string()
}

#[test]
fn quotes_and_markup_in_an_attribute_stay_escaped() {
    assert_eq!(
        clean(r#"<svg width="1" height="1"><rect id='a"b&lt;'/></svg>"#).unwrap(),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect id="a&quot;b&lt;"></rect></svg>"#
    );
}

#[test]
fn a_javascript_url_with_spaces_or_capitals_is_still_one() {
    assert_eq!(
        clean("<svg><rect fill=\" Java Script:x\"/></svg>").unwrap_err(),
        "`svg` can't hold a javascript: URL (fill on <rect>)"
    );
}

#[test]
fn an_svg_root_in_another_namespace_is_not_an_svg() {
    assert_eq!(
        clean("<svg xmlns=\"urn:other\"/>").unwrap_err(),
        "`svg` is one <svg> element, not <urn:other:svg>"
    );
}
