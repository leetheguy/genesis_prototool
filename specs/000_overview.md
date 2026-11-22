# 000 — Overview

Status: living
Created: 2026-11-21

This is the source of truth for what Genesis is today. Read it first. It's rewritten whenever the
project changes; the numbered specs it links to hold the detail and the reasons behind each decision.

**Milestone one is built** ([999](999_todo.md)): the event bus and its workbench, the renderer with
`stack`, `heading`, `text`, and `button`, and `serve` drawing an app in a browser and writing the
mirror. Everything else here is still design: every numbered spec is a draft, and this page describes
the design they add up to.

## What this is

Genesis is a generic interface. Each app is a folder of TOML: the variables it holds, the APIs it
talks to, and a tree of components from a fixed catalog. Genesis reads it at runtime, draws it,
redraws it live whenever a file changes, and wires every read and write to an API. The TOML *is*
the app; there's no generated code to own.

It's the sister product of PurpleFlow and follows the same philosophy, but it's fully decoupled
from PurpleFlow and from any other API. PurpleFlow is a natural engine room behind it (workflows, data,
logic), and a PurpleFlow webhook is just another endpoint. It's for people and agents co-authoring
interfaces in minutes: CRUD apps, review screens, dashboards, and live feeds.

It's a power tool: safeguards are options with defaults, not limits. It fails loud. See `AGENTS.md`.

## The whole model

1. **An app is a folder of TOML** in the configs folder, which agents reach only through dufs, as
   in PurpleFlow. It's loaded into closed Rust types; unknown keys are errors. See [010](010_architecture.md), [020](020_config.md).
2. **One page, many views.** The screen is a tree of nodes from a closed catalog ([040](040_catalog.md)).
   Which parts show depends on variables, `sys.view` among them ([080](080_display_logic.md)).
3. **Every value comes from the state machine.** Components read immutable state; the screen always
   shows its current state. See [050](050_state.md).
4. **Everything runs on one bus.** Actions do things. Every interaction and every completed action
   fires an event, with dotted names (`signout_btn.click`, `auth.logout.ok`, `app.page.changed`). Handlers
   (`[on]`) listen for event patterns and run actions. There are no conditionals: to branch, fire a
   different event. One queue, one event at a time, a chain-depth limit, and timers (`delay`, `every`) for
   loops. `sys.bus` shows who listens to what, and `sys.events` shows what happened. See [100](100_events.md).
