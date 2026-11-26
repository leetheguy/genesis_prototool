# 010 — Architecture

Status: draft
Created: 2026-11-21

Genesis runs like PurpleFlow: a server with a configs folder, which agents reach only through a
file server. The renderer runs in a browser or a Tauri window, and every app it shows comes from
that server.

## Services

Docker Compose runs two services, like PurpleFlow's `app` and `files`
(PurpleFlow spec 090):

| Service | What it does |
|---|---|
| `serve` | `genesis-serve` (Rust, axum): serves the renderer, loads and watches the configs, sends them to clients, makes every API call, and proxies `/fs/` to `files`. Mounts the configs folder **read-only**, and the logs folder, which nothing else mounts ([110](110_agents.md)) |
| `files` | dufs, serving the configs folder **read-write**, on a network of its own. Only `serve` can reach it |

```
host                         containers
$CONFIGS_PATH/ (./configs)   the app
  app.toml                   files:  /data          read-write
  AGENTS.md                  serve:  /app/configs   read-only
  crm/
    api.toml
    contact_row.toml
$STATE_PATH/ (./state)       files:  /data/.state   read-only
  {client}.json              serve:  /app/state     read-write
$LOGS_PATH/ (./logs)         serve:  /app/logs      read-write
  genesis.log
```

The state folder holds the mirror ([050](050_state.md)): `serve` writes it, and agents read it at `/fs/.state/`.

- **The configs folder is the app** ([120](120_one_app.md)): the user's, and it can be its own git repo. Nothing in this repo lives in it.
- **Agents never reach the server.** They read and write configs through `/fs/` (with
  `GENESIS_AGENT_TOKEN`) or, later, the MCP adapter ([999](999_todo.md)). They can't reach the
  env file, the credentials, or the code.
- **The env file holds the secrets** ([070](070_secrets.md)). Only `serve` reads it. `files` gets
  dufs's own settings and nothing else.

## Routes on `serve`

| Route | What |
|---|---|
| `GET /` | the renderer, drawing the app |
| `GET /api/config` | server-sent events: the app's config whenever it loads, good or bad ([060](060_hot_reload.md)), and pushed events ([110](110_agents.md)) |
| `POST /api/call` | fire one of the app's operations; the reply is the response, or a stream of it ([030](030_connections.md)) |
| `POST /api/call/files` | the same, as a multipart form: the call as its `call` part, and each file its params reference as a part named by the file's ID ([040](040_catalog.md)) |
| `POST /api/mirror` | the client's state and event trail, written to the state folder ([050](050_state.md)) |
| `POST /api/push/{name}` | fire `push.{name}` in every open client, behind `GENESIS_AGENT_TOKEN` ([110](110_agents.md)) |
| `GET /api/log` | the log of every failure, behind `GENESIS_AGENT_TOKEN` ([110](110_agents.md)) |
| `GET /agents` | the configs folder's `AGENTS.md`, open ([110](110_agents.md)) |
| `GET /status` | the app's load state and errors, as JSON, so an agent can check its change landed |
| `/fs/*` | the files service, behind `GENESIS_AGENT_TOKEN` |
| `GET /health` | `200` when the server is up |

`serve` only fires operations the app's config declares. A call naming anything else is an error,
so `serve` is never an open proxy.

## Clients

- **Browser:** open `https://genesis.example.com/`.
- **Desktop (Tauri 2):** a native window on a `serve` URL, which the window asks for
  the first time it opens or takes as an argument. The window loads the renderer from `serve` like a browser
  does. There's no IPC and no copy of the renderer in the app, so desktop and browser can't drift apart.
- **Mobile (later):** the same Tauri shell on Android.

## Crates

```
crates/genesis-core   pure Rust, no IO: config types, loading checks, components, templates,
                      conditions, formatters, and the state machine. Native and wasm.
crates/genesis-ui     the Leptos renderer (wasm): draws state, fires events
apps/serve            genesis-serve
apps/bench            genesis-bench: the app's bus with no screen, for tests and agents ([100](100_events.md))
apps/desktop          the Tauri shell
```

genesis-core does all the thinking that isn't IO, so it's tested natively and fast. Both sides use
its structs: serde reads them from TOML on the server and from JSON in the browser.

## The state machine

