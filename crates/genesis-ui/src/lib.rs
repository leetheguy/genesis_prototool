//! Genesis's renderer: draws an app's nodes from its state machine, with
//! [Leptos](https://leptos.dev) and [daisyUI](https://daisyui.com).
//!
//! The screen holds no data of its own ([spec 050](../../../specs/050_state.md)). A [`Host`] owns
//! the app's [`Machine`]; every node reads the machine's current state, and everything a node does
//! fires an event into it. After each event, the screen redraws whatever read state.
//!
//! Every catalog type is drawn the same way ([spec 040](../../../specs/040_catalog.md), "How a
//! component is built"):
//!
//! - It implements [`Component`]: the events it can fire, and a `view` that draws it.
//! - It's always drawn through [`Wired`], which knows all the wiring: the node's name, reading
//!   state, firing events, and drawing children. A component never touches the machine itself.
//!
//! Leptos terms in a few words: a *view* is a piece of the page; a *signal* is a value the page
//! watches, so whatever read it redraws when it changes.

use genesis_core::bus::{Call, Effect, Machine, Reply};
use genesis_core::config::{App, Node, body_place, child_place};
use genesis_core::style::Placed;
use genesis_core::template;
use leptos::prelude::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use style::ClassList;

mod components;
mod controls;
pub mod style;

/// Makes a call and answers it, as [`Host::set_caller`] takes it.
type Caller = Arc<dyn Fn(Call) + Send + Sync>;

/// One running app: its state machine, and the signals the screen redraws from.
///
/// It's cheap to copy: every copy shares the same machine.
#[derive(Clone)]
pub struct Host {
    machine: Arc<Mutex<Machine>>,
    /// Notified after every event, so whatever read state redraws.
    changes: RwSignal<()>,
    /// Toasts waiting to be seen, oldest first.
    toasts: RwSignal<Vec<String>>,
    /// Notified when a new config is swapped in, so the screen redraws from it.
    config: RwSignal<()>,
    /// The latest bad config's problems, shown over the last good screen.
    problems: RwSignal<Option<String>>,
    /// Calls waiting for a caller.
    calls: Arc<Mutex<Vec<Call>>>,
    /// Whoever makes the calls, once there is one.
    caller: Arc<Mutex<Option<Caller>>>,
    /// Whether the first mirror has been taken.
    mirrored: Arc<AtomicBool>,
}

impl Host {
    /// A host for `app`, at its starting state. Call [`Host::open`] once the screen is up.
    pub fn new(app: App) -> Host {
        let mut machine = Machine::new(app);
        machine.tap();
        Host {
            machine: Arc::new(Mutex::new(machine)),
            changes: RwSignal::new(()),
            toasts: RwSignal::new(Vec::new()),
            config: RwSignal::new(()),
            problems: RwSignal::new(None),
            calls: Arc::new(Mutex::new(Vec::new())),
            caller: Arc::new(Mutex::new(None)),
            mirrored: Arc::new(AtomicBool::new(false)),
        }
    }

    /// What to send `serve` for the mirror ([spec 050](../../../specs/050_state.md), "The mirror"):
    /// `{ "state": ..., "events": [...] }`, with every trail entry since the last mirror, oldest
    /// first. The first call always has one; after that, `None` when nothing has happened since.
    pub fn mirror(&self) -> Option<Value> {
        let mut machine = self.machine();
        let events = machine.take_tapped();
        let first = !self.mirrored.swap(true, Ordering::Relaxed);
        (first || !events.is_empty())
            .then(|| json!({ "state": machine.snapshot(), "events": events }))
    }

    /// Swaps in a new config, keeping the state ([`Machine::reload`]), clears any problems
    /// banner, and redraws the screen from the new config.
    pub fn reload(&self, app: App) {
        let effects = self.machine().reload(app);
        self.problems.set(None);
        self.config.notify();
        self.after(effects);
    }

