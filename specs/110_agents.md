# 110 — Working with agents

Status: draft
Created: 2026-11-24

Agents build the screen and drive it. This spec covers what they need from `serve` to do both on
their own: a guide at a known address, one log of every failure, and a webhook that sends events into
the open screens.

## The guide: `GET /agents`

As in PurpleFlow (its spec 090): `GET /agents` answers with `AGENTS.md` from the top of the configs
folder, as Markdown, with every `BASE` replaced by the address the request came to
(`https://my-domain.com`): the scheme and host a proxy in front passes on (`X-Forwarded-Proto` and
`X-Forwarded-Host`), else `http` and the request's `Host`. Only `BASE` as a word of its own is replaced,
not `DATABASE`. It's open, since an agent needs it before it has a token, and it holds only
what's in that file. Without the file, it's a `404` that says to add one.

A person sends an agent to `my-domain.com/agents` and hands it `GENESIS_AGENT_TOKEN`; the guide covers
the rest. The repo's copy is `docs/authoring.md`, and a person copies it into the configs folder as
`AGENTS.md` (the **copies** binding in `AGENTS.md`). It's written for agents, and says:

- **what Genesis is for**: a screen the agent builds for a person on the fly, to show whatever is most
  useful in whatever form, so the two can work together by clicking, choosing, and typing, not only in
  prose. The agent should reach for it whenever a screen beats a paragraph.
- how to reach the configs folder (`BASE/fs/`, with the agent token), and how the app's files fit together
- the atoms, the style keys, components, actions, display logic, connections, and sources, each with an example
- how to check a change: `BASE/status`, `BASE/api/log`, and the mirror at `BASE/fs/.state/`
- how to send events in (`BASE/api/push/{name}`)
- the caution from [100](100_events.md): wiring that starts to compute belongs behind the API

## The log

Every failure goes into one log, so an agent can see and trace them all in one place, in order.

```
2026-11-24T10:02:11Z load contact_row.toml:12:5 unknown key "colour"
2026-11-24T10:02:40Z load ok
2026-11-24T10:05:03Z call crm.contacts.update 422 {"error":"email taken"} (client 7f3a, from save_btn.click)
2026-11-24T10:05:03Z run  client 7f3a: step 2 of save_btn.click: app.nope isn't declared
2026-11-24T10:07:30Z push review_ready: 400 the body isn't a JSON object
```

Each line is the time, the kind, and what happened, with where it happened:

- **`load`**: each problem a load finds ([020](020_config.md), "Checks"), as `file:line:column` with the file's
  name, which is unique in the app ([120](120_one_app.md)). `load ok` when the app next loads clean, so the
  latest `load` line always says where it stands.
- **`call`**: each failed call `serve` makes: the operation, the status, the API's answer, the client, and the
  event that started it. Never the request's headers or credentials.
- **`run`**: each failure while a client runs wiring: a failed step, a chain that went too deep
  ([100](100_events.md)). Clients already report these in `sys.errors` through the mirror ([050](050_state.md));
  `serve` adds each new one, once.
- **`push`**: each push `serve` refused, and why.

The log is `genesis.log` in a logs folder of its own (`--logs`, default `./logs`), which only `serve`
mounts. It isn't in the configs folder or behind `/fs/`. Agents read it through `serve`:

- `GET /api/log` answers it as plain text, behind the agent token.
- `?since=2026-11-24T10:00:00Z` gives only the lines from then on; `?tail=100` the last 100.

It keeps its last `log_lines` lines (`[app] log_lines`, default 10,000), so it can't grow forever.

## Sending events in: `POST /api/push/{name}`

An agent or a workflow fires an event in every open client:

```
POST /api/push/review_ready
Authorization: Bearer $GENESIS_AGENT_TOKEN
{"contact": {"id": 42, "name": "Ada"}, "note": "Ready for review"}
```

- The event is `push.{name}` (`push.review_ready`), and its payload is the JSON body, read as
  `{{ event.contact.name }}`. The body must be a JSON object. `name` is lowercase letters, digits, and `_`.
- Handlers react as to any event: `"push.review_ready" = [{ set = "app.review", to = "{{ event.contact }}" }]`.
- `serve` sends it over each client's config stream ([060](060_hot_reload.md)). A pushed event lands in the
  trail and the mirror like any other, so the sender can read what it set off.
- The answer is `{"delivered": N}`, the number of open clients that got it. Zero isn't an error: no one has the
  screen open.
- It takes the agent token. A wrong or missing token is `401`; a body that isn't a JSON object, or a bad
  `name`, is `400`, with the reason. Each refusal is logged.

## Tests

- `GET /agents` answers the configs folder's `AGENTS.md` with `BASE` filled in from the request, and needs no token
- `GET /agents` with no `AGENTS.md` is a `404` saying to add one
- `BASE` takes the scheme and host a proxy passes on; a host that isn't a host name (with an optional port) isn't used
- a load problem is logged with its time, file, line, and column; a clean load after a bad one logs `load ok`
- a failed call is logged with its operation, status, answer, client, and starting event, and no headers or credentials
- a `run` failure a client reports through the mirror is logged once, though the mirror sends it again
- a refused push is logged with its reason
- `GET /api/log` needs the agent token, and answers the log; `since` and `tail` narrow it
- the log keeps only its last `log_lines` lines
- the log isn't reachable through `/fs/`
- a push reaches every open client as `push.{name}`, with the body as its payload, and handlers run
- a push answers how many clients got it, and zero when none are open
- a push without the agent token is `401`; with a body that isn't an object, or a bad name, `400`
- a pushed event shows in the client's trail and mirror
- deployed: an agent's `curl` to the push route changes an open screen in a real browser

## Tasks

Milestone three ([999](999_todo.md)):

- [ ] `GET /agents`, as PurpleFlow's
- [ ] the log: `load` and `call` lines, `log_lines`, and the logs folder in Compose
- [ ] the log: `run` lines from the mirror
- [ ] `GET /api/log`, with `since` and `tail`
- [ ] the push route, over the config stream, with its `push` lines and a deployed check

## Log

- 2026-11-25: `BASE` is the scheme and host a proxy in front passes on (`X-Forwarded-Proto`, `X-Forwarded-Host`),
  else `http` and `Host`, since `serve` sits behind one in Compose ([010](010_architecture.md)). A host header
  that isn't a host name, with an optional port, falls back to `serve`'s own `--listen` address, so nothing
  but an address ever goes into the guide. The guide goes only to the one who asked, so the host they sent is
  theirs to get back.