The state machine ([050](050_state.md)) and the bus ([100](100_events.md)) live in genesis-core and run
in the client. Together they're a pure function: `handle(state, event) -> (state, effects)`. Components
only ever see immutable snapshots of state (Leptos signals that only the machine writes). What a
component does fires an event, the matching handlers' actions change the state, and the machine
returns any effects (a call, a timer, a write to local storage, a mirror update). The renderer
performs each effect, and its result comes back as another event. Because it's pure, `genesis-bench`
and the tests drive exactly the code the browser runs.

```
config ─► serve loads ─► client ─► machine ─► state ─► components draw
                                      ▲                     │
                result event ◄─ serve call ◄─ effect    event (click, input)
```

Nothing flows back into the config. The config is immutable once loaded; a change on disk is a new config.

### Drawing only what changed

The screen is drawn once, and after that only what changed is touched. The machine keeps a revision
that goes up only when something on screen may have changed: an event ran, a toast was dismissed or ran
out, a source reloaded, state was restored. The clock moving on (every 50ms in the browser) leaves it
alone. The renderer tells the components to look again only when the revision moved, and each one
compares what it reads with what it showed: a text whose words are the same, a list whose rows are the
same, a `show` that still holds, all leave the page alone. Where something did change, the smallest
part is patched: an `each` row's text is changed in place, not drawn again; a new row is added at the
end; a select's options are drawn again only when the list or the value changes. The `ago` format is
the one thing on a clock: it reads the clock once a second.

## Styling

daisyUI 5 on Tailwind CSS 4, compiled ahead of time from the classes genesis-ui uses, and every class
the style keys can give ([040](040_catalog.md)). The theme is the `sys.theme` variable, set as `data-theme`
on the root. The TOML never names a CSS class.

## Honest edges

Every data change is a round trip to an API. Genesis suits screens an agent builds for a person: CRUD,
review screens, dashboards, choices, and live feeds. It isn't for 60fps local interaction.

## Tests

- core parses the same config from TOML and from its JSON form
- core compiles to `wasm32-unknown-unknown`
- `serve` refuses a call to an operation the app doesn't declare, and makes no request
- `serve` mounts configs read-only, and only it mounts the logs; `files` has no env file (deployed check)
- `/fs/` needs `GENESIS_AGENT_TOKEN`; without it, `401`
- `/status` says whether the app loaded, with its errors
- the desktop window shows the same screen as a browser on the same URL (deployed check)
- `/` draws the app from the configs folder's `app.toml`
- `docker compose up` brings up both services healthy (deployed check)
- the mirror appears at `/fs/.state/`, and dufs can't write to it
- the clock moving on with nothing to do leaves the revision, and nothing redraws; a timer firing, a toast
  running out, a dismiss, and a reload move it (`genesis-core/tests/revision.rs`, `genesis-ui/tests/redraw.rs`)
- an idle screen changes nothing in the page for a second, and an `each` row keeps its element when its
  values change (in a real browser: `tests/deployed/each.mjs`)

## Tasks

The first milestone ([999](999_todo.md)), after the bus ([100](100_events.md)) and the renderer ([040](040_catalog.md)):

- [x] `serve`: loads `configs/{app}/app.toml` when a client asks, serves the renderer at `/{app}/`, and sends the config
- [x] `serve`: `POST /api/mirror` writes `state/{client}.json` and `.events.ndjson`
- [x] hello: `samples/hello/app.toml`, where a button's clicks are logged and counted on screen and in the mirror (deployed check: `scripts/check-hello.sh`)

Milestone two ([999](999_todo.md)):

- [x] `serve`: `GET /` lists the apps
- [x] `serve`: `GET /status`: each app loaded or failed, with its errors
- [x] Docker Compose: `serve` (configs read-only, state read-write) and `files` (dufs: configs read-write, state read-only at `.state`), with `/fs/` proxied behind `GENESIS_AGENT_TOKEN` (deployed check)
- [x] the Tauri 2 desktop shell: a window on a `serve` URL and app (built, and checked here on a virtual display)
- [ ] a person opens the desktop window on a real desktop

## Log

- 2026-11-26: the screen draws only what changed. Every 50ms clock tick had told every component to look
  again, and lists and the outlet drew their rows and pages again each time, so a button could be
  swapped out from under a click. Lee: "Ideally, it shouldn't need to redraw at all, or only redraw when
  data changes, and even then only redraw as little as possible. If you can just update the data without
  updating the page, that would be best, or even any components."
