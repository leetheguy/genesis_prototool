# 999 — To-do

Status: living
Created: 2026-11-21

The list of planned work and open questions, in one place. Like [000](000_overview.md), this spec
is living: it's kept current, not kept as history. Add an item when something is planned or left
open; delete it when it ships or is dropped (say why in the commit). Each item links to where it
came from.

## Planned

- **Mirror housekeeping** ([050](050_state.md), "The mirror"): deleting the files of clients gone for a day isn't
  built yet, so `state/` only grows. The 10,000-event cap per client is built.
- **Load checks for wiring** ([020](020_config.md), "Checks"): a `set` on an undeclared variable, a step naming
  no action, a bad template, all caught when the app loads, not only when they run. The bus already fails them
  loudly at runtime ([100](100_events.md)); these checks move the failure to load time, with a line number.
- **The desktop window, opened by a person.** `genesis-desktop`'s window (the `app` feature) builds on the dev
  box, and on a virtual display (Xvfb) it loads a `serve` page and holds its config stream open. Left: a person
  opens it on a real desktop, and `cargo tauri build` packages it. From [010](010_architecture.md).
- **Then:** Table, Detail, Form, and display logic ([040](040_catalog.md), [080](080_display_logic.md)); partials
  ([090](090_components.md)); signing in ([050](050_state.md), [070](070_secrets.md)); and the CRM hello world.
- **Streams** for sources: tickers, live dashboards, and streamed AI answers ([030](030_connections.md), [050](050_state.md)).
- **A webhook for sending events in.** `POST /api/{app}/push/{name}` with a JSON payload fires `push.{name}` in every
  open client of that app, so agents and workflows can drive a screen: an agent tells it `review_ready`, a PurpleFlow
  workflow tells it `job_scored`, and handlers react. Agents send `GENESIS_AGENT_TOKEN`, as for `/fs/`; workflows
  can use a credential of their own, like a PurpleFlow webhook's `auth`. Pushed events land in the trail and the
  mirror like any other, so the sender can see what its event set off. Asked for by Lee (2026-11-22). From [100](100_events.md).
- **MCP adapter for the configs folder**: PurpleFlow spec 270's tools, rebuilt in `serve`, so Claude can
  edit apps as a connector ([010](010_architecture.md)).
- **Mobile**: the Tauri shell on Android, on a `serve` URL ([010](010_architecture.md)).
- **`docs/authoring.md`**, the guide for agents writing Genesis apps, written with the first build. It
  carries the caution from [100](100_events.md): wiring that starts to compute belongs behind the API.
- **CI and a remote.** `.github/workflows/ci.yml` once the repo is on GitHub.
- **Brand themes.** A custom daisyUI theme (colors, fonts, radius) declared in `[app]`, so a prototype
  can wear a client's branding. `sys.theme` can select it like any other theme ([050](050_state.md)).

## Open questions

- **Sessions across `serve` restarts and replicas.** Sessions live in `serve`'s memory, so a restart signs
  everyone out (their state survives), and two `serve`s can't share sessions. That's fine for one server. A
  session store would be the fix if it starts to matter. From [070](070_secrets.md).
- **Shared devices without `user`.** Kept state is cleared when a different person signs in, but only if
  `[app.auth] user` is set. Should apps with `[app.auth]` be required to set it? From [050](050_state.md).
- **Uploads.** A file field in a Form, sent as multipart (PurpleFlow webhooks take files: PurpleFlow spec 190). From [040](040_catalog.md).
- **A credentials UI and host locks.** A human-only page for credentials like PurpleFlow's (spec 080), and
  per-credential allowed hosts, if env files prove clumsy. From [070](070_secrets.md).

## Ideas

- **Compiled plugin components.** Agent-written Leptos components, built into the renderer by a
  separate build step and loaded as plugins. They can't hot-reload the way TOML does, and they'd be agent
  code running in users' browsers, so this waits for real demand. From [090](090_components.md).
- **A config editor UI**, like PurpleFlow's files page, beyond what dufs's own UI gives. From [010](010_architecture.md).
- **An admin panel for app state**: a Genesis app that shows another app's `sys.*` and variables, for dogfooding.
- **Dogfood apps**: the job search review screen, Lee's microtask drip on mobile, and the CRM hello world.
