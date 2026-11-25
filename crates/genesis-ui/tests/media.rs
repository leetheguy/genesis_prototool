//! Drawing `svg` and `divider` (spec 040, "Media and embeds").

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;

fn html(node: &str) -> String {
    let toml = format!("[app]\nname = \"x\"\n[[body]]\n{node}\n");
    Owner::new().with(|| app_view(Host::new(App::from_toml(&toml).unwrap())).to_html())
}

const DOT: &str = "svg = '<svg viewBox=\"0 0 2 2\"><circle cx=\"1\" cy=\"1\" r=\"1\" fill=\"currentColor\"/></svg>'";
const DRAWN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2 2" width="1em" height="1em"><circle cx="1" cy="1" r="1" fill="currentColor"></circle></svg>"#;

#[test]
fn an_svg_with_a_label_is_a_named_image_drawn_from_its_cleaned_copy() {
    let out = html(&format!(
        "type = \"svg\"\nlabel = \"Online\"\ncolor = \"success\"\nsize = \"xl\"\nspan = 2\n{DOT}"
    ));
    assert!(
        out.contains(&format!(
            r#"<span role="img" aria-label="Online" class="inline-flex text-xl text-success col-span-2">{DRAWN}</span>"#
        )),
        "{out}"
    );
}

#[test]
fn an_svg_without_a_label_is_decoration_hidden_from_screen_readers() {
    let out = html(&format!("type = \"svg\"\n{DOT}"));
    assert!(
        out.contains(&format!(
            r#"<span aria-hidden="true" class="inline-flex">{DRAWN}</span>"#
        )),
        "{out}"
    );
}

#[test]
fn a_divider_is_an_hr_in_the_themes_line_color_or_its_own() {
    assert!(html("type = \"divider\"").contains(r#"<hr class="border-base-300">"#));
    let out = html("type = \"divider\"\ncolor = \"primary\"\nmargin_y = 6");
    assert!(out.contains(r#"<hr class="border-primary my-6">"#), "{out}");
}
