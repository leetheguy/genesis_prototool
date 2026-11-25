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
- **Display logic** ([080](080_display_logic.md)): conditions for `show` and `disabled`, checks on controls, and formats.
- **`confirm` and `toast`** ([100](100_events.md)).
- **For agents** ([110](110_agents.md)): the guide at `GET /agents`; one log of every failure, read at
  `GET /api/log`; and `POST /api/push/{name}`, which fires `push.{name}` in every open client.
- **Load checks for wiring** ([020](020_config.md), "Checks"): a `set` on an undeclared variable, a step naming
  no action, a bad template, all caught at load with a line number, and logged. The bus already fails them
  loudly at runtime ([100](100_events.md)).
- **`docs/authoring.md`**, the guide, written as each piece lands. It's written for agents, and starts from what
  Genesis is for: a way to show a person what's most useful, on the fly, and to work with them by more than words.
- **The CRM**, as a sample with its own API: two tables, companies and contacts (each contact at one company),
  each with full CRUD. Lists you can search (the API filters), a company's page with its contacts, forms to add
  and edit with checks, deletes that ask first, toasts when things save, and loud failures. Built only from
  atoms and its own components. The API is a small mock in the repo, keeping its data in a JSON file so it
  survives restarts.

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
  connection timeouts; `toast` isn't built, and a connection's `timeout` runs on the server, per connection
  ([030](030_connections.md)). Open: each built-in's name and length, and whether `timers.toml` may set a
  built-in's `ms` or use its name. From [100](100_events.md), "Timers".
- **What `go` names, as read.** Lee: "Go's target should be a TOML page." Built as: `go` names a component's
  TOML file by the name it goes by, its file name without `.toml` (`{ go = "contacts" }` for
  `views/contacts.toml`), written out, not a template, and not a path. Any component without required props
  can be that page. To check with Lee. From [100](100_events.md), "Going".
- **When a bad `go` fails.** A name `go` can't use (no such component, one that needs props, or no outlet)
  fails when the step runs, not at load. Being confirmed with Lee. From [100](100_events.md), "Going".
- **The outlet's other details, built the smaller way.** The component in the outlet gets no props, and an
  outlet inside a `form` or an `each` is a config error. Decided by Lee: `sys.outlet` is kept on the device,
  and a component keeps its locals while it's out of the outlet ([050](050_state.md)).
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
