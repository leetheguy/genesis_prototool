# 040 — Catalog

Status: draft
Created: 2026-11-21

The catalog is the closed set of node types. Each one is a variant of the `Node` enum in
genesis-core (tagged by `type`) and a Leptos component in genesis-ui, drawn with daisyUI. Adding a
type takes a log entry here (or a new spec) and code. A config can't invent a type; it can only
compose them into components ([090](090_components.md)).

**Every type is an atom**, as small as plain HTML ([120](120_one_app.md)). A `cell` is a `div`: it lays
out and places what's in it, and that's all. Everything with behavior of its own (a link, a button,
each form control, a form, a dialog, a video) is its own atom, drawn as the real HTML element, so keyboards,
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
| `image` | `img` | `src` (with templates), `alt` (required: `""` for decoration), the box keys |
| `svg` | inline `svg`, checked at load | `svg` (the markup), `label`, the type keys |
| `divider` | `hr` | `color` (a theme color), the box keys |
| `audio` | `audio` | `src`, `controls`, `autoplay`, `loop`, `muted` |
| `video` | `video` | as `audio`, plus `poster` and the box keys |
| `iframe` | `iframe`, sandboxed | `src`, `title`, `sandbox`, the box keys |
| `link` | `a` | `text`, `href` (with templates); an outside link (`http://`, `https://`, or `//`) opens in a new tab |
| `button` | `button` | `label`, `disabled`, `submit`, `variant`, `size` |
| `input` | `input` | `name` or `var`, `kind` (`text`, `email`, `password`, `number`, `date`, `search`), `label`, `placeholder`, `disabled`, checks |
| `textarea` | `textarea` | as `input`, without `kind`, plus `rows` |
| `select` | `select` | as `input`, without `kind`, plus `options` |
| `checkbox` | a checkbox | `name` or `var`, `label`, `disabled` |
| `toggle` | a switch | as `checkbox` |
| `radio` | a radio button | `name` or `var` (its group), `value`, `label`, `disabled` |
| `file` | a file picker | `name`, `label`, `accept`, `multiple`, `max_size`, `disabled`, checks |
| `form` | `form` | `children`, `data` (to start from) |
| `dialog` | `dialog` | `children`, `open` (a variable), `title` |
| `markdown` | Markdown, drawn safely (no raw HTML) | `text` |
| `use` | a component ([090](090_components.md)) | `component`, `props` |

A `text` with `level` is a heading (`h1`–`h3`) for screen readers, with a heading's size and weight
unless its own keys say otherwise. `markdown` is for longer prose, and for streamed AI answers later.

### Media and embeds

- **`svg`** draws its markup inline, so it can take the theme's colors (`currentColor` follows `color`) and the
  type keys' `size`: with no `width` or `height` of its own, it's 1em square. The markup is read as XML at load
  and may hold only shapes, text, and paint (`path`, `circle`, `rect`, `g`, `text`, gradients, and the like):
  anything else (`script`, `foreignObject`, `a`, animation), an `on*` attribute, a `javascript:` URL, or a link
  out of the svg (`href` not starting with `#`, `url(...)` in `style`) is a config error at the key's line,
  never silently dropped. Markup that isn't one well-formed `svg` element is too. What's drawn is Genesis's own
  copy, written out from the checked tree. With `label` it's an image with that name for screen readers;
  without one it's decoration they skip.
- **`divider`** is a plain `hr`, for a rule between sections, in the theme's border color unless `color` names another.
- **`audio`** and **`video`** play `src` (with templates). `controls` defaults to `true`; `autoplay`, `loop`,
  and `muted` default to `false` (browsers play on their own only when muted). With an `id`, each fires
  `.play`, `.pause`, and `.ended`.
- **`iframe`** embeds another page by `src` (with templates). `title` is required: it's what a screen reader
  announces. It's sandboxed by default, allowing scripts, forms, and popups but not acting as this app's
  origin or navigating it; `sandbox = false` lifts that, as a power tool should allow. It fills its parent's
  width unless `width` says otherwise, and it's 24rem tall unless the box key `height` says otherwise.

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
- When the variable holds something that isn't a list, the cell shows a loud error saying so.

The `Wired` wrapper draws these faces, so they look the same everywhere. `each` is a variable's path:
`app.*`, or `item` and `item.*` inside another `each` (the inner element shadows the outer). Anything
else, or `empty` or `error` on a cell without `each`, is a config error naming the cell's place.

### Controls

A control (`input`, `textarea`, `select`, `checkbox`, `toggle`, `radio`, `file`) works one of two ways:

- **Inside a `form`, with `name`:** its value is the form's draft, and goes in the form's `form` payload on submit.
  It starts from `data`'s `/name` when the form has `data`.
