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
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::style::styled;
use crate::template;

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
    /// Every component, by its file name ([spec 090](../../../specs/090_components.md)). Each is a
    /// file of its own, never a table in `app.toml`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, Component>,
    /// The app's timers, by name, from `timers.toml` ([spec 100](../../../specs/100_events.md),
    /// "Timers"). They join Genesis's own in one registry ([`timers`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub timers: BTreeMap<String, Timer>,
}

/// A timer: how long it runs, and whether it starts again each time it completes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timer {
    /// Milliseconds from its start to its `.complete`. At least 1.
    pub ms: u64,
    /// Starts again each time it completes, until stopped. Default `false`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeat: bool,
}

/// Genesis's own timers, built in ([spec 100](../../../specs/100_events.md), "Timers"). None yet: the
/// first comes with `toast`.
const SYSTEM_TIMERS: [(&str, Timer); 0] = [];

/// Every timer the app can start, by name: Genesis's own, then the app's from `timers.toml`.
pub fn timers(app: &App) -> BTreeMap<String, Timer> {
    let mut all: BTreeMap<String, Timer> = SYSTEM_TIMERS
        .into_iter()
        .map(|(name, timer)| (name.to_string(), timer))
        .collect();
    all.extend(app.timers.clone());
    all
}

/// A component: a file with a `[component]` table, used by its file name with a `use` node
/// ([spec 090](../../../specs/090_components.md)).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// The props every caller passes in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<String>,
    /// Props a caller may leave out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<String>,
    /// `[state]`: each instance's own variables (`local.*`) and their starting values.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub state: Map<String, Value>,
    /// `[[body]]`: what it draws, in the `use` node's place.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<Node>,
}

impl Component {
    /// Whether `name` is one of its props, required or optional.
    pub fn takes(&self, name: &str) -> bool {
        self.props.iter().chain(&self.optional).any(|p| p == name)
    }
}

/// A component's file, as written: `[component]`, `[state]`, and `[[body]]`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentFile {
    component: ComponentHead,
    #[serde(default)]
    state: Map<String, Value>,
    #[serde(default)]
    body: Vec<Node>,
}

/// `[component]`: its props.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentHead {
    #[serde(default)]
    props: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
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
    /// An inline drawing, checked at load.
    Svg(Svg),
    /// A rule between sections.
    Divider(Divider),
    /// A sound player.
    Audio(Audio),
    /// A video player.
    Video(Video),
    /// Another page, embedded and sandboxed.
    Iframe(Iframe),
    /// A one-line text box.
    Input(Input),
    /// A box for longer text.
    Textarea(Textarea),
    /// A drop-down of options.
    Select(Select),
    /// A checkbox.
    Checkbox(Checkbox),
    /// A switch.
    Toggle(Toggle),
    /// One radio button of a group.
    Radio(Radio),
    /// A form: its named controls fill a draft, which goes out on `.submit`.
    Form(Form),
    /// A file picker, inside a form.
    File(File),
    /// A modal dialog, open while its variable says so.
    Dialog(Dialog),
    /// Markdown, drawn safely.
    Markdown(Markdown),
    /// A component, by name, with its props.
    Use(Use),
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
            Node::Svg(n) => n.placed(),
            Node::Divider(n) => n.placed(),
            Node::Audio(n) => n.placed(),
            Node::Video(n) => n.placed(),
            Node::Iframe(n) => n.placed(),
            Node::Input(n) => n.placed(),
            Node::Textarea(n) => n.placed(),
            Node::Select(n) => n.placed(),
            Node::Checkbox(n) => n.placed(),
            Node::Toggle(n) => n.placed(),
            Node::Radio(n) => n.placed(),
            Node::Form(n) => n.placed(),
            Node::File(n) => n.placed(),
            Node::Dialog(n) => n.placed(),
            Node::Markdown(n) => n.placed(),
            Node::Use(n) => n.placed(),
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
            Node::Svg(n) => n.id.as_deref(),
            Node::Divider(n) => n.id.as_deref(),
            Node::Audio(n) => n.id.as_deref(),
            Node::Video(n) => n.id.as_deref(),
            Node::Iframe(n) => n.id.as_deref(),
            Node::Input(n) => n.id.as_deref(),
            Node::Textarea(n) => n.id.as_deref(),
            Node::Select(n) => n.id.as_deref(),
            Node::Checkbox(n) => n.id.as_deref(),
            Node::Toggle(n) => n.id.as_deref(),
            Node::Radio(n) => n.id.as_deref(),
            Node::Form(n) => n.id.as_deref(),
            Node::File(n) => n.id.as_deref(),
            Node::Dialog(n) => n.id.as_deref(),
            Node::Markdown(n) => n.id.as_deref(),
            Node::Use(n) => n.id.as_deref(),
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
        /// A variable holding a list (`app.contacts`, or `item.tags` inside another `each`): draws
        /// the children once per element, with `item.*` set to it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub each: Option<String>,
        /// With `each`, what it shows when the list is empty; "Nothing here." unless it says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub empty: Option<String>,
        /// With `each`, a line of its own above the detail when its source fails.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
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

