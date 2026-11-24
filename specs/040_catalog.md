# 040 — Catalog

Status: draft
Created: 2026-11-21

The catalog is the closed set of node types. Each one is a variant of the `Node` enum in
genesis-core (tagged by `type`) and a Leptos component in genesis-ui, drawn with daisyUI. Adding a
type takes a log entry here (or a new spec) and code. A config can't invent a type; it can only
compose them into components ([090](090_components.md)).

**Every type is an atom**, as small as plain HTML ([120](120_one_app.md)). A `cell` is a `div`: it lays
out and places what's in it, and that's all. Everything with behavior of its own (a link, a button,
each form control, a form, a dialog) is its own atom, drawn as the real HTML element, so keyboards,
screen readers, autofill, and phones all work. A stack, a row, a grid, a card, a table, tabs, a badge:
each is cells and text laid out a certain way, built by an agent or taken from the composite library
([999](999_todo.md)). If a screen can't be built from the atoms, the missing piece is a new atom,
never a composite.

## Keys every node takes

| Key | Meaning |
|---|---|
| `id` | the node's name: it fires its events under it (`save_btn.click`), and its local state and form draft survive reloads by it |
| `show` | a condition; the node and its children are drawn only while it holds ([080](080_display_logic.md)) |
| `state` | local variables for this node and its children |
| placement | `span`, `offset`, `grow`, and `width`: where the node sits in its parent (below) |

## The atoms

| Type | Draws | Takes |
|---|---|---|
| `cell` | `div` | `children`, `each`, the style keys, `on_click` |
| `text` | `p`, or `h1`–`h3` with `level` | `text` (with templates), `format`, `level`, the type and box keys |
| `image` | `img` | `src`, `alt` |
| `link` | `a` | `text`, `href` (with templates); an outside link opens in a new tab |
| `button` | `button` | `label`, `disabled`, `submit`, `variant`, `size` |
| `input` | `input` | `name` or `var`, `kind` (`text`, `email`, `password`, `number`, `date`, `search`), `label`, `placeholder`, `disabled`, checks |
| `textarea` | `textarea` | as `input`, without `kind`, plus `rows` |
| `select` | `select` | as `input`, without `kind`, plus `options` |
| `checkbox` | a checkbox | `name` or `var`, `label`, `disabled` |
| `toggle` | a switch | as `checkbox` |
| `form` | `form` | `children`, `data` (to start from) |
| `dialog` | `dialog` | `children`, `open` (a variable), `title` |
| `markdown` | Markdown, drawn safely (no raw HTML) | `text` |
| `use` | a component ([090](090_components.md)) | `component`, `props` |

A `text` with `level` is a heading (`h1`–`h3`) for screen readers, with a heading's size and weight
unless its own keys say otherwise. `markdown` is for longer prose, and for streamed AI answers later.

### Repeating: `each`

A cell with `each = "app.contacts"` (a variable holding a list) draws its children once per element,
with `item.*` set to that element:

```toml
[[body.children]]
type = "cell"
each = "app.contacts"
flow = "column"
gap = 2

[[body.children.children]]
type = "text"
text = "{{ item.name }} at {{ item.company }}"
```

It follows the variable:

- While its source is `loading`, the cell shows daisyUI's loading state in its own place.
- When it's `failed`, the cell shows a loud error in its place: the operation, the status, and the message.
  `error = "..."` adds a line of its own on top; it can't hide the detail.
- When the list is `null` or `[]`, the cell shows `empty = "..."` (default "Nothing here.").

The `Wired` wrapper draws these faces, so they look the same everywhere.

### Controls

A control (`input`, `textarea`, `select`, `checkbox`, `toggle`) works one of two ways:

- **Inside a `form`, with `name`:** its value is the form's draft, and goes in the form's `form` payload on submit.
  It starts from `data`'s `/name` when the form has `data`.
- **Anywhere, with `var`:** it reads and writes that variable directly, with no submit: a search box, a filter,
  a theme picker on `sys.theme`.

A `select` takes `options` (`[{ label, value }]`, or `options = "app.companies"` with `option_label` and
`option_value` pointers). Checks are in [080](080_display_logic.md). A value keeps its JSON type: a
number from a `number` input, a boolean from a `checkbox`.

