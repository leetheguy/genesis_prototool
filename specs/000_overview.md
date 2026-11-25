# 000 — Overview

Status: living
Created: 2026-11-21

This is the source of truth for what Genesis is today. Read it first. It's rewritten whenever the
project changes; the numbered specs it links to hold the detail and the reasons behind each decision.

**Milestones one and two are built** ([999](999_todo.md)): the event bus and its workbench; the renderer
with `cell` (with its layout, box, and type keys), `text` (a heading with `level`, with box and type keys), and `button`; `serve` drawing the one app, the configs folder, at `/`
([120](120_one_app.md)), reporting it at `/status`, and reloading it live; connections, the `call` step, and sources, with
credentials only `serve` holds; sessions kept on the device; the mirror; Docker Compose with dufs behind
`/fs/`; and the desktop shell, whose window builds and loads a `serve` page (a person has yet to open it on a
real desktop, [999](999_todo.md)).
Everything else here is still design: every numbered spec but [120](120_one_app.md) is a draft, and this page describes the
design they add up to.

**Milestone three is under way** ([999](999_todo.md)): an agent builds a CRM on its own. One app
([120](120_one_app.md)) is built. Of the atoms ([040](040_catalog.md)), these are built: `cell` with the style keys
(layout, box, type, placement, all by screen size) and `each`; `text`; `image`; `link`; `button` with variants
and sizes; `divider`; `svg`; `audio`; `video`; `iframe`; the controls (`input`, `textarea`,
`select`, `checkbox`, `toggle`, and `radio` groups), with `var` or with `name` in a `form`, and the `submit`
button; `file`, whose picked files go out with a call as multipart, through `serve`; `dialog`; `markdown`; and the
`outlet`, which `go` fills. That's every atom on 040's task list. Components
([090](090_components.md)) are built: component files, the `use` node, `props.*` (steps too), each instance's
`local.*`, and events inside a component. A component's own steps fire events (no `call`, `reload`, or custom action),
and the app's handlers for them do the rest. A partial sees only its props; the page in the outlet also reads `app.*`. Of what agents need from `serve` ([110](110_agents.md)), `GET /agents`
is built: it serves the configs folder's `AGENTS.md` with `BASE` filled in. So is the log: each load's
problems, each failed call, and each failure a client reports through the mirror go into `genesis.log` in the logs
folder, and agents read it at `GET /api/log`. So is the webhook: `POST /api/push/{name}` fires `push.{name}` in
every open screen. Of display logic ([080](080_display_logic.md)), conditions are built, in `show` on every node and
`disabled` on buttons and controls; checks on controls and formats are still to come.

## What this is

Genesis is a screen an AI builds on the fly. Whatever an agent wants to show a person, in whatever form
it judges most useful, it shows at once: a table to review, choices to pick from, a form to fill, a
dashboard to watch. The person answers by clicking, choosing, and typing, so the two work together
without everything going through prose. It isn't a tool for building apps; it's a way for people and
AI to interact by more than language.

The screen is one app: a folder of TOML holding the variables, the APIs it talks to, and a tree of
atoms from a fixed catalog. Genesis reads it at runtime, draws it, redraws it live whenever a file
changes, and wires every read and write to an API. An agent rebuilds, rearranges, and redesigns it
by writing files. The TOML *is* the app; there's no generated code to own.

It's the sister product of PurpleFlow and follows the same philosophy, but it's fully decoupled
from PurpleFlow and from any other API. PurpleFlow is a natural engine room behind it (workflows, data,
logic), and a PurpleFlow webhook is just another endpoint.

It's a power tool: safeguards are options with defaults, not limits. It fails loud. See `AGENTS.md`.

## The whole model

1. **The app is the configs folder**: `app.toml` at the top, and any other TOML files in any folders,
   put together. Folders only organize; every file name is unique across the whole app, and nothing is
   defined twice. Agents reach it only through dufs, as in PurpleFlow. It's
   loaded into closed Rust types; unknown keys are errors. See [020](020_config.md), [120](120_one_app.md).
