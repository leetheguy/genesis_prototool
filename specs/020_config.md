# 020 — Config

Status: draft
Created: 2026-11-21

The app is the configs folder ([120](120_one_app.md)): `app.toml` at its top, and any other `.toml`
files, in any folders. TOML, so people and agents can write it by hand together.

## A whole app

`app.toml` holds `[app]` and the screen:

```toml
[app]
name = "CRM"
theme = "corporate"          # sys.theme starts here

[state]                      # app variables and their starting values
page = 1
selected = ""

[[body]]
type = "cell"
max_width = "4xl"
margin_x = "auto"
pad = 6
gap = 4

[[body.children]]
type = "text"
text = "Contacts"
level = 1

[[body.children]]
type = "cell"
each = "app.contacts"
empty = "No contacts yet."
gap = 2

[[body.children.children]]
type = "use"
component = "contact_row"
props = { contact = "{{ item }}" }

[[body.children]]
type = "button"
label = "Next page"
on_click = [{ step = "app.page", by = 1 }]
```

Any other file adds to the app. `crm/api.toml`, say, holds what talks to the API:

```toml
[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.crud.contacts]
style = "post"

[sources.contacts]           # a variable fed by an operation
call = "crm.contacts.list"
params = { page = "{{ app.page }}" }
select = "/rows"

[sources.contact]
call = "crm.contacts.get"
params = { id = "{{ app.selected }}" }
load_on = ["app.selected.changed"]
```

And `contact_row.toml`, anywhere in the folder, is a component ([090](090_components.md)): a file with a
`[component]` table.

## Top level

| Table | What | Where |
|---|---|---|
| `[app]` | `name` (required), `theme` (default `"light"`), `forget` (app variables never kept, [050](050_state.md)), `locale` (the language formats write in, [080](080_display_logic.md)), `log_lines` ([110](110_agents.md)), `toast_ms` (how long a toast shows, default `5000`, [100](100_events.md)), `[app.auth]` (signing in, [050](050_state.md)) | `app.toml` only |
| `[[body]]` | the screen: a list of nodes ([040](040_catalog.md)) | `app.toml`, or a component |
| `[state]` | app variables and their starting values (in a component, its local ones) | any file |
| `[sources.NAME]` | app variables fed by an operation or a stream ([050](050_state.md)) | any file but a component |
| `[on]` | handlers: event patterns and the actions they run ([100](100_events.md)) | any file but a component |
| `[actions.NAME]` | custom actions: named lists of steps ([100](100_events.md)) | any file but a component |
| timers | each table a timer: `ms`, and `repeat` ([100](100_events.md), "Timers") | `timers.toml` only |
| `[connections.NAME]` | the APIs and their operations ([030](030_connections.md)) | any file but a component |
| `[component]` | marks the file as a component, with its props ([090](090_components.md)) | a component |

The app is everything in every file, put together. Folders only organize the files: they add nothing
to names, and moving a file changes nothing else. **Every file name is unique across the whole app**:
two files with one name, in any folders, are a load problem naming both paths. Names inside the files
may repeat ([120](120_one_app.md)), but a definition can't be made twice: the same `[state]` variable,
source, connection, or action in two files is a load problem naming both places. Handlers for one event
in several files all run, `app.toml` first and then the other files in path order.

It's one screen. What it shows changes by `show` on any variable ([080](080_display_logic.md)), and by an
agent rewriting the files. Nodes nest through `children`. A tree too deep to read is split into components ([090](090_components.md)).

## Variables

| Name | Scope | Written by |
|---|---|---|
| `app.*` | the whole app: `[state]` and `[sources]`; read in `app.toml` and any component it draws (by `use` or in its outlet), not in one drawn inside another component | actions, and sources |
| `local.*` | one node and its children, or one component instance | actions inside it |
| `sys.*` | Genesis's own: `theme`, `auth`, `errors`, `events`, `bus` ([050](050_state.md)) | actions (all but `theme` are read-only) |
| `event.*` | inside a handler: the event's `name` and payload ([100](100_events.md)) | read-only |
| `props.*` | inside a component: what the caller passed in | read-only |
| `item.*` | inside a cell with `each`: that element | read-only |
| `form.*` | inside a `form`: its controls' values | the controls |