    /// Shows a config's problems in a banner over the screen, which keeps running the last good
    /// config ([spec 060](../../../specs/060_hot_reload.md)). The next good config clears it.
    pub fn show_problems(&self, problems: &str) {
        self.problems.set(Some(problems.to_string()));
    }

    /// What this session keeps on the device ([`Machine::kept`]).
    pub fn kept(&self) -> Value {
        self.machine().kept()
    }

    /// Brings back a kept session before the app opens ([`Machine::restore`]), and redraws.
    /// Returns notes on anything it couldn't bring back, for the host to log.
    pub fn restore(&self, kept: &Value) -> Vec<String> {
        let notes = self.machine().restore(kept);
        self.changes.notify();
        notes
    }

    /// Fires `app.opened`.
    pub fn open(&self) {
        let effects = self.machine().open();
        self.after(effects);
    }

    /// Fires an event, as a node does when something happens to it.
    pub fn fire(&self, name: &str, payload: Value) {
        let effects = self.machine().fire(name, payload);
        self.after(effects);
    }

    /// Moves the machine's clock on by `ms` milliseconds, running timers that fall due.
    pub fn advance(&self, ms: u64) {
        let effects = self.machine().advance(ms);
        self.after(effects);
    }

    /// The value at a dotted path (`app.clicks`, `sys.theme`). Reading it inside a view makes
    /// that part of the view redraw when state changes.
    pub fn get(&self, path: &str) -> Option<Value> {
        self.changes.track();
        self.machine().get(path)
    }

    /// The machine itself, locked, for a host that needs more than [`Host::get`]: the mirror
    /// takes its snapshot and trail from here.
    pub fn machine(&self) -> std::sync::MutexGuard<'_, Machine> {
        // A panic while the machine was held would be a bug in the machine; carry on with its state.
        self.machine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Hands every call the machine makes to `caller`, which makes it and answers with
    /// [`Host::reply`]: the browser posts it to serve. Calls made before there was a caller go to
    /// it at once, oldest first.
    pub fn set_caller(&self, caller: impl Fn(Call) + Send + Sync + 'static) {
        let caller: Caller = Arc::new(caller);
        *self.caller.lock().unwrap_or_else(|p| p.into_inner()) = Some(caller.clone());
        for call in self.take_calls() {
            caller(call);
        }
    }

    /// Calls the machine is waiting on, oldest first, for the host to make and answer with
    /// [`Host::reply`].
    pub fn take_calls(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Hands the machine the reply to call `id`.
    pub fn reply(&self, id: u64, reply: Reply) {
        let effects = self.machine().reply(id, reply);
        self.after(effects);
    }

    fn after(&self, effects: Vec<Effect>) {
        let mut texts = Vec::new();
        for effect in effects {
            match effect {
                Effect::Toast { text } => texts.push(text),
                Effect::Call(call) => {
                    // Cloned out first, so the caller runs with no lock held.
                    let caller = self
                        .caller
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .clone();
                    match caller {
                        Some(caller) => caller(call),
                        None => self
                            .calls
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .push(call),
                    }
                }
            }
        }
        if !texts.is_empty() {
            self.toasts.update(|toasts| toasts.extend(texts));
        }
        self.changes.notify();
    }
}

/// A catalog type: what every node type implements to be drawn.
pub trait Component {
    /// The events it can fire, named after its node: `["click"]` makes `NAME.click`.
    const EVENTS: &'static [&'static str];

    /// Draws it. `wired` reads state, fires events, and draws children for it.
    fn view(&self, wired: Wired) -> AnyView;
}

/// The wiring every component is drawn through, the part a base class would play elsewhere.
#[derive(Clone)]
pub struct Wired {
    host: Host,
    /// The node's name: its events fire as `NAME.EVENT`.
    name: String,
    /// Where it sits in the tree, so its children can be named.
    place: String,
    /// Where it sits in its parent: its placement keys, the same for every type.
    placed: Placed,
    /// Inside an `each`, the element it's drawn for: `item.*`.
    item: Option<Value>,
}

