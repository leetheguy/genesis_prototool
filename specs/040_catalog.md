# 040 — Catalog

Status: draft
Created: 2026-11-21

The catalog is the closed set of node types. Each one is a variant of the `Node` enum in
genesis-core (tagged by `type`) and a Leptos component in genesis-ui, drawn with daisyUI. Adding a
type takes a log entry here (or a new spec) and code. A config can't invent a type; it can only
compose them into partials ([090](090_components.md)).

**Every type is atomic.** Each draws one thing: a layout, a piece of content, or one control. There
are no composites in the catalog: a data table with search and paging, a sign-in form, or a contact
form is built from these parts, by an agent or as a partial from the composite library
([999](999_todo.md)). If a screen can't be built from the parts, the missing part is a new atom, never a
composite.

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

`list`, `table`, `stat`, and `form` take `data`. The `Wired` wrapper draws these faces the same way for every one.

## Layout

| Type | Takes | Draws |
|---|---|---|
| `stack` | `children`, `gap` (`"sm"`, `"md"`, `"lg"`) | children top to bottom |
| `row` | `children`, `gap`, `wrap`, `align` (`"start"`, `"center"`, `"end"`, `"between"`) | children side by side, wrapping on narrow screens |
| `grid` | `children`, `cols` (1–6), `gap` | children in equal columns, one column on narrow screens |
| `card` | `children`, `title`, `variant` (`"plain"`, `"bordered"`) | a card |
| `divider` | `text` | a dividing line, with optional text on it |
| `tabs` | `var`, `tabs` (`[{ label, value }]`) | tab buttons that set `var`; the content is any nodes with `show` on it |
| `modal` | `children`, `open` (a variable), `title` | a dialog while `open` is true; closing it sets `open` to false |
| `use` | `component`, `props` | a partial ([090](090_components.md)) |

## Content

| Type | Takes | Draws |
|---|---|---|
| `heading` | `text`, `level` (1–3) | a heading |
| `text` | `text`, `format`, `variant` (`"body"`, `"muted"`, `"code"`) | a paragraph; `text` may hold templates |
| `badge` | `text`, `variant` (`"neutral"`, `"info"`, `"success"`, `"warning"`, `"error"`) | a small label |
| `alert` | `text`, `variant` (as `badge`) | a message box across its space |
| `stat` | `label`, `data`, `field`, `format` | one value, large |
| `link` | `text`, `href` | a link; `href` may hold templates, and outside links open in a new tab |
| `image` | `src`, `alt` | an image from a URL |
| `loading` | — | a spinner, for a node that waits on something other than `data` |
| `markdown` | `text` | Markdown, drawn safely (no raw HTML), for streamed AI answers among other things |

## Repeating

| Type | Takes | Draws |
|---|---|---|
| `list` | `data`, `children` | `children` once per element, with `row.*` set to the element |
| `table` | `data`, `columns` (`[{ label, children }]`) | a table: a header of labels, then one row per element, each cell drawing its `children` with `row.*` set |

A `table` is a layout, like a `grid` with a header: what's in a cell is ordinary nodes (`text` with a
`format`, a `badge`, a `button`). Paging, search, sorting, and row actions are other atoms wired around it
([999](999_todo.md) plans the data-table composite).

Values with no `format` are shown as text: strings as they are, numbers and booleans in their JSON
form, `null` as empty, and arrays and objects as compact JSON.

## Input

| Type | Takes | Draws |
|---|---|---|
| `form` | `children`, `data` (to start from) | a form: gathers the `input`s inside it, and submits them together |
| `input` | `name` or `var`, `kind`, `label`, `placeholder`, `disabled`, checks, `options` | one control |
| `button` | `label`, `disabled`, `variant` (`"primary"`, `"ghost"`, `"danger"`), `submit` | a button |

An **`input`** works one of two ways:

- **Inside a `form`, with `name`:** its value is the form's draft, and goes in the form's `form` payload on submit.
  It starts from `data`'s `/name` when the form has `data`.
- **Anywhere, with `var`:** it reads and writes that variable directly, with no submit: a search box, a filter,
  a theme picker on `sys.theme`.

Its `kind` is `text`, `email`, `textarea`, `password`, `number`, `checkbox`, `toggle`, `select`, or `date`.
A `select` takes `options` (`[{ label, value }]`, or `options = "app.tags"` with `option_label` and
`option_value` pointers). Checks are in [080](080_display_logic.md).

A **`button`** with `submit = true` submits the form it's in. The form fires `.submit` only when every
check inside it passes; otherwise `.invalid`, and each failing input shows its message.

## Events each node fires

A node with an `id` fires these ([100](100_events.md)); `on_click`, `on_change`, `on_submit`, and `on_row` are shortcuts that wire them inline.

| Type | Events | Payload |
|---|---|---|
| `button` | `.click` | — |
| `input` | `.change` | `value` |
| `form` | `.submit` (only when its checks pass), `.invalid` | `form` |
| `table`, `list` | `.row_click` | `row` |
| `tabs` | `.change` | `value` |
| `modal` | `.close` | — |

Inside a row, a node's events carry that row as `row` too.

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
- a `list` and a `table` draw one row per element; a node in a row (a cell's `button`, `on_row`) sees its own row as `row.*`
- a `table` draws a header of its column labels, and each cell's `children`
- value display with no `format`: string, number, boolean, `null`, array, object
- `grid` draws `cols` columns; `row` aligns and wraps
- `tabs` sets its variable; `modal` opens with its variable, and closing it sets the variable false and fires `.close`
- `link` opens an outside URL in a new tab; `alert` and `badge` take only their variants
- `markdown` draws Markdown and escapes raw HTML
- a `form` starts its inputs from `data`; a `submit` button fires `.submit` with each named input in `form`
- a `form` with a failing check fires `.invalid`, not `.submit`, and shows each message
- an `input` with `var` reads and writes its variable on every change; one on `sys.theme` changes the theme
- each `input` kind draws its control and gives its value its JSON type (a number for `number`, a boolean for `checkbox`)
- an `input` with both `name` and `var`, or with `name` outside a `form`, is a config error
- each type fires its events with its payload; one without an `id` (and no `on_*`) fires nothing
- the wrapper handles `show`, locals, and the data faces the same way for every type
- a `variant` outside the type's list is a config error

## Tasks

The first milestone ([999](999_todo.md)), after the bus ([100](100_events.md)):

- [x] genesis-ui: a Leptos app that draws a config from the state machine, with daisyUI 5 built ahead of time
- [x] genesis-ui: the `Component` trait and the `Wired` wrapper (naming by `id` or place, reading state, firing events; `show` comes with conditions, [080](080_display_logic.md))
- [x] components: `stack`, `heading`, `text` (with templates), and `button`

Milestone three ([999](999_todo.md)), each with its tests:

- [ ] layout: `row`, `grid`, `card`, `divider`
- [ ] content: `badge`, `alert`, `link`, `image`, `loading`
- [ ] the data faces in `Wired`, then `list` and `stat`
- [ ] `table`
- [ ] `input` with `var`, every kind
- [ ] `form`, `input` with `name`, and the `submit` button
- [ ] `tabs` and `modal`
- [ ] `markdown`
