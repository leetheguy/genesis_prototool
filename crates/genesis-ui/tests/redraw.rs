//! When the screen catches up with the machine (spec 010, "The state machine"): after a change,
//! and never for a clock tick that changed nothing, so an idle screen draws nothing.

use genesis_core::config::App;
use genesis_ui::Host;
use leptos::prelude::*;
use serde_json::json;

const APP: &str = r#"
[app]
name = "Idle"

[state]
count = 0

[[body]]
type = "button"
id = "bump"
label = "Bump {{ app.count }}"
on_click = [{ step = "app.count" }]
"#;

#[test]
fn idle_ticks_redraw_nothing_and_a_change_redraws_once() {
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(APP).unwrap());
        host.open();
        let opened = host.redraws();
        for _ in 0..20 {
            host.advance(50);
        }
        assert_eq!(host.redraws(), opened, "a second of idle ticks");
        host.fire("bump.click", json!({}));
        assert_eq!(host.redraws(), opened + 1);
        assert_eq!(host.get("app.count"), Some(json!(1)));
    });
}
