//! The config types: an app's TOML, loaded into closed Rust types.
//!
//! An app is the configs folder: `app.toml` at its top, and any other `.toml` files in any folders,
//! put together ([spec 020](../../../specs/020_config.md), [spec 120](../../../specs/120_one_app.md)).
//! serde (Rust's standard library for turning text formats into typed values) reads them into
//! [`App`], and [`App::from_files`] puts them together and checks the whole. The same types read and
//! write JSON too, which is how the server hands a loaded app to the browser.
//!
//! Every table refuses keys it doesn't know (`deny_unknown_fields`), so a typo is an error with a
//! line number, never a setting that's silently ignored.
//!
//! ```
//! use genesis_core::config::App;
//!
//! let app = App::from_toml("[app]\nname = \"Hello\"\n").unwrap();
//! assert_eq!(app.app.theme, "light");
//! ```

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;

use crate::style::styled;

/// A whole app: the top-level tables of `app.toml` and every other file, put together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    /// `[app]`: the app's name and settings.
    pub app: Info,
    /// `[state]`: app variables (`app.NAME`) and their starting values.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub state: Map<String, Value>,
    /// `[on]`: handlers, in the order the file lists them ([spec 100](../../../specs/100_events.md)).
    #[serde(
        default,
        with = "ordered_handlers",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub on: Vec<Handler>,
    /// `[actions.NAME]`: custom actions, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, Action>,
    /// `[sources.NAME]`: app variables an API fills ([spec 050](../../../specs/050_state.md), "Sources").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, Source>,
    /// `[connections.NAME]`: the APIs the app calls ([spec 030](../../../specs/030_connections.md)).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub connections: BTreeMap<String, Connection>,
    /// `[[body]]`: the screen, as a tree of nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<Node>,
}

/// `[app]`: the app's name and its settings, each with a default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Info {
    /// The window or tab title.
    pub name: String,
    /// The daisyUI theme `sys.theme` starts at. Default `"light"`.
    #[serde(default = "defaults::theme")]
    pub theme: String,
    /// The view `sys.view` starts at. Default `"main"`.
    #[serde(default = "defaults::start")]
    pub start: String,
    /// How deep a chain of events may go before it's stopped. Default `100`.
    #[serde(default = "defaults::max_chain")]
    pub max_chain: u32,
    /// How many recent events `sys.events` keeps. Default `200`.
    #[serde(default = "defaults::event_trail")]
    pub event_trail: usize,
    /// App variables a session never keeps on the device ([spec 050](../../../specs/050_state.md), "Persisting").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forget: Vec<String>,
}

/// One entry of `[on]`: an event pattern and the steps to run when an event matches it.
#[derive(Debug, Clone, PartialEq)]
pub struct Handler {
    /// An event name, with `*` and `?` as wildcards.
    pub pattern: String,
    /// The steps to run, in order.
    pub steps: Vec<Step>,
}

/// A custom action: a named list of steps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// The steps, in order. Written `do` in TOML, which is a Rust keyword.
    #[serde(rename = "do")]
    pub steps: Vec<Step>,
}

/// A node on the screen: one type from the closed catalog ([spec 040](../../../specs/040_catalog.md)).
///
/// The TOML names the type in a `type` key; serde picks the variant from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    /// A `div` that lays out its children.
    Cell(Cell),
    /// A paragraph, or a heading with `level`.
    Text(Text),
    /// A button.
    Button(Button),
    /// A picture.
    Image(Image),
    /// A link to another page.
    Link(Link),
}

impl Node {
    /// Its placement keys: where it sits in its parent. Every node takes them.
    pub fn placed(&self) -> crate::style::Placed {
        match self {
            Node::Cell(n) => n.placed(),
            Node::Text(n) => n.placed(),
            Node::Button(n) => n.placed(),
            Node::Image(n) => n.placed(),
            Node::Link(n) => n.placed(),
        }
    }

    /// The node's `id`, if it has one.
    pub fn id(&self) -> Option<&str> {
        match self {
            Node::Cell(n) => n.id.as_deref(),
            Node::Text(n) => n.id.as_deref(),
            Node::Button(n) => n.id.as_deref(),
            Node::Image(n) => n.id.as_deref(),
            Node::Link(n) => n.id.as_deref(),
        }
    }

