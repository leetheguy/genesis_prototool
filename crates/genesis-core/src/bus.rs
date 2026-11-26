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

use crate::config::{
    App, Audio, Component, Handler, InputKind, Node, Source, Step, Video, body_place, child_place,
};
use crate::{pattern, template};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Something the machine needs its host to do, since the machine itself does no IO.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Show a short message. Every failure nobody handles becomes one.
    Toast {
        /// The message.
        text: String,
    },
    /// Make a call to an operation, and answer it with [`Machine::reply`].
    Call(Call),
}

/// A call for the host to make ([spec 030](../../../specs/030_connections.md)). The machine only
/// names the operation: the host that holds the credentials (serve) looks up its method and URL
/// in the app's own config, so a call can never reach anything the config doesn't declare.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// Answer with this, in [`Machine::reply`].
    pub id: u64,
    /// The operation's name: `crm.contacts.list`.
    pub op: String,
    /// Its params, templates filled in.
    pub params: Value,
    /// Its headers, with `app.*` and `sys.*` templates filled in and `{{ creds.NAME }}` left for
    /// the host to fill.
    pub headers: BTreeMap<String, String>,
}

/// Where an event fired from inside a component: which instance, and its props as filled where
/// the `use` node is ([spec 090](../../../specs/090_components.md)). The renderer passes it with
/// [`Machine::fire_within`], so the component's own steps read `props.*` and its `local.*`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Within {
    /// The `use` node's name (`card`, or `list.card` inside another component).
    pub name: String,
    /// The row of an `each` the `use` node is drawn in, if any.
    pub row: Option<String>,
    /// Its props, filled.
    pub props: Value,
}

/// The component a handler belongs to: the `use` node's name, and the component's.
#[derive(Debug, Clone, PartialEq)]
struct Owner {
    name: String,
    component: String,
}

/// The component instance a run's steps belong to: whose `local.*` they read and write, and the
/// props they read.
#[derive(Debug, Clone, PartialEq)]
struct Scope {
    /// The `use` node's name.
    name: String,
    component: String,
    /// Where its locals are kept: its name, and its row ([`draft_key`]).
    key: String,
    props: Value,
}

/// How a call came out.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    /// The API answered: its status, and its body as JSON (`null` when it sent none).
    Response {
        /// The HTTP status.
        status: u16,
        /// The body.
        body: Value,
    },
    /// No answer to read: a timeout, a refused connection, a body that isn't JSON, a credential that
    /// isn't set. Says what went wrong, with no secrets in it.
    Failed(String),
}

/// A run waiting on a call.
struct Waiting {
    run: Run,
    /// The source this call loads, if it's one.
    source: Option<String>,
    op: String,
    into: BTreeMap<String, String>,
    event: Option<String>,
}

/// Why settling a reply failed, and whether a `.failed` event already says so.
struct CallFailed {
    message: String,
    reported: bool,
}

/// A list of steps being run, which can wait on a timer and carry on later.
struct Run {
    /// The lists it's inside, innermost last: a handler's list, then each custom action it ran.
    frames: Vec<Frame>,
    /// The event that started it, which its templates read as `event.*`.
    event: Queued,
    /// The source whose handler started it, if one did: its calls set the source's status.
    source: Option<String>,
}

/// One list of steps, and how far through it a run has got.
struct Frame {
    steps: Vec<Step>,
    next: usize,
    /// The custom action this list belongs to, if it's one; it fires `NAME.done` when the list ends.
    action: Option<String>,
}

/// Something waiting for the clock.
struct Timer {
    due: u64,
    /// The name `cancel` stops it by.
    id: Option<String>,
    kind: TimerKind,
}

enum TimerKind {
    /// A run paused by `delay`.
    Resume(Box<Run>),
    /// An `every`: fires `fire` with `payload`, then sets itself again `interval` later.
    Every {
        fire: String,
        payload: Value,
        interval: u64,
    },
}

/// One app's state and its queue of events.
pub struct Machine {
    app: App,
    /// Every handler: nodes' `on_*` shortcuts first (in tree order), then `[on]` (in file order).
    handlers: Vec<Handler>,
    /// Which source each handler belongs to, if any, in step with `handlers`.
    handler_sources: Vec<Option<String>>,
    /// Which component each handler belongs to, if any, in step with `handlers`.
    handler_owners: Vec<Option<Owner>>,
    /// Every component instance's `use` node name, and its component's: whose locals may be kept.
    instances: BTreeMap<String, String>,
    /// Every event name the config can fire, for `sys.bus`.
    fires: Vec<String>,
    /// Every form, and the named controls that fill them.
    forms: Forms,
    state: State,
    queue: VecDeque<Queued>,
    /// What the host must do, gathered while the queue runs.
    effects: Vec<Effect>,
    /// App variables a reply's `into` filled last, which a session doesn't keep.
    filled: BTreeSet<String>,
    /// Runs waiting on calls, by call id.
    waiting: BTreeMap<u64, Waiting>,
    /// The next call's id.
    next_call: u64,
    /// Waiting runs and repeating events, in the order they were set.
    timers: Vec<Timer>,
    /// Trail entries kept for [`Machine::take_tapped`], once [`Machine::tap`] is called.
    tapped: Option<Vec<Value>>,
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
    /// `sys.errors`, newest first.
    errors: VecDeque<Value>,
    /// `sys.sources`: each source's status: `idle`, `loading`, `ok`, or `failed`.
    sources: BTreeMap<String, &'static str>,
    /// Each form's draft, by its name (and row, inside an `each`: [`draft_key`]): the values
    /// typed and not yet submitted, by field.
    drafts: BTreeMap<String, Map<String, Value>>,
    /// Each component instance's `local.*` that have been written, by its name (and row, like a
    /// draft); the rest are at the component's starting values.
    locals: BTreeMap<String, Map<String, Value>>,
}

/// An event waiting its turn.
#[derive(Clone)]
struct Queued {
    name: String,
    payload: Value,
    /// The names of the events that led to this one, oldest first. Its length is the depth.
    chain: Vec<String>,
    /// Inside an `each`, the row the node that fired it is drawn in ([`Machine::fire_at`]).
    row: Option<String>,
    /// Inside a component, where it fired from ([`Machine::fire_within`]).
    within: Option<Within>,
    /// While a component's handler runs it, that instance.
    scope: Option<Scope>,
}

