//! The host hands calls to whoever makes them, and replies back to the machine (spec 030).

use genesis_core::bus::{Call, Reply};
use genesis_core::config::App;
use genesis_ui::Host;
use leptos::prelude::*;
use serde_json::json;
use std::sync::{Arc, Mutex};

const APP: &str = "[app]\nname = \"x\"\n[connections.api]\nbase_url = \"http://api\"\n[connections.api.ops.hi]\nmethod = \"GET\"\npath = \"/hi\"\n[sources.greeting]\ncall = \"api.hi\"\n";

#[test]
fn a_caller_gets_each_call_and_its_reply_reaches_the_machine() {
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(APP).unwrap());
        let seen: Arc<Mutex<Vec<Call>>> = Arc::default();
        let into = seen.clone();
        host.set_caller(move |call| into.lock().unwrap().push(call));
        host.open();
        let call = seen.lock().unwrap()[0].clone();
        assert_eq!(call.op, "api.hi");
        host.reply(
            call.id,
            Reply::Response {
                status: 200,
                body: json!("hello"),
            },
        );
        assert_eq!(host.get("app.greeting"), Some(json!("hello")));
    });
}

#[test]
fn calls_made_before_there_is_a_caller_go_to_it_once_there_is() {
    Owner::new().with(|| {
        let host = Host::new(App::from_toml(APP).unwrap());
        host.open();
        let seen: Arc<Mutex<Vec<Call>>> = Arc::default();
        let into = seen.clone();
        host.set_caller(move |call| into.lock().unwrap().push(call));
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(host.take_calls().is_empty());
    });
}
