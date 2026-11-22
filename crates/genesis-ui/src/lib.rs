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

use genesis_core::bus::{Effect, Machine};
use genesis_core::config::{App, Node, body_place, child_place};
use genesis_core::template;
use leptos::prelude::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

mod components;

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
}

impl Host {
    /// A host for `app`, at its starting state. Call [`Host::open`] once the screen is up.
    pub fn new(app: App) -> Host {
        Host {
            machine: Arc::new(Mutex::new(Machine::new(app))),
            changes: RwSignal::new(()),
            toasts: RwSignal::new(Vec::new()),
        }
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

    fn after(&self, effects: Vec<Effect>) {
        let texts: Vec<String> = effects
            .into_iter()
            .map(|Effect::Toast { text }| text)
            .collect();
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
}

impl Wired {
    /// Fires `NAME.event` with `payload`.
    pub fn fire(&self, event: &str, payload: Value) {
        self.host.fire(&format!("{}.{event}", self.name), payload);
    }

    /// Display text with its templates filled from the current state; redraws as state changes.
    pub fn text(&self, text: &str) -> impl Fn() -> String + Send + Sync + 'static {
        let (host, text) = (self.host.clone(), text.to_string());
        move || template::text(&text, |path| host.get(path))
    }

    /// Draws the `i`th child of this node.
    pub fn child(&self, i: usize, node: &Node) -> AnyView {
        draw(node, child_place(&self.place, i), &self.host)
    }
}

/// Draws one node through its wiring.
fn draw(node: &Node, place: String, host: &Host) -> AnyView {
    let wired = Wired {
        host: host.clone(),
        name: node.name(&place),
        place,
    };
    match node {
        Node::Stack(n) => n.view(wired),
        Node::Heading(n) => n.view(wired),
        Node::Text(n) => n.view(wired),
        Node::Button(n) => n.view(wired),
    }
}

/// The whole screen: the app's nodes, wearing its theme, with toasts on top.
pub fn app_view(host: Host) -> AnyView {
    let app = host.machine().app().clone();
    let nodes: Vec<AnyView> = app
        .body
        .iter()
        .enumerate()
        .map(|(i, node)| draw(node, body_place(i), &host))
        .collect();
    let theme = {
        let host = host.clone();
        move || {
            host.get("sys.theme")
                .and_then(|t| t.as_str().map(String::from))
                .unwrap_or_default()
        }
    };
    let toasts = host.toasts;
    view! {
        <div data-theme=theme class="min-h-screen bg-base-100 text-base-content p-6">
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