styled! {
    [boxed, typed, placed]
    /// `type = "markdown"`: longer prose as Markdown, drawn safely: raw HTML shows as text, and
    /// only `http`, `https`, `mailto`, `tel`, and relative links and images are kept
    /// ([`crate::markdown`]).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Markdown {
        /// The node's name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The Markdown; may hold templates, filled in before it's read as Markdown.
        pub text: String,
    }
}

styled! {
    [placed]
    /// `type = "use"`: draws a component in its place ([spec 090](../../../specs/090_components.md)).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Use {
        /// The node's name; events inside the component fire under it (`card.more_btn.click`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The component's name: its file name, without `.toml`.
        pub component: String,
        /// What it passes in, by prop: templates, filled where the `use` node is drawn.
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        pub props: Map<String, Value>,
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
        /// Shortcut for a handler on this button's `.click`: a list of steps, or one step alone,
        /// like `"{{ props.on_open }}"` inside a component.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "one_or_more")]
        pub on_click: Option<Vec<Step>>,
        /// Submits the form it's in.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub submit: bool,
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

styled! {
    [typed, placed]
    /// `type = "svg"`: a drawing, inline, so it takes the theme's colors (`currentColor` follows
    /// `color`) and `size`. Its markup is checked at load ([`crate::svg`]).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Svg {
        /// The node's name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The markup: one `<svg>` element, cleaned.
        pub svg: crate::svg::SvgMarkup,
        /// What it shows, for screen readers. Without it, it's decoration.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
    }
}

styled! {
    [boxed, placed]
    /// `type = "divider"`: a plain `hr`, a rule between sections.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Divider {
        /// The node's name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The line's theme color; the theme's own border color unless it says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub color: Option<crate::style::keys::LineColor>,
    }
}

styled! {
    [placed]
    /// `type = "audio"`: a sound player. With an `id`, fires `.play`, `.pause`, and `.ended`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Audio {
        /// The node's name; its events fire under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// Where it plays from; may hold templates.
        pub src: String,
        /// Shows the player's controls. On unless it says otherwise.
        #[serde(default = "defaults::yes")]
        pub controls: bool,
        /// Starts playing on its own (browsers allow it only when `muted`).
        #[serde(default)]
        pub autoplay: bool,
        /// Starts again at the end.
        #[serde(default, rename = "loop")]
        pub r#loop: bool,
        /// Starts with no sound.
        #[serde(default)]
        pub muted: bool,
    }
}

styled! {
    [boxed, placed]
    /// `type = "video"`: a video player; as `audio`, with a `poster`. With an `id`, fires `.play`,
    /// `.pause`, and `.ended`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Video {
        /// The node's name; its events fire under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// Where it plays from; may hold templates.
        pub src: String,
        /// Shows the player's controls. On unless it says otherwise.
        #[serde(default = "defaults::yes")]
        pub controls: bool,
        /// Starts playing on its own (browsers allow it only when `muted`).
        #[serde(default)]
        pub autoplay: bool,
        /// Starts again at the end.
        #[serde(default, rename = "loop")]
        pub r#loop: bool,
        /// Starts with no sound.
        #[serde(default)]
        pub muted: bool,
        /// A picture to show before it plays; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub poster: Option<String>,
    }
}

styled! {
    [boxed, placed]
    /// `type = "iframe"`: another page, embedded. Sandboxed unless `sandbox = false`; 24rem tall
    /// unless its `height` says otherwise.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Iframe {
        /// The node's name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The page; may hold templates.
        pub src: String,
        /// What it is, for screen readers. Required.
        pub title: String,
        /// Runs the page with scripts, forms, and popups, but not as this app's origin and
        /// without navigating it. On unless it says otherwise.
        #[serde(default = "defaults::yes")]
        pub sandbox: bool,
    }
}

// The controls ([spec 040](../../../specs/040_catalog.md), "Controls"). Each reads and writes the
// variable its `var` names: a change fires `NAME.change` with the new `value`, and the bus runs a
// `set` of `var` for it (see `crate::bus`), so only an action ever changes state.

styled! {
    [placed]
    /// `type = "input"`: a one-line box; its value is text, or a number for `kind = "number"`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Input {
        /// The node's name; its `.change` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The variable it reads and writes, outside a form: `app.NAME` or `sys.theme`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub var: Option<String>,
        /// Its name in the form it's in: its value goes in the form's draft, and out on `.submit`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// What it takes: `text` unless it says otherwise.
        #[serde(default, skip_serializing_if = "InputKind::is_text")]
        pub kind: InputKind,
        /// Its label; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
        /// Shown while it's empty; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub placeholder: Option<String>,
    }
}

/// What an `input` takes, as HTML's own input types.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    /// Any text.
    #[default]
    Text,
    /// An email address.
    Email,
    /// Text drawn as dots.
    Password,
    /// A number; empty is `null`.
    Number,
    /// A date, as `YYYY-MM-DD`.
    Date,
    /// Search text.
    Search,
}