A `button` with `submit = true` submits the form it's in. The form fires `.submit` only when every check
inside it passes; otherwise `.invalid`, and each failing control shows its message.

`button` takes `variant` (`"primary"`, `"secondary"`, `"accent"`, `"neutral"`, `"ghost"`, `"link"`, `"error"`)
and `size` (`"xs"`, `"sm"`, `"md"`, `"lg"`), daisyUI's own. Controls are drawn as daisyUI's controls.

Values in text with no `format` are shown as text: strings as they are, numbers and booleans in their
JSON form, `null` as empty, and arrays and objects as compact JSON.

## Style keys

Cells (and, for type and box, `text`) take a closed set of style keys. Each value is a word or number
Genesis knows, and Genesis turns it into Tailwind classes, built ahead of time. Colors are daisyUI's
theme colors, so every theme recolors every screen. There are no CSS classes in the TOML.

**Layout**, for a cell's children:

| Key | Values | Does |
|---|---|---|
| `flow` | `"column"` (default), `"row"` | lays children top to bottom, or side by side |
| `grid` | `1`–`12`, or `"fit"` | lays children in a grid of that many equal columns, filling rows left to right; `"fit"` fits as many columns of at least 6rem as there's room for |
| `gap` | spacing | space between children |
| `align` | `"start"`, `"center"`, `"end"`, `"stretch"` | lines children up across the flow |
| `justify` | `"start"`, `"center"`, `"end"`, `"between"`, `"around"` | spreads children along the flow |
| `wrap` | `true` | lets a row wrap |

**Placement**, for any node, in its parent:

| Key | Values | Does |
|---|---|---|
| `span` | `1`–`12`, or `"full"` | how many grid columns the node covers |
| `offset` | `1`–`11` | how many grid columns to skip before it |
| `grow` | `true` | takes the room left over in a row |
| `width` | `"full"`, `"fit"`, `"1/2"`, `"1/3"`, `"2/3"`, `"1/4"`, `"3/4"` | its width |
| `max_width` | `"sm"`, `"md"`, `"lg"`, `"xl"`, `"2xl"`, `"4xl"`, `"6xl"`, `"prose"` | caps its width |

**Box**:

| Key | Values | Does |
|---|---|---|
| `pad`, `pad_x`, `pad_y` | spacing | space inside |
| `margin`, `margin_x`, `margin_y` | spacing, or `"auto"` | space outside (`margin_x = "auto"` centers) |
| `bg` | a color | the background |
| `border` | `true`, or a color | a border |
| `radius` | `"none"`, `"sm"`, `"md"`, `"lg"`, `"xl"`, `"full"`, `"box"` | rounded corners; `"box"` is the theme's own |
| `shadow` | `"none"`, `"sm"`, `"md"`, `"lg"` | a shadow |
| `height` | `"full"`, `"screen"` | its height |
| `scroll` | `true` | scrolls what doesn't fit |

**Type**, for a cell's text and `text`:

| Key | Values |
|---|---|
| `size` | `"xs"`, `"sm"`, `"base"`, `"lg"`, `"xl"`, `"2xl"`, `"3xl"`, `"4xl"` |
| `weight` | `"normal"`, `"medium"`, `"semibold"`, `"bold"` |
| `color` | a color, or `"muted"` |
| `text_align` | `"left"`, `"center"`, `"right"` |
| `font` | `"sans"`, `"mono"` |

**Spacing** is Tailwind's scale: `0`, `1`, `2`, `3`, `4`, `5`, `6`, `8`, `10`, `12`, `16` (each step is 0.25rem).
**Colors** are daisyUI's: `"base-100"`, `"base-200"`, `"base-300"`, `"base-content"`, `"neutral"`,
`"primary"`, `"secondary"`, `"accent"`, `"info"`, `"success"`, `"warning"`, `"error"`, and each one's
`-content` (the text color that reads on it).

**Screen sizes.** Any style key can take a table by screen width instead of one value, smallest first, as
Tailwind does it: `base` for every screen, then `sm` (40rem and up), `md` (48rem), `lg` (64rem), `xl` (80rem).