    /// The name the node's events fire under: its `id`, or else its `place` in the tree, as
    /// [`body_place`] and [`child_place`] write it. The bus and the renderer both name nodes
    /// through this, so they always agree.
    pub fn name(&self, place: &str) -> String {
        self.id().map_or_else(|| place.to_string(), String::from)
    }
}

/// The place of the `i`th top-level node: `body.0`.
pub fn body_place(i: usize) -> String {
    format!("body.{i}")
}

/// The place of the `i`th child of the node at `parent`: `body.0.children.1`.
pub fn child_place(parent: &str, i: usize) -> String {
    format!("{parent}.children.{i}")
}

styled! {
    [layout, boxed, typed, placed]
    /// `type = "cell"`: a `div` holding other nodes, laid out and styled by its style keys
    /// ([spec 040](../../../specs/040_catalog.md)).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Cell {
        /// The node's name; its events fire under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The nodes inside it.
        #[serde(default)]
        pub children: Vec<Node>,
    }
}

styled! {
    [boxed, typed, placed]
    /// `type = "text"`: a paragraph, or with `level`, a heading.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Text {
        /// The node's name; its events fire under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The words; may hold templates.
        pub text: String,
        /// 1, 2, or 3: a heading (`h1`–`h3`) for screen readers, with a heading's size and weight.
        /// Without it, a paragraph.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "heading_level"
        )]
        pub level: Option<u8>,
    }
}

/// Reads `level`, refusing anything but 1, 2, or 3.
fn heading_level<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u8>, D::Error> {
    let level = i64::deserialize(deserializer)?;
    match level {
        1..=3 => Ok(Some(level as u8)),
        _ => Err(de::Error::custom(format!(
            "`level` is 1, 2, or 3, not {level}"
        ))),
    }
}

styled! {
    [placed]
    /// `type = "button"`. Fires `.click`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Button {
        /// The node's name; its events fire under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The button's text.
        pub label: String,
        /// Its look: `"primary"` unless it says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant: Option<crate::style::keys::Variant>,
        /// Its size: daisyUI's `"md"` unless it says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub size: Option<crate::style::keys::ButtonSize>,
        /// Shortcut for a handler on this button's `.click`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub on_click: Option<Vec<Step>>,
    }
}

styled! {
    [boxed, placed]
    /// `type = "image"`: a picture ([spec 040](../../../specs/040_catalog.md)).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Image {
        /// The node's name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// Where the picture is; may hold templates.
        pub src: String,
        /// What it shows, for screen readers and when it can't load; `""` for decoration. Required.
        pub alt: String,
    }
}

styled! {
    [placed]
    /// `type = "link"`: a link to another page. One outside the app opens in a new tab.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Link {
        /// The node's name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The words; may hold templates.
        pub text: String,
        /// Where it goes; may hold templates.
        pub href: String,
    }
}