2. **One screen of atoms.** The screen is a tree of nodes from a closed catalog of atoms, as small as
   plain HTML: `cell` (a `div`) for layout, and `text`, `image`, `svg`, `divider`, `audio`, `video`, `iframe`,
   `link`, `button`, each form control (including `radio` and `file`), `form`, and `dialog`. Cells take closed style keys (no CSS classes) for layout, placement, box, and type, with
   screen sizes. Stacks, grids, cards, and tables are built from cells ([040](040_catalog.md)). Which parts
   show depends on variables ([080](080_display_logic.md)). There's no list of pages: `app.toml` is the
   top-level page, and its one `outlet` draws the component `go` last named. Every other component is a
   partial, and within a screen it's all show and hide.
3. **Every value comes from the state machine.** Components read immutable state; the screen always
   shows its current state. See [050](050_state.md).
4. **Everything runs on one bus.** Actions do things. Every interaction and every completed action
   fires an event, with dotted names (`signout_btn.click`, `auth.logout.ok`, `app.page.changed`). Handlers
   (`[on]`) listen for event patterns and run actions. There are no conditionals: to branch, fire a
   different event. One queue, one event at a time, a chain-depth limit, and named timers
   (`timers.toml`, run with `start` and `stop`) for anything on a clock. `sys.bus` shows who listens to what, and `sys.events` shows what happened. See [100](100_events.md).