impl InputKind {
    /// Its HTML `type`.
    pub fn as_str(self) -> &'static str {
        match self {
            InputKind::Text => "text",
            InputKind::Email => "email",
            InputKind::Password => "password",
            InputKind::Number => "number",
            InputKind::Date => "date",
            InputKind::Search => "search",
        }
    }

    fn is_text(&self) -> bool {
        *self == InputKind::Text
    }
}

styled! {
    [placed]
    /// `type = "textarea"`: a box for longer text.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Textarea {
        /// The node's name; its `.change` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The variable it reads and writes, outside a form: `app.NAME` or `sys.theme`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub var: Option<String>,
        /// Its name in the form it's in: its value goes in the form's draft, and out on `.submit`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// Its label; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
        /// Shown while it's empty; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub placeholder: Option<String>,
        /// How many lines tall it is; the browser's own unless it says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub rows: Option<u32>,
    }
}

styled! {
    [placed]
    /// `type = "select"`: a drop-down. Its value is the picked option's `value`, with its type.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Select {
        /// The node's name; its `.change` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The variable it reads and writes, outside a form: `app.NAME` or `sys.theme`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub var: Option<String>,
        /// Its name in the form it's in: its value goes in the form's draft, and out on `.submit`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// The options: a list here, or a variable holding one.
        pub options: Options,
        /// With options from a variable, the JSON Pointer to each one's label: `/label` unless it
        /// says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub option_label: Option<String>,
        /// With options from a variable, the JSON Pointer to each one's value: `/value` unless it
        /// says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub option_value: Option<String>,
        /// Its label; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
        /// Shown while the value matches no option; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub placeholder: Option<String>,
    }
}

/// A `select`'s options: written out, or the path of a variable holding a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Options {
    /// `[{ label, value }]`.
    List(Vec<Choice>),
    /// A variable's path, like `"app.companies"`.
    From(String),
}

/// One written-out option: the words shown, and the value it picks (any JSON value).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    /// What it shows.
    pub label: String,
    /// What picking it sets the variable to.
    pub value: Value,
}

styled! {
    [placed]
    /// `type = "checkbox"`: its value is `true` or `false`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Checkbox {
        /// The node's name; its `.change` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The variable it reads and writes, outside a form: `app.NAME` or `sys.theme`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub var: Option<String>,
        /// Its name in the form it's in: its value goes in the form's draft, and out on `.submit`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// Its label; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
    }
}

styled! {
    [placed]
    /// `type = "toggle"`: a switch; its value is `true` or `false`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Toggle {
        /// The node's name; its `.change` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The variable it reads and writes, outside a form: `app.NAME` or `sys.theme`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub var: Option<String>,
        /// Its name in the form it's in: its value goes in the form's draft, and out on `.submit`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// Its label; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
    }
}

styled! {
    [placed]
    /// `type = "radio"`: one radio button. Every radio on one `var` is a group; picking one sets
    /// the variable to its `value`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Radio {
        /// The node's name; its `.change` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The variable its group reads and writes, outside a form: `app.NAME` or `sys.theme`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub var: Option<String>,
        /// Its group's name in the form it's in: its value goes in the form's draft, and out on `.submit`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// What picking it sets the variable to (any JSON value).
        pub value: Value,
        /// Its label; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
    }
}

impl Node {
    /// A control's `var`: the variable it reads and writes. `None` for every other type.
    pub fn var(&self) -> Option<&str> {
        self.control().and_then(|(var, _)| var)
    }

    /// A control's `name`: its field in the form it's in. `None` for every other type.
    pub fn field(&self) -> Option<&str> {
        self.control().and_then(|(_, name)| name)
    }

    /// A control's `var` and `name`; `None` when it isn't a control.
    #[allow(clippy::type_complexity)]
    pub(crate) fn control(&self) -> Option<(Option<&str>, Option<&str>)> {
        let (var, name) = match self {
            Node::Input(n) => (&n.var, &n.name),
            Node::Textarea(n) => (&n.var, &n.name),
            Node::Select(n) => (&n.var, &n.name),
            Node::Checkbox(n) => (&n.var, &n.name),
            Node::Toggle(n) => (&n.var, &n.name),
            Node::Radio(n) => (&n.var, &n.name),
            Node::File(n) => (&n.var, &n.name),
            _ => return None,
        };
        Some((var.as_deref(), name.as_deref()))
    }

    /// The steps of its `on_*` shortcuts.
    pub(crate) fn shortcuts(&self) -> impl Iterator<Item = &Step> {
        let steps = match self {
            Node::Button(n) => n.on_click.as_deref(),
            Node::Form(n) => n.on_submit.as_deref(),
            Node::Dialog(n) => n.on_close.as_deref(),
            _ => None,
        };
        steps.into_iter().flatten()
    }

    /// The nodes directly inside it: a cell's or a form's children.
    pub fn children(&self) -> &[Node] {
        match self {
            Node::Cell(n) => &n.children,
            Node::Form(n) => &n.children,
            Node::Dialog(n) => &n.children,
            _ => &[],
        }
    }
}