/// One step of a handler or an action: a built-in action, or the name of a custom one.
///
/// In TOML a step is a plain string (`"cheer"` runs the custom action `cheer`) or a table whose
/// key names the built-in (`{ set = "app.x", to = 1 }`). See [spec 100](../../../specs/100_events.md).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Step {
    /// Runs a custom action.
    Run(String),
    /// Sets a variable.
    Set {
        /// The variable.
        set: String,
        /// The value; may be a template.
        to: Value,
    },
    /// Adds to a number, within optional bounds.
    Step {
        /// The variable.
        step: String,
        /// How much to add. Default `1`.
        by: f64,
        /// The lowest it may go.
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        /// The highest it may go.
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
    },
    /// Flips a boolean.
    Toggle {
        /// The variable.
        toggle: String,
    },
    /// Sets `sys.view`.
    Go {
        /// The view.
        go: String,
    },
    /// Fires an event.
    Fire {
        /// The event's name.
        fire: String,
        /// Its payload; values may be templates.
        #[serde(skip_serializing_if = "Map::is_empty")]
        with: Map<String, Value>,
    },
    /// Adds a line to the event trail.
    Log {
        /// The line; may hold templates.
        log: String,
    },
    /// Waits, then carries on with the list.
    Delay {
        /// Milliseconds.
        delay: u64,
        /// A name `cancel` can stop it by.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// Fires an event on a repeating timer.
    Every {
        /// Milliseconds between events.
        every: u64,
        /// The event's name.
        fire: String,
        /// A name `cancel` can stop it by.
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// The events' payload.
        #[serde(skip_serializing_if = "Map::is_empty")]
        with: Map<String, Value>,
    },
    /// Stops the `delay` or `every` with that `id`.
    Cancel {
        /// The `id`.
        cancel: String,
    },
    /// Calls these sources again: `app.NAME`, each a `[sources.NAME]`.
    Reload {
        /// The sources' variables.
        reload: Vec<String>,
    },
    /// Calls an operation and waits for the reply ([spec 030](../../../specs/030_connections.md)).
    Call {
        /// The operation: `connection.op` or `connection.resource.action`.
        call: String,
        /// Its params; values may be templates.
        #[serde(skip_serializing_if = "Map::is_empty")]
        params: Map<String, Value>,
        /// Variables to fill from the reply, each from a JSON Pointer into its body (`""` is all of it).
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        into: BTreeMap<String, String>,
        /// A JSON Pointer to a value in the reply that names one more event to fire.
        #[serde(skip_serializing_if = "Option::is_none")]
        event: Option<String>,
    },
}

/// A problem found loading an app, with where it is when that's known.
///
/// It prints as `FILE:LINE:COLUMN: MESSAGE`, leaving out what isn't known.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError {
    /// The file it's in, by its path in the configs folder (`crm/api.toml`).
    pub file: Option<String>,
    /// What's wrong.
    pub message: String,
    /// The line, counting from 1.
    pub line: Option<usize>,
    /// The column, counting from 1.
    pub column: Option<usize>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{file}:")?;
        }
        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(f, "{line}:{column}:")?;
        }
        if self.file.is_some() || self.line.is_some() {
            write!(f, " ")?;
        }
        write!(f, "{}", self.message)
    }
}

/// `[sources.NAME]`: the variable `app.NAME`, filled by calling an operation. It calls when the app
/// opens (or on `load_on`'s events instead), again whenever a variable in its params changes, and
/// on a `reload` naming it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The operation.
    pub call: String,
    /// Its params; values may be templates.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
    /// A JSON Pointer to the value in the reply. Default `""`, the whole body.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub select: String,
    /// The events to call on instead of `app.opened`; `[]` calls only on a `reload`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_on: Option<Vec<String>>,
}

/// `[connections.NAME]`: an HTTP API ([spec 030](../../../specs/030_connections.md)).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// Where its requests go; each operation's path is joined onto it.
    pub base_url: String,
    /// Headers on every request; values may hold `{{ creds.NAME }}` and `{{ app.* }}` templates.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Seconds before a request fails; `0` waits forever. Default `30`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    /// `[connections.NAME.ops.OP]`: its operations, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ops: BTreeMap<String, OpConfig>,
    /// `[connections.NAME.crud.RESOURCE]`: resources that expand into five operations each.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub crud: BTreeMap<String, Crud>,
}

/// One operation, as written in the TOML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpConfig {
    /// The HTTP method.
    pub method: Method,
    /// Joined onto the connection's `base_url`; `{name}` takes the param `name`.
    pub path: String,
    /// Headers merged over the connection's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Seconds before it fails, over the connection's `timeout`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// A `crud` resource: `list`, `get`, `create`, `update`, and `delete` by convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crud {
    /// The param naming one record. Default `"id"`.
    #[serde(default = "defaults::key")]
    pub key: String,
    /// `rest` (the default) or `post`.
    #[serde(default)]
    pub style: CrudStyle,
    /// For `rest`: the base path. Default `/RESOURCE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// For `post`: the name before `-list` and the rest. Default the resource's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Replaces the conventional `list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<OpConfig>,
    /// Replaces the conventional `get`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get: Option<OpConfig>,
    /// Replaces the conventional `create`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<OpConfig>,
    /// Replaces the conventional `update`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<OpConfig>,
    /// Replaces the conventional `delete`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<OpConfig>,
}

