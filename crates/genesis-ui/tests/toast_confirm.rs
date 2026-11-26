//! Drawing `sys.toasts` and `sys.asking` (spec 100, "Toast and confirm"): toasts in the corner in
//! their colors, and the first question in a dialog with its buttons.

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::prelude::*;
use serde_json::json;

const APP: &str = r#"
[app]
name = "Ask"

[state]
done = false

[on]
"save" = [{ toast = "Saved.", variant = "success" }]
"wipe" = [{ confirm = "Wipe it all?", yes = "Wipe", no = "Keep" }, { set = "app.done", to = true }]
"#;

#[test]
fn toasts_show_in_their_colors_and_a_tap_takes_one_away() {
    Owner::new().with(|| {
        let host = Host::new(App::from_files([("app.toml", APP)]).unwrap());
        host.fire("save", json!({}));
        let out = app_view(host.clone()).to_html();
        assert!(out.contains("alert alert-success"), "{out}");
        assert!(out.contains("Saved."), "{out}");
        let id = host.get("sys.toasts.0.id").unwrap().as_u64().unwrap();
        host.dismiss(id);
        assert!(!app_view(host).to_html().contains("Saved."));
    });
}

#[test]
fn a_question_shows_in_a_dialog_with_its_buttons_until_its_answered() {
    Owner::new().with(|| {
        let host = Host::new(App::from_files([("app.toml", APP)]).unwrap());
        host.fire("wipe", json!({}));
        let out = app_view(host.clone()).to_html();
        assert!(out.contains("Wipe it all?"), "{out}");
        assert!(
            out.contains(">Wipe</button>") && out.contains(">Keep</button>"),
            "{out}"
        );
        let id = host.get("sys.asking.0.id").unwrap().as_u64().unwrap();
        host.answer(id, true);
        assert_eq!(host.get("app.done"), Some(json!(true)));
        assert!(!app_view(host).to_html().contains("Wipe it all?"));
    });
}