impl Machine {
    /// A machine for `app`, at its starting state. Nothing has fired yet; see [`Machine::open`].
    pub fn new(app: App) -> Machine {
        let Wiring {
            handlers,
            sources: handler_sources,
            owners: handler_owners,
            instances,
            fires,
            forms,
        } = wiring(&app);
        let sources = app
            .sources
            .keys()
            .map(|name| (name.clone(), "idle"))
            .collect();
        let state = State {
            app: starting(&app),
            sources,
            view: app.app.start.clone(),
            theme: app.app.theme.clone(),
            events: VecDeque::new(),
            errors: VecDeque::new(),
            drafts: BTreeMap::new(),
            locals: BTreeMap::new(),
        };
        Machine {
            app,
            handlers,
            handler_sources,
            handler_owners,
            instances,
            fires,
            forms,
            state,
            queue: VecDeque::new(),
            effects: Vec::new(),
            filled: BTreeSet::new(),
            waiting: BTreeMap::new(),
            next_call: 1,
            timers: Vec::new(),
            tapped: None,
            now: 0,
        }
    }

    /// What a session keeps on the device ([spec 050](../../../specs/050_state.md), "Persisting"):
    /// what the user did, never what an API said. That's `app.*` (except sources, variables a reply
    /// filled last, and `[app] forget`), and `sys.view`, `sys.theme`, `sys.errors`, and `sys.events`.
    pub fn kept(&self) -> Value {
        let app: Map<String, Value> = self
            .state
            .app
            .iter()
            .filter(|(k, _)| self.keeps(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        json!({
            "app": app,
            "sys": { "view": self.state.view, "theme": self.state.theme, "errors": self.state.errors, "events": self.state.events },
            "drafts": self.shown_drafts(),
            "locals": self.state.locals,
        })
    }

    /// Every form's draft, without what a `password` input holds: what a session keeps and the
    /// mirror shows.
    fn shown_drafts(&self) -> BTreeMap<&str, Map<String, Value>> {
        self.state
            .drafts
            .iter()
            .map(|(form, draft)| {
                let hidden = self.forms.forms.get(form_of(form)).map(|f| &f.hidden);
                let shown = draft
                    .iter()
                    .filter(|(field, _)| !hidden.is_some_and(|h| h.contains(*field)))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                (form.as_str(), shown)
            })
            .filter(|(_, draft): &(&str, Map<String, Value>)| !draft.is_empty())
            .collect()
    }

    /// The value typed into `form`'s control named `field`, and not yet submitted. `None` when
    /// nothing has been: the control shows its form's `data` instead.
    pub fn draft(&self, form: &str, field: &str) -> Option<Value> {
        self.draft_at(form, None, field)
    }

    /// The value of `local.NAME` for the component instance whose `use` node is named `instance`,
    /// drawn in `row` of an `each` if it is: what its steps wrote, else its component's starting
    /// value. `None` when the component doesn't declare it.
    pub fn local(&self, instance: &str, row: Option<&str>, name: &str) -> Option<Value> {
        self.state
            .locals
            .get(&draft_key(instance, row))
            .and_then(|locals| locals.get(name))
            .or_else(|| {
                let component = self.instances.get(instance)?;
                self.app.components.get(component)?.state.get(name)
            })
            .cloned()
    }

    /// The same as [`Machine::draft`], for a form drawn in `row` of an `each`.
    pub fn draft_at(&self, form: &str, row: Option<&str>, field: &str) -> Option<Value> {
        self.state
            .drafts
            .get(&draft_key(form, row))?
            .get(field)
            .cloned()
    }

    /// Brings back what [`Machine::kept`] gave, before the app opens. A variable the config no
    /// longer declares, or whose starting value now has another JSON type, starts fresh instead;
    /// each of those, and kept state that isn't kept state at all, comes back as a note for the
    /// host to log. Nothing fires.
    pub fn restore(&mut self, kept: &Value) -> Vec<String> {
        let Some(kept) = kept.as_object() else {
            return vec!["the kept state isn't kept state; starting fresh".into()];
        };
        let mut notes = Vec::new();
        for (key, value) in kept
            .get("app")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            match self.state.app.get(key) {
                None => notes.push(format!(
                    "app.{key} is gone from the config; dropped what was kept"
                )),
                Some(_) if !self.keeps(key) => {}
                Some(start) if !start.is_null() && kind(start) != kind(value) => {
                    notes.push(format!(
                        "app.{key} is a {} now, not a {}; starting it fresh",
                        kind(start),
                        kind(value)
                    ))
                }
                Some(_) => {
                    self.state.app.insert(key.clone(), value.clone());
                }
            }
        }
        let sys = kept.get("sys");
        if let Some(view) = sys.and_then(|s| s["view"].as_str()) {
            self.state.view = view.to_string();
        }
        if let Some(theme) = sys.and_then(|s| s["theme"].as_str()) {
            self.state.theme = theme.to_string();
        }
        let trail = self.app.app.event_trail;
        let list = |key: &str| -> Option<VecDeque<Value>> {
            sys?.get(key)?
                .as_array()
                .map(|a| a.iter().take(trail).cloned().collect())
        };
        if let Some(errors) = list("errors") {
            self.state.errors = errors;
        }
        if let Some(events) = list("events") {
            self.state.events = events;
        }
        let drafts = kept.get("drafts").and_then(Value::as_object);
        for (form, draft) in drafts.into_iter().flatten() {
            let Some(wired) = self.forms.forms.get(form_of(form)) else {
                notes.push(format!(
                    "the form {} is gone from the config; dropped its draft",
                    form_of(form)
                ));
                continue;
            };
            for (field, value) in draft.as_object().into_iter().flatten() {
                if !wired.fields.contains(field) {
                    notes.push(format!(
                        "{} has no control named {field} now; dropped what was typed in it",
                        form_of(form)
                    ));
                } else if !wired.hidden.contains(field) {
                    self.state
                        .drafts
                        .entry(form.clone())
                        .or_default()
                        .insert(field.clone(), value.clone());
                }
            }
        }
        let locals = kept.get("locals").and_then(Value::as_object);
        for (key, values) in locals.into_iter().flatten() {
            let declared = self
                .instances
                .get(form_of(key))
                .and_then(|c| self.app.components.get(c))
                .map(|c| &c.state);
            let Some(declared) = declared else {
                notes.push(format!(
                    "the component {} is gone from the config; dropped its local state",
                    form_of(key)
                ));
                continue;
            };
            for (name, value) in values.as_object().into_iter().flatten() {
                match declared.get(name) {
                    Some(start) if start.is_null() || kind(start) == kind(value) => {
                        self.state
                            .locals
                            .entry(key.clone())
                            .or_default()
                            .insert(name.clone(), value.clone());
                    }
                    _ => notes.push(format!(
                        "{} has no local.{name} of that kind now; starting it fresh",
                        form_of(key)
                    )),
                }
            }
        }
        notes
    }

    /// Whether the app variable `key` is one a session keeps.
    fn keeps(&self, key: &str) -> bool {
        !self.app.sources.contains_key(key)
            && !self.filled.contains(key)
            && !self.app.app.forget.iter().any(|f| f == key)
    }

    /// The app this machine runs, as loaded.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Swaps in a new config for the app, keeping its state ([spec 050](../../../specs/050_state.md),
    /// "Across a config reload"): variables still declared keep their values, new ones start at
    /// their starting values, and removed ones are dropped. The view, theme, trail, errors, and
    /// timers carry over. Then it fires `config.loaded`, and returns what the host must do.
    pub fn reload(&mut self, app: App) -> Vec<Effect> {
        let Wiring {
            handlers,
            sources,
            owners,
            instances,
            fires,
            forms,
        } = wiring(&app);
        // A component instance's locals stay while it does, each while it's still declared with
        // the same kind of value.
        self.state.locals.retain(|key, locals| {
            let Some(state) = instances
                .get(form_of(key))
                .and_then(|c| app.components.get(c))
                .map(|c| &c.state)
            else {
                return false;
            };
            locals.retain(|name, value| {
                state
                    .get(name)
                    .is_some_and(|start| start.is_null() || kind(start) == kind(value))
            });
            true
        });
        (self.handler_owners, self.instances) = (owners, instances);
        // A draft stays while its form and field do.
        self.state.drafts.retain(|key, draft| {
            let Some(wired) = forms.forms.get(form_of(key)) else {
                return false;
            };
            draft.retain(|field, _| wired.fields.contains(field));
            true
        });
        self.forms = forms;
        let old = std::mem::take(&mut self.state.app);
        self.state.app = starting(&app)
            .into_iter()
            .map(|(k, start)| {
                let kept = old.get(&k).cloned().unwrap_or(start);
                (k, kept)
            })
            .collect();
        let statuses = std::mem::take(&mut self.state.sources);
        self.state.sources = app
            .sources
            .keys()
            .map(|name| (name.clone(), statuses.get(name).copied().unwrap_or("idle")))
            .collect();
        (self.app, self.handlers, self.handler_sources, self.fires) =
            (app, handlers, sources, fires);
        self.fire("config.loaded", json!({}))
    }

    /// Hands the machine the reply to call `id`, and handles everything that follows: `OP.ok`
    /// and `OP.{status}` (and the reply's own event, with `event`) and the list carrying on, or
    /// `OP.failed` and the list stopping. Returns what the host must do as a result.
    pub fn reply(&mut self, id: u64, reply: Reply) -> Vec<Effect> {
        match self.waiting.remove(&id) {
            None => {
                let message = format!("a reply came for call {id}, which isn't waiting");
                self.fail(json!({ "name": "reply.unknown", "message": message }), true);
            }
            Some(Waiting {
                run,
                source,
                op,
                into,
                event,
            }) => {
                let settled = self.settle(&run.event, &op, &into, event.as_deref(), reply);
                if let Some(name) = source {
                    self.state
                        .sources
                        .insert(name, if settled.is_ok() { "ok" } else { "failed" });
                }
                match settled {
                    Ok(()) => self.execute(run),
                    Err(failed) => self.unwind(run, None, failed.message, failed.reported),
                }
            }
        }
        self.drain();
        std::mem::take(&mut self.effects)
    }

    /// Fires `app.opened`, and handles everything that follows from it.
    pub fn open(&mut self) -> Vec<Effect> {
        self.fire("app.opened", json!({}))
    }

    /// Fires the event `name` with its `payload`, and handles it and every event that follows,
    /// until the queue is empty. Returns what the host must do as a result.
    pub fn fire(&mut self, name: &str, payload: Value) -> Vec<Effect> {
        self.fire_at(name, payload, None)
    }

    /// The same as [`Machine::fire`], for a node drawn inside an `each`: `row` is where it's
    /// drawn, as the renderer writes it (`"2"`, or `"0.2"` inside two). A form drawn in a row
    /// keeps a draft of its own for that row, so each row's form is filled and submitted apart.
    pub fn fire_at(&mut self, name: &str, payload: Value, row: Option<&str>) -> Vec<Effect> {
        self.fire_within(name, payload, row, None)
    }

    /// The same as [`Machine::fire_at`], for a node drawn inside a component: `within` says which
    /// instance, and its props, so the component's own steps read its `local.*` and `props.*`.
    pub fn fire_within(
        &mut self,
        name: &str,
        payload: Value,
        row: Option<&str>,
        within: Option<Within>,
    ) -> Vec<Effect> {
        self.queue.push_back(Queued {
            name: name.to_string(),
            payload,
            chain: Vec::new(),
            row: row.map(String::from),
            within,
            scope: None,
        });
        self.drain();
        std::mem::take(&mut self.effects)
    }

    /// Moves the clock on by `ms` milliseconds. Every timer that falls due runs in its turn,
    /// each with the clock at its own moment, and everything that follows from it is handled.
    /// Returns what the host must do as a result.
    pub fn advance(&mut self, ms: u64) -> Vec<Effect> {
        let until = self.now + ms;
        while let Some(i) = self.next_timer(until) {
            let timer = self.timers.remove(i);
            self.now = timer.due;
            match timer.kind {
                TimerKind::Resume(mut run) => {
                    // After a pause the rest of the list starts a new chain, so a loop with a
                    // delay in it never hits max_chain.
                    run.event.chain.clear();
                    self.execute(*run);
                }
                TimerKind::Every {
                    fire,
                    payload,
                    interval,
                } => {
                    self.queue.push_back(Queued {
                        name: fire.clone(),
                        payload: payload.clone(),
                        chain: Vec::new(),
                        row: None,
                        within: None,
                        scope: None,
                    });
                    let kind = TimerKind::Every {
                        fire,
                        payload,
                        interval,
                    };
                    self.set_timer(self.now + interval, timer.id, kind);
                }
            }
            self.drain();
        }
        self.now = until;
        std::mem::take(&mut self.effects)
    }

    /// Handles queued events until there are none.
    fn drain(&mut self) {
        while let Some(event) = self.queue.pop_front() {
            self.handle(event);
        }
    }

    /// The timer due soonest, no later than `until`; of two due at once, the one set first.
    fn next_timer(&self, until: u64) -> Option<usize> {
        let due = self
            .timers
            .iter()
            .enumerate()
            .filter(|(_, t)| t.due <= until);
        // min_by_key keeps the first of equal keys, and the list is in the order timers were set.
        due.min_by_key(|(_, t)| t.due).map(|(i, _)| i)
    }

    fn set_timer(&mut self, due: u64, id: Option<String>, kind: TimerKind) {
        self.timers.push(Timer { due, id, kind });
    }

    /// Starts keeping a copy of every trail entry as it's recorded, for [`Machine::take_tapped`].
    /// The bench prints them, and the mirror will write them; neither should lose entries
    /// that the trail's cap has dropped.
    pub fn tap(&mut self) {
        self.tapped.get_or_insert_with(Vec::new);
    }

    /// Every trail entry recorded since the last call, oldest first. Empty without [`Machine::tap`].
    pub fn take_tapped(&mut self) -> Vec<Value> {
        self.tapped.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// The state as one value: `{ "app": {...}, "sys": { "view", "theme", "errors" }, "drafts": {...} }`,
    /// with no `password` input's value in it.
    pub fn snapshot(&self) -> Value {
        json!({
            "app": self.state.app,
            "sys": { "view": self.state.view, "theme": self.state.theme, "errors": self.state.errors },
            "drafts": self.shown_drafts(),
            "locals": self.state.locals,
        })
    }

    /// The value at a dotted path, as a template would see it: `app.clicks`, `sys.view`,
    /// `sys.events`, `sys.bus`. `None` when nothing is there.
    pub fn get(&self, path: &str) -> Option<Value> {
        self.look(path, None)
    }

    fn handle(&mut self, mut event: Queued) {
        self.fill_form(&mut event);
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
            "payload": self.trail_payload(&event),
        }));
        // A failure is logged before any handler runs, and is shown if nobody handles it.
        if event.name.ends_with(".failed") {
            let message = match event.payload.get("error") {
                Some(Value::String(error)) => error.clone(),
                _ => event.payload.to_string(),
            };
            self.fail(
                json!({ "name": event.name, "message": message }),
                matched.is_empty(),
            );
        }
        for i in matched {
            let frame = Frame {
                steps: self.handlers[i].steps.clone(),
                next: 0,
                action: None,
            };
            let source = self.handler_sources[i].clone();
            let mut event = event.clone();
            event.scope = self.handler_owners[i].as_ref().map(|owner| {
                // The renderer says which instance fired; an event pushed in from outside is
                // taken as the one in its row.
                let (row, props) = match &event.within {
                    Some(within) if within.name == owner.name => {
                        (within.row.clone(), within.props.clone())
                    }
                    _ => (event.row.clone(), Value::Null),
                };
                Scope {
                    name: owner.name.clone(),
                    component: owner.component.clone(),
                    key: draft_key(&owner.name, row.as_deref()),
                    props,
                }
            });
            self.execute(Run {
                frames: vec![frame],
                event,
                source,
            });
        }
    }

    /// Runs a list of steps until it ends, fails, or waits on a timer.
    ///
    /// A plain-string step runs a custom action by putting its list on top of the run's stack;
    /// when a list ends, its action fires `NAME.done`. A `delay` parks the whole run on a timer.
    /// A failure stops the run ([`Machine::unwind`]).
    fn execute(&mut self, mut run: Run) {
        loop {
            let Some(frame) = run.frames.last_mut() else {
                return;
            };
            let Some(step) = frame.steps.get(frame.next).cloned() else {
                if let Some(name) = run.frames.pop().and_then(|f| f.action) {
                    self.enqueue(&format!("{name}.done"), json!({}), &run.event);
                }
                continue;
            };
            frame.next += 1;
            let result = match &step {
                Step::Run(name) => self.enter(&mut run, name),
                Step::Delay { delay, id } => {
                    self.set_timer(
                        self.now + delay,
                        id.clone(),
                        TimerKind::Resume(Box::new(run)),
                    );
                    return;
                }
                Step::Every {
                    every,
                    fire,
                    id,
                    with,
                } => self.every(*every, fire, id, with, &run.event),
                Step::Call { call, params, .. } => {
                    match self.begin_call(call, params, &run.event) {
                        Ok(call) => {
                            let Step::Call {
                                call: op,
                                into,
                                event,
                                ..
                            } = step
                            else {
                                unreachable!()
                            };
                            let source = run.source.clone();
                            if let Some(name) = &source {
                                self.state.sources.insert(name.clone(), "loading");
                            }
                            self.waiting.insert(
                                call.id,
                                Waiting {
                                    run,
                                    source,
                                    op,
                                    into,
                                    event,
                                },
                            );
                            self.effects.push(Effect::Call(call));
                            return;
                        }
                        Err(message) => Err(message),
                    }
                }
                Step::Reload { reload } => {
                    reload
                        .iter()
                        .try_for_each(|var| match var.strip_prefix("app.") {
                            Some(name) if self.app.sources.contains_key(name) => {
                                self.enqueue(&format!("app.{name}.reload"), json!({}), &run.event);
                                Ok(())
                            }
                            _ => Err(format!("{var} isn't a source")),
                        })
                }
                Step::Cancel { cancel } => {
                    self.timers
                        .retain(|t| t.id.as_deref() != Some(cancel.as_str()));
                    Ok(())
                }
                other => self.run_step(other, &run.event),
            };
            if let Err(message) = result {
                return self.unwind(run, Some(&step), message, false);
            }
        }
    }

    /// Starts the custom action `name` inside `run`, up to `max_chain` actions deep.
    fn enter(&self, run: &mut Run, name: &str) -> Result<(), String> {
        let max = self.app.app.max_chain as usize;
        if run.frames.len() > max {
            return Err(format!("actions nested more than max_chain ({max}) deep"));
        }
        let action = self
            .app
            .actions
            .get(name)
            .ok_or_else(|| format!("there's no action named {name}"))?;
        run.frames.push(Frame {
            steps: action.steps.clone(),
            next: 0,
            action: Some(name.to_string()),
        });
        Ok(())
    }

    /// Sets an `every` timer, its payload filled in now.
    fn every(
        &mut self,
        every: u64,
        fire: &str,
        id: &Option<String>,
        with: &Map<String, Value>,
        event: &Queued,
    ) -> Result<(), String> {
        if every == 0 {
            return Err("every must be at least 1 millisecond".into());
        }
        let payload = template::value(&Value::Object(with.clone()), |path| {
            self.look(path, Some(event))
        })?;
        let kind = TimerKind::Every {
            fire: fire.to_string(),
            payload,
            interval: every,
        };
        self.set_timer(self.now + every, id.clone(), kind);
        Ok(())
    }

    /// Stops a run whose `step` failed. Every custom action it was inside fires `NAME.failed`
    /// with the error, innermost first; so does an action that couldn't start. A failure nothing
    /// reported (no action, and no `.failed` event already, as a failed call fires) is logged
    /// under the event's name, and toasted.
    fn unwind(&mut self, run: Run, step: Option<&Step>, message: String, reported: bool) {
        let starting = match step {
            Some(Step::Run(name)) => Some(name.clone()),
            _ => None,
        };
        let actions: Vec<String> = starting
            .into_iter()
            .chain(run.frames.into_iter().rev().filter_map(|f| f.action))
            .collect();
        for name in &actions {
            self.enqueue(
                &format!("{name}.failed"),
                json!({ "error": message }),
                &run.event,
            );
        }
        if actions.is_empty() && !reported {
            self.fail(json!({ "name": run.event.name, "message": message }), true);
        }
    }

    /// Makes the call a `call` step asks for: the operation must be declared, and every template
    /// in its params must have a value.
    fn begin_call(
        &mut self,
        op: &str,
        params: &Map<String, Value>,
        event: &Queued,
    ) -> Result<Call, String> {
        let operation = self
            .app
            .operation(op)
            .ok_or_else(|| format!("there's no operation named {op}"))?;
        let params = template::value(&Value::Object(params.clone()), |path| {
            self.look(path, Some(event))
        })?;
        // Credentials are the host's to fill, so their templates pass through as they are.
        let fill = |path: &str| match path.strip_prefix("creds.") {
            Some(_) => Some(json!(format!("{{{{ {path} }}}}"))),
            None => self.look(path, Some(event)),
        };
        let headers = operation
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), template::text(v, fill)))
            .collect();
        let id = self.next_call;
        self.next_call += 1;
        Ok(Call {
            id,
            op: op.to_string(),
            params,
            headers,
        })
    }

    /// Turns a reply into events and variables. A good reply fills `into`, then fires `OP.ok`,
    /// `OP.{status}`, and the reply's own event; a bad one fires `OP.failed` (and `OP.{status}`).
    fn settle(
        &mut self,
        cause: &Queued,
        op: &str,
        into: &BTreeMap<String, String>,
        event: Option<&str>,
        reply: Reply,
    ) -> Result<(), CallFailed> {
        let (status, body) = match reply {
            Reply::Failed(message) => {
                self.enqueue(&format!("{op}.failed"), json!({ "error": message }), cause);
                return Err(CallFailed {
                    message,
                    reported: true,
                });
            }
            Reply::Response { status, body } => (status, body),
        };
        if !(200..300).contains(&status) {
            let text: String = text_of(&body).chars().take(500).collect();
            let message = format!("HTTP {status}: {text}");
            self.enqueue(
                &format!("{op}.failed"),
                json!({ "status": status, "body": body, "error": message }),
                cause,
            );
            self.enqueue(
                &format!("{op}.{status}"),
                json!({ "status": status, "body": body }),
                cause,
            );
            return Err(CallFailed {
                message,
                reported: true,
            });
        }
        let at = |pointer: &str| {
            body.pointer(pointer).cloned().ok_or_else(|| CallFailed {
                message: format!("nothing at {pointer} in the reply to {op}"),
                reported: false,
            })
        };
        let values = into
            .iter()
            .map(|(var, pointer)| Ok((var.clone(), at(pointer)?)))
            .collect::<Result<Vec<_>, CallFailed>>()?;
        let branch = event
            .map(|pointer| at(pointer).map(|v| text_of(&v)))
            .transpose()?;
        let payload = json!({ "status": status, "body": body });
        for name in ["ok".to_string(), status.to_string()]
            .into_iter()
            .chain(branch)
        {
            self.enqueue(&format!("{op}.{name}"), payload.clone(), cause);
        }
        for (var, value) in values {
            self.write(&var, value, cause)
                .map_err(|message| CallFailed {
                    message,
                    reported: false,
                })?;
            if let Some(key) = var.strip_prefix("app.") {
                self.filled.insert(key.to_string());
            }
        }
        Ok(())
    }

    /// Runs one built-in step that neither pauses nor nests.
    fn run_step(&mut self, step: &Step, event: &Queued) -> Result<(), String> {
        let fill = |value: &Value| template::value(value, |path| self.look(path, Some(event)));
        match step {
            Step::Log { log } => {
                let line = template::text(log, |path| self.look(path, Some(event)));
                self.record(json!({ "at": self.now, "log": line }));
            }
            Step::Fire { fire, with } => {
                let payload = fill(&Value::Object(with.clone()))?;
                self.enqueue(fire, payload, event);
            }
            Step::Set { set, to } => {
                let value = fill(to)?;
                self.write(set, value, event)?;
            }
            Step::Step { step, by, min, max } => {
                let now = self.writable(step, event)?;
                let n = now
                    .as_f64()
                    .ok_or_else(|| format!("{step} isn't a number"))?;
                let sum = (n + by)
                    .max(min.unwrap_or(f64::MIN))
                    .min(max.unwrap_or(f64::MAX));
                self.write(step, number(sum), event)?;
            }
            Step::Toggle { toggle } => {
                let on = self
                    .writable(toggle, event)?
                    .as_bool()
                    .ok_or_else(|| format!("{toggle} isn't true or false"))?;
                self.write(toggle, json!(!on), event)?;
            }
            Step::Go { go } => self.write("sys.view", json!(go), event)?,
            _ => {}
        }
        Ok(())
    }

    /// The current value of a variable actions may write: a declared `app.*`, `sys.view`, or
    /// `sys.theme`, or inside a component, a declared `local.*`. Anything else is an error saying why.
    fn writable(&self, var: &str, cause: &Queued) -> Result<Value, String> {
        match var.split_once('.') {
            Some(("local", key)) => {
                let scope = cause
                    .scope
                    .as_ref()
                    .ok_or_else(|| format!("{var} is only inside a component"))?;
                self.state
                    .locals
                    .get(&scope.key)
                    .and_then(|locals| locals.get(key))
                    .or_else(|| self.app.components.get(&scope.component)?.state.get(key))
                    .cloned()
                    .ok_or_else(|| format!("{var} isn't declared in {}'s [state]", scope.component))
            }
            Some(("app", key)) => self
                .state
                .app
                .get(key)
                .cloned()
                .ok_or_else(|| format!("{var} isn't declared in [state]")),
            Some(("sys", "view")) => Ok(json!(self.state.view)),
            Some(("sys", "theme")) => Ok(json!(self.state.theme)),
            _ => Err(format!("{var} can't be written")),
        }
    }

    /// Writes a variable and, when the value changed, fires `{var}.changed` with `value` and `was`.
    fn write(&mut self, var: &str, value: Value, cause: &Queued) -> Result<(), String> {
        let was = self.writable(var, cause)?;
        // Written by an action, it's the user's again; settle() marks what a reply fills.
        if let Some(key) = var.strip_prefix("app.") {
            self.filled.remove(key);
        }
        if was == value {
            return Ok(());
        }
        let mut changed = format!("{var}.changed");
        match var.split_once('.') {
            Some(("app", key)) => {
                self.state.app.insert(key.to_string(), value.clone());
            }
            Some(("local", key)) => {
                // writable() found the scope. Its change fires under the `use` node's name, as
                // every event inside a component does.
                if let Some(scope) = &cause.scope {
                    self.state
                        .locals
                        .entry(scope.key.clone())
                        .or_default()
                        .insert(key.to_string(), value.clone());
                    changed = format!("{}.{changed}", scope.name);
                }
            }
            Some((_, "view")) => self.state.view = text_of(&value),
            _ => self.state.theme = text_of(&value),
        }
        self.enqueue(&changed, json!({ "value": value, "was": was }), cause);
        Ok(())
    }

    /// Puts an event a handler fired at the back of the queue, one deeper than its cause.
    /// Past `max_chain` the event is dropped instead, and `chain.too_deep` fails loud.
    fn enqueue(&mut self, name: &str, payload: Value, cause: &Queued) {
        let mut chain = cause.chain.clone();
        chain.push(cause.name.clone());
        if chain.len() > self.app.app.max_chain as usize {
            chain.push(name.to_string());
            let message = format!(
                "a chain of events went past max_chain ({}): {}",
                self.app.app.max_chain,
                chain.join(" > ")
            );
            let error = json!({ "name": "chain.too_deep", "message": message, "chain": chain });
            self.fail(error, true);
            return;
        }
        self.queue.push_back(Queued {
            name: name.to_string(),
            payload,
            chain,
            row: None,
            within: None,
            scope: None,
        });
    }

    /// Logs a failure in `sys.errors` and, when `show` is set, toasts it.
    fn fail(&mut self, mut error: Value, show: bool) {
        if show {
            let text = format!(
                "{}: {}",
                text_of(&error["name"]),
                text_of(&error["message"])
            );
            self.effects.push(Effect::Toast { text });
        }
        error["at"] = json!(self.now);
        self.state.errors.push_front(error);
        self.state.errors.truncate(self.app.app.event_trail);
    }

    /// Adds an entry to the front of the trail, dropping the oldest past `event_trail`.
    fn record(&mut self, entry: Value) {
        if let Some(tapped) = &mut self.tapped {
            tapped.push(entry.clone());
        }
        self.state.events.push_front(entry);
        self.state.events.truncate(self.app.app.event_trail);
    }

    /// A named control's `.change` puts its value in its form's draft. A form's `.submit` takes
    /// its draft: the payload's `form` gets every named control's value (typed, else from the
    /// form's `data`, else `null`), and the draft starts over.
    fn fill_form(&mut self, event: &mut Queued) {
        if let Some((form, field)) = self.forms.fields.get(&event.name).cloned() {
            let value = event.payload.get("value").cloned().unwrap_or(Value::Null);
            self.state
                .drafts
                .entry(draft_key(&form, event.row.as_deref()))
                .or_default()
                .insert(field, value);
            return;
        }
        let Some(name) = event.name.strip_suffix(".submit") else {
            return;
        };
        let Some(wired) = self.forms.forms.get(name) else {
            return;
        };
        let draft = self
            .state
            .drafts
            .remove(&draft_key(name, event.row.as_deref()))
            .unwrap_or_default();
        let data = wired
            .data
            .as_deref()
            .and_then(|path| self.data(path, event));
        let mut form: Map<String, Value> = wired
            .fields
            .iter()
            .map(|field| {
                let value = draft
                    .get(field)
                    .or_else(|| data.as_ref()?.get(field))
                    .cloned()
                    .unwrap_or(Value::Null);
                (field.clone(), value)
            })
            .collect();
        // An event pushed in with a `form` of its own keeps what it says.
        if let Some(Value::Object(given)) = event.payload.get("form") {
            form.extend(given.clone());
        }
        if !event.payload.is_object() {
            event.payload = json!({});
        }
        event.payload["form"] = Value::Object(form);
    }

    /// A form's `data`, read for one event: `item` and `item.*` from the element it fired for,
    /// inside a component `props.*` and `local.*` from the instance it fired in, anything else
    /// from state.
    fn data(&self, path: &str, event: &Queued) -> Option<Value> {
        let mut keys = path.split('.');
        let within = event.within.as_ref();
        let root = match keys.next() {
            Some("item") => event.payload.get("item")?.clone(),
            Some("props") => within?.props.clone(),
            Some("local") => {
                let within = within?;
                let name = keys.next()?;
                self.local(&within.name, within.row.as_deref(), name)?
            }
            _ => return self.look(path, None),
        };
        keys.try_fold(&root, |at, key| match at {
            Value::Array(items) => items.get(key.parse::<usize>().ok()?),
            _ => at.get(key),
        })
        .cloned()
    }

    /// An event's payload as the trail shows it: with what a `password` input holds hidden.
    fn trail_payload(&self, event: &Queued) -> Value {
        let mut payload = event.payload.clone();
        let hidden = json!("(hidden)");
        if let Some((form, field)) = self.forms.fields.get(&event.name) {
            if self.forms.forms[form].hidden.contains(field)
                && let Some(value) = payload.get_mut("value")
            {
                *value = hidden;
            }
        } else if let Some(wired) = event
            .name
            .strip_suffix(".submit")
            .and_then(|name| self.forms.forms.get(name))
            && let Some(Value::Object(form)) = payload.get_mut("form")
        {
            for field in &wired.hidden {
                if let Some(value) = form.get_mut(field) {
                    *value = hidden.clone();
                }
            }
        }
        payload
    }

    /// Looks a dotted path up: in the event being handled (`event.*`), the app's variables
    /// (`app.*`), or Genesis's own (`sys.*`).
    fn look(&self, path: &str, event: Option<&Queued>) -> Option<Value> {
        let mut keys = path.split('.');
        let root = match (keys.next()?, keys.next()) {
            ("event", Some("name")) => return event.map(|e| json!(e.name)),
            ("event", Some(key)) => event?.payload.get(key)?.clone(),
            ("app", Some(key)) => self.state.app.get(key)?.clone(),
            ("local", Some(key)) => {
                let scope = event?.scope.as_ref()?;
                self.state
                    .locals
                    .get(&scope.key)
                    .and_then(|locals| locals.get(key))
                    .or_else(|| self.app.components.get(&scope.component)?.state.get(key))?
                    .clone()
            }
            ("props", None) => event?.scope.as_ref()?.props.clone(),
            ("props", Some(key)) => event?.scope.as_ref()?.props.get(key)?.clone(),
            ("sys", Some("view")) => json!(self.state.view),
            ("sys", Some("theme")) => json!(self.state.theme),
            ("sys", Some("events")) => json!(self.state.events),
            ("sys", Some("errors")) => json!(self.state.errors),
            ("sys", Some("sources")) => json!(self.state.sources),
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

/// Every handler an app has (nodes' `on_*` shortcuts first, in tree order, then `[on]`), and every
/// event name it can fire.
fn wiring(app: &App) -> Wiring {
    let mut handlers = Vec::new();
    let mut owners = Vec::new();
    let mut instances = BTreeMap::new();
    let mut node_events = Vec::new();
    let mut forms = Forms::default();
    for (i, node) in app.body.iter().enumerate() {
        let mut wire = Wire {
            handlers: &mut handlers,
            owners: &mut owners,
            instances: &mut instances,
            components: &app.components,
            events: &mut node_events,
            forms: &mut forms,
        };
        wire_node(node, &body_place(i), None, None, &mut wire);
    }
    handlers.extend(app.on.iter().cloned());
    owners.resize(handlers.len(), None);
    let mut sources = vec![None; handlers.len()];
    for (name, source) in &app.sources {
        for pattern in source_patterns(name, source) {
            let into = BTreeMap::from([(format!("app.{name}"), source.select.clone())]);
            let load = Step::Call {
                call: source.call.clone(),
                params: source.params.clone(),
                into,
                event: None,
            };
            handlers.push(Handler {
                pattern,
                steps: vec![load],
            });
            sources.push(Some(name.clone()));
            owners.push(None);
        }
    }
    let fires = fires(app, &handlers, node_events);
    Wiring {
        handlers,
        sources,
        owners,
        instances,
        fires,
        forms,
    }
}

/// An app's handlers, which source (if any) each belongs to, every event it can fire, and its forms.
struct Wiring {
    handlers: Vec<Handler>,
    sources: Vec<Option<String>>,
    /// Which component each handler belongs to, if any.
    owners: Vec<Option<Owner>>,
    /// Every `use` node's name, and its component's.
    instances: BTreeMap<String, String>,
    fires: Vec<String>,
    forms: Forms,
}

/// An app's forms, and the named controls that fill them.
#[derive(Default)]
struct Forms {
    /// Each form, by its name.
    forms: BTreeMap<String, FormWiring>,
    /// Each named control's `.change` event: the form and the field it fills.
    fields: BTreeMap<String, (String, String)>,
}

/// One form: what it starts from, and its fields.
#[derive(Default)]
struct FormWiring {
    /// Its `data`: the variable its controls start from.
    data: Option<String>,
    /// Its controls' names, once each, in tree order: what goes out on `.submit`.
    fields: Vec<String>,
    /// The names of its `password` inputs, whose values are never kept, mirrored, or on the trail.
    hidden: BTreeSet<String>,
}

/// What [`wire_node`] adds to as it walks the tree.
struct Wire<'a> {
    handlers: &'a mut Vec<Handler>,
    /// Which component each handler belongs to, in step with `handlers`.
    owners: &'a mut Vec<Option<Owner>>,
    instances: &'a mut BTreeMap<String, String>,
    components: &'a BTreeMap<String, Component>,
    events: &'a mut Vec<String>,
    forms: &'a mut Forms,
}

impl Wire<'_> {
    /// Adds a handler, belonging to the component `owner` if it's inside one.
    fn handle(&mut self, handler: Handler, owner: Option<&Owner>) {
        self.handlers.push(handler);
        self.owners.push(owner.cloned());
    }
}