/// How a `crud` resource maps onto requests.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrudStyle {
    /// `GET /contacts`, `GET /contacts/{id}`, `POST /contacts`, `PATCH /contacts/{id}`, `DELETE /contacts/{id}`.
    #[default]
    Rest,
    /// Every action is `POST /contacts-ACTION`, as PurpleFlow webhooks take them.
    Post,
}

/// An HTTP method. Written in any case in TOML; always upper case in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET`
    Get,
    /// `HEAD`
    Head,
    /// `POST`
    Post,
    /// `PUT`
    Put,
    /// `PATCH`
    Patch,
    /// `DELETE`
    Delete,
    /// `OPTIONS`
    Options,
}

impl Method {
    const ALL: [(Method, &'static str); 7] = [
        (Method::Get, "GET"),
        (Method::Head, "HEAD"),
        (Method::Post, "POST"),
        (Method::Put, "PUT"),
        (Method::Patch, "PATCH"),
        (Method::Delete, "DELETE"),
        (Method::Options, "OPTIONS"),
    ];

    /// The method's name, in upper case.
    pub fn as_str(self) -> &'static str {
        Method::ALL
            .iter()
            .find(|(m, _)| *m == self)
            .map_or("GET", |(_, name)| name)
    }
}

impl Serialize for Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        let upper = name.to_ascii_uppercase();
        Method::ALL
            .iter()
            .find(|(_, n)| *n == upper)
            .map(|(m, _)| *m)
            .ok_or_else(|| {
                let names: Vec<&str> = Method::ALL.iter().map(|(_, n)| *n).collect();
                de::Error::custom(format!(
                    "unknown method `{name}`; a method is one of {}",
                    names.join(", ")
                ))
            })
    }
}

/// An operation ready to call: its method, full URL, merged headers, and timeout.
#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    /// The HTTP method.
    pub method: Method,
    /// The base URL and path joined; `{name}` placeholders still in it.
    pub url: String,
    /// The connection's headers with the operation's over them; templates still in them.
    pub headers: BTreeMap<String, String>,
    /// Seconds before it fails; `0` waits forever.
    pub timeout: u64,
}

const CRUD_ACTIONS: [&str; 5] = ["list", "get", "create", "update", "delete"];

impl App {
    /// The operation called `name`: `connection.op`, or `connection.resource.action` for `crud`.
    /// `None` when no connection declares it.
    pub fn operation(&self, name: &str) -> Option<Operation> {
        let (connection_name, rest) = name.split_once('.')?;
        let connection = self.connections.get(connection_name)?;
        let op = match connection.ops.get(rest) {
            Some(op) => op.clone(),
            None => {
                let (resource, action) = rest.split_once('.')?;
                crud_op(resource, connection.crud.get(resource)?, action)?
            }
        };
        let mut headers = connection.headers.clone();
        headers.extend(op.headers.clone());
        Some(Operation {
            method: op.method,
            url: format!("{}{}", connection.base_url.trim_end_matches('/'), op.path),
            headers,
            timeout: op.timeout.or(connection.timeout).unwrap_or(30),
        })
    }

