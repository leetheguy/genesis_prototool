# 999 — To-do

Status: living
Created: 2026-11-21

The list of planned work and open questions, in one place. Like [000](000_overview.md), this spec
is living: it's kept current, not kept as history. Add an item when something is planned or left
open; delete it when it ships or is dropped (say why in the commit). Each item links to where it
came from.

## Milestone three: an agent builds a CRM

**Done when** a fresh agent, given only `my-domain.com/agents` and the agent token, builds the CRM below
with no human help, and it works. A deployed check drives the finished CRM in a real browser.

- **The atoms** ([040](040_catalog.md)): `cell` (a `div`, with style keys for layout, placement, box, and
  type, mapped to Tailwind and daisyUI, with screen sizes), `text`, `image`, `svg`, `divider`, `audio`, `video`,
  `iframe`, `link`, `button`, `input`, `textarea`, `select`, `checkbox`, `toggle`, `radio`, `file`, `form`, `dialog`,
  and `markdown`. A cell with `each` repeats.
- **Components** ([090](090_components.md)): TOML files of atoms with props and local state, used by name.
- **For agents** ([110](110_agents.md)): the guide at `GET /agents`; one log of every failure, read at
  `GET /api/log`; and `POST /api/push/{name}`, which fires `push.{name}` in every open client.
- **A fresh agent builds the CRM.** The CRM itself is built as a sample, by hand ([130](130_crm_sample.md)),
  with its API and a deployed check. Left: a fresh agent, given only the guide and the token, builds one like it
  on its own, and the same check drives it.

## Planned

- **The composite library**: components built only from atoms, ready to use: a card, a stack and a row with
  set spacing, tabs, a badge, an alert, a stat, a data table (search, paging, row actions), a detail view, a
  sign-in form, a contact form, a confirm dialog, a nav bar. Each one is something an agent can also build
  from scratch ([040](040_catalog.md), [090](090_components.md)).
- **Signing in** ([050](050_state.md), [070](070_secrets.md)).
- **Mirror housekeeping** ([050](050_state.md), "The mirror"): deleting the files of clients gone for a day isn't
  built yet, so `state/` only grows. The 10,000-event cap per client is built.
- **The desktop window, opened by a person.** `genesis-desktop`'s window (the `app` feature) builds on the dev
  box, and on a virtual display (Xvfb) it loads a `serve` page and holds its config stream open. Left: a person
  opens it on a real desktop, and `cargo tauri build` packages it. From [010](010_architecture.md).
- **Streams** for sources: tickers, live dashboards, and streamed AI answers ([030](030_connections.md), [050](050_state.md)).
- **Pushing with a credential of its own.** In milestone three a push takes the agent token. A PurpleFlow
  workflow could use a credential of its own instead, like a PurpleFlow webhook's `auth` ([110](110_agents.md)).
- **MCP adapter for the configs folder**: PurpleFlow spec 270's tools, rebuilt in `serve`, so Claude can
  edit the app as a connector ([010](010_architecture.md)).
- **Mobile**: the Tauri shell on Android, on a `serve` URL ([010](010_architecture.md)).
- **Brand themes.** A custom daisyUI theme (colors, fonts, radius) declared in `[app]`, so a prototype
  can wear a client's branding. `sys.theme` can select it like any other theme ([050](050_state.md)).

## Open questions

- **Which system timers, by what names.** The registry has none built in yet. Lee named toast vanishing and
  connection timeouts. A toast goes away after `[app] toast_ms` or its own `ms`, on the machine's clock, but
  isn't a named timer, and a connection's `timeout` runs on the server, per connection
  ([030](030_connections.md)). Open: each built-in's name and length, and whether `timers.toml` may set a
  built-in's `ms` or use its name. From [100](100_events.md), "Timers".
- **The outlet's other details, built the smaller way.** The component in the outlet gets no props (it reads
  `app.*` instead, as Lee decided: see the Log), and an
  outlet inside a `form` or an `each` is a config error. Decided by Lee: `go` names a page by its plain file name, which
  is unique across the app ([120](120_one_app.md)); `sys.outlet` is kept on the device;
  a component keeps its locals while it's out of the outlet ([050](050_state.md)); and a name `go` can't use
  fails when the step runs, not at load ([100](100_events.md)).
- **How a stream is stopped.** [050](050_state.md) had a `cancel` naming the source, and `cancel` is gone.

- **Sessions across `serve` restarts and replicas.** Sessions live in `serve`'s memory, so a restart signs
  everyone out (their state survives), and two `serve`s can't share sessions. That's fine for one server. A
  session store would be the fix if it starts to matter. From [070](070_secrets.md).
- **Shared devices without `user`.** Kept state is cleared when a different person signs in, but only if
  `[app.auth] user` is set. Should an app with `[app.auth]` be required to set it? From [050](050_state.md).
- **A credentials UI and host locks.** A human-only page for credentials like PurpleFlow's (spec 080), and
  per-credential allowed hosts, if env files prove clumsy. From [070](070_secrets.md).

## Ideas

- **Compiled plugin components.** Agent-written Leptos components, built into the renderer by a
  separate build step and loaded as plugins. They can't hot-reload the way TOML does, and they'd be agent
  code running in users' browsers, so this waits for real demand. From [090](090_components.md).
- **A config editor UI**, like PurpleFlow's files page, beyond what dufs's own UI gives. From [010](010_architecture.md).
- **An admin panel for state**: a second Genesis that shows the first one's `sys.*` and variables, for dogfooding.
- **Dogfood screens**: the job search review screen, Lee's microtask drip on mobile, and the CRM.

## Log

- 2026-11-25: the outlet gap is closed. A page in the outlet couldn't read `app.*` and got no props, so it
  couldn't show the app's data. Lee: "Outlet should be passed the whole app.* by default behind the scenes. No extra wiring necessary." A page reads `app.*` as `app.toml` does, read-only ([090](090_components.md), "The outlet").
- 2026-11-25: any component works both ways. Lee: "anything being loaded by app.toml gets app.*. That's it. No
  restrictions." A component `app.toml` draws reads `app.*`; one drawn inside another doesn't, unless it's passed
  down as props. The load check that refused a `use` node drawing a component that reads `app.*` is gone
  ([090](090_components.md), Log).
- 2026-11-25: display logic ([080](080_display_logic.md)) and `confirm` and `toast` ([100](100_events.md)) are built, so
  they're off milestone three's list.
- 2026-11-25: the load checks for wiring ([020](020_config.md), "Checks") are built, so they're off milestone
  three's list. A `go` to a page that isn't there isn't one of them: it fails when it runs, as Lee decided
  ([100](100_events.md), Log).
- 2026-11-25: the CRM is built as a sample ([130](130_crm_sample.md)): `samples/crm/`, with its API and a deployed
  check in a real browser. What's left of the CRM item is a fresh agent building it. Building it found two gaps,
  now listed above: a form's draft that doesn't start over, and the message for two files with one name.
- 2026-11-25: both gaps the CRM found are fixed (Lee: "Fix both"): a form's draft starts over when its `data`
  changes ([040](040_catalog.md)), and the message for two files with one name gives both paths
  ([120](120_one_app.md)).
- 2026-11-26: the screen draws only what changed ([010](010_architecture.md#drawing-only-what-changed)). The CRM's
  deployed check once failed because a list's Delete button was swapped out while it was being clicked: every
  50ms tick of the clock drew lists and pages again. Lee: "Definitely dig into it."
