//! Style keys drawn as classes (spec 040, "Style keys"), and the vocabulary the CSS is built from.

use genesis_core::config::App;
use genesis_ui::{Host, app_view, style};
use leptos::prelude::*;

/// The classes on the element drawn for one `ty` node with `keys` (TOML lines).
fn classes(ty: &str, keys: &str) -> String {
    let text = match ty {
        "text" => "text = \"hi\"\n",
        "button" => "label = \"Go\"\n",
        _ => "",
    };
    let toml = format!("[app]\nname = \"x\"\n[[body]]\ntype = \"{ty}\"\n{text}{keys}\n");
    let out = Owner::new().with(|| app_view(Host::new(App::from_toml(&toml).unwrap())).to_html());
    // The screen's own wrapper comes first; the node is the next element (past Leptos's `<!>` markers).
    let after = &out[out.find("p-6\">").expect("the screen's wrapper") + 5..];
    let after = after.trim_start_matches("<!>");
    let tag = &after[..after.find('>').unwrap()];
    let class = tag.split_once("class=\"").map_or("", |(_, rest)| rest);
    class[..class.find('"').unwrap_or(0)].to_string()
}

#[test]
fn a_cell_with_no_keys_is_a_column() {
    assert_eq!(classes("cell", ""), "flex flex-col");
}

#[test]
fn each_style_key_on_a_cell_gives_its_classes() {
    let cases = [
        ("flow = \"row\"", "flex flex-row"),
        ("grid = 3", "grid grid-cols-3"),
        (
            "grid = \"fit\"",
            "grid grid-cols-[repeat(auto-fit,minmax(6rem,1fr))]",
        ),
        ("gap = 4", "flex flex-col gap-4"),
        ("align = \"center\"", "flex flex-col items-center"),
        ("justify = \"between\"", "flex flex-col justify-between"),
        ("wrap = true", "flex flex-col flex-wrap"),
        ("wrap = false", "flex flex-col flex-nowrap"),
        ("pad = 2", "flex flex-col p-2"),
        ("pad_x = 6", "flex flex-col px-6"),
        ("pad_y = 0", "flex flex-col py-0"),
        ("margin = \"auto\"", "flex flex-col m-auto"),
        ("margin_x = 4", "flex flex-col mx-4"),
        ("margin_y = \"auto\"", "flex flex-col my-auto"),
        ("bg = \"base-200\"", "flex flex-col bg-base-200"),
        ("border = true", "flex flex-col border border-base-300"),
        ("border = \"accent\"", "flex flex-col border border-accent"),
        ("radius = \"box\"", "flex flex-col rounded-box"),
        ("radius = \"none\"", "flex flex-col rounded-none"),
        ("shadow = \"md\"", "flex flex-col shadow-md"),
        ("height = \"screen\"", "flex flex-col h-screen"),
        ("scroll = true", "flex flex-col overflow-auto"),
        ("scroll = false", "flex flex-col overflow-visible"),
        ("size = \"2xl\"", "flex flex-col text-2xl"),
        ("weight = \"semibold\"", "flex flex-col font-semibold"),
        ("color = \"muted\"", "flex flex-col text-base-content/60"),
        (
            "color = \"primary-content\"",
            "flex flex-col text-primary-content",
        ),
        ("text_align = \"right\"", "flex flex-col text-right"),
        ("font = \"mono\"", "flex flex-col font-mono"),
    ];
    for (keys, want) in cases {
        assert_eq!(classes("cell", keys), want, "{keys}");
    }
}

#[test]
fn a_cell_with_several_keys_gives_them_all_in_spec_order() {
    let keys = "font = \"mono\"\npad = 4\ngrid = 2\nbg = \"neutral\"\ngap = 2";
    assert_eq!(
        classes("cell", keys),
        "grid grid-cols-2 gap-2 p-4 bg-neutral font-mono"
    );
}

#[test]
fn a_text_with_no_keys_has_no_classes() {
    assert_eq!(classes("text", ""), "");
}

#[test]
fn a_text_takes_box_and_type_keys() {
    assert_eq!(
        classes(
            "text",
            "pad = 1\nbg = \"info\"\nsize = \"sm\"\ncolor = \"muted\""
        ),
        "p-1 bg-info text-sm text-base-content/60"
    );
}

#[test]
fn a_headings_size_and_weight_give_way_to_its_own_keys() {
    let cases = [
        ("level = 1", "text-3xl font-bold"),
        ("level = 2", "text-2xl font-bold"),
        ("level = 3", "text-xl font-semibold"),
        ("level = 1\nsize = \"lg\"", "text-lg font-bold"),
        ("level = 2\nweight = \"normal\"", "text-2xl font-normal"),
        (
            "level = 3\ncolor = \"primary\"",
            "text-xl font-semibold text-primary",
        ),
    ];
    for (keys, want) in cases {
        assert_eq!(classes("text", keys), want, "{keys}");
    }
}

