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
| `serve` | `genesis-serve` (Rust, axum): serves the renderer, loads and watches the configs, sends them to clients, makes every API call, and proxies `/fs/` to `files`. Mounts the configs folder **read-only** |
| `files` | dufs, serving the configs folder **read-write**, on a network of its own. Only `serve` can reach it |

```
host                         containers
$CONFIGS_PATH/ (./configs)
  components/                files:  /data          read-write
  crm/                       serve:  /app/configs   read-only
    app.toml
    components/
$STATE_PATH/ (./state)       files:  /data/.state   read-only
  crm/{client}.json          serve:  /app/state     read-write
```

The state folder holds the mirror ([050](050_state.md)): `serve` writes it, and agents read it at `/fs/.state/`.

- **The configs folder is the user's**, and can be its own git repo. Nothing in this repo lives in it.
- **Agents never reach the server.** They read and write configs through `/fs/` (with
  `GENESIS_AGENT_TOKEN`) or, later, the MCP adapter ([999](999_todo.md)). They can't reach the
  env file, the credentials, or the code.
- **The env file holds the secrets** ([070](070_secrets.md)). Only `serve` reads it. `files` gets
  dufs's own settings and nothing else.

## Routes on `serve`

| Route | What |
|---|---|
| `GET /{app}/` | the renderer, opened on the app in `configs/{app}/` |
| `GET /api/{app}/config` | server-sent events: the app's config whenever it loads, good or bad ([060](060_hot_reload.md)) |
| `POST /api/{app}/call` | fire one of the app's operations; the reply is the response, or a stream of it ([030](030_connections.md)) |
| `POST /api/{app}/mirror` | the client's state and event trail, written to the state folder ([050](050_state.md)) |
| `GET /status` | every app's load state and errors, as JSON, so an agent can check its change landed |
| `/fs/*` | the files service, behind `GENESIS_AGENT_TOKEN` |
| `GET /health` | `200` when the server is up |

`serve` only fires operations the app's config declares. A call naming anything else is an error,
so `serve` is never an open proxy.

## Clients

- **Browser:** open `https://genesis.example.com/crm/`.
- **Desktop (Tauri 2):** a native window on a `serve` URL and app, which the window asks for
  the first time it opens or takes as arguments. The window loads the renderer from `serve` like a browser
  does. There's no IPC and no copy of the renderer in the app, so desktop and browser can't drift apart.
- **Mobile (later):** the same Tauri shell on Android.

## Crates

```
crates/genesis-core   pure Rust, no IO: config types, loading checks, partials, templates,
                      conditions, formatters, and the state machine. Native and wasm.
crates/genesis-ui     the Leptos renderer (wasm): draws state, fires events
apps/serve            genesis-serve
apps/bench            genesis-bench: an app's bus with no screen, for tests and agents ([100](100_events.md))
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

## Styling

daisyUI 5 on Tailwind CSS 4, compiled ahead of time from the classes genesis-ui uses. The theme is
the `sys.theme` variable, set as `data-theme` on the root. The TOML never names a CSS class.

## Honest edges

Every data change is a round trip to an API. Genesis suits apps, CRUD, review screens,
dashboards, and live feeds. It isn't for 60fps local interaction.

## Tests

- core parses the same config from TOML and from its JSON form
- core compiles to `wasm32-unknown-unknown`
- `serve` refuses a call to an operation the app doesn't declare, and makes no request
- `serve` mounts configs read-only; `files` has no env file (deployed check)
- `/fs/` needs `GENESIS_AGENT_TOKEN`; without it, `401`
- `/status` lists each app as loaded or failed, with its errors
- the desktop window shows the same app as a browser on the same URL (deployed check)
- `docker compose up` brings up both services healthy (deployed check)
- the mirror appears at `/fs/.state/{app}/`, and dufs can't write to it

## Tasks

The first milestone ([999](999_todo.md)), after the bus ([100](100_events.md)) and the renderer ([040](040_catalog.md)):

- [x] `serve`: loads `configs/{app}/app.toml` when a client asks, serves the renderer at `/{app}/`, and sends the config
- [x] `serve`: `POST /api/{app}/mirror` writes `state/{app}/{client}.json` and `.events.ndjson`
- [ ] hello: `configs/hello/app.toml`, where a button's clicks are logged and counted on screen and in the mirror (deployed check)
