**Read `OP-PARAMS.md` (version 2.0) before you do anything else.** It holds the operating
parameters every agent follows: specs, test-driven development, writing tests worth having, the
quality gates, and the housekeeping review you do before every commit. This file adds what's
specific to Genesis.

## Project bindings

| Binding | Genesis |
|---|---|
| **precommit** | `scripts/precommit.sh`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (and for wasm on `genesis-web`), `cargo machete`, `cargo test`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`, `trunk build` of the browser bundle, and `scripts/check-css.sh` (every style class is in the built CSS) |
| **test** | `cargo test`. The renderer's tests draw to HTML natively (Leptos's server-side mode); what only a real browser does (clicks) is a deployed check with Playwright against the served app |
| **coverage** | `scripts/coverage.sh` (`cargo llvm-cov --workspace --fail-under-lines 90`). `genesis-web` is wasm-only and natively an empty program, so it adds nothing to the count |
| **mutation** | `cargo mutants --in-diff <(git diff)` on the lines a change touches; `cargo mutants -f FILE` to harden a whole file |
| **security** | `cargo audit` (known vulnerabilities) and gitleaks (secrets in history), both in CI |
| **run** | `docker compose up -d --build` (`serve` and `files`); `cargo tauri dev` for the desktop shell; `genesis-bench APP_DIR` for the bus alone |
| **CI** | `.github/workflows/ci.yml` (GitHub Actions), once the repo has a remote |
| **doc tool** | rustdoc: `//!` for modules, `///` for items, and doctests. precommit builds them with warnings as errors |
| **docs** | the specs, `README.md`, `docs/authoring.md` (the guide for agents writing Genesis TOML) |
| **copies** | `configs/AGENTS.md` ← `docs/authoring.md`, by the user (it's what `GET /agents` serves, [110](specs/110_agents.md)) |
| **branching** | a branch per piece of work, merged into `main` locally when asked; the user pushes |

Milestones one and two are built; [000](specs/000_overview.md) says what exists. The desktop window builds with
`cargo build -p genesis-desktop --features app`; with no screen, run it under `xvfb-run`. `scripts/check-compose.sh` checks the Docker stack; run it after
any change to the Dockerfile, `docker-compose.yml`, or `/fs/`.

**Browser checks:** `scripts/check-hello.sh` builds the bundle and `serve`, then drives the hello app in headless
Chromium: clicks and the mirror (`tests/deployed/hello.mjs`), live edits (`tests/deployed/reload.mjs`), and calls
through `serve` to a mock API and PurpleFlow (`tests/deployed/calls.mjs`; `GENESIS_PF_HOOKS` points it at another
PurpleFlow). Run it after any change to the renderer, `apps/web`, or `serve`.

## Genesis rules

**Genesis is a switchboard.** It lays out, shows, reloads, and wires. It never decides anything
about data: that logic lives behind the API. So:

- **No logic about data in the config.** Wiring is allowed: naming, mapping values into params
  (`{{ row.id }}`), and selecting (a JSON Pointer). So is display logic, in its closed TOML-table forms:
  conditions, input checks, and formats ([080](specs/080_display_logic.md)). Filtering, sorting,
  computing, and deciding belong to the API. Never add an expression language.
- **One state machine, one bus.** Components read immutable state and fire events; handlers run
  actions; only actions change state ([050](specs/050_state.md), [100](specs/100_events.md)).
- **No conditionals in wiring.** Conditions live only in `show`, `disabled`, and checks on controls.
  Handlers and sources branch by event name, never by testing a value.
- **The catalog is closed, and atomic.** Every node is one variant of the `Node` enum, as small as plain
  HTML: a `cell` lays out, and each thing with behavior of its own is its own atom. Composites (cards,
  tables, tabs) are TOML components ([090](specs/090_components.md)), never catalog types; a new atom is a
  spec and a code change. Every atom implements the `Component` trait and goes through the `Wired` wrapper;
  it never touches the bus or the state machine itself ([040](specs/040_catalog.md)). No raw CSS classes in
  the TOML: only the closed style keys.
- **One app.** The configs folder is the app, and every file name in it is unique, whatever folder it's in
  ([120](specs/120_one_app.md)).
- **Credentials stay on the server.** The TOML holds `{{ creds.NAME }}`, only in connections; `serve`
  resolves it at call time; `files` never sees the env file ([070](specs/070_secrets.md)).
- **Fail loud.** No silent fallbacks, retries, or special response shapes. Every failure is visible
  on screen, in `sys.errors`, or in `/status`.
- **Check PurpleFlow first.** Before designing something, look at how `~/purple_flow` solved
  it (its specs are in `specs/`) and match it unless there's a reason not to.
- **Power tool.** Timeouts, limits and the like are options with defaults, never hard blocks
  (OP-PARAMS section 1).

**Writing or changing a spec?** Follow `OP-PARAMS.md` section 2. Start with
[`specs/000_overview.md`](specs/000_overview.md): it describes Genesis as it is now and indexes
every other spec.

**Changing what a config can say?** Every doc under the **docs** binding changes in the same
commit, `docs/authoring.md` above all: it's how other agents learn to build Genesis screens.

**Versions:** check current versions on crates.io and npm before adding any dependency.
