//! The bus: events in, handlers matched, actions run, state changed.
//!
//! A [`Machine`] holds one app's state and its queue of events ([spec 100](../../../specs/100_events.md)).
//! Everything that happens is an event: the host (the renderer, the bench, a test) hands one to
//! [`Machine::fire`], and the machine handles it and every event that follows from it, one at a
//! time, first in, first out:
//!
//! 1. It finds every handler whose pattern matches the event's name ([`crate::pattern`]), in file
//!    order, and records the event in the trail (`sys.events`) with those handlers.
//! 2. It runs each handler's steps in order. A step that fires an event puts it at the back of
//!    the queue, one deeper in its chain than the event that ran it.
//!
//! The machine does no IO and reads no clock, so the same code runs in the browser, in
//! `genesis-bench`, and in tests.
//!
//! ```
//! use genesis_core::{bus::Machine, config::App};
//! use serde_json::json;
//!
//! let app = App::from_toml("[app]\nname = \"Hi\"\n[on]\n\"hi\" = [{ log = \"hello\" }]\n").unwrap();
//! let mut machine = Machine::new(app);
//! machine.fire("hi", json!({}));
//! assert_eq!(machine.get("sys.events").unwrap()[0]["log"], "hello");
//! ```

use crate::config::{App, Handler, Node, Step};
use crate::{pattern, template};
use serde_json::{Map, Value, json};
use std::collections::VecDeque;

/// One app's state and its queue of events.
pub struct Machine {
    app: App,
    /// Every handler: nodes' `on_*` shortcuts first (in tree order), then `[on]` (in file order).
    handlers: Vec<Handler>,
    /// Every event name the config can fire, for `sys.bus`.
    fires: Vec<String>,
    state: State,
    queue: VecDeque<Queued>,
    /// Milliseconds since the app opened, as the host last said; stamps the trail.
    now: u64,
}

/// What the machine holds besides the config.
struct State {
    /// `app.*`: the app's variables.
    app: Map<String, Value>,
    /// `sys.view`.
    view: String,
    /// `sys.theme`.
    theme: String,
    /// `sys.events`, newest first.
    events: VecDeque<Value>,
}

/// An event waiting its turn.
struct Queued {
    name: String,
    payload: Value,
    /// The names of the events that led to this one, oldest first. Its length is the depth.
    chain: Vec<String>,
}

impl Machine {
    /// A machine for `app`, at its starting state. Nothing has fired yet; see [`Machine::open`].
    pub fn new(app: App) -> Machine {
        let mut handlers = Vec::new();
        let mut node_events = Vec::new();
        for (i, node) in app.body.iter().enumerate() {
            wire_node(node, &format!("body.{i}"), &mut handlers, &mut node_events);
        }
        handlers.extend(app.on.iter().cloned());
        let fires = fires(&app, &handlers, node_events);
        let state = State {
            app: app.state.clone(),
            view: app.app.start.clone(),
            theme: app.app.theme.clone(),
            events: VecDeque::new(),
        };
        Machine {
            app,
            handlers,
            fires,
            state,
            queue: VecDeque::new(),
            now: 0,
        }
    }

    /// Fires `app.opened`, and handles everything that follows from it.
    pub fn open(&mut self) {
        self.fire("app.opened", json!({}));
    }

    /// Fires the event `name` with its `payload`, and handles it and every event that follows,
    /// until the queue is empty.
    pub fn fire(&mut self, name: &str, payload: Value) {
        self.queue.push_back(Queued {
            name: name.to_string(),
            payload,
            chain: Vec::new(),
        });
        while let Some(event) = self.queue.pop_front() {
            self.handle(event);
        }
    }

    /// The value at a dotted path, as a template would see it: `app.clicks`, `sys.view`,
    /// `sys.events`, `sys.bus`. `None` when nothing is there.
    pub fn get(&self, path: &str) -> Option<Value> {
        self.look(path, None)
    }

    fn handle(&mut self, event: Queued) {
        let matched: Vec<usize> = (0..self.handlers.len())
            .filter(|&i| pattern::matches(&self.handlers[i].pattern, &event.name))
            .collect();
        let patterns: Vec<&str> = matched
            .iter()
            .map(|&i| self.handlers[i].pattern.as_str())
            .collect();
        self.record(json!({
            "at": self.now,
            "name": event.name,
            "depth": event.chain.len(),
            "from": event.chain.last(),
            "handlers": patterns,
            "payload": event.payload,
        }));
        for i in matched {
            let steps = self.handlers[i].steps.clone();
            self.run_steps(&steps, &event);
        }
    }

