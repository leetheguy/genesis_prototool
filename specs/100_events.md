# 100 — Events, actions, and handlers

Status: draft
Created: 2026-11-22

Everything a Genesis app does runs on one bus. **Actions** do things. Every completed action, every
interaction, and every change in the app fires an **event**. **Handlers** listen for events and run
actions. That's the whole engine: pubsub and wiring, with no conditionals and no code. It's a
mailbox and a dispatcher, as in OTP.

```toml
[[body]]
type = "button"
id = "signout_btn"                  # fires signout_btn.click
label = "Sign out"

[on]
"signout_btn.click" = ["sign_out"]
"sign_out.done"     = ["wipe_state"]

[actions.sign_out]
do = [{ call = "auth.logout" }]     # fires auth.logout.ok, .200, ... then sign_out.done
```

## Events

An event is a dotted name, plus a payload that handlers read as `{{ event.* }}`. By convention a name is
*who* then *what*, in `snake_case`.

| Fired by | Name | Payload |
|---|---|---|
| a node, by its `id` | `signout_btn.click`, `search.change`, `contact_form.submit`, `contacts_table.row_click` | `value`, `row`, or `form`, as fits ([040](040_catalog.md)) |
| a call to an operation | `auth.logout.ok` or `.failed`, plus `.200`, `.404`, … | `status`, `body` (selected) |
| a call with `event = "/outcome"` | `score_job.needs_review` (the reply's value at that pointer) | as above |
| a custom action | `sign_out.done`, or `sign_out.failed` if a step failed | the failure, if any |
| a variable changing | `app.page.changed`, `sys.view.changed` | `value`, `was` |
| a `fire` step | any name | its `with` table |
| an agent or workflow, through `POST /api/{app}/push/{name}` ([110](110_agents.md)) | `push.review_ready` | the JSON body it sent |
| Genesis | `app.opened`, `server.connected`, `server.disconnected`, `config.loaded`, `config.failed`, `auth.in`, `auth.out`, `state.wiped` | as fits |

A node without an `id` fires nothing. Give it an `id` to wire it. `.changed` fires only when the value
actually changes.

## Handlers

`[on]` maps an event pattern to a list of steps:

```toml
[on]
"contacts_table.row_click" = [{ set = "app.selected", to = "{{ event.row.id }}" }, { go = "contact" }]
"*.failed"                 = [{ log = "{{ event.name }}: {{ event.body }}" }]
"contacts.4??"             = [{ toast = "Check that contact and try again." }]
```

- A pattern is an event name with `*` (any run of characters) and `?` (one character), the way a
  shell matches file names. Nothing else.
- Every handler whose pattern matches runs, in the order the file lists them.
- **Shortcut:** `on_click = [...]` on a node is `"{id}.click" = [...]` in `[on]`; likewise `on_change`,
  `on_submit`, and `on_row`. The node then needs no `id`: it gets one from its place in the tree.
- **Shortcut:** a step that's a plain string runs that action: `"sign_out"`.

Handlers never test anything. To branch, fire different events. A call's status already does that
(`.ok` and `.failed`), and so does `event = "/outcome"` when the API should pick the branch.
Display conditions (`show`, `disabled`, input checks, [080](080_display_logic.md)) are the only conditions in Genesis.

## Actions

A step is one built-in action, or the name of a custom one. A custom action is a named list of
steps: `[actions.NAME] do = [...]`. A list runs in order and stops at the first failure.

| Action | What it does | Fires |
|---|---|---|
| `{ set = "app.x", to = "..." }` | sets a variable | `app.x.changed` |
| `{ step = "app.page", by = 1, min = 1, max = 99 }` | adds to a number, within optional bounds | `app.page.changed` |
| `{ toggle = "local.open" }` | flips a boolean | `local.open.changed` |
| `{ go = "contact" }` | sets `sys.view` | `sys.view.changed` |
| `{ call = "crm.contacts.update", params = {...}, into = {...}, event = "/ptr" }` | calls an operation ([030](030_connections.md)); waits for the reply | `crm.contacts.update.ok` / `.failed` / `.{status}` |
| `{ reload = ["app.contacts"] }` | calls those sources again ([050](050_state.md)) | their calls' events |
| `{ fire = "ticker.tick", with = {...} }` | fires an event | that event |
| `{ delay = 5000, id = "ticker" }` | waits, then carries on with the list | — |
| `{ every = 5000, fire = "ticker.tick", id = "ticker" }` | fires an event on a repeating timer | that event, each time |
| `{ cancel = "ticker" }` | stops the `delay` or `every` with that `id` | — |
| `{ confirm = "Delete this contact?" }` | asks; "no" stops the list quietly | — |
| `{ toast = "Saved.", variant = "success" }` | shows a short message | — |
| `{ log = "..." }` | adds a line to `sys.events` and the mirror ([050](050_state.md)) | — |
| `"wipe_state"` | clears kept state back to the starting values | `state.wiped` |
| `{ sign_in = {...} }`, `"sign_out"` | as in [050](050_state.md) | `auth.in` / `auth.out` |

A custom action fires `NAME.done` when its list finishes, or `NAME.failed` (with `error`) when a step fails, and
its failure stops the list that ran it. Actions can run actions, up to `max_chain` deep; deeper is a failure, so an
action that runs itself stops instead of running forever.

## The bus

- **One queue, first in, first out, one event at a time.** For each event, every matching handler
  runs in file order. Events those handlers fire join the back of the queue.
- **A `call` or `delay` pauses its list, not the bus.** Other events keep flowing. The list carries on
  when the reply arrives (or the time is up), in its turn on the queue.
- **Chains have a depth limit.** An event fired by a handler is one deeper than the event that ran
  it. Past `[app] max_chain` (default `100`) the event is dropped, and `chain.too_deep` fails loud with
  the chain's names. A timer starts a new chain: `every` fires at depth 0, and a list resumed after a
  `delay` fires its events as if the event that started it had just arrived. So a polling loop
  (`delay`, then `fire`) runs forever, while a runaway loop with no pause is stopped.
- **Timers stop** with `cancel`, on `wipe_state`, and when the app closes. Two due at the same moment
  run in the order they were set. `every = 0` is a failure.
- **Failures are always logged.** Every `*.failed` event goes into `sys.errors` and the event trail
  before any handler runs. If no handler's pattern matches it, Genesis also shows a toast. A handler
  can take over the display; it can't make a failure disappear. A step that fails in a handler's own
  list (not inside a custom action, which reports its own `.failed`) stops that list, and is logged under
  the event's name and toasted.

## What the bus shows

- **`sys.bus`**: every pattern in the config, with the handlers that listen on it, and every event name the
  config can fire. It's read-only. An event nobody listens to is fine (no error, no warning), and the data is there for anyone who looks.
- **`sys.events`**: the most recent events, newest first (`[app] event_trail`, default `200`): the name,
  time, depth, what fired it, the handlers that ran, and a short payload. `log` lines go here too.

Both are mirrored to files that agents can read through dufs ([050](050_state.md)). That way an agent can
see "no listeners on `contacts.list.ok`, no wonder" without a browser.

## A caution, for the docs

Events, timers, `step`, and handlers together can technically compute things. Genesis doesn't forbid that,
but it's the wrong place for it. If wiring starts to look like a program (counting, deciding, looping over data),
the logic belongs behind the API, in PurpleFlow or wherever else.

## The workbench

`genesis-bench APP_DIR` runs an app's bus with no screen, for tests and for agents. It loads the app
(a folder holding `app.toml`, or the file), opens it, and reads one JSON object per line from stdin:
`{"fire": "NAME", "payload": {...}}` fires an event, and `{"advance": MS}` moves the clock on. After
each line it prints every trail entry recorded (events with their handlers, and `log` lines) and any
`{"toast": ...}`, one JSON object per line, and at the end `{"state": ...}`. An app that doesn't load
exits `2`, each problem as `FILE:LINE:COLUMN: ...`; a script line it can't read exits `1`. Each call the app
makes is printed as `{"call": ...}`; with `--mock mocks.toml` (`[[mock]]` tables of `op` and a `status` and
`body`, or an `error`) it's answered at once, and a call with no mock fails saying so.

## Tests

- a node with an `id` fires its events; one without fires nothing; an `on_*` shortcut wires its node
- a call fires `.ok` or `.failed` and `.{status}`; with `event`, also the reply's named event
- a custom action fires `.done`, or `.failed` when a step fails, and stops at that step; its failure stops the list that ran it
- actions nested past `max_chain` fail instead of recursing forever
- `.changed` fires on a real change only, with `value` and `was`
- `*` and `?` match as in the examples; every matching handler runs, in file order
- a plain-string step runs that action
- events are handled in order, one at a time; a paused list resumes in queue order while other events flow
- a chain past `max_chain` is dropped and fails loud with its names; a `delay` loop runs past the limit
- `every` repeats until `cancel`; `wipe_state` and closing the app stop all timers
- every `*.failed` lands in `sys.errors` and the trail; with no matching handler it also toasts
- a step failing in a handler's own list is logged under the event's name and toasted
- `sys.bus` lists every pattern with its handlers and every event the config can fire
- `sys.events` keeps the newest `event_trail` entries, with handlers and depth
- `genesis-bench` replays a script of events against mocks and prints the trail and final state

## Tasks

The first milestone ([999](999_todo.md)): the bus, worked out on the bench first.

- [x] Install the Rust toolchain; create the Cargo workspace (`crates/genesis-core`, `apps/bench`); add `scripts/precommit.sh`, coverage, and mutation (OP-PARAMS section 10)
- [x] core: config types for `[on]`, `[actions]`, `[state]`, and nodes with `id` (TOML and JSON round trip)
- [x] core: event names and `*`/`?` patterns
- [x] core: the bus: queue, matching handlers in file order, `sys.events`, `sys.bus`, with `fire` and `log`
- [x] core: `set`, `step`, `toggle`, `go`, and `.changed` events
- [x] core: custom actions with `.done` and `.failed`; stop at the first failure
- [x] core: chain depth and `max_chain`; failures always logged
- [x] core: `delay`, `every`, and `cancel` on a clock tests control
- [x] `genesis-bench`: stdin events, the printed trail, and the final state (no calls yet)

Milestone three ([999](999_todo.md)):

- [ ] `confirm`: asks in a dialog; "no" stops the list quietly
- [ ] `toast`: a short message, with its variant