/// The events a source calls on: `app.opened` (or its `load_on`), the `.changed` of every
/// variable in its params, and `app.NAME.reload`.
fn source_patterns(name: &str, source: &Source) -> Vec<String> {
    let mut patterns = source
        .load_on
        .clone()
        .unwrap_or_else(|| vec!["app.opened".into()]);
    let texts = source.params.values().filter_map(Value::as_str);
    for path in texts.flat_map(template::paths) {
        let mut keys = path.split('.');
        if let (Some(root @ ("app" | "sys")), Some(key)) = (keys.next(), keys.next()) {
            patterns.push(format!("{root}.{key}.changed"));
        }
    }
    patterns.push(format!("app.{name}.reload"));
    patterns.dedup();
    patterns
}

/// Where a form's draft is kept: under its name, or, for a form drawn in a row of an `each`,
/// its name and its row (`edit#2`).
fn draft_key(form: &str, row: Option<&str>) -> String {
    match row {
        Some(row) => format!("{form}#{row}"),
        None => form.to_string(),
    }
}

/// The form a draft's key names.
fn form_of(key: &str) -> &str {
    key.split_once('#').map_or(key, |(form, _)| form)
}

/// An app's variables at their starting values: `[state]`, and each source, empty.
fn starting(app: &App) -> Map<String, Value> {
    let mut vars = app.state.clone();
    vars.extend(app.sources.keys().map(|name| (name.clone(), Value::Null)));
    vars
}

