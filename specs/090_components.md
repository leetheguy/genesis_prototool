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
- Props can carry steps: a caller can pass `on_open = [{ set = "app.view", to = "contact" }]`, and the component
  runs it with `on_click = "{{ props.on_open }}"`. The steps still run in the app, against its state.
  - `"{{ props.NAME }}"` is one step, so it can stand among others in a list
    (`[{ toggle = "local.open" }, "{{ props.on_open }}"]`), and the list carries on after it.
  - The steps run where they were written, with the event from inside: `event.item` is the element the
    component is drawn for. A component that passes its own prop on (`on_open = "{{ props.on_pick }}"`)
    runs it where it was first written.
  - A prop run as steps takes a list of steps, or one step alone. An optional one that's left out runs nothing.
- **Events inside a component** are named under the `use` node's `id`: a `more_btn` inside `id = "card"` fires
  `card.more_btn.click`. Ids inside a component are unique within it, and the `use` node's `id` makes them
  unique in the app. Every instance in an `each` fires the same name, and the payload's `item` tells them apart.
- A partial sees no `app.*`. It sees only what it's given, so the same component works anywhere on the
  screen, and can move into the composite library ([999](999_todo.md)) as it is. `sys.*` is visible everywhere.
  A page in the outlet is the one exception ([The outlet](#the-outlet)): a component that reads `app.*` is a page,
  and a `use` node drawing it is a config error. No component writes `app.*`: a `set`, `step`, or `toggle` of it,
  or a control or dialog bound to it, is a config error; it fires an event, and the app's handler sets it.
- **A component's own steps fire events; they reach nothing else.** Its steps work on its `local.*`, run its
  props' steps, and `fire` events. A custom action, a `call`, a `reload`, a `go`, a timer's `start` or
  `stop`, or a `log` written in a component is a config error. To change what's shown, a component fires an event and the app's handler for it sets the app's
  variables, so all of an app's navigation can sit in one file if its author wants that (a convention, not
  part of Genesis). Events are universal, though: an event a component fires keeps the name it's given, and the app's
  handlers for it can do anything. Genesis sets no structure on event names. Any scoping a user wants is a
  naming convention they choose.

## The outlet

`app.toml` is the top-level page, and its body may hold one `outlet` ([040](040_catalog.md)). The outlet
draws one component at a time, its `default` first, and `{ go = "NAME" }` swaps in another
([100](100_events.md), "Going"). Any component without required props can go there; every other use of a
component is a partial, drawn by a `use` node. In the outlet, a component is drawn as if by a `use` node
named after it, with no props: its nodes fire as `contacts.save_btn.click`, and its locals stay while it's
out of the outlet, as `contacts.local.*`, kept on the device like any instance's.

The outlet hands its page the whole of `app.*`, with no wiring: a page reads `{{ app.contacts }}`, repeats
`each = "app.contacts"`, and starts a form from `data = "app.contact"`, just as `app.toml`'s body does, and it
follows the app's variables as they change. It reads them as it reads props: it never writes them, and changes
state by firing events the app handles. Any partial it uses still gets only what the page passes it as props.

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
- a passed step list runs against the app's state, in its place among the component's own steps, and a prop
  passed on runs where it was first written (in a real browser: `tests/deployed/components.mjs`)
- a step `"{{ props.NAME }}"` outside a component, a step template that isn't one, and a prop run as steps
  that isn't steps are config errors
- a node inside a component fires its events under the `use` node's `id`
- a partial reading `app.*`, in a template or a variable's path, is a config error where a `use` node draws it; `sys.*` isn't
- a page in the outlet reads `app.*` in its templates, its `each`, and its form's `data`, and follows it as it changes
- writing `app.*` in a component, by a step or a bound control, is a config error
- a custom action, a `call`, a `reload`, a `go`, a `start`, a `stop`, or a `log` in a component's own steps is a config error; its `fire` keeps the
  event's name, and the app's handlers for it run
- a component in `app.toml`, a table a component can't hold, and a prop named twice are config errors
- a component with named controls or a submit button used outside a form, and one holding a form used inside
  a form, are config errors
- the outlet draws its `default`, and after `go`, the component named, wired under its own name (in a
  real browser: `tests/deployed/components.mjs`)
- editing a component redraws every place that uses it ([060](060_hot_reload.md)) (in a real browser:
  `tests/deployed/components.mjs`)

## Tasks

Milestone three ([999](999_todo.md)):

- [x] core: component files, `[component]` and its props, and the `use` node, with the load checks, drawn
  with `props.*` (what's inside a component draws but isn't wired yet: its events and named controls come
  with the next task)
- [x] local state per instance, and events named under the `use` node's `id`
- [x] props that carry steps
- [x] the `outlet`, which `go` fills
- [x] a page in the outlet reads `app.*`

## Log

- 2026-11-25: a component's own steps fire events and reach nothing else. Lee: "They can just call events.
  But the events can in turn reach anything." Events are universal, and Genesis doesn't tell people how to
  structure them; any structure can be a naming convention. This was an open question in [999](999_todo.md).
- 2026-11-25: a component doesn't navigate; the app does, on its events. Lee: "If you want a component to navigate the app, you listen to
  the event and call the action. This has the added bonus of allowing a person or agent to put all of the
  navigation into a single file without having to chase all over the whole app to figure out where the heck the
  navigation just fired from." There's no `go` step at all now ([100](100_events.md)). Timers and `log` are next: Lee calls them system components ([999](999_todo.md)).
- 2026-11-25: a component's own steps can't `start` or `stop` a timer or `log`; they fire events and the app's
  handlers do the rest ([100](100_events.md), "Timers"). Lee, on the question: "Yes to number five."
- 2026-11-25: the `outlet`. `app.toml` is the top-level page, and its one `outlet` draws the component `go`
  last named, its `default` at first; every other component is a partial. A component's own steps can't
  `go`: it fires an event, and the app's handler goes. Lee: "We don't need a list of pages. We need a
  top-level page and a partial." "Let's just make it a special component that's only available in that
  top-level page." And: "Yes to app.toml and outlet." "Yes to a default component named on the outlet."
- 2026-11-25: a component keeps its locals while it's out of the outlet. Lee: "Yes to components maintaining
  their own state while out of the outlet as well."
- 2026-11-25: a page in the outlet reads the app's variables. Lee: "Outlet should be passed the whole app.* by default behind the scenes. No extra wiring necessary." It reads them as `app.*`, as `app.toml`'s
  body does, rather than as `props.app.*`, because that's the plainest: nothing new to learn, and the page reads
  like the top-level page it sits in. They're read-only, as props are. A component that reads `app.*` is a page,
  and a `use` node can't draw it, so partials stay blind and reusable. Before this, an outlet page could show
  only its own locals and `sys.*`.
