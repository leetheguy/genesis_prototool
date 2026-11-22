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

use crate::config::{App, Handler, Node, Step, body_place, child_place};
use crate::{pattern, template};
use serde_json::{Map, Value, json};
use std::collections::VecDeque;

/// Something the machine needs its host to do, since the machine itself does no IO.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Show a short message. Every failure nobody handles becomes one.
    Toast {
        /// The message.
        text: String,
    },
}

/// A list of steps being run, which can wait on a timer and carry on later.
struct Run {
    /// The lists it's inside, innermost last: a handler's list, then each custom action it ran.
    frames: Vec<Frame>,
    /// The event that started it, which its templates read as `event.*`.
    event: Queued,
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
    Resume(Run),
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
    /// Every event name the config can fire, for `sys.bus`.
    fires: Vec<String>,
    state: State,
    queue: VecDeque<Queued>,
    /// What the host must do, gathered while the queue runs.
    effects: Vec<Effect>,
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
}

/// An event waiting its turn.
#[derive(Clone)]
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
            wire_node(node, &body_place(i), &mut handlers, &mut node_events);
        }
        handlers.extend(app.on.iter().cloned());
        let fires = fires(&app, &handlers, node_events);
        let state = State {
            app: app.state.clone(),
            view: app.app.start.clone(),
            theme: app.app.theme.clone(),
            events: VecDeque::new(),
            errors: VecDeque::new(),
        };
        Machine {
            app,
            handlers,
            fires,
            state,
            queue: VecDeque::new(),
            effects: Vec::new(),
            timers: Vec::new(),
            tapped: None,
            now: 0,
        }
    }

    /// Fires `app.opened`, and handles everything that follows from it.
    pub fn open(&mut self) -> Vec<Effect> {
        self.fire("app.opened", json!({}))
    }

    /// Fires the event `name` with its `payload`, and handles it and every event that follows,
    /// until the queue is empty. Returns what the host must do as a result.
    pub fn fire(&mut self, name: &str, payload: Value) -> Vec<Effect> {
        self.queue.push_back(Queued {
            name: name.to_string(),
            payload,
            chain: Vec::new(),
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
                    self.execute(run);
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

    /// The state as one value: `{ "app": {...}, "sys": { "view", "theme", "errors" } }`.
    pub fn snapshot(&self) -> Value {
        json!({
            "app": self.state.app,
            "sys": { "view": self.state.view, "theme": self.state.theme, "errors": self.state.errors },
        })
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
            self.execute(Run {
                frames: vec![frame],
                event: event.clone(),
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
                    self.set_timer(self.now + delay, id.clone(), TimerKind::Resume(run));
                    return;
                }
                Step::Every {
                    every,
                    fire,
                    id,
                    with,
                } => self.every(*every, fire, id, with, &run.event),
                Step::Cancel { cancel } => {
                    self.timers
                        .retain(|t| t.id.as_deref() != Some(cancel.as_str()));
                    Ok(())
                }
                other => self.run_step(other, &run.event),
            };
            if let Err(message) = result {
                return self.unwind(run, &step, message);
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
    /// with the error, innermost first; so does an action that couldn't start. A failure no
    /// action reported is logged under the event's name, and toasted.
    fn unwind(&mut self, run: Run, step: &Step, message: String) {
        let starting = match step {
            Step::Run(name) => Some(name.clone()),
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
        if actions.is_empty() {
            self.fail(json!({ "name": run.event.name, "message": message }), true);
        }
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
                let now = self.writable(step)?;
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
                    .writable(toggle)?
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
    /// `sys.theme`. Anything else is an error saying why.
    fn writable(&self, var: &str) -> Result<Value, String> {
        match var.split_once('.') {
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
        let was = self.writable(var)?;
        if was == value {
            return Ok(());
        }
        match var.split_once('.') {
            Some(("app", key)) => {
                self.state.app.insert(key.to_string(), value.clone());
            }
            Some((_, "view")) => self.state.view = text_of(&value),
            _ => self.state.theme = text_of(&value),
        }
        self.enqueue(
            &format!("{var}.changed"),
            json!({ "value": value, "was": was }),
            cause,
        );
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
            ("sys", Some("errors")) => json!(self.state.errors),
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
            let name = node.name(place);
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
                wire_node(child, &child_place(place, i), handlers, events);
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