#[test]
fn a_key_given_a_table_by_screen_size_gives_each_sizes_classes_with_its_prefix() {
    let cases = [
        ("gap = { base = 2, md = 4 }", "flex flex-col gap-2 md:gap-4"),
        ("pad = { sm = 1, xl = 8 }", "flex flex-col sm:p-1 xl:p-8"),
        (
            "flow = { base = \"column\", md = \"row\" }",
            "flex flex-col md:flex md:flex-row",
        ),
        (
            "flow = { lg = \"row\" }",
            "flex flex-col lg:flex lg:flex-row",
        ),
        (
            "grid = { base = 1, md = 2, lg = 4 }",
            "grid grid-cols-1 md:grid md:grid-cols-2 lg:grid lg:grid-cols-4",
        ),
        ("grid = { md = 3 }", "flex flex-col md:grid md:grid-cols-3"),
        (
            "wrap = { base = true, md = false }",
            "flex flex-col flex-wrap md:flex-nowrap",
        ),
        (
            "border = { md = true }",
            "flex flex-col md:border md:border-base-300",
        ),
        (
            "color = { base = \"muted\", lg = \"primary\" }",
            "flex flex-col text-base-content/60 lg:text-primary",
        ),
    ];
    for (keys, want) in cases {
        assert_eq!(classes("cell", keys), want, "{keys}");
    }
}

#[test]
fn a_heading_keeps_its_own_size_below_the_first_screen_its_size_key_names() {
    assert_eq!(
        classes("text", "level = 1\nsize = { md = \"4xl\" }"),
        "text-3xl md:text-4xl font-bold"
    );
    assert_eq!(
        classes("text", "level = 2\nsize = { base = \"lg\", xl = \"3xl\" }"),
        "text-lg xl:text-3xl font-bold"
    );
}

#[test]
fn placement_keys_give_their_classes_on_every_node() {
    let cases = [
        ("span = 4", "col-span-4"),
        ("span = \"full\"", "col-span-full"),
        ("offset = 2", "col-start-3"),
        ("grow = true", "grow"),
        ("grow = false", "grow-0"),
        ("width = \"1/3\"", "w-1/3"),
        ("width = \"fit\"", "w-fit"),
        ("max_width = \"prose\"", "max-w-prose"),
        ("max_width = \"2xl\"", "max-w-2xl"),
        (
            "span = { base = 12, md = 6, lg = 4 }",
            "col-span-12 md:col-span-6 lg:col-span-4",
        ),
    ];
    for (keys, want) in cases {
        assert_eq!(
            classes("cell", keys),
            format!("flex flex-col {want}"),
            "cell {keys}"
        );
        assert_eq!(classes("text", keys), want, "text {keys}");
        assert_eq!(
            classes("button", keys),
            format!("btn btn-primary {want}"),
            "button {keys}"
        );
    }
}

#[test]
fn a_nodes_placement_comes_after_its_own_classes() {
    assert_eq!(
        classes("text", "span = 2\nlevel = 2\npad = 1"),
        "p-1 text-2xl font-bold col-span-2"
    );
}

#[test]
fn the_vocabulary_has_every_class_a_style_key_can_give_once() {
    let words = style::vocabulary();
    for class in [
        "flex",
        "flex-col",
        "flex-row",
        "grid",
        "grid-cols-1",
        "grid-cols-12",
        "grid-cols-[repeat(auto-fit,minmax(6rem,1fr))]",
        "gap-0",
        "gap-16",
        "px-3",
        "my-auto",
        "border",
        "border-error-content",
        "bg-base-100",
        "rounded-box",
        "shadow-none",
        "h-full",
        "overflow-auto",
        "text-4xl",
        "text-base-content/60",
        "text-center",
        "font-medium",
        "font-sans",
        "flex-nowrap",
        "overflow-visible",
        "sm:flex",
        "md:gap-4",
        "lg:my-auto",
        "xl:grid-cols-[repeat(auto-fit,minmax(6rem,1fr))]",
        "md:text-base-content/60",
        "sm:border-error-content",
        "col-span-1",
        "col-span-full",
        "col-start-2",
        "col-start-12",
        "grow-0",
        "w-3/4",
        "max-w-prose",
        "xl:max-w-6xl",
    ] {
        assert!(words.iter().any(|w| w == class), "{class} missing");
    }
    let mut sorted = words.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), words.len(), "no class twice");
}

/// The CSS is built from `classes.txt` (Tailwind reads it; see `apps/web/style/input.css`), so it
/// must hold exactly the vocabulary. `GENESIS_BLESS=1 cargo test` rewrites it.
#[test]
fn classes_txt_is_the_vocabulary() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/classes.txt");
    let want = style::vocabulary().join("\n") + "\n";
    if std::env::var_os("GENESIS_BLESS").is_some() {
        std::fs::write(path, &want).unwrap();
    }
    let have = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        have == want,
        "{path} is out of date; run GENESIS_BLESS=1 cargo test -p genesis-ui --test style"
    );
}