styled! {
    [placed]
    /// `type = "dialog"`: a modal `dialog`, open while its variable is anything but `false` or
    /// `null`. When the person closes it (Escape, or a click outside), `.close` fires and sets the
    /// variable `false`; setting the variable `false` closes it without firing anything.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Dialog {
        /// The node's name; its `.close` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The variable that opens it: `app.NAME`.
        pub open: String,
        /// A heading at its top; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        /// The nodes inside it.
        #[serde(default)]
        pub children: Vec<Node>,
        /// Shortcut for a handler on this dialog's `.close`, run after the variable is set `false`:
        /// a list of steps, or one step alone.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "one_or_more")]
        pub on_close: Option<Vec<Step>>,
    }
}

styled! {
    [placed]
    /// `type = "file"`: a file picker. Inside a form, with `name`, its value is a reference to the
    /// bytes the browser holds, `{ file, name, type, size }` (a list with `multiple`), never the
    /// bytes themselves ([`crate::file`]).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct File {
        /// The node's name; its `.change` fires under it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// Never set: a file works only inside a form. Read so the mistake is named.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub var: Option<String>,
        /// Its name in the form it's in.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// Its label; may hold templates.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub label: Option<String>,
        /// What the picker offers: `"image/*"`, `".pdf,.csv"`. Anything, unless it says otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub accept: Option<String>,
        /// Takes several files, as a list.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub multiple: bool,
        /// The biggest file it takes, in bytes: 100 MB unless it says otherwise; `0` for no limit.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_size: Option<u64>,
    }
}

styled! {
    [placed]
    /// `type = "form"`: an HTML `form`. Its controls with `name` fill its draft, and a `submit`
    /// button (or Enter in a box) fires `.submit` with the draft as `form`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Form {
        /// The node's name; its `.submit` fires under it, and its draft is kept by it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        /// The nodes inside it.
        #[serde(default)]
        pub children: Vec<Node>,
        /// A variable to start from (`app.contact`, or `item` inside an `each`): each control
        /// with `name` starts from its `/name`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<String>,
        /// Shortcut for a handler on this form's `.submit`: a list of steps, or one step alone.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "one_or_more")]
        pub on_submit: Option<Vec<Step>>,
    }
}

/// A node's `on_*` shortcut: a list of steps, or one step written alone (`on_click = "cheer"`, or
/// `on_click = "{{ props.on_open }}"` inside a component).
fn one_or_more<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<Step>>, D::Error> {
    let steps = match Value::deserialize(deserializer)? {
        Value::Array(steps) => steps,
        one => vec![one],
    };
    steps
        .into_iter()
        .map(|step| serde_json::from_value(step).map_err(|e| de::Error::custom(e.to_string())))
        .collect::<Result<_, _>>()
        .map(Some)
}

/// Steps passed in as a prop: a list of steps, or one step alone. A template among them is only
/// ever a prop passed on (`"{{ props.NAME }}"`), and that only as the whole value.
pub(crate) fn as_steps(value: &Value) -> Result<Vec<Step>, String> {
    let list = match value {
        Value::Array(steps) => steps.clone(),
        one => vec![one.clone()],
    };
    list.into_iter()
        .map(|step| match serde_json::from_value(step) {
            Ok(Step::Run(name)) if name.contains("{{") => {
                Err(format!("{name:?} isn't a step or a custom action's name"))
            }
            Ok(step) => Ok(step),
            Err(e) => Err(e.to_string()),
        })
        .collect()
}

/// The props a component runs as steps: in its own nodes' `on_*` shortcuts, or passed on to a
/// component it uses that runs them, each component looked at once (`seen`).
fn step_props(
    nodes: &[Node],
    components: &BTreeMap<String, Component>,
    seen: &mut BTreeSet<String>,
) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for node in nodes {
        for step in node.shortcuts() {
            if let Step::Run(name) = step
                && let Some(prop) = prop_steps(name)
            {
                found.insert(prop.to_string());
            }
        }
        if let Node::Use(used) = node
            && let Some(component) = components.get(&used.component)
            && seen.insert(used.component.clone())
        {
            let inner = step_props(&component.body, components, seen);
            seen.remove(&used.component);
            for (key, value) in &used.props {
                if let Some(prop) = value.as_str().and_then(prop_steps)
                    && inner.contains(key)
                {
                    found.insert(prop.to_string());
                }
            }
        }
        found.extend(step_props(node.children(), components, seen));
    }
    found
}

