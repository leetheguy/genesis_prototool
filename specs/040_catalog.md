# 040 — Catalog

Status: draft
Created: 2026-11-21

The catalog is the closed set of node types. Each one is a variant of the `Node` enum in
genesis-core (tagged by `type`) and a Leptos component in genesis-ui, drawn with daisyUI. Adding a
type takes a log entry here (or a new spec) and code. A config can't invent a type; it can only
compose them into partials ([090](090_components.md)).

## Keys every node takes

| Key | Meaning |
|---|---|
| `id` | the node's name: it fires its events under it (`save_btn.click`), and its local state and form draft survive reloads by it |
| `show` | a condition; the node and its children are drawn only while it holds ([080](080_display_logic.md)) |
| `state` | local variables for this node and its children |
| `variant` | one of the type's own style names. Never a CSS class |

## Nodes that show data

A node with `data = "app.contacts"` (a variable, not an operation) draws that value, and follows it:

- While the source is `loading`, the node shows daisyUI's loading state in its own place.
- When it's `failed`, the node shows a loud error alert in its place: the operation, the status, and the message.
  `error = "..."` adds a line of its own on top; it can't hide the detail.
- When the value is `null` or `[]`, the node shows `empty = "..."` (default "Nothing here.").

## Layout

| Type | Takes | Draws |
|---|---|---|
| `stack` | `children`, `gap` (`"sm"`, `"md"`, `"lg"`) | children top to bottom |
| `row` | `children`, `gap`, `wrap` | children side by side, wrapping on narrow screens |
| `card` | `children`, `title`, `variant` (`"plain"`, `"bordered"`) | a card |
| `tabs` | `var`, `tabs` (`[{ label, value }]`) | tab buttons that set `var`; the content is any nodes with `show` on it |
| `modal` | `children`, `open` (a variable), `title` | a dialog while `open` is true |
| `use` | `component`, `props` | a partial ([090](090_components.md)) |

## Content

| Type | Takes | Draws |
|---|---|---|
| `heading` | `text`, `level` (1–3) | a heading |
| `text` | `text`, `format`, `variant` (`"body"`, `"muted"`, `"code"`) | a paragraph; `text` may hold templates |
| `markdown` | `text` | Markdown, drawn safely (no raw HTML), for streamed AI answers among other things |
| `stat` | `label`, `data`, `field`, `format` | one value, large |
| `badge` | `text`, `variant` (`"neutral"`, `"info"`, `"success"`, `"warning"`, `"error"`) | a small label |
| `image` | `src`, `alt` | an image from a URL |

## Data

| Type | Takes | Draws |
|---|---|---|
| `table` | `data`, `columns` (`label`, `field`, `format`), `row_actions` (Buttons) | the list in `data`, one row per element |
| `list` | `data`, `children` | `children` once per element, with `row.*` set to the element |
| `detail` | `data`, `fields` (`label`, `field`, `format`) | label and value pairs from one object |
| `form` | `fields`, `data` (to start from), `submit_label` | a form ([080](080_display_logic.md) for its checks) |

Values with no `format` are shown as text: strings as they are, numbers and booleans in their JSON
form, `null` as empty, and arrays and objects as compact JSON.

**Form fields** take `name`, `label`, `kind`, `placeholder`, `disabled` (a condition), checks
([080](080_display_logic.md)), and, for `select`, `options` (`[{ label, value }]`, or `options =
"app.tags"` with `option_label` and `option_value` pointers). The kinds are `text`, `textarea`, `password`,
`number`, `checkbox`, `toggle`, `select`, and `date`. Fields start from `data`'s `/name` when it's
given. On submit, their values are the event's `form` payload.

## Input bound to a variable

| Type | Takes | Draws |
|---|---|---|
| `input` | `var`, `kind` (as a form field), `label`, `options` | one control that reads and writes a variable directly, with no Form and no submit: a search box, a filter, a theme picker on `sys.theme` |
| `button` | `label`, `disabled`, `variant` (`"primary"`, `"ghost"`, `"danger"`) | a button |

## Events each node fires

A node with an `id` fires these ([100](100_events.md)); `on_click`, `on_change`, `on_submit`, and `on_row` are shortcuts that wire them inline.

| Type | Events | Payload |
|---|---|---|
| `button` | `.click` | — |
| `input`, each form field | `.change` | `value` |
| `form` | `.submit` (only when its checks pass), `.invalid` | `form` |
| `table`, `list` | `.row_click` | `row` |
| `tabs` | `.change` | `value` |
| `modal` | `.close` | — |

Inside a row, a row Button's events carry that row as `row` too.

## How a component is built

Rust has no classes, but it has the same idea. Every component is built the same way:

- **The `Component` trait** (Rust's word for an interface that can carry shared behavior). A component
  declares its props (a struct serde reads from the node's keys), the events it can fire, and one
  `view` function that draws it.
- **One shared wrapper, `Wired`**, around every component. It plays the base class's part and knows all the
  wiring: the `id`, `show`, `state` (locals), the loading, failed, and empty faces for `data`, reading
  variables, and firing the component's events onto the bus. A component never touches the bus, a handler,
  or the state machine itself. It reads the values the wrapper hands it, and calls `fire("click", payload)`.

So a new component is a small, regular piece of Rust: its props, its events, its `view`. An agent can write
one by following the existing ones. It's Rust, not JavaScript, and it ships in a build (a commit and a
rebuild), not through dufs. Runtime reuse is TOML partials ([090](090_components.md)).

## Tests

- every type loads with its keys and draws its daisyUI component
- a data node shows loading, the loud failure (with detail even when `error` is set), `empty`, and its data, as its source moves through each status
- a Table and a List draw one row per element; `on_row` and a row Button see their own row as `row.*`
- value display with no `format`: string, number, boolean, `null`, array, object
- `tabs` sets its variable; `modal` opens and closes with its variable
- `markdown` draws Markdown and escapes raw HTML
- a Form starts from `data`; its `.submit` event carries each field in `form`
- `input` reads and writes its variable on every change; one on `sys.theme` changes the theme
- each type fires its events with its payload; one without an `id` (and no `on_*`) fires nothing
- the wrapper handles `show`, locals, and the data faces the same way for every type
- a `variant` outside the type's list is a config error

## Tasks

The first milestone ([999](999_todo.md)), after the bus ([100](100_events.md)):

- [ ] genesis-ui: a Leptos app that draws a config from the state machine, with daisyUI 5 built ahead of time
- [x] genesis-ui: the `Component` trait and the `Wired` wrapper (naming by `id` or place, reading state, firing events; `show` comes with conditions, [080](080_display_logic.md))
- [x] components: `stack`, `heading`, `text` (with templates), and `button`
