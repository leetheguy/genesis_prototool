//! What a client sends to serve for the mirror (spec 050, "The mirror").

use genesis_core::config::App;
use genesis_ui::Host;
use leptos::prelude::*;
use serde_json::json;

fn host() -> Host {
    Host::new(
        App::from_toml(
            "[app]\nname = \"x\"\n[state]\nn = 0\n[on]\n\"go\" = [{ step = \"app.n\" }]\n",
        )
        .unwrap(),
    )
}

#[test]
fn the_first_mirror_holds_the_state_even_before_anything_happens() {
    Owner::new().with(|| {
        let mirror = host().mirror().expect("a first mirror");
        assert_eq!(mirror["state"]["app"], json!({ "n": 0 }));
        assert_eq!(mirror["events"], json!([]));
    });
}

#[test]
fn a_mirror_carries_the_events_since_the_last_one() {
    Owner::new().with(|| {
        let host = host();
        host.mirror();
        host.fire("go", json!({}));
        let mirror = host.mirror().expect("something happened");
        let names: Vec<_> = mirror["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].clone())
            .collect();
        assert_eq!(names, [json!("go"), json!("app.n.changed")]);
        assert_eq!(mirror["state"]["app"]["n"], 1);
    });
}

#[test]
fn nothing_new_means_no_mirror() {
    Owner::new().with(|| {
        let host = host();
        host.mirror();
        assert_eq!(host.mirror(), None);
    });
}