/// The prop a step runs, when the step is `"{{ props.NAME }}"`: the steps a component's caller
/// passed in ([spec 090](../../../specs/090_components.md)).
pub fn prop_steps(name: &str) -> Option<&str> {
    let inner = name.trim().strip_prefix("{{")?.strip_suffix("}}")?.trim();
    inner.strip_prefix("props.").filter(|p| is_plain(p))
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
    /// Starts a timer by its name, or starts it over ([spec 100](../../../specs/100_events.md), "Timers").
    Start {
        /// The timer's name.
        start: String,
    },
    /// Stops a timer by its name.
    Stop {
        /// The timer's name.
        stop: String,
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
    /// `[sources]`, `[connections]`, `[actions]`, and `[on]`, or, with a `[component]` table, is a
    /// component named by its file name. Handlers keep their order: `app.toml`'s
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
        let mut components: BTreeMap<String, Component> = BTreeMap::new();
        // Where each component is written, for its problems.
        let mut component_files: BTreeMap<String, String> = BTreeMap::new();
        // Components whose files didn't read: their uses aren't checked, past that problem.
        let mut broken = BTreeSet::new();
        let mut timers = BTreeMap::new();
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
            seen_files.insert(file_name.clone(), path.to_string());
            if file_name == "timers.toml" {
                match toml::from_str::<BTreeMap<String, Timer>>(text) {
                    Ok(read) => {
                        for (name, timer) in &read {
                            if !is_plain(name) {
                                problems.push(problem_in(
                                    path,
                                    format!("a timer's name is a plain name, like \"poll\", not {name:?}"),
                                ));
                            }
                            if timer.ms == 0 {
                                problems.push(problem_in(
                                    path,
                                    format!("the timer `{name}` needs `ms` of at least 1"),
                                ));
                            }
                        }
                        timers = read;
                    }
                    Err(e) => problems.push(toml_problem(path, text, &e)),
                }
                continue;
            }
            let is_component = path != "app.toml"
                && toml::from_str::<toml::Table>(text).is_ok_and(|t| t.contains_key("component"));
            if is_component {
                let name = file_name
                    .strip_suffix(".toml")
                    .unwrap_or(&file_name)
                    .to_string();
                if let Some(problem) = misplaced(path, text, true) {
                    problems.push(problem);
                    broken.insert(name);
                    continue;
                }
                match toml::from_str::<ComponentFile>(text) {
                    Ok(file) => {
                        component_files.insert(name.clone(), path.to_string());
                        components.insert(
                            name,
                            Component {
                                props: file.component.props,
                                optional: file.component.optional,
                                state: file.state,
                                body: file.body,
                            },
                        );
                    }
                    Err(e) => {
                        problems.push(toml_problem(path, text, &e));
                        broken.insert(name);
                    }
                }
                continue;
            }
            let read = if path == "app.toml" {
                toml::from_str::<App>(text).map(|app| {
                    top = Some(app);
                })
            } else {
                match misplaced(path, text, false) {
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
        if let Some(app) = &mut top {
            if !app.components.is_empty() {
                problems.push(problem_in(
                    "app.toml",
                    "a component is a file of its own, with a [component] table, not a table in app.toml".into(),
                ));
            }
            app.components = components;
            if !app.timers.is_empty() {
                problems.push(problem_in(
                    "app.toml",
                    "timers go in a file of their own, timers.toml, each a table named by the timer".into(),
                ));
            }
            app.timers = timers;
            let checks = Checks {
                file: "app.toml",
                components: &app.components,
                broken: &broken,
                component: None,
            };
            for (i, node) in app.body.iter().enumerate() {
                check_node(node, &body_place(i), false, &checks, &mut problems);
            }
            for (name, component) in &app.components {
                let checks = Checks {
                    file: &component_files[name],
                    components: &app.components,
                    broken: &broken,
                    component: Some(name),
                };
                check_component(component, &checks, &mut problems);
            }
            loops(&app.components, &component_files, &mut problems);
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
fn misplaced(path: &str, text: &str, component: bool) -> Option<ConfigError> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        let table = match trimmed {
            "[app]" => "[app]",
            _ if trimmed.starts_with("[app.") => "[app]",
            "[[body]]" if !component => "[[body]]",
            _ if !component && (trimmed.starts_with("[body") || trimmed.starts_with("[[body.")) => {
                "[[body]]"
            }
            _ => {
                offset += line.len();
                continue;
            }
        };
        let (line, column) = line_and_column(text, offset + (line.len() - line.trim_start().len()));
        return Some(ConfigError {
            file: Some(path.into()),
            message: format!(
                "{table} goes only in app.toml, at the top of the configs folder{}",
                if table == "[[body]]" {
                    ", or in a component"
                } else {
                    ""
                }
            ),
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

/// What [`check_node`] checks against: the file the node is written in, the app's components, and
/// the component it's inside, if any.
struct Checks<'a> {
    file: &'a str,
    components: &'a BTreeMap<String, Component>,
    /// Components whose files didn't read, already reported.
    broken: &'a BTreeSet<String>,
    component: Option<&'a str>,
}

/// What a component holds, through the components it uses.
#[derive(Default)]
struct Holds {
    /// A named control, a file picker, or a submit button outside a form of its own: it fills the
    /// form it's used in, so it needs one.
    form_parts: bool,
    /// A form.
    form: bool,
}

/// What `nodes` hold, through the components they use, each looked at once (`seen`), so a loop of
/// components ends.
fn holds(
    nodes: &[Node],
    components: &BTreeMap<String, Component>,
    seen: &mut BTreeSet<String>,
) -> Holds {
    let mut found = Holds::default();
    for node in nodes {
        match node {
            Node::Form(_) => found.form = true,
            Node::Button(b) if b.submit => found.form_parts = true,
            Node::Use(u) => {
                if let Some(c) = components.get(&u.component)
                    && seen.insert(u.component.clone())
                {
                    let inner = holds(&c.body, components, seen);
                    found.form_parts |= inner.form_parts;
                    found.form |= inner.form;
                }
            }
            _ if node.field().is_some() || matches!(node, Node::File(_)) => found.form_parts = true,
            _ => {}
        }
        if !matches!(node, Node::Form(_)) {
            let inner = holds(node.children(), components, seen);
            found.form_parts |= inner.form_parts;
            found.form |= inner.form;
        }
    }
    found
}

/// The checks for one component: its props, and that it reads nothing of the app's and writes none
/// of its props. Its body is checked as a body is, but without knowing whether it's used in a form;
/// each `use` of it checks that.
fn check_component(component: &Component, checks: &Checks, problems: &mut Vec<ConfigError>) {
    let mut problem = |message: String| problems.push(problem_in(checks.file, message));
    let mut named = BTreeSet::new();
    for prop in component.props.iter().chain(&component.optional) {
        if !is_plain(prop) {
            problem(format!(
                "a prop is a plain name, like \"contact\", not {prop:?}"
            ));
        } else if !named.insert(prop) {
            problem(format!("the prop `{prop}` is named twice"));
        }
    }
    let body = serde_json::to_value(&component.body).unwrap_or_default();
    for (i, node) in body.as_array().into_iter().flatten().enumerate() {
        scan(node, &body_place(i), None, component, &mut problem);
    }
    for (i, node) in component.body.iter().enumerate() {
        check_node(node, &body_place(i), false, checks, problems);
        reaches(node, &body_place(i), &mut |m| {
            problems.push(problem_in(checks.file, m))
        });
    }
}

/// Looks through a component's own steps for one that reaches past it: a custom action, a `call`,
/// a `reload`, a timer's `start` or `stop`, or a `log`. A component's steps work on its own `local.*`, run its props' steps, and fire
/// events; the app's handlers for those events can do anything ([spec 090](../../../specs/090_components.md)).
fn reaches(node: &Node, place: &str, problem: &mut impl FnMut(String)) {
    for step in node.shortcuts() {
        let what = match step {
            Step::Run(name) if prop_steps(name).is_none() && !name.contains("{{") => {
                format!("runs the custom action `{name}`")
            }
            Step::Call { call, .. } => format!("calls `{call}`"),
            Step::Reload { .. } => "reloads a source".to_string(),
            Step::Start { start } => format!("starts the timer `{start}`"),
            Step::Stop { stop } => format!("stops the timer `{stop}`"),
            Step::Log { .. } => "writes to the log".to_string(),
            _ => continue,
        };
        problem(format!(
            "{place}: a component's own steps only fire events, so this step, which {what}, won't work here; fire an event and handle it in the app, or take the steps as a prop"
        ));
    }
    for (i, child) in node.children().iter().enumerate() {
        reaches(child, &child_place(place, i), problem);
    }
}

/// The keys whose value is a variable's path rather than a template.
const PATH_KEYS: [&str; 8] = [
    "var", "each", "data", "open", "options", "set", "step", "toggle",
];
/// Of those, the ones that write the variable.
const WRITE_KEYS: [&str; 5] = ["var", "open", "set", "step", "toggle"];

/// Looks through a component's node, as JSON, for a read of `app.*` (a template, or a path key),
/// a write of `props.*`, and a `local.*` or `props.*` the component doesn't declare. `place` names
/// the node it's in; `key` is the key `value` is under.
fn scan(
    value: &Value,
    place: &str,
    key: Option<&str>,
    component: &Component,
    problem: &mut impl FnMut(String),
) {
    match value {
        Value::String(text) => {
            let head = |path: &str| path.split('.').next().unwrap_or_default().to_string();
            let mut whole: Vec<&str> = template::paths(text);
            if key.is_some_and(|k| PATH_KEYS.contains(&k)) {
                whole.push(text);
            }
            let paths: Vec<String> = whole.iter().map(|p| head(p)).collect();
            for path in &whole {
                let mut parts = path.split('.');
                match (parts.next(), parts.next()) {
                    (Some("local"), name)
                        if !name.is_some_and(|n| component.state.contains_key(n)) =>
                    {
                        problem(format!(
                            "{place}: {path} isn't declared in this component's [state]"
                        ));
                    }
                    (Some("props"), Some(name)) if !component.takes(name) => {
                        problem(format!("{place}: this component has no prop `{name}`"));
                    }
                    _ => {}
                }
            }
            if paths.iter().any(|h| h == "app") {
                problem(format!(
                    "{place}: a component sees no `app.*`, only its props, so {text:?} won't work here; pass it in as a prop"
                ));
            }
            if key.is_some_and(|k| WRITE_KEYS.contains(&k)) && head(text) == "props" {
                problem(format!(
                    "{place}: `props.*` is read-only, so a component can't write {text:?}"
                ));
            }
        }
        Value::Array(items) => {
            for item in items {
                scan(item, place, key, component, problem);
            }
        }
        Value::Object(fields) => {
            for (k, v) in fields {
                if k == "children" {
                    for (i, child) in v.as_array().into_iter().flatten().enumerate() {
                        scan(child, &child_place(place, i), None, component, problem);
                    }
                } else {
                    scan(v, place, Some(k), component, problem);
                }
            }
        }
        _ => {}
    }
}

/// A loop of components (A uses B uses A), once each, written in the file of the first one in it.
fn loops(
    components: &BTreeMap<String, Component>,
    files: &BTreeMap<String, String>,
    problems: &mut Vec<ConfigError>,
) {
    fn uses<'a>(nodes: &'a [Node], out: &mut Vec<&'a str>) {
        for node in nodes {
            if let Node::Use(u) = node {
                out.push(&u.component);
            }
            uses(node.children(), out);
        }
    }
    fn walk<'a>(
        name: &'a str,
        components: &'a BTreeMap<String, Component>,
        path: &mut Vec<&'a str>,
        done: &mut BTreeSet<&'a str>,
        found: &mut Vec<Vec<&'a str>>,
    ) {
        if let Some(at) = path.iter().position(|n| *n == name) {
            found.push(path[at..].to_vec());
            return;
        }
        if !done.insert(name) {
            return;
        }
        let Some(component) = components.get(name) else {
            return;
        };
        path.push(name);
        let mut next = Vec::new();
        uses(&component.body, &mut next);
        for used in next {
            walk(used, components, path, done, found);
        }
        path.pop();
    }
    let mut found = Vec::new();
    let mut done = BTreeSet::new();
    for name in components.keys() {
        walk(name, components, &mut Vec::new(), &mut done, &mut found);
    }
    for cycle in found {
        let mut chain: Vec<&str> = cycle.clone();
        chain.push(cycle[0]);
        problems.push(problem_in(
            &files[cycle[0]],
            format!(
                "components use each other in a loop: {}",
                chain.join(" uses ")
            ),
        ));
    }
}

/// Checks what one key can't: a node's keys that only make sense apart. `place` names the node,
/// since it may have no `id`; `in_form` says whether it's inside a `form`. Inside a component,
/// a control with `name` or a submit button is fine anywhere: the form it fills is where the
/// component is used.
fn check_node(
    node: &Node,
    place: &str,
    in_form: bool,
    checks: &Checks,
    problems: &mut Vec<ConfigError>,
) {
    let mut problem =
        |message: String| problems.push(problem_in(checks.file, format!("{place}: {message}")));
    let inside = checks.component.is_some();
    let in_form_here = in_form;
    let in_form = in_form || inside;
    if matches!(node, Node::File(_)) && (node.var().is_some() || !in_form) {
        problem("a `file` works only inside a `form`, with `name`".into());
    } else if node.control().is_some() {
        match (node.var(), node.field()) {
            (Some(_), Some(_)) => {
                problem("a control takes `name` (inside a form) or `var`, not both".into())
            }
            (None, None) => problem("a control needs `var`, or `name` inside a form".into()),
            (None, Some(_)) if !in_form => {
                problem("`name` works inside a `form`; outside one, a control takes `var`".into())
            }
            (None, Some(name)) if !is_plain(name) => problem(format!(
                "`name` is a plain name, like \"email\", not {name:?}"
            )),
            _ => {}
        }
    }
    for step in node.shortcuts() {
        if let Step::Run(name) = step
            && name.contains("{{")
        {
            if !inside {
                problem(format!(
                    "a step {name:?} runs steps passed into a component, so it works only inside one"
                ));
            } else if prop_steps(name).is_none() {
                problem(format!(
                    "a step can run a prop's steps as \"{{{{ props.NAME }}}}\", not {name:?}"
                ));
            }
        }
    }
    if let Node::Button(button) = node
        && button.submit
        && !in_form
    {
        problem("`submit` works on a button inside a `form`".into());
    }
    if let Node::Dialog(dialog) = node
        && !is_writable_path(&dialog.open)
        && !(inside && (dialog.open.starts_with("props.") || is_local_path(&dialog.open)))
    {
        problem(format!(
            "a dialog's `open` is an app variable, like \"app.editing\", not {:?}",
            dialog.open
        ));
    }
    if let Node::Form(form) = node {
        if in_form_here {
            problem("a form can't hold another form".into());
        }
        if let Some(data) = form.data.as_deref().filter(|d| !is_list_path(d, inside)) {
            problem(format!(
                "`data` is a variable's path, like \"app.contact\" or \"item\", not {data:?}"
            ));
        }
    }
    if node.var().is_some_and(|var| {
        !is_writable_path(var) && !(inside && (var.starts_with("props.") || is_local_path(var)))
    }) {
        problem(format!(
            "`var` is an app variable, like \"app.query\", or sys.theme, not {:?}",
            node.var().unwrap_or_default()
        ));
    }
    if let Node::Select(select) = node {
        match &select.options {
            Options::From(path) if !is_list_path(path, inside) => problem(format!(
                "`options` is a list, or a variable's path like \"app.companies\", not {path:?}"
            )),
            Options::From(_) => {}
            Options::List(_) => {
                for (key, set) in [
                    ("option_label", select.option_label.is_some()),
                    ("option_value", select.option_value.is_some()),
                ] {
                    if set {
                        problem(format!("`{key}` goes with `options` from a variable"));
                    }
                }
            }
        }
    }
    if let Node::Cell(cell) = node {
        if cell.flow.is_some() && cell.grid.is_some() {
            problem("a cell lays out by `flow` or by `grid`, not both".into());
        }
        match &cell.each {
            Some(path) if !is_list_path(path, inside) => problem(format!(
                "`each` is a variable's path, like \"app.contacts\" or \"item.tags\", not {path:?}"
            )),
            Some(_) => {}
            None => {
                for (key, set) in [
                    ("empty", cell.empty.is_some()),
                    ("error", cell.error.is_some()),
                ] {
                    if set {
                        problem(format!("`{key}` goes with `each`"));
                    }
                }
            }
        }
    }
    if let Node::Use(used) = node {
        match checks.components.get(&used.component) {
            None if checks.broken.contains(&used.component) => {}
            None => problem(format!(
                "there's no component named {:?}; a component is a .toml file with a [component] table, named by its file name",
                used.component
            )),
            Some(component) => {
                for prop in &component.props {
                    if !used.props.contains_key(prop) {
                        problem(format!("`{}` needs the prop `{prop}`", used.component));
                    }
                }
                for prop in used.props.keys().filter(|p| !component.takes(p)) {
                    problem(format!("`{}` takes no prop `{prop}`", used.component));
                }
                let mut seen = BTreeSet::from([used.component.clone()]);
                for prop in step_props(&component.body, checks.components, &mut seen) {
                    let Some(value) = used.props.get(&prop) else {
                        continue;
                    };
                    let passed_on = inside && value.as_str().and_then(prop_steps).is_some();
                    if let (false, Err(e)) = (passed_on, as_steps(value)) {
                        problem(format!(
                            "`{}` runs `{prop}` as steps, so pass a list of steps, like [{{ go = \"contact\" }}]: {e}",
                            used.component
                        ));
                    }
                }
                let mut seen = BTreeSet::from([used.component.clone()]);
                let held = holds(&component.body, checks.components, &mut seen);
                if held.form_parts && !in_form {
                    problem(format!(
                        "`{}` holds a control with `name`, a file picker, or a submit button, which fill a form, so use it inside a `form`",
                        used.component
                    ));
                }
                if held.form && in_form_here {
                    problem(format!(
                        "`{}` holds a form, and a form can't hold another form",
                        used.component
                    ));
                }
            }
        }
    }
    let in_form = in_form_here || matches!(node, Node::Form(_));
    for (i, child) in node.children().iter().enumerate() {
        check_node(child, &child_place(place, i), in_form, checks, problems);
    }
}

/// Whether `name` is a plain name: letters, digits, `_`, and `-`.
fn is_plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Whether `path` names a variable a control may write: `app.NAME` or `sys.theme`, the ones a
/// `set` takes.
fn is_writable_path(path: &str) -> bool {
    match path.split_once('.') {
        Some(("app", name)) => {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        }
        _ => path == "sys.theme",
    }
}

/// Whether `path` is `local.NAME`: one of a component's own variables, which its controls and
/// dialogs may write. Whether the component declares it is checked with the rest of its paths.
fn is_local_path(path: &str) -> bool {
    path.strip_prefix("local.").is_some_and(is_plain)
}

/// Whether `path` names a variable an `each` can repeat over: `item`, or a dotted path under
/// `app.` or `item.` of plain names, and `inside` a component, under `props.` or `local.` too.
fn is_list_path(path: &str, inside: bool) -> bool {
    let mut parts = path.split('.');
    let plain = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    match parts.next() {
        Some("item") => parts.all(plain),
        Some(root) if root == "app" || (inside && matches!(root, "props" | "local")) => {
            let rest: Vec<&str> = parts.collect();
            !rest.is_empty() && rest.into_iter().all(plain)
        }
        _ => false,
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

/// The step kinds, keyed by the TOML key that names each one.
const STEP_KINDS: [&str; 9] = [
    "call", "reload", "set", "step", "toggle", "fire", "log", "start", "stop",
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
        if let Some(gone) = ["delay", "every", "cancel"]
            .into_iter()
            .find(|k| fields.contains_key(*k))
        {
            return Err(de::Error::custom(format!(
                "there's no `{gone}` step: a timer is named in timers.toml, and `start` and `stop` run it"
            )));
        }
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
            "start" => Step::Start {
                start: serde_json::from_value::<steps::Start>(fields)
                    .map_err(wrong)?
                    .start,
            },
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
            _ => Step::Stop {
                stop: serde_json::from_value::<steps::Stop>(fields)
                    .map_err(wrong)?
                    .stop,
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
    pub struct Start {
        pub start: String,
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
    pub struct Stop {
        pub stop: String,
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
    pub fn yes() -> bool {
        true
    }
    pub fn theme() -> String {
        "light".into()
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
