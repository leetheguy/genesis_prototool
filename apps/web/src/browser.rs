//! The browser side: the address, the config stream, the clock, and the mirror.

use genesis_core::bus::Reply;
use genesis_core::config::App;
use genesis_ui::{Host, app_view};
use leptos::leptos_dom::helpers::set_interval;
use leptos::prelude::*;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
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
    let source = match EventSource::new("/api/config") {
        Ok(source) => source,
        Err(e) => return show_problems(&format!("can't reach serve: {e:?}")),
    };
    // The running app, once the first good config has drawn it. Later configs swap in live
    // (spec 060), and later problems show over the last good screen.
    let running: Rc<RefCell<Option<Host>>> = Rc::new(RefCell::new(None));
    let on_config = {
        let running = running.clone();
        Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let text = event.data().as_string().unwrap_or_default();
            let app = match serde_json::from_str::<App>(&text) {
                Ok(app) => app,
                Err(e) => {
                    return problems(&running, &format!("the config serve sent didn't read: {e}"));
                }
            };
            let host = running.borrow().clone();
            match host {
                Some(host) => host.reload(app),
                None => *running.borrow_mut() = Some(run(app)),
            }
        })
    };
    let on_problems = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        problems(&running, &event.data().as_string().unwrap_or_default());
    });
    let _ = source.add_event_listener_with_callback("config", on_config.as_ref().unchecked_ref());
    let _ =
        source.add_event_listener_with_callback("problems", on_problems.as_ref().unchecked_ref());
    // The listeners live as long as the page does.
    on_config.forget();
    on_problems.forget();
}

/// Problems over the running app's last good screen, or, before there's one, on a screen of their own.
fn problems(running: &Rc<RefCell<Option<Host>>>, text: &str) {
    match running.borrow().as_ref() {
        Some(host) => host.show_problems(text),
        None => show_problems(text),
    }
}

/// Draws the app and starts its clock and mirror.
fn run(app: App) -> Host {
    let host = Host::new(app);
    let drawn = host.clone();
    leptos::mount::mount_to_body(move || app_view(drawn.clone()));
    caller(host.clone());
    restore(&host);
    host.open();
    keep(host.clone());
    clock(host.clone());
    mirror(host.clone());
    host
}

/// Where the kept session lives in local storage (spec 050, "Persisting").
const KEPT_KEY: &str = "genesis.kept";

/// Brings back this device's kept session, if there is one (spec 050, "Persisting").
fn restore(host: &Host) {
    let storage = window().local_storage().ok().flatten();
    let Some(text) = storage.and_then(|s| s.get_item(KEPT_KEY).ok().flatten()) else {
        return;
    };
    let kept = serde_json::from_str(&text).unwrap_or(Value::Null);
    for note in host.restore(&kept) {
        web_sys::console::warn_1(&format!("genesis: {note}").into());
    }
}

/// Saves the kept session whenever it changes.
fn keep(host: Host) {
    let last = RefCell::new(String::new());
    set_interval(
        move || {
            let kept = host.kept().to_string();
            if *last.borrow() == kept {
                return;
            }
            let saved = window()
                .local_storage()
                .ok()
                .flatten()
                .is_some_and(|s| s.set_item(KEPT_KEY, &kept).is_ok());
            if !saved {
                web_sys::console::warn_1(&"genesis: this browser won't keep the session".into());
            }
            *last.borrow_mut() = kept;
        },
        MIRROR_EVERY,
    );
}

/// Makes the machine's calls through serve, and hands each answer back. A call whose params hold
/// file references goes as a multipart form, the call as its `call` part and each file as a part
/// named by its ID, so serve can send the bytes on (spec 040, "file").
fn caller(host: Host) {
    let client = client_id();
    let replies = host.clone();
    host.set_caller(move |call| {
        let host = replies.clone();
        let client = client.clone();
        leptos::task::spawn_local(async move {
            let body = json!({
                "op": call.op, "params": call.params, "headers": call.headers,
                "client": client, "from": call.from,
            });
            let ids: Vec<String> = call
                .params
                .as_object()
                .map(|params| genesis_core::file::references(params))
                .unwrap_or_default()
                .into_iter()
                .map(String::from)
                .collect();
            let sent = if ids.is_empty() {
                fetch_json("/api/call", &body).await
            } else {
                fetch_files("/api/call/files", &body, &ids).await
            };
            let reply = match sent {
                Ok((200, answer)) => reply_from(answer),
                Ok((status, answer)) => Reply::Failed(format!(
                    "serve refused the call to {} ({status}): {}",
                    call.op, answer
                )),
                Err(why) => {
                    Reply::Failed(format!("the call to {} didn't reach serve: {why}", call.op))
                }
            };
            host.reply(call.id, reply);
        });
    });
}

/// serve's answer to a call: `{"status", "body"}`, or `{"error"}`.
fn reply_from(answer: Value) -> Reply {
    match (answer["status"].as_u64(), answer["error"].as_str()) {
        (Some(status), _) => Reply::Response {
            status: status as u16,
            body: answer["body"].clone(),
        },
        (None, Some(error)) => Reply::Failed(error.to_string()),
        (None, None) => Reply::Failed(format!("serve's answer made no sense: {answer}")),
    }
}

/// POSTs `body` as JSON, and gives back the status and the answer (as JSON, or as text).
async fn fetch_json(url: &str, body: &Value) -> Result<(u16, Value), String> {
    let init = web_sys::RequestInit::new();
    init.set_body(&body.to_string().into());
    let headers = web_sys::Headers::new().map_err(|e| format!("{e:?}"))?;
    let _ = headers.set("content-type", "application/json");
    init.set_headers(&headers);
    fetch(url, init).await
}

/// POSTs `body` as the `call` part of a multipart form, with each held file in `ids` as a part
/// named by its ID. A file the page no longer holds is left out, and serve says so.
async fn fetch_files(url: &str, body: &Value, ids: &[String]) -> Result<(u16, Value), String> {
    let form = web_sys::FormData::new().map_err(|e| format!("{e:?}"))?;
    form.append_with_str("call", &body.to_string())
        .map_err(|e| format!("{e:?}"))?;
    for id in ids {
        if let Some(file) = genesis_ui::files::get(id) {
            form.append_with_blob_and_filename(id, &file, &file.name())
                .map_err(|e| format!("{e:?}"))?;
        }
    }
    let init = web_sys::RequestInit::new();
    // The browser sets the content type itself, with the form's boundary.
    init.set_body(&form);
    fetch(url, init).await
}

/// POSTs `init`, and gives back the status and the answer (as JSON, or as text).
async fn fetch(url: &str, init: web_sys::RequestInit) -> Result<(u16, Value), String> {
    init.set_method("POST");
    let response =
        wasm_bindgen_futures::JsFuture::from(window().fetch_with_str_and_init(url, &init))
            .await
            .map_err(|e| format!("{e:?}"))?;
    let response: web_sys::Response = response.dyn_into().map_err(|e| format!("{e:?}"))?;
    let text = response.text().map_err(|e| format!("{e:?}"))?;
    let text = wasm_bindgen_futures::JsFuture::from(text)
        .await
        .map_err(|e| format!("{e:?}"))?
        .as_string()
        .unwrap_or_default();
    let answer = serde_json::from_str(&text).unwrap_or(Value::String(text));
    Ok((response.status(), answer))
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
fn mirror(host: Host) {
    let client = client_id();
    let url = "/api/mirror".to_string();
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
