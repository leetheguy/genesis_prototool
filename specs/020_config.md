# 020 — Config

Status: draft
Created: 2026-11-21

Each app is a folder in the configs folder: `configs/{app}/app.toml`, plus any partials it uses
([090](090_components.md)). TOML, so people and agents can write it by hand together.

## A whole app

```toml
[app]
name = "CRM"
theme = "corporate"          # sys.theme starts here
start = "contacts"           # sys.view starts here

[state]                      # app variables and their starting values
page = 1
selected = ""

[sources.contacts]           # a variable fed by an operation
call = "crm.contacts.list"
params = { page = "{{ app.page }}" }
select = "/rows"

[sources.contact]
call = "crm.contacts.get"
params = { id = "{{ app.selected }}" }
load_on = ["app.selected.changed"]

[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.crud.contacts]
style = "post"

[[body]]
type = "card"
title = "Contacts"
show = { var = "sys.view", is = "contacts" }

[[body.children]]
type = "table"
data = "app.contacts"
empty = "No contacts yet."
columns = [
  { label = "Name", field = "/name" },
  { label = "Added", field = "/inserted_at", format = { date = "%b %d, %Y" } },
]
on_row = [{ set = "app.selected", to = "{{ row.id }}" }, { go = "contact" }]

[[body.children]]
type = "button"
label = "Next page"
on_click = [{ step = "app.page", by = 1 }]
```

## Top level

| Table | What |
|---|---|
| `[app]` | `name` (required), `theme` (default `"light"`), `start` (the first `sys.view`, default `"main"`), `forget` (app variables never kept, [050](050_state.md)), `[app.auth]` (signing in, [050](050_state.md)) |
| `[state]` | app variables and their starting values |
| `[sources.NAME]` | app variables fed by an operation or a stream ([050](050_state.md)) |
| `[on]` | handlers: event patterns and the actions they run ([100](100_events.md)) |
| `[actions.NAME]` | custom actions: named lists of steps ([100](100_events.md)) |
| `[connections.NAME]` | the APIs and their operations ([030](030_connections.md)) |
| `[[body]]` | the screen: a list of nodes ([040](040_catalog.md)) |

It's one page. Different screens are top-level nodes shown by `sys.view` or any other variable
([080](080_display_logic.md)). Nodes nest through `children`. A tree too deep to read is split into partials ([090](090_components.md)).

## Variables

| Name | Scope | Written by |
|---|---|---|
| `app.*` | the whole app: `[state]` and `[sources]` | actions, and sources |
| `local.*` | one node and its children, or one partial instance | actions inside it |
| `sys.*` | Genesis's own: `view`, `theme`, `auth`, `errors`, `events`, `bus` ([050](050_state.md)) | actions (all but `view` and `theme` are read-only) |
| `event.*` | inside a handler: the event's `name` and payload ([100](100_events.md)) | read-only |
| `props.*` | inside a partial: what the caller passed in | read-only |
| `row.*` | inside a Table row or a list item: that element | read-only |
| `form.*` | inside a Form: its fields' values | the fields |

A node declares local variables with `state = { open = false }`.

## Templates

A template is `{{ name.path }}`: a lookup, never an expression. The path is dot-separated keys
(`app.contact.address.city`). Same rule as PurpleFlow: a string that's *only* one template keeps
the value's type (a number stays a number, a list a list); otherwise the value becomes text in the string.

- In **params**, a missing value is a loud error, and the call isn't made.
- In **display text**, a missing value shows as empty.
- `{{ creds.NAME }}` and `{{ session.token }}` work only in `[connections.*]`, and `serve` fills them in ([070](070_secrets.md)).

## Checks

The whole app is checked each time it loads, partials included. Any failure keeps the last good
version running ([060](060_hot_reload.md)), and every problem is reported with its file, line, and column:

- unknown keys anywhere, and unknown node `type`s, condition operators, steps, or formats
- a `call` naming an operation no connection declares
- a variable that isn't declared where it's used (`local.x` outside the node that declares it,
  `row` outside a row, `form` outside a Form)
- an action writing a read-only variable (`props`, `row`, `event`, a read-only `sys.*`)
- a step naming a custom action that doesn't exist, or a pattern using anything but `*` and `?`
- `{{ creds.* }}` or `{{ session.* }}` outside `[connections.*]`, or `session.*` without `[app.auth]`
- a `select` or `field` that isn't a JSON Pointer, or a template that isn't a plain lookup
- a partial that doesn't exist, a missing required prop, or partials that use each other in a loop
- two `.toml` files in one app with the same name, in any folders ([110](110_agents.md))

Each problem is also written to the `.log` beside the file it's in ([110](110_agents.md)).

## Tests

- the app above loads into the expected config
- defaults: no `theme` is `"light"`, no `start` is `"main"`
- `children` nest to any depth
- an unknown key is an error naming the key, file, line, and column
- every check above fails with its own message, and all problems are reported at once
- a whole-string template keeps its type; a template inside text becomes text
- a missing value in params is an error and no call is made; in text it shows empty
- `local.*` resolves to the nearest node that declares it