impl Wired {
    /// Fires `NAME.event` with `payload`; inside an `each`, the payload carries the element as `item`.
    pub fn fire(&self, event: &str, mut payload: Value) {
        if let (Some(item), Value::Object(fields)) = (&self.item, &mut payload) {
            fields.insert("item".into(), item.clone());
        }
        self.host.fire(&format!("{}.{event}", self.name), payload);
    }

    /// Display text with its templates filled from the current state (and `item.*` inside an
    /// `each`); redraws as state changes.
    pub fn text(&self, text: &str) -> impl Fn() -> String + Clone + Send + Sync + 'static {
        let (host, item, text) = (self.host.clone(), self.item.clone(), text.to_string());
        move || template::text(&text, |path| look(&host, item.as_ref(), path))
    }

    /// The current value at `path` (and `item.*` inside an `each`); redraws as state changes.
    pub fn read(&self, path: &str) -> impl Fn() -> Option<Value> + Clone + Send + Sync + 'static {
        let (host, item, path) = (self.host.clone(), self.item.clone(), path.to_string());
        move || look(&host, item.as_ref(), &path)
    }

    /// The `class` for the node's element: `own` (its component's classes), then its placement.
    pub(crate) fn class(&self, own: &mut ClassList) -> String {
        own.placed(self.placed).attr()
    }

    /// Draws the `i`th child of this node.
    pub fn child(&self, i: usize, node: &Node) -> AnyView {
        draw(
            node,
            child_place(&self.place, i),
            &self.host,
            self.item.clone(),
        )
    }

    /// Draws `children` once per element of the list at `path`, each seeing its element as
    /// `item.*`, or the face for where the list stands: loading or failed (when its variable is a
    /// source), empty, or not a list at all. The faces look the same everywhere.
    pub(crate) fn each(
        &self,
        path: &str,
        empty: Option<&str>,
        error: Option<&str>,
        children: &[Node],
    ) -> impl IntoView + use<> {
        let (host, item, place) = (self.host.clone(), self.item.clone(), self.place.clone());
        let (path, children) = (path.to_string(), children.to_vec());
        let empty = empty.unwrap_or("Nothing here.").to_string();
        let error = error.map(String::from);
        move || {
            if let Some(face) = source_face(&host, &path, error.as_deref()) {
                return face;
            }
            match look(&host, item.as_ref(), &path) {
                None | Some(Value::Null) => empty_face(&empty),
                Some(Value::Array(elements)) if elements.is_empty() => empty_face(&empty),
                Some(Value::Array(elements)) => elements
                    .into_iter()
                    .flat_map(|element| {
                        children
                            .iter()
                            .enumerate()
                            .map(|(i, child)| {
                                draw(child, child_place(&place, i), &host, Some(element.clone()))
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect_view()
                    .into_any(),
                Some(other) => failed_face(
                    None,
                    &format!("each needs a list, and {path} is {}", kind(&other)),
                ),
            }
        }
    }
}

/// Looks `path` up: `item` and `item.*` in the element being drawn, anything else in state.
fn look(host: &Host, item: Option<&Value>, path: &str) -> Option<Value> {
    let Some(item) = item else {
        return host.get(path);
    };
    if path == "item" {
        return Some(item.clone());
    }
    match path.strip_prefix("item.") {
        Some(rest) => rest
            .split('.')
            .try_fold(item, |value, key| match value {
                Value::Array(list) => list.get(key.parse::<usize>().ok()?),
                _ => value.get(key),
            })
            .cloned(),
        None => host.get(path),
    }
}

/// The loading or failed face for a list that's a source (`app.NAME`) while it's loading or
/// has failed; `None` otherwise.
fn source_face(host: &Host, path: &str, error: Option<&str>) -> Option<AnyView> {
    let name = path.strip_prefix("app.").filter(|n| !n.contains('.'))?;
    let call = host.machine().app().sources.get(name)?.call.clone();
    match host.get(&format!("sys.sources.{name}"))?.as_str()? {
        "loading" => Some(
            view! { <span class="loading loading-spinner" role="status" aria-label="Loading"></span> }
                .into_any(),
        ),
        "failed" => {
            // The detail is the newest failure of its operation; `error` can't hide it.
            let failed = format!("{call}.failed");
            let message = host
                .get("sys.errors")
                .and_then(|errors| {
                    errors.as_array()?.iter().find(|e| e["name"] == failed.as_str()).cloned()
                })
                .and_then(|e| e["message"].as_str().map(String::from));
            let detail = match message {
                Some(message) => format!("{call} failed: {message}"),
                None => format!("{call} failed"),
            };
            Some(failed_face(error, &detail))
        }
        _ => None,
    }
}

/// The loud face: an error alert with the cell's own line, if it has one, above the detail.
pub(crate) fn failed_face(line: Option<&str>, detail: &str) -> AnyView {
    let line = line.map(|line| view! { <p class="font-bold">{line.to_string()}</p> });
    let detail = detail.to_string();
    view! { <div role="alert" class="alert alert-error"><div>{line}<p>{detail}</p></div></div> }
        .into_any()
}

/// The face for an empty list.
fn empty_face(text: &str) -> AnyView {
    let text = text.to_string();
    view! { <p class="text-base-content/60">{text}</p> }.into_any()
}

/// A value's kind, in words, for a failure.
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "a table",
        Value::String(_) => "text",
        Value::Number(_) => "a number",
        Value::Bool(_) => "true or false",
        Value::Array(_) | Value::Null => "a list",
    }
}

/// Draws one node through its wiring; inside an `each`, `item` is the element it's drawn for.
fn draw(node: &Node, place: String, host: &Host, item: Option<Value>) -> AnyView {
    let wired = Wired {
        host: host.clone(),
        name: node.name(&place),
        place,
        placed: node.placed(),
        item,
    };
    match node {
        Node::Cell(n) => n.view(wired),
        Node::Text(n) => n.view(wired),
        Node::Button(n) => n.view(wired),
        Node::Image(n) => n.view(wired),
        Node::Link(n) => n.view(wired),
        Node::Svg(n) => n.view(wired),
        Node::Divider(n) => n.view(wired),
        Node::Audio(n) => n.view(wired),
        Node::Video(n) => n.view(wired),
        Node::Iframe(n) => n.view(wired),
        Node::Input(n) => n.view(wired),
        Node::Textarea(n) => n.view(wired),
        Node::Select(n) => n.view(wired),
        Node::Checkbox(n) => n.view(wired),
        Node::Toggle(n) => n.view(wired),
        Node::Radio(n) => n.view(wired),
    }
}

/// The whole screen: the app's nodes, wearing its theme, with toasts on top.
pub fn app_view(host: Host) -> AnyView {
    let nodes = {
        let host = host.clone();
        // Redrawn whenever a new config is swapped in; the state behind it carries over.
        move || {
            host.config.track();
            let body = host.machine().app().body.clone();
            body.iter()
                .enumerate()
                .map(|(i, node)| draw(node, body_place(i), &host, None))
                .collect_view()
        }
    };
    let theme = {
        let host = host.clone();
        move || {
            host.get("sys.theme")
                .and_then(|t| t.as_str().map(String::from))
                .unwrap_or_default()
        }
    };
    let (toasts, problems) = (host.toasts, host.problems);
    view! {
        <div data-theme=theme class="min-h-screen bg-base-100 text-base-content p-6">
            {move || problems.get().map(|text| view! {
                <div role="alert" class="alert alert-warning mb-4 items-start">
                    <div>
                        <p class="font-bold">"This app's config has problems; the screen keeps the last good one."</p>
                        <pre class="whitespace-pre-wrap text-sm">{text}</pre>
                    </div>
                </div>
            })}
            {nodes}
            <div class="toast toast-end">
                {move || toasts.get().into_iter().map(|text| view! { <div class="alert alert-error">{text}</div> }).collect_view()}
            </div>
        </div>
    }
    .into_any()
}

/// The payload a plain interaction carries: nothing.
fn nothing() -> Value {
    json!({})
}