- **Anywhere, with `var`:** it reads and writes that variable directly, with no submit: a search box, a filter,
  a theme picker on `sys.theme`. `var` is a variable a `set` can write: `app.NAME`, `sys.view`, or `sys.theme`.
  A control with `var` is a shortcut for a handler, as `on_click` is: its `NAME.change` runs
  `{ set = VAR, to = "{{ event.value }}" }`, so only an action changes state, `sys.bus` shows it, and the
  variable's `.changed` fires for sources and handlers to follow. A control without an `id` is named by its
  place, as a button is. A text box writes on every keystroke.

**Radios are grouped by name.** Every `radio` with the same `name` in one form, or the same `var`, is one
group: picking one unpicks the rest, and the group's value is the picked radio's `value` (any JSON value,
so `value = 3` gives the number 3). The arrow keys move within the group, as in plain HTML. A radio is
picked when its group's value equals its `value`; none is picked while the value matches none.

**A `file` works only inside a `form`, with `name`.** As in PurpleFlow (its spec 190), the bytes never go
into state: the control's value is a reference, `{ file, name, type, size }`, where `file` is an ID for the
bytes the browser holds. With `multiple = true` it's a list of those. A call whose params hold a reference
(`params = { doc = "{{ event.form.doc }}" }`) goes out as `multipart/form-data`, from the browser through
`serve` to the API, with each referenced file as a part and every other param as a field, whatever the
operation's method would otherwise send. So a PurpleFlow webhook gets it just as it gets any upload. A
reference is good while its page is open; one that's gone fails the call, loudly. `accept` limits what the
picker offers (`"image/*"`, `".pdf,.csv"`). A file bigger than `max_size` (default 10 MB, in bytes; `0` for
no limit) fails its control's check with a message naming both sizes.

A `select` takes `options` (`[{ label, value }]`, or `options = "app.companies"` with `option_label` and
`option_value` pointers, `/label` and `/value` unless they say otherwise). It picks the option whose value
equals its variable's; while none does, it shows `placeholder`, if it has one. Options from a variable that
holds something other than a list (or `null`) are a loud failure in the select's place. A `label` wraps its
control, which names it for screen readers. Checks are in [080](080_display_logic.md). A value keeps its JSON type: a
number from a `number` input, a boolean from a `checkbox`.

A `button` with `submit = true` submits the form it's in. The form fires `.submit` only when every check
inside it passes; otherwise `.invalid`, and each failing control shows its message.

`button` takes `variant` (`"primary"`, `"secondary"`, `"accent"`, `"neutral"`, `"ghost"`, `"link"`, `"error"`)
and `size` (`"xs"`, `"sm"`, `"md"`, `"lg"`), daisyUI's own. Like the style keys, either can take a table by screen size. Controls are drawn as daisyUI's controls.

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
| `grid` | `1`–`12`, or `"fit"` | lays children in a grid of that many equal columns, filling rows left to right; `"fit"` fits as many columns of at least 6rem as there's room for. A cell takes `flow` or `grid`, not both |
| `gap` | spacing | space between children; none unless it's set |
| `align` | `"start"`, `"center"`, `"end"`, `"stretch"` | lines children up across the flow |
| `justify` | `"start"`, `"center"`, `"end"`, `"between"`, `"around"` | spreads children along the flow |
| `wrap` | `true`, `false` | lets a row wrap; `false` stops it, as at a larger screen size |

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
| `height` | `"full"`, `"screen"`, or rem: `4`, `8`, `12`, `16`, `24`, `32`, `48`, `64` | its height; a fixed one with `scroll` makes a panel that scrolls |
| `scroll` | `true`, `false` | scrolls what doesn't fit; `false` stops it |

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

A key that names only larger screens leaves smaller ones as they'd be without it: a cell is a column, and a
heading keeps its own size and weight. A table naming any other screen, or none, is a config error.

A value outside a key's list is a config error naming the key and the values it takes, at the key's line.

The CSS is built ahead of time, so genesis-ui lists every class a key can give in
`crates/genesis-ui/classes.txt`, which Tailwind reads. A test keeps that list the same as the code, and
precommit checks that the built CSS has every class in it (`scripts/check-css.sh`).

## Events each node fires

A node with an `id` fires these ([100](100_events.md)); `on_click`, `on_change`, and `on_submit` are shortcuts that wire them inline.

