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
| `[app]` | `name` (required), `theme` (default `"light"`), `start` (the first `sys.view`, default `"main"`), `forget` (app variables never kept, [050](050_state.md)), `log_lines` ([110](110_agents.md)), `[app.auth]` (signing in, [050](050_state.md)) | `app.toml` only |
| `[[body]]` | the screen: a list of nodes ([040](040_catalog.md)) | `app.toml`, or a component |
| `[state]` | app variables and their starting values (in a component, its local ones) | any file |
| `[sources.NAME]` | app variables fed by an operation or a stream ([050](050_state.md)) | any file but a component |
| `[on]` | handlers: event patterns and the actions they run ([100](100_events.md)) | any file but a component |
| `[actions.NAME]` | custom actions: named lists of steps ([100](100_events.md)) | any file but a component |
| `[connections.NAME]` | the APIs and their operations ([030](030_connections.md)) | any file but a component |
| `[component]` | marks the file as a component, with its props ([090](090_components.md)) | a component |

The app is everything in every file, put together. Folders only organize the files: they add nothing
to names, and moving a file changes nothing else. **Every name is unique across the whole app**: file
names, node ids, variables, sources, connections, actions, components, and handler patterns. Two of
anything with one name, in one file or two, is a load problem naming both places.

It's one screen. What it shows changes by `show` on any variable ([080](080_display_logic.md)), and by an
agent rewriting the files. Nodes nest through `children`. A tree too deep to read is split into components ([090](090_components.md)).

## Variables

| Name | Scope | Written by |
|---|---|---|
| `app.*` | the whole app: `[state]` and `[sources]` | actions, and sources |
| `local.*` | one node and its children, or one component instance | actions inside it |
| `sys.*` | Genesis's own: `view`, `theme`, `auth`, `errors`, `events`, `bus` ([050](050_state.md)) | actions (all but `view` and `theme` are read-only) |
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
- a `call` naming an operation no connection declares
- a variable that isn't declared where it's used (`local.x` outside the node that declares it,
  `item` outside an `each`, `form` outside a `form`)
- an action writing a read-only variable (`props`, `item`, `event`, a read-only `sys.*`)
- a step naming a custom action that doesn't exist, or a pattern using anything but `*` and `?`
- `{{ creds.* }}` or `{{ session.* }}` outside `[connections.*]`, or `session.*` without `[app.auth]`
- a `select` or `field` that isn't a JSON Pointer, or a template that isn't a plain lookup
- a component that doesn't exist, a missing required prop, or components that use each other in a loop
- two of anything with one name: files (in any folders), node ids, variables, sources, connections,
  actions, components, or handler patterns ([120](120_one_app.md))
- no `app.toml` at the top, `[app]` anywhere else, or a table in a file that can't hold it

Each problem is also written to the log ([110](110_agents.md)).

## Tests

- the app above loads into the expected config
- defaults: no `theme` is `"light"`, no `start` is `"main"`
- an app from several files in several folders loads as one, and moving a file changes nothing
- the same name twice, in one file or across two, is a problem naming both places
- `children` nest to any depth
- an unknown key is an error naming the key, file, line, and column
- every check above fails with its own message, and all problems are reported at once
- a whole-string template keeps its type; a template inside text becomes text
- a missing value in params is an error and no call is made; in text it shows empty
- `local.*` resolves to the nearest node that declares it