/// Turns a node's `on_*` shortcuts into handlers, and notes the events it fires and the forms it
/// holds, for it and its children. A node without an `id` is named by its place in the tree
/// (`body.0.children.1`). `form` is the name of the form it's inside, if any, and `owner` the
/// component: inside one, a node is named under its `use` node (`card.more_btn`).
fn wire_node(node: &Node, place: &str, form: Option<&str>, owner: Option<&Owner>, wire: &mut Wire) {
    let name = match owner {
        Some(owner) => format!("{}.{}", owner.name, node.name(place)),
        None => node.name(place),
    };
    match node {
        Node::Button(button) => {
            if button.id.is_some() || button.on_click.is_some() {
                wire.events.push(format!("{name}.click"));
            }
            if let Some(steps) = &button.on_click {
                wire.handle(
                    Handler {
                        pattern: format!("{name}.click"),
                        steps: steps.clone(),
                    },
                    owner,
                );
            }
        }
        Node::Form(f) => {
            wire.events.push(format!("{name}.submit"));
            if let Some(steps) = &f.on_submit {
                wire.handle(
                    Handler {
                        pattern: format!("{name}.submit"),
                        steps: steps.clone(),
                    },
                    owner,
                );
            }
            wire.forms.forms.insert(
                name.clone(),
                FormWiring {
                    data: f.data.clone(),
                    ..FormWiring::default()
                },
            );
        }
        Node::Dialog(dialog) => {
            // The person closed it: its variable says so the one way anything does, a `set`.
            wire.events.push(format!("{name}.close"));
            let mut steps = vec![Step::Set {
                set: dialog.open.clone(),
                to: json!(false),
            }];
            steps.extend(dialog.on_close.iter().flatten().cloned());
            wire.handle(
                Handler {
                    pattern: format!("{name}.close"),
                    steps,
                },
                owner,
            );
        }
        Node::Audio(Audio { id: Some(_), .. }) | Node::Video(Video { id: Some(_), .. }) => {
            wire.events
                .extend(["play", "pause", "ended"].map(|e| format!("{name}.{e}")));
        }
        Node::Input(_)
        | Node::Textarea(_)
        | Node::Select(_)
        | Node::Checkbox(_)
        | Node::Toggle(_)
        | Node::Radio(_)
        | Node::File(_) => {
            wire.events.push(format!("{name}.change"));
            if let (Some(field), Some(form)) = (node.field(), form) {
                // A named control fills its form's draft; nothing changes state until `.submit`.
                let wired = wire.forms.forms.entry(form.to_string()).or_default();
                if !wired.fields.iter().any(|f| f == field) {
                    wired.fields.push(field.to_string());
                }
                if matches!(node, Node::Input(i) if i.kind == InputKind::Password) {
                    wired.hidden.insert(field.to_string());
                }
                wire.forms.fields.insert(
                    format!("{name}.change"),
                    (form.to_string(), field.to_string()),
                );
            } else if let Some(var) = node.var() {
                // A control writes its variable the one way anything does: a `set`, run by its `.change`.
                wire.handle(
                    Handler {
                        pattern: format!("{name}.change"),
                        steps: vec![Step::Set {
                            set: var.to_string(),
                            to: json!("{{ event.value }}"),
                        }],
                    },
                    owner,
                );
            }
        }
        Node::Cell(_)
        | Node::Text(_)
        | Node::Image(_)
        | Node::Link(_)
        | Node::Svg(_)
        | Node::Divider(_)
        | Node::Audio(_)
        | Node::Video(_)
        | Node::Iframe(_)
        | Node::Markdown(_) => {}
        Node::Use(used) => {
            // A loop of components is refused on load; one handed over another way stops here.
            if let Some(component) = wire.components.get(&used.component)
                && name.matches('.').count() < MAX_NESTING
            {
                wire.instances.insert(name.clone(), used.component.clone());
                let inner = Owner {
                    name: name.clone(),
                    component: used.component.clone(),
                };
                for (i, child) in component.body.iter().enumerate() {
                    wire_node(child, &body_place(i), form, Some(&inner), wire);
                }
            }
        }
    }
    let form = if matches!(node, Node::Form(_)) {
        Some(name.as_str())
    } else {
        form
    };
    for (i, child) in node.children().iter().enumerate() {
        wire_node(child, &child_place(place, i), form, owner, wire);
    }
}