    fn run_steps(&mut self, steps: &[Step], event: &Queued) {
        for step in steps {
            match step {
                Step::Log { log } => {
                    let line = template::text(log, |path| self.look(path, Some(event)));
                    self.record(json!({ "at": self.now, "log": line }));
                }
                Step::Fire { fire, with } => {
                    let Ok(payload) = template::value(&Value::Object(with.clone()), |path| {
                        self.look(path, Some(event))
                    }) else {
                        // Failing a step is spec 100's next tasks; until then a bad payload fires nothing.
                        return;
                    };
                    let mut chain = event.chain.clone();
                    chain.push(event.name.clone());
                    self.queue.push_back(Queued {
                        name: fire.clone(),
                        payload,
                        chain,
                    });
                }
                _ => {}
            }
        }
    }

    /// Adds an entry to the front of the trail, dropping the oldest past `event_trail`.
    fn record(&mut self, entry: Value) {
        self.state.events.push_front(entry);
        self.state.events.truncate(self.app.app.event_trail);
    }

    /// Looks a dotted path up: in the event being handled (`event.*`), the app's variables
    /// (`app.*`), or Genesis's own (`sys.*`).
    fn look(&self, path: &str, event: Option<&Queued>) -> Option<Value> {
        let mut keys = path.split('.');
        let root = match (keys.next()?, keys.next()) {
            ("event", Some("name")) => return event.map(|e| json!(e.name)),
            ("event", Some(key)) => event?.payload.get(key)?.clone(),
            ("app", Some(key)) => self.state.app.get(key)?.clone(),
            ("sys", Some("view")) => json!(self.state.view),
            ("sys", Some("theme")) => json!(self.state.theme),
            ("sys", Some("events")) => json!(self.state.events),
            ("sys", Some("bus")) => self.bus(),
            _ => return None,
        };
        keys.try_fold(root, |at, key| match at {
            Value::Object(mut fields) => fields.remove(key),
            Value::Array(mut items) => key
                .parse()
                .ok()
                .filter(|&i| i < items.len())
                .map(|i| items.swap_remove(i)),
            _ => None,
        })
    }

    /// `sys.bus`: who listens to what, and what the config can fire.
    fn bus(&self) -> Value {
        let handlers: Vec<Value> = self
            .handlers
            .iter()
            .map(|h| json!({ "pattern": h.pattern, "steps": h.steps }))
            .collect();
        json!({ "handlers": handlers, "fires": self.fires })
    }
}

/// Turns a node's `on_*` shortcuts into handlers, and notes the events it fires, for it and its
/// children. A node without an `id` is named by its place in the tree (`body.0.children.1`).
fn wire_node(node: &Node, place: &str, handlers: &mut Vec<Handler>, events: &mut Vec<String>) {
    match node {
        Node::Button(button) => {
            let name = button.id.clone().unwrap_or_else(|| place.to_string());
            if button.id.is_some() || button.on_click.is_some() {
                events.push(format!("{name}.click"));
            }
            if let Some(steps) = &button.on_click {
                handlers.push(Handler {
                    pattern: format!("{name}.click"),
                    steps: steps.clone(),
                });
            }
        }
        Node::Stack(stack) => {
            for (i, child) in stack.children.iter().enumerate() {
                wire_node(child, &format!("{place}.children.{i}"), handlers, events);
            }
        }
        Node::Heading(_) | Node::Text(_) => {}
    }
}

/// Every event name the config can fire, without repeats, for `sys.bus`.
fn fires(app: &App, handlers: &[Handler], node_events: Vec<String>) -> Vec<String> {
    let mut names = vec!["app.opened".to_string()];
    names.extend(node_events);
    names.extend(app.state.keys().map(|k| format!("app.{k}.changed")));
    names.extend([
        "sys.view.changed".to_string(),
        "sys.theme.changed".to_string(),
    ]);
    for name in app.actions.keys() {
        names.extend([format!("{name}.done"), format!("{name}.failed")]);
    }
    let all_steps = handlers
        .iter()
        .map(|h| &h.steps)
        .chain(app.actions.values().map(|a| &a.steps));
    for step in all_steps.flatten() {
        if let Step::Fire { fire, .. } | Step::Every { fire, .. } = step {
            names.push(fire.clone());
        }
    }
    let mut seen = std::collections::HashSet::new();
    names.retain(|name| seen.insert(name.clone()));
    names
}