    /// Every operation's name, connection by connection.
    pub fn operation_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for (name, connection) in &self.connections {
            names.extend(connection.ops.keys().map(|op| format!("{name}.{op}")));
            for resource in connection.crud.keys() {
                names.extend(
                    CRUD_ACTIONS
                        .iter()
                        .map(|action| format!("{name}.{resource}.{action}")),
                );
            }
        }
        names
    }

    /// Loads an app from the text of its `app.toml` alone.
    ///
    /// The same as [`App::from_files`] with only that file.
    pub fn from_toml(text: &str) -> Result<App, Vec<ConfigError>> {
        App::from_files([("app.toml", text)])
    }

    /// Loads an app from its files: each one's path in the configs folder (`crm/api.toml`, with `/`
    /// between folders) and its text, in any order.
    ///
    /// `app.toml` at the top holds `[app]` and `[[body]]`; every other file may add `[state]`,
    /// `[sources]`, `[connections]`, `[actions]`, and `[on]`. Handlers keep their order: `app.toml`'s
    /// first, then each other file's in path order. Fails with every problem in every file at once:
    /// a file that doesn't read, a table where it can't go, two files with one name in any folders, or
    /// one variable, source, connection, or action defined twice ([spec 020](../../../specs/020_config.md), "Checks").
    ///
    /// ```
    /// use genesis_core::config::App;
    ///
    /// let app = App::from_files([
    ///     ("app.toml", "[app]\nname = \"CRM\"\n"),
    ///     ("crm/state.toml", "[state]\npage = 1\n"),
    /// ])
    /// .unwrap();
    /// assert_eq!(app.state["page"], 1);
    /// ```
    pub fn from_files<P, T>(
        files: impl IntoIterator<Item = (P, T)>,
    ) -> Result<App, Vec<ConfigError>>
    where
        P: AsRef<str>,
        T: AsRef<str>,
    {
        let mut files: Vec<(P, T)> = files.into_iter().collect();
        // app.toml first, then the others by path: the order handlers run in.
        files.sort_by(|(a, _), (b, _)| {
            let (a, b) = (a.as_ref(), b.as_ref());
            (a != "app.toml", a).cmp(&(b != "app.toml", b))
        });
        let mut problems = Vec::new();
        let mut top: Option<App> = None;
        let mut parts = Vec::new();
        let mut names = Defined::default();
        let mut seen_files: BTreeMap<String, String> = BTreeMap::new();
        for (path, text) in &files {
            let (path, text) = (path.as_ref(), text.as_ref());
            let file_name = path.rsplit('/').next().unwrap_or(path).to_string();
            if let Some(first) = seen_files.get(&file_name) {
                problems.push(problem_in(
                    path,
                    format!(
                        "the file name {file_name} is also {first}; give one of them another name"
                    ),
                ));
                continue;
            }
            seen_files.insert(file_name, path.to_string());
            let read = if path == "app.toml" {
                toml::from_str::<App>(text).map(|app| {
                    top = Some(app);
                })
            } else {
                match misplaced(path, text) {
                    Some(problem) => {
                        problems.push(problem);
                        continue;
                    }
                    None => toml::from_str::<Part>(text).map(|part| parts.push(part)),
                }
            };
            match read {
                Ok(()) => names.add(path, text, &mut problems),
                Err(e) => problems.push(toml_problem(path, text, &e)),
            }
        }
        if !seen_files.get("app.toml").is_some_and(|p| p == "app.toml") {
            problems.insert(
                0,
                ConfigError {
                    file: None,
                    message: "app.toml not found at the top of the configs folder".into(),
                    line: None,
                    column: None,
                },
            );
        }
        if let Some(app) = &top {
            for (i, node) in app.body.iter().enumerate() {
                check_node(node, &body_place(i), &mut problems);
            }
        }
        names.clashes(&mut problems);
        match top {
            Some(mut app) if problems.is_empty() => {
                for part in parts {
                    app.state.extend(part.state);
                    app.on.extend(part.on);
                    app.actions.extend(part.actions);
                    app.sources.extend(part.sources);
                    app.connections.extend(part.connections);
                }
                Ok(app)
            }
            _ => Err(problems),
        }
    }
}

/// What a file other than `app.toml` may hold: everything but `[app]` and `[[body]]`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Part {
    #[serde(default)]
    state: Map<String, Value>,
    #[serde(default, with = "ordered_handlers")]
    on: Vec<Handler>,
    #[serde(default)]
    actions: BTreeMap<String, Action>,
    #[serde(default)]
    sources: BTreeMap<String, Source>,
    #[serde(default)]
    connections: BTreeMap<String, Connection>,
}

/// The names a file defines, each with where it's written.
#[derive(Deserialize)]
struct Names {
    #[serde(default)]
    state: BTreeMap<toml::Spanned<String>, de::IgnoredAny>,
    #[serde(default)]
    sources: BTreeMap<toml::Spanned<String>, de::IgnoredAny>,
    #[serde(default)]
    connections: BTreeMap<toml::Spanned<String>, de::IgnoredAny>,
    #[serde(default)]
    actions: BTreeMap<toml::Spanned<String>, de::IgnoredAny>,
}