5. **Variables have scopes:** `app.*` for the whole app, `local.*` for one node or component,
   `sys.*` for Genesis's own (`theme`, `outlet`, `auth`, `errors`), plus `item.*`, `form.*`, and `props.*` where they apply.
   **A session persists:** what the user did (`app.*`, most of `sys.*`, locals, drafts) is kept on the device;
   what an API said (sources, call results, whether you're signed in) is fetched fresh on every open. Each client
   also mirrors its state and event trail to files that agents read through dufs. See [050](050_state.md).
6. **Sources feed variables from APIs**, by a call or a stream. They call again when the variables in
   their params change (a shortcut for handlers). That's how pagination, search, and selection work without logic.
7. **Connections are HTTP APIs**: a name, a base URL, headers, and operations, with `crud` as a
   shortcut. There's only one kind. Every call fires its result as events (`.ok`, `.failed`, `.404`). See [030](030_connections.md).
8. **Display logic only.** Conditions (show, disable), checks on controls, and formats, written as TOML tables, never as
   expressions. Logic about data lives behind the API. See [080](080_display_logic.md).
9. **Fail loud.** A failed call is shown where it happened and kept in `sys.errors`. There are no retries
   and no special response shapes; the API signals problems with its status. See [030](030_connections.md).
10. **Atoms and components.** Components are reusable TOML files of atoms, with props and local state, used
    by name, and they hot-reload ([090](090_components.md)). A partial sees only what it's given; the page in
    the outlet reads the app's variables too, as `app.toml` does. Atoms are Rust, all built the same way: the `Component` trait plus one shared `Wired` wrapper that
    knows all the wiring. A new one is a small, regular piece of code that ships in a build ([040](040_catalog.md)).
11. **Changing a file changes every open screen, live, in every build.** A bad change keeps the last
    good version running and says where it went wrong: on screen, in `/status`, and in the log. See
    [060](060_hot_reload.md), [110](110_agents.md).
12. **The server makes every call.** Credentials live in its env file, set by a person, and never
    reach the configs folder or a browser. See [070](070_secrets.md).
13. **Signing in is the API's job; `serve` holds the session.** The token stays on `serve` behind an
    `httpOnly` cookie, every open asks the API whether you're still signed in, and signing back in
    resumes where you were. With `[app.auth]`, `serve` refuses all other calls until you sign in. See [050](050_state.md), [070](070_secrets.md).
14. **Agents build and drive the screen.** `GET /agents` is the guide; they write configs through `/fs/`
    and check `/status`, the mirror, and one log of every failure (`GET /api/log`). `POST /api/push/{name}`
    sends an event, with a JSON payload, into every open client. See [110](110_agents.md).

## Rules

- **No logic about data.** Wiring (naming, mapping, selecting) and display logic (conditions, checks, formats) are
  Genesis's. Filtering, sorting, computing, and deciding are the API's.
- **The catalog is closed, and atomic.** A new node type is a spec and a code change, and it's an atom, never a
  composite. Composites are components. No raw CSS classes in the TOML: only the style keys.
- **No secrets in the TOML, ever.** Only `{{ creds.NAME }}` references, only in connections.
- **Fail loud.** Every failure is visible on screen, in `sys.errors`, in `/status` (for config), and in the log. Every `*.failed` event is logged before any handler runs.
- **No conditionals in wiring.** Conditions exist only in `show`, `disabled`, and checks on controls. Handlers branch by event name.
- **One-way flow.** Configs are immutable once loaded; state changes only through the state machine;
  nothing flows back into a config.
- **Check PurpleFlow first.** Before designing a solution, see whether PurpleFlow already solved
  the same problem, and match it unless there's a reason not to.
- **Light docs in code.** Module docs and comments are short and plain. Someone who doesn't know
  Rust should be able to follow them; explain Rust terms in a few words when they come up.

## Architecture

| Piece | What |
|---|---|
| `serve` service | genesis-serve (Rust, axum): serves the renderer, loads and watches the configs, makes every call, keeps the log, proxies `/fs/` |
| `files` service | dufs: the configs folder, read-write, reachable only through `serve` |
| Browser | opens `/` on `serve` |
| Desktop | a Tauri 2 window on a `serve` URL, the same renderer |
| Mobile (later) | the same Tauri shell on Android |

See [010](010_architecture.md).

## Where things live

Built so far, then planned ([010](010_architecture.md)):

| Path | What's there |
|---|---|
| `crates/genesis-core` | pure Rust, native and wasm: the config types (`config`), event patterns (`pattern`), templates (`template`), and the bus and state machine (`bus`) |
| `crates/genesis-ui` | the Leptos renderer: `Host`, the `Component` trait, the `Wired` wrapper, and the components |
| `apps/bench` | `genesis-bench`: runs the app's bus with no screen |
| `apps/web` | the browser entry (wasm), built by trunk with Tailwind and daisyUI |
| `apps/serve` | `genesis-serve`: the renderer, the config stream, calls, and the mirror |
| `samples/` | sample apps (`hello`, `contacts`), each a whole configs folder; the real configs folder is the user's |
| `tests/deployed/` | checks against a real `serve` and a real browser (`scripts/check-hello.sh`) |
| `state/` | the mirror: each client's state and event trail (read-only to agents at `/fs/.state/`) |
| `logs/` | `genesis.log`: every load problem, failed call, and client failure, one line each, read at `/api/log` ([110](110_agents.md)) |
| `apps/desktop` | the Tauri shell: which `serve` page the window opens (the window itself is behind the `app` feature) |
| `.github/workflows/ci.yml` | CI on every pull request and push to `main`: precommit, coverage, `cargo audit`, gitleaks, and the deployed checks (browser and Docker) |
| `Dockerfile`, `docker-compose.yml` | `serve` and `files` (dufs), as PurpleFlow runs its files service |
| `docs/authoring.md` | the guide for agents and the people they work with, copied into the configs folder as `AGENTS.md` and served at `/agents` |

## Dependencies

In use now, then planned. Check current versions before adding any.

| Need | Package |
|---|---|
| Config types, TOML, JSON | `serde`, `toml`, `serde_json` |
| SVG and Markdown, read safely | `roxmltree`, `pulldown-cmark` |
| Renderer (Rust to wasm, signals) | `leptos` 0.8 (client-side in the browser, server-side for tests), built with `trunk` |
| Styling and themes | daisyUI 5 on Tailwind CSS 4 (the standalone binary, built ahead of time) |
| Server | `axum`, `tokio`, `tower-http`, `reqwest` (rustls), `notify` |
| Desktop and mobile shell | `tauri` 2 (behind `genesis-desktop`'s `app` feature) |
| Config files for agents | dufs v0.47 (container image) |
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
- [090 — Components](090_components.md): *draft.* TOML files of atoms, with props and local state, used by name.
- [100 — Events, actions, and handlers](100_events.md): *draft.* The bus: event names, `[on]`, actions, the queue, depth, timers, the workbench.
- [110 — Working with agents](110_agents.md): *draft.* The guide at `/agents`, one log of every failure, and pushing events in.
- [120 — One app, built on the fly](120_one_app.md): *implemented.* The pivot: the configs folder is one app with unique file names, and the catalog is atoms.
