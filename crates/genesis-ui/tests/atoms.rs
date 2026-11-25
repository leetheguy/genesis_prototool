//! Drawing the atoms (spec 040, "The atoms"): each as its own HTML element.

use genesis_core::config::App;
use genesis_ui::{Host, app_view, style};
use leptos::prelude::*;

/// The HTML drawn for one node (TOML lines), with `[state]` lines too.
fn html(node: &str, state: &str) -> String {
    let toml = format!("[app]\nname = \"x\"\n[state]\n{state}\n[[body]]\n{node}\n");
    Owner::new().with(|| app_view(Host::new(App::from_toml(&toml).unwrap())).to_html())
}

#[test]
fn an_image_is_an_img_with_its_src_filled_alt_and_classes() {
    let out = html(
        "type = \"image\"\nsrc = \"/avatars/{{ app.who }}.png\"\nalt = \"Ada\"\nradius = \"full\"\nspan = 2",
        "who = \"ada\"",
    );
    assert!(
        out.contains(r#"<img src="/avatars/ada.png" alt="Ada" class="rounded-full col-span-2""#),
        "{out}"
    );
}

#[test]
fn a_link_is_an_a_with_its_href_and_text_filled() {
    let out = html(
        "type = \"link\"\ntext = \"Page {{ app.n }}\"\nhref = \"/pages/{{ app.n }}\"",
        "n = 3",
    );
    assert!(
        out.contains(r#"<a href="/pages/3" class="link">Page 3</a>"#),
        "{out}"
    );
}

#[test]
fn an_outside_link_opens_in_a_new_tab_and_others_do_not() {
    for href in [
        "https://example.com",
        "http://example.com/x",
        "//cdn.example.com",
    ] {
        let out = html(
            &format!("type = \"link\"\ntext = \"Out\"\nhref = \"{href}\""),
            "",
        );
        assert!(
            out.contains(r#"target="_blank" rel="noopener noreferrer""#),
            "{href}: {out}"
        );
    }
    for href in ["/contacts", "#top", "mailto:ada@example.com", "contacts/2"] {
        let out = html(
            &format!("type = \"link\"\ntext = \"In\"\nhref = \"{href}\""),
            "",
        );
        assert!(!out.contains("target="), "{href}: {out}");
    }
}

#[test]
fn a_link_follows_its_href_to_outside_and_back() {
    let out = html(
        "type = \"link\"\ntext = \"x\"\nhref = \"{{ app.url }}\"",
        "url = \"https://a.example\"",
    );
    assert!(out.contains(r#"target="_blank""#), "{out}");
}

#[test]
fn a_buttons_variant_and_size_give_daisyuis_classes() {
    let cases = [
        ("", "btn btn-primary"),
        ("variant = \"ghost\"", "btn btn-ghost"),
        ("variant = \"error\"", "btn btn-error"),
        ("size = \"sm\"", "btn btn-primary btn-sm"),
        (
            "variant = \"link\"\nsize = { base = \"xs\", lg = \"lg\" }",
            "btn btn-link btn-xs lg:btn-lg",
        ),
        (
            "variant = \"neutral\"\nspan = 3",
            "btn btn-neutral col-span-3",
        ),
    ];
    for (keys, want) in cases {
        let out = html(&format!("type = \"button\"\nlabel = \"Go\"\n{keys}"), "");
        assert!(
            out.contains(&format!(r#"<button class="{want}""#)),
            "{keys}: {out}"
        );
    }
}

#[test]
fn the_vocabulary_has_every_variant_and_size() {
    let words = style::vocabulary();
    for class in [
        "btn-primary",
        "btn-ghost",
        "btn-link",
        "btn-error",
        "btn-xs",
        "btn-lg",
        "md:btn-sm",
        "xl:btn-accent",
    ] {
        assert!(words.iter().any(|w| w == class), "{class} missing");
    }
}