/// Every definition so far, by kind and name, with the first place each is written
/// (`FILE:LINE:COLUMN`). A definition made twice is a problem at the second place.
#[derive(Default)]
struct Defined {
    /// By kind (`state`, `sources`, `connections`, `actions`), then name.
    kinds: BTreeMap<&'static str, BTreeMap<String, String>>,
}

impl Defined {
    /// Adds a file's definitions, with a problem for each one already made.
    fn add(&mut self, path: &str, text: &str, problems: &mut Vec<ConfigError>) {
        // The file has already read as its full type, so it reads as names too.
        let Ok(names) = toml::from_str::<Names>(text) else {
            return;
        };
        let kinds = [
            ("state", names.state),
            ("sources", names.sources),
            ("connections", names.connections),
            ("actions", names.actions),
        ];
        for (kind, defined) in kinds {
            let known = self.kinds.entry(kind).or_default();
            for name in defined.into_keys() {
                let (line, column) = line_and_column(text, name.span().start);
                let place = format!("{path}:{line}:{column}");
                let name = name.into_inner();
                match known.get(&name) {
                    Some(first) => problems.push(ConfigError {
                        file: Some(path.into()),
                        message: format!(
                            "{} is defined twice; it's also at {first}",
                            what(kind, &name)
                        ),
                        line: Some(line),
                        column: Some(column),
                    }),
                    None => {
                        known.insert(name, place);
                    }
                }
            }
        }
    }

    /// A problem for each app variable that's both a `[state]` variable and a source.
    fn clashes(&self, problems: &mut Vec<ConfigError>) {
        let (Some(state), Some(sources)) = (self.kinds.get("state"), self.kinds.get("sources"))
        else {
            return;
        };
        for (name, place) in state {
            let Some(source) = sources.get(name) else {
                continue;
            };
            let mut at = place.rsplitn(3, ':');
            let column = at.next().and_then(|c| c.parse().ok());
            let line = at.next().and_then(|l| l.parse().ok());
            problems.push(ConfigError {
                file: at.next().map(String::from),
                message: format!(
                    "app.{name} is both a [state] variable and a source (at {source}); give one another name"
                ),
                line,
                column,
            });
        }
    }
}

/// How a problem names a definition: `app.page`, `connection crm`, `source contacts`, `action refresh`.
fn what(kind: &str, name: &str) -> String {
    match kind {
        "state" => format!("app.{name}"),
        "sources" => format!("source {name}"),
        "connections" => format!("connection {name}"),
        _ => format!("action {name}"),
    }
}

/// A problem with a whole file, with no line.
fn problem_in(path: &str, message: String) -> ConfigError {
    ConfigError {
        file: Some(path.into()),
        message,
        line: None,
        column: None,
    }
}

/// `[app]` or `[[body]]` in a file other than `app.toml`, where its header is written.
fn misplaced(path: &str, text: &str) -> Option<ConfigError> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        let table = match trimmed {
            "[app]" => "[app]",
            _ if trimmed.starts_with("[app.") => "[app]",
            "[[body]]" => "[[body]]",
            _ if trimmed.starts_with("[body") || trimmed.starts_with("[[body.") => "[[body]]",
            _ => {
                offset += line.len();
                continue;
            }
        };
        let (line, column) = line_and_column(text, offset + (line.len() - line.trim_start().len()));
        return Some(ConfigError {
            file: Some(path.into()),
            message: format!("{table} goes only in app.toml, at the top of the configs folder"),
            line: Some(line),
            column: Some(column),
        });
    }
    None
}

/// A problem the TOML reader found in a file, with its line and column when the reader knows them.
fn toml_problem(path: &str, text: &str, e: &toml::de::Error) -> ConfigError {
    let message = e.message().to_string();
    let at = e
        .span()
        .map(|span| unknown_key_offset(text, span.start, &message).unwrap_or(span.start));
    let (line, column) = at.map(|offset| line_and_column(text, offset)).unzip();
    ConfigError {
        file: Some(path.into()),
        message,
        line,
        column,
    }
}