/// How many dots deep a name inside nested components may go before wiring stops: far past any
/// real screen, short of a stack overflow from a loop that wasn't checked on load.
const MAX_NESTING: usize = 64;

/// Every event name the config can fire, without repeats, for `sys.bus`.
fn fires(app: &App, handlers: &[Handler], node_events: Vec<String>) -> Vec<String> {
    let mut names = vec!["app.opened".to_string(), "config.loaded".to_string()];
    names.extend(node_events);
    names.extend(app.state.keys().map(|k| format!("app.{k}.changed")));
    names.extend([
        "sys.view.changed".to_string(),
        "sys.theme.changed".to_string(),
    ]);
    for name in app.sources.keys() {
        names.extend([format!("app.{name}.changed"), format!("app.{name}.reload")]);
    }
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

/// A JSON value's kind, in words: `"number"`, `"text"`, and so on.
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "true or false",
        Value::Number(_) => "number",
        Value::String(_) => "text",
        Value::Array(_) => "list",
        Value::Object(_) => "table",
    }
}

/// A number as JSON, whole numbers as integers, so `step` on `1` gives `2`, not `2.0`.
fn number(n: f64) -> Value {
    // A whole number that survives the trip to an integer and back is written as one.
    let whole = n as i64;
    if whole as f64 == n {
        json!(whole)
    } else {
        json!(n)
    }
}

/// A view or theme name: a string as it is, anything else as JSON text.
fn text_of(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), String::from)
}
