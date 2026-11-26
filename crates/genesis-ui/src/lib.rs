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

use genesis_core::bus::{Call, Effect, Machine, Reply, Within};
use genesis_core::condition::Condition;
use genesis_core::config::{App, Node, body_place, child_place};
use genesis_core::style::Placed;
use genesis_core::template;
use leptos::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use style::ClassList;

mod components;
mod controls;
pub mod files;
#[cfg(target_arch = "wasm32")]
mod intl;
pub mod style;

/// The element of an `each` a row is drawn for: it follows the list, so the row is kept and only
/// what reads a changed value is drawn again.
type Item = Signal<Value>;

/// Makes a call and answers it, as [`Host::set_caller`] takes it.
type Caller = Arc<dyn Fn(Call) + Send + Sync>;

/// One running app: its state machine, and the signals the screen redraws from.
///
/// It's cheap to copy: every copy shares the same machine.
#[derive(Clone)]
pub struct Host {
    machine: Arc<Mutex<Machine>>,
    /// Notified when the machine's state may have changed ([`Machine::revision`]), so whatever
    /// read state works itself out again. Nothing redraws on a clock tick that changed nothing.
    changes: RwSignal<()>,
    /// The machine's revision the screen last caught up with.
    seen: Arc<AtomicU64>,
    /// How many times the screen was told to catch up ([`Host::redraws`]).
    redraws: Arc<AtomicU64>,
    /// Notified about once a second ([`Host::tick`]), for text that says how long ago a time was.
    clock: RwSignal<()>,
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
            seen: Arc::new(AtomicU64::new(0)),
            redraws: Arc::new(AtomicU64::new(0)),
            clock: RwSignal::new(()),
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
        self.caught_up();
        notes
    }

    /// Fires `app.opened`.
    pub fn open(&self) {
        let effects = self.machine().open();
        self.after(effects);
    }

    /// Fires an event, as a node does when something happens to it.
    pub fn fire(&self, name: &str, payload: Value) {
        self.fire_at(name, payload, None);
    }

    /// Fires an event from a node drawn in `row` of an `each` ([`Machine::fire_at`]).
    pub fn fire_at(&self, name: &str, payload: Value, row: Option<&str>) {
        self.fire_within(name, payload, row, None);
    }

    /// Fires an event from a node drawn inside a component ([`Machine::fire_within`]).
    pub fn fire_within(
        &self,
        name: &str,
        payload: Value,
        row: Option<&str>,
        within: Option<Within>,
    ) {
        let effects = self.machine().fire_within(name, payload, row, within);
        self.after(effects);
    }

    /// A component instance's `local.NAME` ([`Machine::local`]). Reading it inside a view makes
    /// that part of the view redraw when state changes.
    pub fn local(&self, instance: &str, row: Option<&str>, name: &str) -> Option<Value> {
        self.changes.track();
        self.machine().local(instance, row, name)
    }

    /// Moves the machine's clock on by `ms` milliseconds, running timers that fall due.
    pub fn advance(&self, ms: u64) {
        let effects = self.machine().advance(ms);
        self.after(effects);
    }

    /// What's typed into `form`'s control named `field`, not yet submitted ([`Machine::draft`]).
    /// Reading it inside a view makes that part of the view redraw when state changes.
    pub fn draft(&self, form: &str, field: &str) -> Option<Value> {
        self.draft_at(form, None, field)
    }

    /// Whether `form` (in `row` of an `each`) was last submitted with a check failing.
    pub fn tried(&self, form: &str, row: Option<&str>) -> bool {
        self.changes.track();
        self.machine().tried(form, row)
    }

    /// The same as [`Host::draft`], for a form drawn in `row` of an `each`.
    pub fn draft_at(&self, form: &str, row: Option<&str>, field: &str) -> Option<Value> {
        self.changes.track();
        self.machine().draft_at(form, row, field)
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

    /// Answers the question `id` ([`Machine::answer`]): yes carries its list on, no stops it.
    pub fn answer(&self, id: u64, yes: bool) {
        let effects = self.machine().answer(id, yes);
        self.after(effects);
    }

    /// Takes the toast `id` off the screen ([`Machine::dismiss`]).
    pub fn dismiss(&self, id: u64) {
        self.machine().dismiss(id);
        self.caught_up();
    }

    /// Tells text that says how long ago a time was (`ago`) to work itself out again. The browser
    /// calls it about once a second; nothing else redraws for it.
    pub fn tick(&self) {
        self.clock.notify();
    }

    /// How many times the screen has been told to catch up with the machine: once for each
    /// change, never for a clock tick that changed nothing.
    pub fn redraws(&self) -> u64 {
        self.redraws.load(Ordering::Relaxed)
    }

    /// Tells whatever read state to work itself out again, when the machine's state may have
    /// changed since the last time.
    fn caught_up(&self) {
        let revision = self.machine().revision();
        if self.seen.swap(revision, Ordering::Relaxed) != revision {
            self.redraws.fetch_add(1, Ordering::Relaxed);
            self.changes.notify();
        }
    }

    fn after(&self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                // The screen draws `sys.toasts` and `sys.asking`, so these need nothing more.
                Effect::Toast { .. } | Effect::Confirm { .. } => {}
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
        self.caught_up();
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
    /// Inside an `each`, the element it's drawn for: `item.*`. A signal, so a row stays on screen
    /// while its element changes, and only what reads the change is drawn again.
    item: Option<Item>,
    /// Inside an `each`, which row it's drawn in: `"2"`, or `"0.2"` inside two.
    row: Option<String>,
    /// Inside a `form`, that form.
    form: Option<FormAt>,
    /// Inside a component, where it's used: its props, and its name for the nodes inside.
    scope: Option<Arc<Scope>>,
}

/// Where a component is drawn: the `use` node's name, which the names of the nodes inside it go
/// under, and its props as written, filled from where the `use` node sits.
pub(crate) struct Scope {
    /// The `use` node's name.
    name: String,
    /// The row of an `each` the `use` node is drawn in: with its name, which instance it is.
    row: Option<String>,
    /// Its props, as written: templates.
    props: serde_json::Map<String, Value>,
    /// What the props are filled from: the element the `use` node is drawn for, if any...
    item: Option<Item>,
    /// ...and the component it's inside, if any.
    parent: Option<Arc<Scope>>,
}

/// The form a node is inside: its name, the row it's drawn in, and the variable its controls
/// start from.
#[derive(Clone)]
pub(crate) struct FormAt {
    name: String,
    row: Option<String>,
    data: Option<String>,
}

/// What's wrong with a control's value, while it's worth saying ([`Wired::complaint`]).
#[derive(Clone)]
pub struct Complaint {
    touched: RwSignal<bool>,
    message: Arc<dyn Fn() -> Option<String> + Send + Sync>,
}

impl Complaint {
    /// The person changed the control, so a failing check now says so.
    pub fn touch(&self) {
        self.touched.set(true);
    }

    /// The message to show under the control, if any.
    pub fn message(&self) -> Option<String> {
        (self.message)()
    }
}

impl Wired {
    /// Fires `NAME.event` with `payload`; inside an `each`, the payload carries the element as
    /// `item`. Inside a component it's the element of the nearest `each`, the component's own or
    /// the one its `use` node is in, so instances tell themselves apart.
    pub fn fire(&self, event: &str, mut payload: Value) {
        let mut scopes = std::iter::successors(self.scope.as_ref(), |s| s.parent.as_ref());
        let item = self
            .item
            .as_ref()
            .or_else(|| scopes.find_map(|s| s.item.as_ref()));
        if let (Some(item), Value::Object(fields)) = (item, &mut payload) {
            fields.insert("item".into(), item.get_untracked());
        }
        // A control in a form fires from its form's row, which is the draft it fills.
        let row = match &self.form {
            Some(form) => form.row.as_deref(),
            None => self.row.as_deref(),
        };
        let within = self.scope.as_ref().map(|scope| within(&self.host, scope));
        self.host
            .fire_within(&format!("{}.{event}", self.name), payload, row, within);
    }

    /// Display text with its templates filled from the current state (and `item.*` inside an
    /// `each`); redraws as state changes.
    pub fn text(&self, text: &str) -> impl Fn() -> String + Clone + Send + Sync + 'static {
        let (host, item, scope) = (self.host.clone(), self.item, self.scope.clone());
        let text = text.to_string();
        move || {
            template::text(&text, |path| {
                look(&host, item.as_ref(), scope.as_ref(), path)
            })
        }
    }

    /// The same as [`Wired::text`], with each value its templates fill in shown in `format`. A
    /// value the format can't take is shown as it is, and the browser console says why.
    pub fn formatted(
        &self,
        text: &str,
        format: Option<&genesis_core::format::Format>,
    ) -> impl Fn() -> String + Clone + Send + Sync + 'static + use<> {
        let (host, item, scope) = (self.host.clone(), self.item, self.scope.clone());
        let (text, format) = (text.to_string(), format.cloned());
        move || {
            let look = |path: &str| look(&host, item.as_ref(), scope.as_ref(), path);
            let Some(format) = &format else {
                return template::text(&text, look);
            };
            // How long ago a time was changes as time goes by, with no change in state.
            if *format == genesis_core::format::Format::Ago {
                host.clock.track();
            }
            let locale = locale(&host);
            template::text_with(&text, look, |value| {
                format.apply(value, locale.as_ref()).unwrap_or_else(|why| {
                    leptos::logging::warn!("format: {why}; shown as it is");
                    template::as_text(value)
                })
            })
        }
    }

    /// The current value at `path` (and `item.*` inside an `each`); redraws as state changes.
    pub fn read(
        &self,
        path: &str,
    ) -> impl Fn() -> Option<Value> + Clone + Send + Sync + 'static + use<> {
        let (host, item, scope) = (self.host.clone(), self.item, self.scope.clone());
        let path = path.to_string();
        move || look(&host, item.as_ref(), scope.as_ref(), &path)
    }

    /// Whether `condition` holds right now, redrawing as state changes; `false` with none.
    /// `form.*` reads the draft of the form it's in, as its controls show it.
    pub fn holds(
        &self,
        condition: Option<&Condition>,
    ) -> impl Fn() -> bool + Clone + Send + Sync + 'static + use<> {
        let (host, item, scope) = (self.host.clone(), self.item, self.scope.clone());
        let (form, condition) = (self.form.clone(), condition.cloned());
        move || {
            let Some(condition) = &condition else {
                return false;
            };
            condition.holds(&|path: &str| match path.strip_prefix("form.") {
                Some(rest) => {
                    let form = form.as_ref()?;
                    let (name, rest) = match rest.split_once('.') {
                        Some((name, rest)) => (name, Some(rest)),
                        None => (rest, None),
                    };
                    let value = host
                        .draft_at(&form.name, form.row.as_deref(), name)
                        .or_else(|| {
                            let data = form.data.as_ref()?;
                            look(
                                &host,
                                item.as_ref(),
                                scope.as_ref(),
                                &format!("{data}.{name}"),
                            )
                        })?;
                    dig(&value, rest)
                }
                None => look(&host, item.as_ref(), scope.as_ref(), path),
            })
        }
    }

    /// What's wrong with a control's value, while it's worth saying: its first failing check's
    /// message, once the person has changed it or its form's last submit failed a check. `None`
    /// for a control with no checks. Call [`Complaint::touch`] when the person changes it.
    pub fn complaint(
        &self,
        checks: Vec<genesis_core::condition::Check>,
        var: Option<&str>,
        name: Option<&str>,
    ) -> Option<Complaint> {
        if checks.is_empty() {
            return None;
        }
        let value = self.bound(var, name);
        let touched = RwSignal::new(false);
        // A named control speaks once something's typed into its form's draft for it, or its form
        // was submitted with a check failing; after a good submit the draft goes and it's quiet again.
        let named = match (name, &self.form) {
            (Some(name), Some(form)) => Some((self.host.clone(), form.clone(), name.to_string())),
            _ => None,
        };
        let speaks = move || match &named {
            Some((host, form, name)) => {
                host.tried(&form.name, form.row.as_deref())
                    || host
                        .draft_at(&form.name, form.row.as_deref(), name)
                        .is_some()
            }
            None => touched.get(),
        };
        let message = move || {
            if !speaks() {
                return None;
            }
            let value = value();
            checks
                .iter()
                .find(|check| !check.passes(value.as_ref()))
                .map(|check| check.message.clone())
        };
        Some(Complaint {
            touched,
            message: Arc::new(message),
        })
    }

    /// A control's current value: its `var`'s, or, with `name` inside a form, what's typed into
    /// it, else its form's `data` at its name. Redraws as state changes.
    pub fn bound(
        &self,
        var: Option<&str>,
        name: Option<&str>,
    ) -> impl Fn() -> Option<Value> + Clone + Send + Sync + 'static + use<> {
        let read = var.map(|var| self.read(var));
        let named = match (name, &self.form) {
            (Some(name), Some(form)) => Some((
                self.host.clone(),
                (form.name.clone(), form.row.clone()),
                name.to_string(),
                form.data
                    .as_ref()
                    .map(|data| self.read(&format!("{data}.{name}"))),
            )),
            _ => None,
        };
        move || match (&read, &named) {
            (Some(read), _) => read(),
            (None, Some((host, (form, row), name, data))) => host
                .draft_at(form, row.as_deref(), name)
                .or_else(|| data.as_ref().and_then(|data| data())),
            (None, None) => None,
        }
    }

    /// The name a radio's group goes by in the browser: its `var`, or its form's name and its own.
    pub(crate) fn group(&self, var: Option<&str>, name: Option<&str>) -> String {
        match (var, name, &self.form) {
            (Some(var), _, _) => var.to_string(),
            (None, Some(name), Some(form)) => match &form.row {
                Some(row) => format!("{}#{row}.{name}", form.name),
                None => format!("{}.{name}", form.name),
            },
            _ => String::new(),
        }
    }

    /// The same wiring, for the children of a form whose controls start from `data`.
    pub(crate) fn in_form(&self, data: Option<String>) -> Wired {
        Wired {
            form: Some(FormAt {
                name: self.name.clone(),
                row: self.row.clone(),
                data,
            }),
            ..self.clone()
        }
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
            self.item,
            self.row.clone(),
            self.form.clone(),
            self.scope.clone(),
        )
    }

    /// Draws `component`'s body where this `use` node is, with `props` filled from here. Inside,
    /// `props.*` reads them, `item` starts over, and names go under this node's.
    pub(crate) fn component(
        &self,
        component: &genesis_core::config::Component,
        props: &serde_json::Map<String, Value>,
    ) -> Vec<AnyView> {
        let scope = Arc::new(Scope {
            name: self.name.clone(),
            row: self.row.clone(),
            props: props.clone(),
            item: self.item,
            parent: self.scope.clone(),
        });
        component
            .body
            .iter()
            .enumerate()
            .map(|(i, node)| {
                draw(
                    node,
                    body_place(i),
                    &self.host,
                    None,
                    self.row.clone(),
                    self.form.clone(),
                    Some(scope.clone()),
                )
            })
            .collect()
    }

    /// The app's component named `name`.
    pub(crate) fn find_component(&self, name: &str) -> Option<genesis_core::config::Component> {
        self.host.machine().app().components.get(name).cloned()
    }

    /// Whether the node has any placement keys.
    pub(crate) fn is_placed(&self) -> bool {
        self.placed != Placed::default()
    }

    /// Draws `children` once per element of the list at `path`, each seeing its element as
    /// `item.*`, or the face for where the list stands: loading or failed (when its variable is a
    /// source), empty, or not a list at all. The faces look the same everywhere. With `key`, a row
    /// is kept by its element's key; without, by its place in the list.
    pub(crate) fn each(
        &self,
        path: &str,
        empty: Option<&str>,
        error: Option<&str>,
        key: Option<&str>,
        children: &[Node],
    ) -> impl IntoView + use<> {
        let (host, item, place) = (self.host.clone(), self.item, self.place.clone());
        let (form, row, scope) = (self.form.clone(), self.row.clone(), self.scope.clone());
        let (path, children) = (path.to_string(), children.to_vec());
        let empty = empty.unwrap_or("Nothing here.").to_string();
        let error = error.map(String::from);
        // Worked out again on each change, but each memo passes a change on only when its value
        // changes, so the rows are drawn once and kept: a changed element changes only what reads it.
        let looked = {
            let (host, scope, path) = (host.clone(), scope.clone(), path.clone());
            Memo::new(move |_| look(&host, item.as_ref(), scope.as_ref(), &path))
        };
        let elements = Memo::new(move |_| match looked.get() {
            Some(Value::Array(elements)) => elements,
            _ => Vec::new(),
        });
        let count = Memo::new(move |_| elements.with(Vec::len));
        // What each row is kept by, in list order: its key's text, or its place.
        let rows = {
            let key = key.map(String::from);
            Memo::new(move |_| elements.with(|elements| row_keys(elements, key.as_deref())))
        };
        // Where each row's element is in the list now.
        let at = Memo::new(move |_| {
            rows.with(|rows| {
                rows.as_ref()
                    .map(|rows| {
                        rows.iter()
                            .enumerate()
                            .map(|(n, row)| (row.clone(), n))
                            .collect::<HashMap<String, usize>>()
                    })
                    .unwrap_or_default()
            })
        });
        let status = {
            let (host, path) = (host.clone(), path.clone());
            Memo::new(move |_| source_status(&host, &path))
        };
        let face = Memo::new(move |_| match status.get() {
            Some(Status::Failed(detail)) => Face::Failed(detail),
            // A list that's loading again keeps the rows it has until the new ones come.
            Some(Status::Loading) if count.get() == 0 => Face::Loading,
            _ => looked.with(|looked| match looked {
                None | Some(Value::Null) => Face::Empty,
                Some(Value::Array(elements)) if elements.is_empty() => Face::Empty,
                Some(Value::Array(_)) => match rows.with(|rows| rows.as_ref().err().cloned()) {
                    Some(why) => Face::BadKey(why),
                    None => Face::Rows,
                },
                Some(other) => Face::NotList(kind(other)),
            }),
        });
        move || {
            let face = face.get();
            untrack(|| match face {
                Face::Loading => loading_face(),
                Face::Failed(detail) => failed_face(error.as_deref(), &detail),
                Face::Empty => empty_face(&empty),
                Face::NotList(kind) => {
                    failed_face(None, &format!("each needs a list, and {path} is {kind}"))
                }
                Face::BadKey(why) => failed_face(None, &why),
                Face::Rows => {
                    let (host, place, row) = (host.clone(), place.clone(), row.clone());
                    let (form, scope, children) = (form.clone(), scope.clone(), children.clone());
                    // A row is never swapped out: when the list changes, each row's element
                    // follows it by its key (or its place), new rows are drawn where they come,
                    // and a row whose key is gone is taken away with its elements.
                    let draw_row = move |kept: String| {
                        let element: Item = {
                            let kept = kept.clone();
                            Memo::new(move |_| {
                                let n = at.with(|at| at.get(&kept).copied());
                                n.and_then(|n| elements.with(|e| e.get(n).cloned()))
                                    .unwrap_or(Value::Null)
                            })
                            .into()
                        };
                        let row = Some(match &row {
                            Some(outer) => format!("{outer}.{kept}"),
                            None => kept,
                        });
                        children
                            .iter()
                            .enumerate()
                            .map(|(i, child)| {
                                draw(
                                    child,
                                    child_place(&place, i),
                                    &host,
                                    Some(element),
                                    row.clone(),
                                    form.clone(),
                                    scope.clone(),
                                )
                            })
                            .collect_view()
                    };
                    let each = move || rows.get().unwrap_or_default();
                    view! { <For each=each key=|kept: &String| kept.clone() children=draw_row /> }
                        .into_any()
                }
            })
        }
    }
}