/// For an error about one key (an unknown key, or a message starting with a key in backticks, as
/// `` `gap` is ... ``), where that key is written, searching from the start of the table the
/// error points at.
///
/// Nodes are read through a `type` tag, so the TOML reader can only point at the table holding a
/// bad key, not the key itself. The table's own lines are searched for it.
fn unknown_key_offset(text: &str, table_start: usize, message: &str) -> Option<usize> {
    let key = message
        .strip_prefix("unknown field `")
        .or_else(|| message.strip_prefix('`'))?
        .split('`')
        .next()?;
    let mut offset = table_start;
    for line in text.get(table_start..)?.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed
            .strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
        {
            return Some(offset + (line.len() - trimmed.len()));
        }
        offset += line.len();
    }
    None
}

/// Checks what one key can't: a node's keys that only make sense apart. `place` names the node,
/// since it may have no `id`.
fn check_node(node: &Node, place: &str, problems: &mut Vec<ConfigError>) {
    if let Node::Cell(cell) = node {
        if cell.flow.is_some() && cell.grid.is_some() {
            problems.push(problem_in(
                "app.toml",
                format!("{place}: a cell lays out by `flow` or by `grid`, not both"),
            ));
        }
        for (i, child) in cell.children.iter().enumerate() {
            check_node(child, &child_place(place, i), problems);
        }
    }
}

/// A `crud` resource's operation for `action`: its override, or the convention for its style.
fn crud_op(resource: &str, crud: &Crud, action: &str) -> Option<OpConfig> {
    let replaced = match action {
        "list" => &crud.list,
        "get" => &crud.get,
        "create" => &crud.create,
        "update" => &crud.update,
        "delete" => &crud.delete,
        _ => return None,
    };
    if let Some(op) = replaced {
        return Some(op.clone());
    }
    let (method, path) = match crud.style {
        CrudStyle::Post => (
            Method::Post,
            format!("/{}-{action}", crud.prefix.as_deref().unwrap_or(resource)),
        ),
        CrudStyle::Rest => {
            let base = crud.path.clone().unwrap_or_else(|| format!("/{resource}"));
            let one = format!("{base}/{{{}}}", crud.key);
            match action {
                "list" => (Method::Get, base),
                "get" => (Method::Get, one),
                "create" => (Method::Post, base),
                "update" => (Method::Patch, one),
                _ => (Method::Delete, one),
            }
        }
    };
    Some(OpConfig {
        method,
        path,
        headers: BTreeMap::new(),
        timeout: None,
    })
}

/// Turns a byte offset into a 1-based line and column.
fn line_and_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

