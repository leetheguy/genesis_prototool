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

- **The atomic catalog** ([040](040_catalog.md)): `row`, `grid`, `card`, `divider`, `tabs`, `modal`; `badge`,
  `alert`, `stat`, `link`, `image`, `loading`, `markdown`; `list` and `table`; `form` and `input`. With the four
  built already, that's every basic part for layout, display, and forms. No composites: those are built from the parts.
- **Display logic** ([080](080_display_logic.md)): conditions for `show` and `disabled`, checks on inputs, and formats.
- **`confirm` and `toast`** ([100](100_events.md)).
- **For agents** ([110](110_agents.md)): the guide at `GET /agents`; one name per file within an app; a `.log`
  beside each file that fails; and `POST /api/{app}/push/{name}`, which fires `push.{name}` in every open client.
- **Load checks for wiring** ([020](020_config.md), "Checks"): a `set` on an undeclared variable, a step naming
  no action, a bad template, all caught at load with a line number, and logged ([110](110_agents.md)). The bus
  already fails them loudly at runtime ([100](100_events.md)).
- **`docs/authoring.md`**, the guide itself, written as each piece lands.
- **The CRM**, as a sample with its own API: two tables, companies and contacts (each contact at one company),
  each with full CRUD. Lists you can search (the API filters), a company's page with its contacts, forms to add
  and edit with checks, deletes that ask first, toasts when things save, and loud failures. The API is a small
  mock in the repo, keeping its data in a JSON file so it survives restarts.

## Planned

- **The composite library**: partials built only from catalog atoms, shipped as shared components an app can
  use: a data table (search, paging, row actions), a detail view, a sign-in form, a contact form, a confirm
  dialog, a nav bar. Each one is something an agent can also build from scratch. Waits on partials
  ([090](090_components.md)).
- **Partials** ([090](090_components.md)), and **signing in** ([050](050_state.md), [070](070_secrets.md)).
- **Mirror housekeeping** ([050](050_state.md), "The mirror"): deleting the files of clients gone for a day isn't
  built yet, so `state/` only grows. The 10,000-event cap per client is built.
- **The desktop window, opened by a person.** `genesis-desktop`'s window (the `app` feature) builds on the dev
  box, and on a virtual display (Xvfb) it loads a `serve` page and holds its config stream open. Left: a person
  opens it on a real desktop, and `cargo tauri build` packages it. From [010](010_architecture.md).
- **Streams** for sources: tickers, live dashboards, and streamed AI answers ([030](030_connections.md), [050](050_state.md)).
- **Pushing with a credential of its own.** In milestone three a push takes the agent token. A PurpleFlow
  workflow could use a credential of its own instead, like a PurpleFlow webhook's `auth` ([110](110_agents.md)).
- **MCP adapter for the configs folder**: PurpleFlow spec 270's tools, rebuilt in `serve`, so Claude can
  edit apps as a connector ([010](010_architecture.md)).
- **Mobile**: the Tauri shell on Android, on a `serve` URL ([010](010_architecture.md)).
- **CI.** `.github/workflows/ci.yml`, now that the repo is on GitHub.
- **Brand themes.** A custom daisyUI theme (colors, fonts, radius) declared in `[app]`, so a prototype
  can wear a client's branding. `sys.theme` can select it like any other theme ([050](050_state.md)).

## Open questions

Decisions for milestone three, each with what I'd pick:

- **`serve` writes logs into the configs folder,** so it mounts it read-write ([110](110_agents.md)). It writes
  only `.log` files. The other way is to write them through dufs, keeping the read-only mount, at the cost of a
  second write path when `serve` runs without Docker. I'd take the read-write mount.
- **What a log holds.** Load problems, and failures while running wiring from that file, reported by clients
  through the mirror, kept to the last 1,000 lines by default. Or only load problems, leaving run failures to
  `sys.errors` and the mirror. I'd log both: one place for an agent to look.
- **The guide is built into `serve`,** not a file you copy into the configs folder as in PurpleFlow, so it
  always matches the build's catalog ([110](110_agents.md)). The cost: you can't add your own notes to it. I'd
  build it in, and add a `configs/AGENTS.md` for your own notes later if you want one.
- **`table` is an atom.** It lays out columns of ordinary nodes (`text`, `badge`, `button`) under a header,
  the way `grid` lays out cells, and does nothing else. The data table (search, paging, sorting, row actions)
  is a composite built around it. Atomic enough?
- **Names are unique per app, not across all apps.** Every app can have an `app.toml`; within one app (its
  folder plus the shared components), no two files share a name.

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
