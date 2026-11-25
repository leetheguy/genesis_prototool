//! The atoms' own keys (spec 040, "The atoms"): what each loads with, and what it refuses.

use genesis_core::config::{App, Node};

fn app(node: &str) -> Result<App, Vec<genesis_core::config::ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"Atoms\"\n\n[[body]]\n{node}\n"))
}

fn problem(node: &str) -> String {
    app(node)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Loads `node`, checks it survives the trip to the browser (JSON), and hands it back.
fn round_trip(node: &str) -> Node {
    let app = app(node).unwrap_or_else(|e| panic!("{node}: {e:?}"));
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app, "{node}");
    app.body[0].clone()
}

#[test]
fn an_image_loads_with_src_alt_and_the_box_and_placement_keys() {
    let Node::Image(image) = round_trip(
        "type = \"image\"\nsrc = \"{{ item.avatar }}\"\nalt = \"Ada\"\nradius = \"full\"\nshadow = \"sm\"\nwidth = \"1/4\"",
    ) else {
        panic!("not an image")
    };
    assert_eq!(
        (image.src.as_str(), image.alt.as_str()),
        ("{{ item.avatar }}", "Ada")
    );
}

#[test]
fn an_image_without_alt_is_a_problem() {
    // A picture that's only decoration says so with alt = "".
    assert!(problem("type = \"image\"\nsrc = \"/a.png\"").contains("missing field `alt`"));
    round_trip("type = \"image\"\nsrc = \"/a.png\"\nalt = \"\"");
}

#[test]
fn a_link_loads_with_text_and_href() {
    let Node::Link(link) = round_trip(
        "type = \"link\"\ntext = \"Docs\"\nhref = \"https://example.com/{{ app.page }}\"\nspan = 2",
    ) else {
        panic!("not a link")
    };
    assert_eq!(link.href, "https://example.com/{{ app.page }}");
    assert!(problem("type = \"link\"\ntext = \"Docs\"").contains("missing field `href`"));
}

#[test]
fn a_button_takes_each_variant_and_size() {
    for variant in [
        "primary",
        "secondary",
        "accent",
        "neutral",
        "ghost",
        "link",
        "error",
    ] {
        round_trip(&format!(
            "type = \"button\"\nlabel = \"Go\"\nvariant = \"{variant}\""
        ));
    }
    for size in ["xs", "sm", "md", "lg"] {
        round_trip(&format!(
            "type = \"button\"\nlabel = \"Go\"\nsize = \"{size}\""
        ));
    }
    round_trip("type = \"button\"\nlabel = \"Go\"\nsize = { base = \"sm\", md = \"lg\" }");
}

#[test]
fn a_bad_variant_or_size_is_a_problem_naming_its_values_on_its_line() {
    assert_eq!(
        problem("type = \"button\"\nlabel = \"Go\"\nvariant = \"danger\""),
        "app.toml:7:1: `variant` is \"primary\", \"secondary\", \"accent\", \"neutral\", \"ghost\", \"link\", or \"error\", not \"danger\""
    );
    assert_eq!(
        problem("type = \"button\"\nlabel = \"Go\"\nsize = \"xl\""),
        "app.toml:7:1: `size` is \"xs\", \"sm\", \"md\", or \"lg\", not \"xl\""
    );
}

#[test]
fn image_and_link_take_an_id_and_hand_over_their_placement() {
    for node in [
        "type = \"image\"\nid = \"pic\"\nsrc = \"/a.png\"\nalt = \"\"\ngrow = true",
        "type = \"link\"\nid = \"go\"\ntext = \"Go\"\nhref = \"/\"\ngrow = true",
    ] {
        let node = round_trip(node);
        assert!(matches!(node.id(), Some("pic" | "go")), "{node:?}");
        assert!(node.placed().grow.is_some(), "{node:?}");
    }
}