/// Looks `path` up: `item` and `item.*` in the element being drawn, `props.*` and `local.*` in
/// the component it's inside, anything else in state. `app.*` is there only for what `app.toml`
/// draws: its own body, and a component it draws (by a `use` node in its body, or in its outlet).
/// A component drawn inside another reads none of it; it gets what it needs as props.
fn look(host: &Host, item: Option<&Item>, scope: Option<&Arc<Scope>>, path: &str) -> Option<Value> {
    let (head, rest) = match path.split_once('.') {
        Some((head, rest)) => (head, Some(rest)),
        None => (path, None),
    };
    let base = match (head, item, scope) {
        ("item", Some(item), _) => item.get(),
        ("app", _, Some(scope)) if scope.parent.is_some() => return None,
        ("local", _, Some(scope)) => {
            let (key, rest) = match rest?.split_once('.') {
                Some((key, rest)) => (key, Some(rest)),
                None => (rest?, None),
            };
            return dig(&host.local(&scope.name, scope.row.as_deref(), key)?, rest);
        }
        ("props", _, Some(scope)) => match rest {
            None => Value::Object(
                scope
                    .props
                    .iter()
                    .map(|(key, prop)| (key.clone(), fill(host, scope, prop)))
                    .collect(),
            ),
            Some(rest) => {
                let (key, rest) = match rest.split_once('.') {
                    Some((key, rest)) => (key, Some(rest)),
                    None => (rest, None),
                };
                let prop = fill(host, scope, scope.props.get(key)?);
                return dig(&prop, rest);
            }
        },
        _ => return host.get(path),
    };
    dig(&base, rest)
}

