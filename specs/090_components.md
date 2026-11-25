# 090 — Components

Status: draft
Created: 2026-11-21

A component is a TOML file of catalog atoms, with its own props and local state, that the app uses by
name. It's how an agent builds the pieces of a screen once and arranges them many times: a card, a
contact row, a data table, a sign-in form. Components have no code in them, so they hot-reload like
everything else.

## A component

Any `.toml` file with a `[component]` table, in any folder of the app. Its name is its file name:
`views/people/contact_card.toml` is `contact_card`. Names are unique across the app
([120](120_one_app.md)), so a name always means one file.

```toml
[component]
props = ["contact", "on_open"]     # names the caller passes in
optional = ["compact"]             # props that may be left out

[state]                            # this instance's local variables
expanded = false

[[body]]
type = "cell"
pad = 4
bg = "base-100"
border = true
radius = "box"
gap = 2

[[body.children]]
type = "text"
text = "{{ props.contact.name }}"
weight = "semibold"

[[body.children]]
type = "text"
text = "{{ props.contact.email }}"
color = "muted"
show = { var = "local.expanded", is = true }

[[body.children]]
type = "button"
label = "More"
variant = "ghost"
size = "sm"
on_click = [{ toggle = "local.expanded" }]
```

## Using one

```toml
[[body]]
type = "cell"
each = "app.contacts"
grid = "fit"
gap = 4

[[body.children]]
type = "use"
component = "contact_card"
props = { contact = "{{ item }}" }
```

- `props` values are templates, filled where the `use` node is. A whole-string template passes the value itself, so
  `{{ item }}` passes the whole element; one that names nothing set passes `null`. A prop may be a table or a list
  of templates, and `{{ props }}` reads them all.
- The `use` node takes the placement keys. Without them, the component's body sits straight in the parent's
  layout, as if it were written there; with them, it sits in a box placed like any node.
- Inside a component, `item` starts over: the component's own `each` sets it, and an `item` from where it's
  used reaches it only as a prop.
- A component with controls that have `name`, a file picker, or a `submit` button fills the form it's used in,
  so it's used inside a `form`. One that holds a form of its own isn't used inside another.
- A `use` node without an `id` is named by its place, like any node.
- Inside, `props.*` is read-only, and `local.*` is that instance's own: ten cards have ten `expanded`s.
  An instance is its `use` node and the row of the `each` it's drawn in, so its locals are kept on the device
  and survive a reload by them, as a form's draft does ([050](050_state.md)). A change to one fires under the
  `use` node, like every event inside it: `card.local.expanded.changed`.
- Inside a component, `each`, a form's `data`, and a select's `options` may read `props.*` and `local.*`.
- A component can use other components. Using one in a loop (A uses B uses A) is a config error.
- Props can carry steps: a caller can pass `on_open = [{ go = "contact" }]`, and the component
  runs it with `on_click = "{{ props.on_open }}"`. The steps still run in the app, against its state.
- **Events inside a component** are named under the `use` node's `id`: a `more_btn` inside `id = "card"` fires
  `card.more_btn.click`. Ids inside a component are unique within it, and the `use` node's `id` makes them
  unique in the app. Every instance in an `each` fires the same name, and the payload's `item` tells them apart.
- A component sees no `app.*`. It sees only what it's given, so the same component works anywhere on the
  screen, and can move into the composite library ([999](999_todo.md)) as it is. `sys.*` is visible everywhere.

## What a component isn't

It isn't code. A new kind of atom (a chart, a map, a calendar) is a new catalog type, written in Rust
by a developer or an agent, following the `Component` trait, and shipped in a build ([040](040_catalog.md)). Compiled plugin components are an idea in [999](999_todo.md).

## Tests

- a component in any folder loads and draws with its props, by its file name
- each instance has its own local state, kept on the device and across a config reload while its component
  still declares it (in a real browser: `tests/deployed/components.mjs`)
- a named control inside a component fills the form it's used in, and a form inside one starts from
  `props.*` or `local.*`
- an undeclared `local.*`, or a `props.*` the component doesn't take, is a config error
- `props.*` is read-only; writing it is a config error
- a missing required prop, an unknown prop, an unknown component, and a loop of components are config errors
- a passed step list runs against the app's state
- a node inside a component fires its events under the `use` node's `id`
- `app.*` inside a component is a config error, in a template or a variable's path; `sys.*` isn't
- a component in `app.toml`, a table a component can't hold, and a prop named twice are config errors
- a component with named controls or a submit button used outside a form, and one holding a form used inside
  a form, are config errors
- editing a component redraws every place that uses it ([060](060_hot_reload.md)) (in a real browser:
  `tests/deployed/components.mjs`)

## Tasks

Milestone three ([999](999_todo.md)):

- [x] core: component files, `[component]` and its props, and the `use` node, with the load checks, drawn
  with `props.*` (what's inside a component draws but isn't wired yet: its events and named controls come
  with the next task)
- [x] local state per instance, and events named under the `use` node's `id`
- [ ] props that carry steps