A node declares local variables with `state = { open = false }`.

## Templates

A template is `{{ name.path }}`: a lookup, never an expression. The path is dot-separated keys
(`app.contact.address.city`). Same rule as PurpleFlow: a string that's *only* one template keeps
the value's type (a number stays a number, a list a list); otherwise the value becomes text in the string.

- In **params**, a missing value is a loud error, and the call isn't made.
- In **display text**, a missing value shows as empty.
- `{{ creds.NAME }}` and `{{ session.token }}` work only in `[connections.*]`, and `serve` fills them in ([070](070_secrets.md)).

## Checks

The whole app is checked each time any file in it changes, every file together. Any failure keeps the last good
version running ([060](060_hot_reload.md)), and every problem is reported with its file, line, and column:

- unknown keys anywhere, and unknown node `type`s, condition operators, steps, or formats
- a `call` naming an operation no connection declares, a `start` or `stop` naming a timer `timers.toml`
  doesn't have, or a `reload` of something that isn't a source
- a variable that isn't declared where it's used: `app.x` with no starting value in `[state]` and no source,
  `local.*` and `props.*` outside a component, `item` outside an `each`, `form` outside a `form`, `event`
  outside a step, or a `sys.*` Genesis doesn't have. A step reads the row it fired in as `event.item`, and a
  submitted form as `event.form`
- an action writing what it can't: an undeclared `app.*`, `event`, `item`, a read-only `sys.*`, or `local.*`
  outside a component
- a step naming a custom action that doesn't exist, or a pattern using anything but `*` and `?`
- `{{ creds.* }}` or `{{ session.* }}` outside `[connections.*]`, or `session.*` without `[app.auth]`
- a `select` or `field` that isn't a JSON Pointer, or a template that isn't a plain lookup
- a component that doesn't exist, a missing required prop, or components that use each other in a loop
- two files with one name, in any folders ([120](120_one_app.md))
- one `[state]` variable, source, connection, or action defined in two files, or a variable that's both
  in `[state]` and a source
- no `app.toml` at the top, `[app]` anywhere else, or a table in a file that can't hold it

Each problem is also written to the log ([110](110_agents.md)).

These are checks on wiring only. What can be right later is left to fail when it runs: a `go` to a page that
isn't there ([100](100_events.md)), and a value missing inside a variable that is declared (`app.contact.name`).

## Tests

- the app above loads into the expected config
- defaults: no `theme` is `"light"`
- an app from several files in several folders loads as one, and moving a file changes nothing
- two files with one name, in different folders, are a problem naming both paths
- the same variable, source, connection, or action defined in two files is a problem naming both places
- handlers for one event in two files both run, `app.toml`'s first
- `[app]` or `[[body]]` outside `app.toml`, and no `app.toml` at the top, are problems
- `children` nest to any depth
- an unknown key is an error naming the key, file, line, and column
- every check above fails with its own message, and all problems are reported at once
- a step or a template that would only fail when it runs is refused at load, pointing at its own text; the
  steps a `use` node passes into a component are checked where they're written; a `go` to a page that
  isn't there still loads
- a whole-string template keeps its type; a template inside text becomes text
- a missing value in params is an error and no call is made; in text it shows empty
- `local.*` resolves to the nearest node that declares it

## Log

- 2026-11-25: `[app] start` is gone, with `sys.view` and the `go` step ([100](100_events.md)). Lee: "Yeah, there is no go. It doesn't exist. Not yet anyway. We may create it as a convenience down the road, but for now it's not necessary." There are no pages: "It's all show and hide everywhere. If you want to navigate from one page to another, you just hide one stack of components and show another one."
- 2026-11-25: `timers.toml` holds the app's timers, each a table named by the timer ([100](100_events.md)).
- 2026-11-25: the load checks for wiring are built: a step naming an action, timer, operation, or source that
  isn't there, a step writing what it can't, and a template that isn't a plain lookup or reads a variable that
  isn't there where it's written. A `go` to a page that isn't there is left out, as Lee decided ("When it
  runs"), and so is anything else that could be right later. The bus still fails each of them on its own,
  for an app that reached it some other way.
- 2026-11-25: a template may read `sys.auth` before signing in is built; it shows nothing until then. Lee:
  "Allow sys.auth".