/// The value at `path` (dotted keys, and indexes into lists) inside `value`; all of it with none.
fn dig(value: &Value, path: Option<&str>) -> Option<Value> {
    let Some(path) = path else {
        return Some(value.clone());
    };
    path.split('.')
        .try_fold(value, |value, key| match value {
            Value::Array(list) => list.get(key.parse::<usize>().ok()?),
            _ => value.get(key),
        })
        .cloned()
}

/// Where an event inside a component fired, for the machine: the instance, its props filled, and
/// the instances it's inside.
fn within(host: &Host, scope: &Arc<Scope>) -> Within {
    Within {
        name: scope.name.clone(),
        row: scope.row.clone(),
        props: Value::Object(
            scope
                .props
                .iter()
                .map(|(key, prop)| (key.clone(), fill(host, scope, prop)))
                .collect(),
        ),
        outer: scope
            .parent
            .as_ref()
            .map(|parent| Box::new(within(host, parent))),
    }
}

/// A prop, filled from where its `use` node sits. A prop that's one template and names nothing
/// set is `null`; one with text around its templates shows what's missing as empty, as display
/// text does.
fn fill(host: &Host, scope: &Arc<Scope>, prop: &Value) -> Value {
    let look = |path: &str| look(host, scope.item.as_ref(), scope.parent.as_ref(), path);
    match prop {
        Value::String(text) => {
            template::value(prop, look).unwrap_or_else(|_| match template::paths(text).as_slice() {
                [_] if text.trim().starts_with("{{") && text.trim().ends_with("}}") => Value::Null,
                _ => Value::String(template::text(text, look)),
            })
        }
        Value::Array(items) => Value::Array(items.iter().map(|v| fill(host, scope, v)).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), fill(host, scope, v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Where a list that's a source (`app.NAME`) stands, while it's loading or has failed; `None`
/// otherwise.
fn source_status(host: &Host, path: &str) -> Option<Status> {
    let name = path.strip_prefix("app.").filter(|n| !n.contains('.'))?;
    let call = host.machine().app().sources.get(name)?.call.clone();
    match host.get(&format!("sys.sources.{name}"))?.as_str()? {
        "loading" => Some(Status::Loading),
        "failed" => {
            // The detail is the newest failure of its operation; `error` can't hide it.
            let failed = format!("{call}.failed");
            let message = host
                .get("sys.errors")
                .and_then(|errors| {
                    errors
                        .as_array()?
                        .iter()
                        .find(|e| e["name"] == failed.as_str())
                        .cloned()
                })
                .and_then(|e| e["message"].as_str().map(String::from));
            Some(Status::Failed(match message {
                Some(message) => format!("{call} failed: {message}"),
                None => format!("{call} failed"),
            }))
        }
        _ => None,
    }
}

/// Where a source a list shows stands, when it isn't simply there.
#[derive(Clone, PartialEq)]
enum Status {
    Loading,
    /// Its detail: the operation, and its newest failure.
    Failed(String),
}

/// What a cell with `each` shows: a face, or its rows.
#[derive(Clone, PartialEq)]
enum Face {
    Loading,
    Failed(String),
    Empty,
    /// Not a list: what it is.
    NotList(&'static str),
    /// A list whose rows' keys are missing or shared: why.
    BadKey(String),
    Rows,
}

/// What each element is kept by, in order: with `key` (`item.id`), the text of its key, which every
/// element needs and no two may share; without, its place in the list.
fn row_keys(elements: &[Value], key: Option<&str>) -> Result<Vec<String>, String> {
    let Some(key) = key else {
        return Ok((0..elements.len()).map(|n| n.to_string()).collect());
    };
    let rest = key
        .strip_prefix("item")
        .and_then(|rest| rest.strip_prefix('.'));
    let mut first = HashMap::new();
    let mut keys = Vec::with_capacity(elements.len());
    for (n, element) in elements.iter().enumerate() {
        let text = match dig(element, rest) {
            Some(Value::String(text)) => text,
            Some(Value::Number(number)) => number.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            None | Some(Value::Null) => {
                return Err(format!(
                    "each row needs a key at {key}, and row {} has none",
                    n + 1
                ));
            }
            Some(other) => {
                return Err(format!(
                    "a row's key at {key} is text or a number, and row {}'s is {}",
                    n + 1,
                    kind(&other)
                ));
            }
        };
        if let Some(earlier) = first.insert(text.clone(), n) {
            return Err(format!(
                "rows {} and {} share the key {text:?} at {key}; each row's key has to be its own",
                earlier + 1,
                n + 1
            ));
        }
        keys.push(text);
    }
    Ok(keys)
}

/// The face for a list that's loading.
fn loading_face() -> AnyView {
    view! { <span class="loading loading-spinner" role="status" aria-label="Loading"></span> }
        .into_any()
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

/// The language formats write in: the browser's, or `[app] locale`'s. Outside a browser (in tests),
/// US English in UTC.
fn locale(host: &Host) -> Box<dyn genesis_core::format::Locale> {
    #[cfg(target_arch = "wasm32")]
    {
        let tag = host.machine().app().app.locale.clone();
        Box::new(intl::Browser { tag })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = host;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        Box::new(genesis_core::format::English::at(now))
    }
}

/// Draws one node through its wiring; inside an `each`, `item` is the element it's drawn for and
/// `row` where, inside a `form`, `form` is that form, and inside a component, `scope` is where
/// it's used. A node inside a component is named under its `use` node: `card.more`.
fn draw(
    node: &Node,
    place: String,
    host: &Host,
    item: Option<Item>,
    row: Option<String>,
    form: Option<FormAt>,
    scope: Option<Arc<Scope>>,
) -> AnyView {
    let name = match &scope {
        Some(scope) => format!("{}.{}", scope.name, node.name(&place)),
        None => node.name(&place),
    };
    let wired = Wired {
        host: host.clone(),
        name,
        place,
        placed: node.placed(),
        item,
        row,
        form,
        scope,
    };
    let Some(show) = node.show() else {
        return view_of(node, wired);
    };
    // Drawn only while `show` holds. The memo redraws it only when that changes, and nothing read
    // while drawing it makes it draw again.
    let holds = wired.holds(Some(show));
    let shown = Memo::new(move |_| holds());
    let node = node.clone();
    (move || {
        shown
            .get()
            .then(|| untrack(|| view_of(&node, wired.clone())))
    })
    .into_any()
}

/// Draws `node` through `wired`, as its type does.
fn view_of(node: &Node, wired: Wired) -> AnyView {
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
        Node::Form(n) => n.view(wired),
        Node::File(n) => n.view(wired),
        Node::Dialog(n) => n.view(wired),
        Node::Markdown(n) => n.view(wired),
        Node::Use(n) => n.view(wired),
        Node::Outlet(n) => n.view(wired),
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
            // Nothing read while drawing makes the whole screen draw again; each node redraws
            // only what changed in it.
            untrack(|| {
                body.iter()
                    .enumerate()
                    .map(|(i, node)| draw(node, body_place(i), &host, None, None, None, None))
                    .collect_view()
            })
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
    let problems = host.problems;
    let toasts = toasts(host.clone());
    let asking = asking(host.clone());
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
            {toasts}
            {asking}
        </div>
    }
    .into_any()
}

/// `sys.toasts`, in the corner, each in its variant's color; a tap takes one away.
fn toasts(host: Host) -> AnyView {
    // Keyed by id, so a toast coming or going leaves the others' elements alone.
    let shown = Memo::new({
        let host = host.clone();
        move |_| {
            host.get("sys.toasts")
                .and_then(|t| t.as_array().cloned())
                .unwrap_or_default()
        }
    });
    view! {
        <div class="toast toast-end">
            <For each=move || shown.get() key=|toast| toast["id"].as_u64() children=move |toast| {
                let id = toast["id"].as_u64().unwrap_or_default();
                let text = toast["text"].as_str().unwrap_or_default().to_string();
                let class = match toast["variant"].as_str() {
                    Some("success") => "alert alert-success cursor-pointer",
                    Some("warning") => "alert alert-warning cursor-pointer",
                    Some("error") => "alert alert-error cursor-pointer",
                    _ => "alert alert-info cursor-pointer",
                };
                let host = host.clone();
                view! { <div role="status" class=class on:click=move |_| host.dismiss(id)>{text}</div> }
            } />
        </div>
    }
    .into_any()
}

/// The first question in `sys.asking`, in a modal dialog with its yes and no buttons. Closing it
/// any other way (Escape, a click outside it) answers no.
fn asking(host: Host) -> AnyView {
    // Memoized, so the dialog is drawn once per question, not again on every change elsewhere.
    let first = Memo::new({
        let host = host.clone();
        move |_| host.get("sys.asking.0")
    });
    view! {
        {move || first.get().map(|question| untrack(|| {
            let id = question["id"].as_u64().unwrap_or_default();
            let label = |key: &str| question[key].as_str().unwrap_or_default().to_string();
            let (text, yes, no) = (label("text"), label("yes"), label("no"));
            let dialog = NodeRef::<leptos::html::Dialog>::new();
            // The browser makes a dialog modal only through `showModal()`.
            leptos::prelude::Effect::new(move |_| {
                if let Some(element) = dialog.get()
                    && !element.open()
                {
                    let _ = element.show_modal();
                }
            });
            let still = {
                let host = host.clone();
                move || host.machine().get("sys.asking").is_some_and(|a| {
                    a.as_array().is_some_and(|a| a.iter().any(|q| q["id"].as_u64() == Some(id)))
                })
            };
            let on_close = {
                let (host, still) = (host.clone(), still.clone());
                move |_| if still() { host.answer(id, false) }
            };
            let (host_yes, host_no) = (host.clone(), host.clone());
            view! {
                <dialog node_ref=dialog class="modal" on:close=on_close>
                    <div class="modal-box">
                        <p class="py-2">{text}</p>
                        <div class="modal-action">
                            <button class="btn" on:click=move |_| host_no.answer(id, false)>{no}</button>
                            <button class="btn btn-primary" on:click=move |_| host_yes.answer(id, true)>{yes}</button>
                        </div>
                    </div>
                    <form method="dialog" class="modal-backdrop"><button>"close"</button></form>
                </dialog>
            }
        }))}
    }
    .into_any()
}

/// The payload a plain interaction carries: nothing.
fn nothing() -> Value {
    json!({})
}