```toml
[[body]]
type = "cell"
grid = 12
gap = 4

[[body.children]]
type = "cell"
span = { base = 12, md = 6, lg = 4 }    # full width on a phone, half on a tablet, a third on a desktop
pad = 4
bg = "base-200"
radius = "box"
```

A value outside a key's list is a config error naming the key and the values it takes.

## Events each node fires

A node with an `id` fires these ([100](100_events.md)); `on_click`, `on_change`, and `on_submit` are shortcuts that wire them inline.

| Type | Events | Payload |
|---|---|---|
| `button`, `cell` | `.click` | — |
| a control | `.change` | `value` |
| `form` | `.submit` (only when its checks pass), `.invalid` | `form` |
| `dialog` | `.close` | — |

Inside an `each`, a node's events carry that element as `item` too. A cell fires `.click` only when
it's wired, and then it's drawn as something you can click and reach with the keyboard.

## How a component is built

Rust has no classes, but it has the same idea. Every catalog component is built the same way:

- **The `Component` trait** (Rust's word for an interface that can carry shared behavior). A component
  declares its props (a struct serde reads from the node's keys), the events it can fire, and one
  `view` function that draws it.
- **One shared wrapper, `Wired`**, around every component. It plays the base class's part and knows all the
  wiring: the `id`, `show`, `state` (locals), placement, the faces for `each`, reading variables, and firing
  the component's events onto the bus. A component never touches the bus, a handler, or the state machine
  itself. It reads the values the wrapper hands it, and calls `fire("click", payload)`.

So a new atom is a small, regular piece of Rust: its props, its events, its `view`. It ships in a build
(a commit and a rebuild), not through dufs. Runtime reuse is TOML components ([090](090_components.md)).

## Tests

- every type loads with its keys and draws its HTML element, styled by daisyUI where it has a style
- each style key gives its classes, and every class it can give is in the built CSS
- a style key given a table by screen size gives each size's class with its prefix
- a style key value outside its list is a config error naming the key and its values
- `grid = 12` with children of `span = 4` lays them three to a row; `offset` skips columns; `"fit"` fits columns
- a `text` with `level` is a heading element
- a cell with `each` draws its children once per element, each seeing its own element as `item.*`
- a cell with `each` shows loading, the loud failure (with detail even when `error` is set), `empty`, and its children, as its source moves through each status
- value display with no `format`: string, number, boolean, `null`, array, object
- `dialog` opens with its variable, and closing it sets the variable false and fires `.close`
- `link` opens an outside URL in a new tab
- `markdown` draws Markdown and escapes raw HTML
- a `form` starts its controls from `data`; a `submit` button fires `.submit` with each named control in `form`
- a `form` with a failing check fires `.invalid`, not `.submit`, and shows each message
- a control with `var` reads and writes its variable on every change; a `select` on `sys.theme` changes the theme
- each control gives its value its JSON type
- a control with both `name` and `var`, or with `name` outside a `form`, is a config error
- a wired cell fires `.click`, and can be reached and pressed with the keyboard
- each type fires its events with its payload; one without an `id` (and no `on_*`) fires nothing
- the wrapper handles `show`, locals, placement, and the `each` faces the same way for every type

## Tasks

The first milestone ([999](999_todo.md)), after the bus ([100](100_events.md)):

- [x] genesis-ui: a Leptos app that draws a config from the state machine, with daisyUI 5 built ahead of time
- [x] genesis-ui: the `Component` trait and the `Wired` wrapper (naming by `id` or place, reading state, firing events; `show` comes with conditions, [080](080_display_logic.md))
- [x] components: `stack`, `heading`, `text` (with templates), and `button`

Milestone three ([999](999_todo.md)), each with its tests:

- [ ] `cell` with the layout and box keys, replacing `stack`; `text` with `level` and the type keys, replacing `heading` ([120](120_one_app.md))
- [ ] screen sizes for every style key, and the whole style vocabulary in the built CSS
- [ ] placement keys on every node
- [ ] `image` and `link`; `button`'s variants and sizes
- [ ] `each`, with its faces in `Wired`, and `item.*`
- [ ] controls with `var`: `input`, `textarea`, `select`, `checkbox`, `toggle`
- [ ] `form`, controls with `name`, and the `submit` button
- [ ] `dialog`
- [ ] `markdown`