/// The step kinds, keyed by the TOML key that names each one. `every` comes before `fire`
/// because an `every` step has a `fire` key too.
const STEP_KINDS: [&str; 11] = [
    "call", "reload", "every", "set", "step", "toggle", "go", "fire", "log", "delay", "cancel",
];

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // A step's shape depends on which key it has, so read it whole first, then pick.
        let fields = match Value::deserialize(deserializer)? {
            Value::String(name) => return Ok(Step::Run(name)),
            Value::Object(fields) => fields,
            other => {
                return Err(de::Error::custom(format!(
                    "a step is a string or a table, not {other}"
                )));
            }
        };
        let Some(kind) = STEP_KINDS.into_iter().find(|k| fields.contains_key(*k)) else {
            let keys: Vec<&str> = fields.keys().map(String::as_str).collect();
            return Err(de::Error::custom(format!(
                "unknown step with keys `{}`; a step is one of `{}`, or a custom action's name",
                keys.join("`, `"),
                STEP_KINDS.join("`, `")
            )));
        };
        let fields = Value::Object(fields);
        let wrong = |e: serde_json::Error| de::Error::custom(format!("in a `{kind}` step: {e}"));
        Ok(match kind {
            "set" => {
                let s: steps::Set = serde_json::from_value(fields).map_err(wrong)?;
                Step::Set {
                    set: s.set,
                    to: s.to,
                }
            }
            "step" => {
                let s: steps::Step = serde_json::from_value(fields).map_err(wrong)?;
                Step::Step {
                    step: s.step,
                    by: s.by,
                    min: s.min,
                    max: s.max,
                }
            }
            "toggle" => Step::Toggle {
                toggle: serde_json::from_value::<steps::Toggle>(fields)
                    .map_err(wrong)?
                    .toggle,
            },
            "go" => Step::Go {
                go: serde_json::from_value::<steps::Go>(fields)
                    .map_err(wrong)?
                    .go,
            },
            "fire" => {
                let s: steps::Fire = serde_json::from_value(fields).map_err(wrong)?;
                Step::Fire {
                    fire: s.fire,
                    with: s.with,
                }
            }
            "log" => Step::Log {
                log: serde_json::from_value::<steps::Log>(fields)
                    .map_err(wrong)?
                    .log,
            },
            "delay" => {
                let s: steps::Delay = serde_json::from_value(fields).map_err(wrong)?;
                Step::Delay {
                    delay: s.delay,
                    id: s.id,
                }
            }
            "every" => {
                let s: steps::Every = serde_json::from_value(fields).map_err(wrong)?;
                Step::Every {
                    every: s.every,
                    fire: s.fire,
                    id: s.id,
                    with: s.with,
                }
            }
            "reload" => Step::Reload {
                reload: serde_json::from_value::<steps::Reload>(fields)
                    .map_err(wrong)?
                    .reload,
            },
            "call" => {
                let s: steps::Call = serde_json::from_value(fields).map_err(wrong)?;
                Step::Call {
                    call: s.call,
                    params: s.params,
                    into: s.into,
                    event: s.event,
                }
            }
            _ => Step::Cancel {
                cancel: serde_json::from_value::<steps::Cancel>(fields)
                    .map_err(wrong)?
                    .cancel,
            },
        })
    }
}

/// One strict struct per step kind, so an extra key in a step is an error naming it.
mod steps {
    use serde::Deserialize;
    use serde_json::{Map, Value};

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Set {
        pub set: String,
        pub to: Value,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Step {
        pub step: String,
        #[serde(default = "super::defaults::by")]
        pub by: f64,
        pub min: Option<f64>,
        pub max: Option<f64>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Toggle {
        pub toggle: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Go {
        pub go: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Fire {
        pub fire: String,
        #[serde(default)]
        pub with: Map<String, Value>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Log {
        pub log: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Delay {
        pub delay: u64,
        pub id: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Every {
        pub every: u64,
        pub fire: String,
        pub id: Option<String>,
        #[serde(default)]
        pub with: Map<String, Value>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Reload {
        pub reload: Vec<String>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Call {
        pub call: String,
        #[serde(default)]
        pub params: Map<String, Value>,
        #[serde(default)]
        pub into: std::collections::BTreeMap<String, String>,
        pub event: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Cancel {
        pub cancel: String,
    }
}

/// Reads `[on]` as a list that keeps the file's order (a plain map would sort or scramble it),
/// and writes it back as a table.
mod ordered_handlers {
    use super::*;

    pub fn serialize<S: Serializer>(
        handlers: &[Handler],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(handlers.len()))?;
        for handler in handlers {
            map.serialize_entry(&handler.pattern, &handler.steps)?;
        }
        map.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<Handler>, D::Error> {
        struct InOrder;
        impl<'de> Visitor<'de> for InOrder {
            type Value = Vec<Handler>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a table of event patterns and their steps")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut handlers = Vec::new();
                while let Some((pattern, steps)) = map.next_entry()? {
                    handlers.push(Handler { pattern, steps });
                }
                Ok(handlers)
            }
        }
        deserializer.deserialize_map(InOrder)
    }
}

/// Default values, as functions because that's what serde's `default = "..."` takes.
mod defaults {
    pub fn theme() -> String {
        "light".into()
    }
    pub fn start() -> String {
        "main".into()
    }
    pub fn max_chain() -> u32 {
        100
    }
    pub fn event_trail() -> usize {
        200
    }
    pub fn key() -> String {
        "id".into()
    }
    pub fn by() -> f64 {
        1.0
    }
}
