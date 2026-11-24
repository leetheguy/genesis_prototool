# 110 — Working with agents

Status: draft
Created: 2026-11-24

Agents build Genesis apps and drive them. This spec covers what they need from `serve` to do both
on their own: a guide at a known address, a log beside every file that fails, file names that
never collide, and a webhook that sends events into open screens. PurpleFlow does the first three
the same way (PurpleFlow specs 090, 110, 270).

## The guide: `GET /agents`

`GET /agents` answers with the guide for agents writing Genesis apps, as Markdown, with `BASE`
filled in as the address the request came to (`https://my-domain.com`). It's open, like `/status`:
it holds no secrets. A person sends an agent to `my-domain.com/agents` and hands it
`GENESIS_AGENT_TOKEN`; the guide covers the rest.

The guide is `docs/authoring.md`, built into `serve`, so it always describes the catalog of the
build that serves it. It says:

- how to reach the configs folder (`BASE/fs/`, with the agent token) and what an app's folder holds
- the catalog, the actions, display logic, connections, and sources, each with an example
- how to check a change: `BASE/status`, the `.log` beside a file, and the mirror at `BASE/fs/.state/`
- how to send events in (`BASE/api/{app}/push/{name}`)
- the caution from [100](100_events.md): wiring that starts to compute belongs behind the API

PurpleFlow serves an `AGENTS.md` that a person copies into its workflows folder. Genesis builds its
guide in instead, because the guide describes the catalog compiled into `serve`, and a copied file goes
stale on the next upgrade.

## One name per file

Within an app, every TOML file has a name of its own, whatever folder it's in. An app sees its own
folder and the shared `configs/components/` ([090](090_components.md)), and no two `.toml` files across
them share a file name: `crm/app.toml` and `crm/views/app.toml` clash, and so do
`crm/components/card.toml` and `components/card.toml`. A clash is a load problem naming both paths.

A file's name is how a log, a partial, and a problem point at it, so it's never ambiguous. Different
apps can each have an `app.toml`; it's only within one app that names are unique.

## Logs beside the files

When something fails, `serve` writes it to a `.log` file beside the TOML file it came from, with the
same name: problems in `crm/components/contact_row.toml` go to `crm/components/contact_row.log`. An agent
that wrote a file reads the log beside it, through `/fs/`.

A log gets one line per problem, oldest first, each starting with the time:

```
2026-11-24T10:02:11Z load contact_row.toml:12:5: unknown key "colour"
2026-11-24T10:02:40Z load ok
2026-11-24T10:05:03Z run client 7f3a: crm.contacts.update.failed: 422 {"error":"email taken"}
```

- **`load`**: each problem a load finds in this file ([020](020_config.md), "Checks"), and `load ok` when
  the file next loads clean, so the last lines always say where it stands.
- **`run`**: each failure while a client runs wiring from this file: a failed step, a failed call, a chain
  that went too deep ([100](100_events.md)). Clients already report these in `sys.errors` through the mirror
  ([050](050_state.md)); `serve` adds the new ones to the log, with the client's id.

A log keeps its last `log_lines` lines (`[app] log_lines`, default 1000), so it can't grow forever. A
log never holds credentials: a call's line has the operation, the status, and the API's answer, never the
request's headers. `serve` ignores `.log` files when it watches for changes, and an agent may delete one; it
starts again at the next line.

To write logs, `serve` mounts the configs folder read-write, and writes nothing there but `.log`
files ([010](010_architecture.md)).

## Sending events in: `POST /api/{app}/push/{name}`

An agent or a workflow fires an event in every open client of an app:

```
POST /api/crm/push/review_ready
Authorization: Bearer $GENESIS_AGENT_TOKEN
{"contact": {"id": 42, "name": "Ada"}, "note": "Ready for review"}
```

- The event is `push.{name}` (`push.review_ready`), and its payload is the JSON body, read as
  `{{ event.contact.name }}`. The body must be a JSON object. `name` is lowercase letters, digits, and `_`.
- Handlers react as to any event: `"push.review_ready" = [{ set = "app.review", to = "{{ event.contact }}" }, { go = "review" }]`.
- `serve` sends it over each client's config stream ([060](060_hot_reload.md)). A pushed event lands in the
  trail and the mirror like any other, so the sender can read what it set off.
- The answer is `{"delivered": N}`, the number of open clients that got it. Zero isn't an error: no one has the
  app open.
- It takes the agent token. A wrong or missing token is `401`; an app that doesn't exist or isn't loaded is `404`;
  a body that isn't a JSON object, or a bad `name`, is `400`, with the reason.

## Tests

- `GET /agents` answers the guide, with `BASE` filled in from the request, and needs no token
- two `.toml` files with the same name in one app, in different folders, are a load problem naming both paths
- the same name in two different apps is fine
- a load problem in a file is written to the `.log` beside it, with its time, file, line, and column
- a clean load after a bad one writes `load ok`
- a `run` failure a client reports through the mirror is written to the log of the file whose wiring failed, once
- a log keeps only its last `log_lines` lines
- a failed call's log line has no request headers or credentials
- writing a `.log` doesn't reload the app
- a push reaches every open client of the app as `push.{name}`, with the body as its payload, and handlers run
- a push answers how many clients got it, and zero when none are open
- a push without the agent token is `401`; to an unknown app `404`; with a body that isn't an object `400`
- a pushed event shows in the client's trail and mirror
- deployed: an agent's `curl` to the push route changes an open screen in a real browser

## Tasks

Milestone three ([999](999_todo.md)):

- [ ] `GET /agents`, serving `docs/authoring.md` with `BASE` filled in
- [ ] one name per file: the load check
- [ ] logs: `load` lines and `load ok`, and the read-write mount in Compose
- [ ] logs: `run` lines from the mirror, and `log_lines`
- [ ] the push route, over the config stream, with a deployed check
