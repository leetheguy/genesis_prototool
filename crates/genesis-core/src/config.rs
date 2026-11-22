//! The config types: an app's TOML, loaded into closed Rust types.
//!
//! An app is one `app.toml` ([spec 020](../../../specs/020_config.md)). serde (Rust's standard
//! library for turning text formats into typed values) reads it into [`App`]. The same types read
//! and write JSON too, which is how the server hands a loaded app to the browser.
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

/// A whole app: the top-level tables of `app.toml`.
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
    /// Children, top to bottom.
    Stack(Stack),
    /// A heading.
    Heading(Heading),
    /// A paragraph of text.
    Text(Text),
    /// A button.
    Button(Button),
}

impl Node {
    /// The node's `id`, if it has one.
    pub fn id(&self) -> Option<&str> {
        match self {
            Node::Stack(n) => n.id.as_deref(),
            Node::Heading(n) => n.id.as_deref(),
            Node::Text(n) => n.id.as_deref(),
            Node::Button(n) => n.id.as_deref(),
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

/// `type = "stack"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    /// The node's name; its events fire under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The nodes inside it.
    #[serde(default)]
    pub children: Vec<Node>,
}

/// `type = "heading"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heading {
    /// The node's name; its events fire under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The heading's text; may hold templates.
    pub text: String,
    /// 1 to 3. Default `1`.
    #[serde(default = "defaults::level")]
    pub level: u8,
}

/// `type = "text"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    /// The node's name; its events fire under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The paragraph; may hold templates.
    pub text: String,
}

/// `type = "button"`. Fires `.click`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Button {
    /// The node's name; its events fire under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The button's text.
    pub label: String,
    /// Shortcut for a handler on this button's `.click`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_click: Option<Vec<Step>>,
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
}

/// A problem found loading an app, with where it is when that's known.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError {
    /// What's wrong.
    pub message: String,
    /// The line, counting from 1.
    pub line: Option<usize>,
    /// The column, counting from 1.
    pub column: Option<usize>,
}

impl App {
    /// Loads an app from the text of its `app.toml`.
    ///
    /// Fails with every problem found, each with its line and column when the TOML reader knows them.
    pub fn from_toml(text: &str) -> Result<App, Vec<ConfigError>> {
        toml::from_str(text).map_err(|e| {
            let message = e.message().to_string();
            let at = e
                .span()
                .map(|span| unknown_key_offset(text, span.start, &message).unwrap_or(span.start));
            let (line, column) = at.map(|offset| line_and_column(text, offset)).unzip();
            vec![ConfigError {
                message,
                line,
                column,
            }]
        })
    }
}

/// For an unknown-key error, where that key is written, searching from the start of the table
/// the error points at.
///
/// Nodes are read through a `type` tag, so the TOML reader can only point at the table holding a
/// bad key, not the key itself. The table's own lines are searched for it.
fn unknown_key_offset(text: &str, table_start: usize, message: &str) -> Option<usize> {
    let key = message.strip_prefix("unknown field `")?.split('`').next()?;
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

/// Turns a byte offset into a 1-based line and column.
fn line_and_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

/// The step kinds, keyed by the TOML key that names each one. `every` comes before `fire`
/// because an `every` step has a `fire` key too.
const STEP_KINDS: [&str; 9] = [
    "every", "set", "step", "toggle", "go", "fire", "log", "delay", "cancel",
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
    pub fn level() -> u8 {
        1
    }
    pub fn by() -> f64 {
        1.0
    }
}
