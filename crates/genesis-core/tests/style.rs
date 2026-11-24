//! The closed style keys (spec 040, "Style keys"): every value each key takes loads, and nothing else does.

use genesis_core::config::App;

/// An app whose one node is a `ty` with `keys` (TOML lines) on it.
fn node(ty: &str, keys: &str) -> String {
    let text = if ty == "text" { "text = \"hi\"\n" } else { "" };
    format!("[app]\nname = \"Styled\"\n\n[[body]]\ntype = \"{ty}\"\n{text}{keys}\n")
}

/// Every value each key takes, as TOML, by the groups spec 040 gives them.
const LAYOUT: &[(&str, &[&str])] = &[
    ("flow", &[r#""column""#, r#""row""#]),
    ("grid", &["1", "6", "12", r#""fit""#]),
    ("gap", SPACING),
    (
        "align",
        &[r#""start""#, r#""center""#, r#""end""#, r#""stretch""#],
    ),
    (
        "justify",
        &[
            r#""start""#,
            r#""center""#,
            r#""end""#,
            r#""between""#,
            r#""around""#,
        ],
    ),
    ("wrap", &["true", "false"]),
];

const BOX: &[(&str, &[&str])] = &[
    ("pad", SPACING),
    ("pad_x", SPACING),
    ("pad_y", SPACING),
    ("margin", &["0", "4", "16", r#""auto""#]),
    ("margin_x", &["2", r#""auto""#]),
    ("margin_y", &["8", r#""auto""#]),
    ("bg", COLORS),
    ("border", &["true", r#""primary""#, r#""base-300""#]),
    (
        "radius",
        &[
            r#""none""#,
            r#""sm""#,
            r#""md""#,
            r#""lg""#,
            r#""xl""#,
            r#""full""#,
            r#""box""#,
        ],
    ),
    ("shadow", &[r#""none""#, r#""sm""#, r#""md""#, r#""lg""#]),
    ("height", &[r#""full""#, r#""screen""#]),
    ("scroll", &["true", "false"]),
];

const TYPE: &[(&str, &[&str])] = &[
    (
        "size",
        &[
            r#""xs""#,
            r#""sm""#,
            r#""base""#,
            r#""lg""#,
            r#""xl""#,
            r#""2xl""#,
            r#""3xl""#,
            r#""4xl""#,
        ],
    ),
    (
        "weight",
        &[r#""normal""#, r#""medium""#, r#""semibold""#, r#""bold""#],
    ),
    (
        "color",
        &[r#""muted""#, r#""primary""#, r#""error-content""#],
    ),
    ("text_align", &[r#""left""#, r#""center""#, r#""right""#]),
    ("font", &[r#""sans""#, r#""mono""#]),
];

const SPACING: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "8", "10", "12", "16"];

const COLORS: &[&str] = &[
    r#""base-100""#,
    r#""base-200""#,
    r#""base-300""#,
    r#""base-content""#,
    r#""neutral""#,
    r#""neutral-content""#,
    r#""primary""#,
    r#""primary-content""#,
    r#""secondary""#,
    r#""secondary-content""#,
    r#""accent""#,
    r#""accent-content""#,
    r#""info""#,
    r#""info-content""#,
    r#""success""#,
    r#""success-content""#,
    r#""warning""#,
    r#""warning-content""#,
    r#""error""#,
    r#""error-content""#,
];

/// Loads `ty` with each value of each key in `groups`, and checks it survives the trip to the
/// browser (JSON) unchanged.
fn every_value_loads(ty: &str, groups: &[&[(&str, &[&str])]]) {
    for group in groups {
        for (key, values) in *group {
            for value in *values {
                let toml = node(ty, &format!("{key} = {value}"));
                let app =
                    App::from_toml(&toml).unwrap_or_else(|e| panic!("{ty} {key} = {value}: {e:?}"));
                let json = serde_json::to_string(&app).unwrap();
                let back: App = serde_json::from_str(&json).unwrap();
                assert_eq!(back, app, "{ty} {key} = {value} through JSON");
                assert!(
                    json.contains(&format!("\"{key}\":{value}")),
                    "{ty} {key} = {value} is sent as written: {json}"
                );
            }
        }
    }
}

#[test]
fn a_cell_takes_every_layout_box_and_type_value() {
    every_value_loads("cell", &[LAYOUT, BOX, TYPE]);
}

#[test]
fn a_text_takes_every_box_and_type_value() {
    every_value_loads("text", &[BOX, TYPE]);
}

#[test]
fn a_text_takes_no_layout_keys() {
    for (key, values) in LAYOUT {
        let problems =
            App::from_toml(&node("text", &format!("{key} = {}", values[0]))).unwrap_err();
        assert!(
            problems[0]
                .message
                .contains(&format!("unknown field `{key}`")),
            "{key}: {problems:?}"
        );
    }
}

#[test]
fn a_value_outside_a_keys_list_is_a_problem_naming_the_key_and_its_values_on_its_line() {
    let cases = [
        (
            "gap = 7",
            "`gap` is 0, 1, 2, 3, 4, 5, 6, 8, 10, 12, or 16, not 7",
        ),
        (
            "flow = \"diagonal\"",
            "`flow` is \"column\" or \"row\", not \"diagonal\"",
        ),
        (
            "grid = 13",
            "`grid` is 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, or \"fit\", not 13",
        ),
        ("wrap = \"yes\"", "`wrap` is true or false, not \"yes\""),
        (
            "margin_x = \"centre\"",
            "`margin_x` is 0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16, or \"auto\", not \"centre\"",
        ),
        (
            "radius = 4",
            "`radius` is \"none\", \"sm\", \"md\", \"lg\", \"xl\", \"full\", or \"box\", not 4",
        ),
        (
            "weight = \"heavy\"",
            "`weight` is \"normal\", \"medium\", \"semibold\", or \"bold\", not \"heavy\"",
        ),
    ];
    for (line, message) in cases {
        let problems = App::from_toml(&node("cell", &format!("id = \"x\"\n{line}"))).unwrap_err();
        assert_eq!(
            problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
            [format!("app.toml:7:1: {message}")],
            "{line}"
        );
    }
}

#[test]
fn a_bad_color_lists_every_theme_color() {
    let problems = App::from_toml(&node("cell", "bg = \"red\"")).unwrap_err();
    let colors = COLORS
        .join(", ")
        .replace(", \"error-content\"", ", or \"error-content\"");
    assert_eq!(
        problems[0].message,
        format!("`bg` is {colors}, not \"red\"")
    );
    let problems = App::from_toml(&node("text", "color = \"red\"")).unwrap_err();
    assert!(
        problems[0]
            .message
            .starts_with("`color` is \"muted\", \"base-100\""),
        "{problems:?}"
    );
    let problems = App::from_toml(&node("cell", "border = false")).unwrap_err();
    assert!(
        problems[0]
            .message
            .starts_with("`border` is true, \"base-100\""),
        "{problems:?}"
    );
}

#[test]
fn a_cell_with_both_flow_and_grid_is_a_problem_naming_its_place() {
    let toml = "[app]\nname = \"x\"\n\n[[body]]\ntype = \"cell\"\n\n[[body.children]]\ntype = \"cell\"\nflow = \"row\"\ngrid = 3\n";
    let problems = App::from_toml(toml).unwrap_err();
    assert_eq!(
        problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["app.toml: body.0.children.0: a cell lays out by `flow` or by `grid`, not both"]
    );
}
