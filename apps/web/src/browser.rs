//! The browser side: the address, the config stream, the clock, and the mirror.

use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::leptos_dom::helpers::set_interval;
use leptos::prelude::*;
use serde_json::{Value, json};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, MessageEvent};

/// How often the clock moves the machine on. Timers fire within this of their moment.
const TICK: Duration = Duration::from_millis(50);
/// The mirror sends at most this often (spec 050).
const MIRROR_EVERY: Duration = Duration::from_millis(250);

pub fn start() {
    console_error_panic_hook::set_once();
    let name = app_name();
    let source = match EventSource::new(&format!("/api/{name}/config")) {
        Ok(source) => source,
        Err(e) => return show_problems(&format!("can't reach serve: {e:?}")),
    };
    let drawn = Rc::new(Cell::new(false));
    // A config is drawn once; drawing a newer one live is hot reload (spec 060), not this milestone.
    let on_config = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if drawn.replace(true) {
            return;
        }
        let text = event.data().as_string().unwrap_or_default();
        match serde_json::from_str::<App>(&text) {
            Ok(app) => run(app, name.clone()),
            Err(e) => show_problems(&format!("the config serve sent didn't read: {e}")),
        }
    });
    let on_problems = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        show_problems(&event.data().as_string().unwrap_or_default());
    });
    let _ = source.add_event_listener_with_callback("config", on_config.as_ref().unchecked_ref());
    let _ =
        source.add_event_listener_with_callback("problems", on_problems.as_ref().unchecked_ref());
    // The listeners live as long as the page does.
    on_config.forget();
    on_problems.forget();
}

/// Draws the app and starts its clock and mirror.
fn run(app: App, name: String) {
    let host = Host::new(app);
    let drawn = host.clone();
    leptos::mount::mount_to_body(move || app_view(drawn.clone()));
    host.open();
    clock(host.clone());
    mirror(host, name);
}

/// Moves the machine's clock on with real time.
fn clock(host: Host) {
    let started = now();
    let advanced = Cell::new(0u64);
    set_interval(
        move || {
            let elapsed = (now() - started) as u64;
            host.advance(elapsed - advanced.replace(elapsed));
        },
        TICK,
    );
}

/// Posts the mirror whenever something has happened.
fn mirror(host: Host, name: String) {
    let client = client_id();
    let url = format!("/api/{name}/mirror");
    set_interval(
        move || {
            if let Some(mut body) = host.mirror() {
                body["client"] = json!(client);
                post(url.clone(), body);
            }
        },
        MIRROR_EVERY,
    );
}

fn post(url: String, body: Value) {
    leptos::task::spawn_local(async move {
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_body(&body.to_string().into());
        let headers = web_sys::Headers::new().expect("headers");
        let _ = headers.set("content-type", "application/json");
        init.set_headers(&headers);
        let sent =
            wasm_bindgen_futures::JsFuture::from(window().fetch_with_str_and_init(&url, &init))
                .await;
        let ok = sent
            .ok()
            .and_then(|r| r.dyn_into::<web_sys::Response>().ok())
            .is_some_and(|r| r.ok());
        if !ok {
            // Fail loud, but not in the app's face: the mirror is for agents, not people.
            web_sys::console::error_1(
                &format!("genesis: the mirror didn't reach serve ({url})").into(),
            );
        }
    });
}

/// This browser's ID for the mirror, kept in local storage; a fresh one when storage won't keep it.
fn client_id() -> String {
    let storage = window().local_storage().ok().flatten();
    if let Some(id) = storage
        .as_ref()
        .and_then(|s| s.get_item("genesis.client").ok().flatten())
    {
        return id;
    }
    let id: String = (0..16)
        .map(|_| format!("{:x}", (js_sys::Math::random() * 16.0) as u8))
        .collect();
    if let Some(storage) = storage {
        let _ = storage.set_item("genesis.client", &id);
    }
    id
}

/// The app's name: the first part of the address's path (`/hello/` is `hello`).
fn app_name() -> String {
    let path = window().location().pathname().unwrap_or_default();
    path.trim_matches('/')
        .split('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn show_problems(problems: &str) {
    let problems = problems.to_string();
    leptos::mount::mount_to_body(move || {
        view! {
            <div class="p-6">
                <div role="alert" class="alert alert-error"><span>"This app didn't load."</span></div>
                <pre class="mt-4 whitespace-pre-wrap">{problems.clone()}</pre>
            </div>
        }
    });
}

fn now() -> f64 {
    window().performance().map_or(0.0, |p| p.now())
}
