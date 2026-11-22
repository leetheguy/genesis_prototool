# 060 — Hot reload

Status: draft
Created: 2026-11-21

Hot reload is a feature of the product, not a convenience for developers. Configs are read at runtime, so a
saved change reaches every open screen in every build, production included. Someone can watch the
screen while an agent edits the config through dufs, and see each edit land. This is PurpleFlow's
live reload (PurpleFlow spec 090), applied to screens.

## Watching

`serve` watches the configs folder (`notify`). A change reloads every app it touches: a file in
`configs/{app}/` reloads that app, and a shared partial in `configs/components/` reloads every app that
uses it. Editors and file servers save in bursts, so a reload waits until the files have been quiet
for `GENESIS_DEBOUNCE_MS` (default `150`).

## Loading

1. `serve` reads the app and its partials, and genesis-core checks the whole thing ([020](020_config.md)).
2. **Good:** it becomes the app's current version and goes to every connected client over
   `GET /api/{app}/config`. Each client swaps it in and keeps its state ([050](050_state.md)). If
   `sys.view` no longer matches anything shown, nothing special happens: the screen shows what the config says it shows.
3. **Bad:** the app keeps its last good version. Clients get the errors and show a banner over the
   still-working screen, listing each problem's file, line, column, and message. The next good load clears the banner.
4. **Bad, with no good version yet:** clients show only the error screen.

A deleted `app.toml` is a bad load (`app.toml not found`). The app keeps running its last good version
until the file is back. A new folder with an `app.toml` is a new app, live at once.

## Telling an agent

An agent can't see the screen, so `GET /status` answers for every app: `loaded` or `failed`, when, and
the errors. It doesn't need the agent token, and it never includes credentials or config contents. After a
write through `/fs/`, an agent checks `/status` to know whether its change took.

## Tests

- a saved change reaches a connected client as a new config, without a restart
- a burst of writes within the debounce window makes one reload
- a change to a shared partial reloads every app that uses it, and no others
- a bad edit keeps the last good version running and sends every error with file, line, and column; the next good edit clears it
- a bad app at start shows the error screen
- a deleted `app.toml` keeps the last good version; recreating it reloads
- a new app folder is live without a restart
- `/status` reports each app's state and errors, and holds no credentials
- reload works in a release build exactly as in dev (deployed check)
