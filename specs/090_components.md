# 090 — Components

Status: draft
Created: 2026-11-21

A component is a partial: a TOML file of catalog nodes, with its own props and local state, that an
app uses by name. Components make deep trees readable and screens reusable. They're
structured components with data links, with no code in them, so they hot-reload like everything else.

## A component

`configs/components/contact_card.toml`, shared by every app, or `configs/crm/components/contact_card.toml`,
for this app only. A component's name is its file name, so it's never ambiguous: an app's own
component and a shared one with the same name is a load problem, like any two files in one app with the
same name ([110](110_agents.md)).

```toml
[component]
props = ["contact", "on_open"]     # names the caller passes in
optional = ["compact"]             # props that may be left out

[state]                            # this instance's local variables
expanded = false

[[body]]
type = "card"
title = "{{ props.contact.name }}"

[[body.children]]
type = "text"
text = "{{ props.contact.email }}"
show = { var = "local.expanded", is = true }

[[body.children]]
type = "button"
label = "More"
on_click = [{ toggle = "local.expanded" }]
```

## Using one

```toml
[[body]]
type = "list"
data = "app.contacts"

[[body.children]]
type = "use"
component = "contact_card"
props = { contact = "{{ row }}" }
```

- `props` values are templates. A whole-string template passes the value itself, so `{{ row }}` passes the whole row.
- Inside, `props.*` is read-only, and `local.*` is that instance's own: ten cards have ten `expanded`s.
- A component can use other components. Using one in a loop (A uses B uses A) is a config error.
- Props can carry steps: a caller can pass `on_open = [{ go = "contact" }]`, and the component
  runs it with `on_click = "{{ props.on_open }}"`. The steps still run in the caller's app, against its state.
- **Events inside a component** are named under the `use` node's `id`: a `more_btn` inside `id = "card"` fires
  `card.more_btn.click`. Every instance in a List fires the same name, and the payload's `row` tells them apart.
- A component sees no `app.*` of its own. It sees only what it's given, so it's reusable across apps. `sys.*` is visible everywhere.

## What a component isn't

It isn't code. A new kind of node (a chart, a map, a calendar) is a new catalog type, written in Rust
by a developer or an agent, following the `Component` trait, and shipped in a build ([040](040_catalog.md)). Compiled plugin components are an idea in [999](999_todo.md).

## Tests

- a component loads and draws with its props, from the app's folder or the shared one; the app's own wins
- each instance has its own local state
- `props.*` is read-only; writing it is a config error
- a missing required prop, an unknown prop, and a loop of components are config errors
- a passed step list runs against the caller's state
- a node inside a component fires its events under the `use` node's `id`
- `app.*` inside a component is a config error
- editing a component reloads every app that uses it ([060](060_hot_reload.md))