| Type | Events | Payload |
|---|---|---|
| `button`, `cell` | `.click` | — |
| a control | `.change` | `value` |
| `audio`, `video` | `.play`, `.pause`, `.ended` | — |
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
- a style key given a table by screen size gives each size's class with its prefix, and goes to the browser as a table
- a table by screen size naming another screen, or none, is a config error; so is a bad value at one screen, naming it
- a style key value outside its list is a config error naming the key and its values, at its line
- a cell with both `flow` and `grid` is a config error naming its place
- `grid = 12` with children of `span = 4` lays them three to a row; `offset` skips columns; `"fit"` fits columns; a span by screen size changes with the window (in a real browser: `tests/deployed/layout.mjs`)
- every node takes the placement keys, drawn after its own classes
- a `text` with `level` is a heading element
- a cell with `each` draws its children once per element, each seeing its own element as `item.*`; a nested `each` over `item.*` sees its own
- a click inside an `each` carries its element as `event.item` (in a real browser: `tests/deployed/each.mjs`)
- a value that isn't a list shows a loud failure; a bad `each` path, or `empty` or `error` without `each`, is a config error
- a cell with `each` shows loading, the loud failure (with detail even when `error` is set), `empty`, and its children, as its source moves through each status
- value display with no `format`: string, number, boolean, `null`, array, object
- `dialog` opens with its variable, and closing it sets the variable false and fires `.close`
- `link` opens an outside URL in a new tab, and any other in the same one
- an `image` without `alt` is a config error
- `button`'s `variant` and `size` give daisyUI's classes, by screen size too
- `markdown` draws Markdown and escapes raw HTML
- `svg` draws Genesis's own copy of its markup inline; anything that could run or reach outside it (an element outside the list, an `on*` attribute, a `javascript:` URL, an outside `href` or `url()`) is a config error at its line; with `label` it's a named image, without one it's hidden from screen readers; markup that isn't one `svg` is a config error
- `divider` is an `hr`, in the theme's border color or its own
- `audio` and `video` take `src` with templates and default to controls on, autoplay off; with an `id` they fire `.play`, `.pause`, and `.ended` (in a real browser: `tests/deployed/media.mjs`), and without one they fire nothing
- `iframe` is sandboxed unless `sandbox = false`, full width and 24rem tall unless its keys say otherwise; one with no `title` is a config error
- radios with one `name` in a form, or one `var`, are one group: picking one sets the group's value to its `value`, with its JSON type, and unpicks the rest
- a `file` gives `{ file, name, type, size }` (a list with `multiple`), never its bytes; one over `max_size` fails its check; a `file` with `var`, or outside a `form`, is a config error
- a call whose params hold a file reference reaches the API as `multipart/form-data`, the file as a part with its name and type and the other params as fields; a reference that's gone fails the call
- a `form` starts its controls from `data`; a `submit` button fires `.submit` with each named control in `form`
- a `form` with a failing check fires `.invalid`, not `.submit`, and shows each message
- a control with `var` reads and writes its variable on every change; a `select` on `sys.theme` changes the theme
- each control gives its value its JSON type
- a control with both `name` and `var`, or with `name` outside a `form`, is a config error
- a control with `var` naming anything but `app.NAME`, `sys.view`, or `sys.theme` is a config error; `option_label` or `option_value` with written-out options is too, and so is `options` that's neither a list nor a variable's path
- a control's `.change` runs a `set` of its `var`, shown in `sys.bus`, and fires the variable's `.changed`; one without an `id` writes by its place
- a select picks the option equal to its variable, shows its placeholder while none is, takes options from a variable by its pointers, and shows a loud failure when that variable isn't a list
- a number box keeps what's typed on the way to a number (`-`, `2.`) while it still means the variable's value (in a real browser: `tests/deployed/controls.mjs`)
- a wired cell fires `.click`, and can be reached and pressed with the keyboard
- each type fires its events with its payload; one without an `id` (and no `on_*`) fires nothing
- the wrapper handles `show`, locals, placement, and the `each` faces the same way for every type

## Tasks

The first milestone ([999](999_todo.md)), after the bus ([100](100_events.md)):

- [x] genesis-ui: a Leptos app that draws a config from the state machine, with daisyUI 5 built ahead of time
- [x] genesis-ui: the `Component` trait and the `Wired` wrapper (naming by `id` or place, reading state, firing events; `show` comes with conditions, [080](080_display_logic.md))
- [x] components: `stack`, `heading`, `text` (with templates), and `button`

Milestone three ([999](999_todo.md)), each with its tests:

- [x] `cell` with the layout and box keys, replacing `stack`; `text` with `level` and the type keys, replacing `heading` ([120](120_one_app.md))
- [x] screen sizes for every style key, and the whole style vocabulary in the built CSS
- [x] placement keys on every node
- [x] `image` and `link`; `button`'s variants and sizes
- [x] `divider` and `svg`
- [x] `audio`, `video`, and `iframe`
- [x] `each`, with its faces in `Wired`, and `item.*`
- [x] controls with `var`: `input`, `textarea`, `select`, `checkbox`, `toggle`, and `radio` groups (`disabled` and checks come with display logic, [080](080_display_logic.md))
- [ ] `form`, controls with `name`, and the `submit` button
- [ ] `file`, and calls that send files as multipart through `serve`
- [ ] `dialog`
- [ ] `markdown`