5. **Variables have scopes:** `app.*` for the whole app, `local.*` for one node or component,
   `sys.*` for Genesis's own (`view`, `theme`, `auth`, `errors`), plus `row.*`, `form.*`, and `props.*` where they apply.
   **A session persists:** what the user did (`app.*`, `sys.*`, locals, drafts) is kept on the device;
   what an API said (sources, call results, whether you're signed in) is fetched fresh on every open. Each client
   also mirrors its state and event trail to files that agents read through dufs. See [050](050_state.md).
6. **Sources feed variables from APIs**, by a call or a stream. They call again when the variables in
   their params change (a shortcut for handlers). That's how pagination, search, and selection work without logic.
7. **Connections are HTTP APIs**: a name, a base URL, headers, and operations, with `crud` as a
   shortcut. There's only one kind. Every call fires its result as events (`.ok`, `.failed`, `.404`). See [030](030_connections.md).
8. **Display logic only.** Conditions (show, disable), input checks, and formats, written as TOML tables, never as
   expressions. Logic about data lives behind the API. See [080](080_display_logic.md).
9. **Fail loud.** A failed call is shown where it happened and kept in `sys.errors`. There are no retries
   and no special response shapes; the API signals problems with its status. See [030](030_connections.md).
10. **Two kinds of component.** Partials are reusable TOML with props and local state, and they hot-reload ([090](090_components.md)).
    Catalog components are Rust, all built the same way: the `Component` trait plus one shared `Wired` wrapper that
    knows all the wiring. A new one is a small, regular piece of code that ships in a build ([040](040_catalog.md)).
11. **Changing a file changes every open screen, live, in every build.** A bad change keeps the last
    good version running and says where it went wrong. See [060](060_hot_reload.md).
12. **The server makes every call.** Credentials live in its env file, set by a person, and never
    reach the configs folder or a browser. See [070](070_secrets.md).
13. **Signing in is the API's job; `serve` holds the session.** The token stays on `serve` behind an
    `httpOnly` cookie, every open asks the API whether you're still signed in, and signing back in
    resumes where you were. With `[app.auth]`, `serve` refuses all other calls until you sign in. See [050](050_state.md), [070](070_secrets.md).

## Rules

- **No logic about data.** Wiring (naming, mapping, selecting) and display logic (conditions, checks, formats) are
  Genesis's. Filtering, sorting, computing, and deciding are the API's.
- **The catalog is closed.** A new node type is a spec and a code change. No raw CSS classes in the TOML.
- **No secrets in the TOML, ever.** Only `{{ creds.NAME }}` references, only in connections.
- **Fail loud.** Every failure is visible on screen, in `sys.errors`, and (for config) in `/status`. Every `*.failed` event is logged before any handler runs.
- **No conditionals in wiring.** Conditions exist only in `show`, `disabled`, and input checks. Handlers branch by event name.
- **One-way flow.** Configs are immutable once loaded; state changes only through the state machine;
  nothing flows back into a config.
- **Check PurpleFlow first.** Before designing a solution, see whether PurpleFlow already solved
  the same problem, and match it unless there's a reason not to.
- **Light docs in code.** Module docs and comments are short and plain. Someone who doesn't know
  Rust should be able to follow them; explain Rust terms in a few words when they come up.

## Architecture

| Piece | What |
|---|---|
| `serve` service | genesis-serve (Rust, axum): serves the renderer, loads and watches configs, makes every call, proxies `/fs/` |
| `files` service | dufs: the configs folder, read-write, reachable only through `serve` |
| Browser | opens `/{app}/` on `serve` |
| Desktop | a Tauri 2 window on a `serve` URL, the same renderer |
| Mobile (later) | the same Tauri shell on Android |

See [010](010_architecture.md).

## Where things live

Built so far, then planned ([010](010_architecture.md)):

| Path | What's there |
|---|---|
| `crates/genesis-core` | pure Rust, native and wasm: the config types (`config`), event patterns (`pattern`), templates (`template`), and the bus and state machine (`bus`) |
| `crates/genesis-ui` | the Leptos renderer: `Host`, the `Component` trait, the `Wired` wrapper, and the components |
| `apps/bench` | `genesis-bench`: runs an app's bus with no screen |
| `apps/web` | the browser entry (wasm), built by trunk with Tailwind and daisyUI |
| `apps/serve` | `genesis-serve`: the renderer, each app's config stream, and the mirror |
| `samples/` | sample apps (`hello`); the configs folder itself is the user's |
| `tests/deployed/` | checks against a real `serve` and a real browser (`scripts/check-hello.sh`) |
| `state/` | the mirror: each client's state and event trail (read-only to agents at `/fs/.state/`, once dufs is in) |
| `apps/desktop` | *planned:* the Tauri shell |
| `docker-compose.yml` | *planned:* `serve` and `files` |
| `docs/authoring.md` | *planned:* the guide for agents writing Genesis apps |

## Dependencies

In use now, then planned. Check current versions before adding any.

| Need | Package |
|---|---|
| Config types, TOML, JSON | `serde`, `toml`, `serde_json` |
| Renderer (Rust to wasm, signals) | `leptos` 0.8 (client-side in the browser, server-side for tests), built with `trunk` |
| Styling and themes | daisyUI 5 on Tailwind CSS 4 (the standalone binary, built ahead of time) |
| Server | `axum`, `tokio`, `tower-http` (now); `reqwest`, `notify` (planned) |
| Desktop and mobile shell | `tauri` 2 (planned) |
| Config files for agents | dufs (container image, planned) |
| Browser checks | `playwright-core` (Node, in `tests/deployed/`) |

## Specs

**Living:**

- [000 — Overview](000_overview.md): this page.
- [999 — To-do](999_todo.md): planned work, open questions, and ideas.

**History** (each one as decided, with later changes in its log):

- [010 — Architecture](010_architecture.md): *draft.* `serve` and `files`, routes, clients, crates, the state machine's place.
- [020 — Config](020_config.md): *draft.* An app's TOML: tables, variables and scopes, templates, checks.
- [030 — Connections](030_connections.md): *draft.* HTTP operations, `crud`, streams, selecting, fail loud.
- [040 — Catalog](040_catalog.md): *draft.* The node types, their events, and how a component is built.
- [050 — State](050_state.md): *draft.* Variables, sources, what persists, signing in, and the mirror.
- [060 — Hot reload](060_hot_reload.md): *draft.* Watching configs, keeping the last good version, `/status`.
- [070 — Secrets](070_secrets.md): *draft.* Credentials and sessions, both kept on the server.
- [080 — Display logic](080_display_logic.md): *draft.* Conditions, input checks, and formats.
- [090 — Components](090_components.md): *draft.* Partials with props and local state.
- [100 — Events, actions, and handlers](100_events.md): *draft.* The bus: event names, `[on]`, actions, the queue, depth, timers, the workbench.
