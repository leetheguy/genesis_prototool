# Genesis Prototool

[![CI](https://github.com/leetheguy/genesis_prototool/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/leetheguy/genesis_prototool/actions/workflows/ci.yml)

A screen an AI builds on the fly. Whatever an agent wants to show you (a table to review, choices to
pick from, a form to fill, a dashboard to watch), it writes as TOML, and Genesis draws it at once,
redraws it live as the files change, and wires it to your data through APIs. You answer by clicking,
choosing, and typing, so you and the agent can work together by more than words.

It's a dumb interface on purpose. Genesis holds display logic and nothing more. Every read and
write of data goes through an HTTP API: a [PurpleFlow](https://purple.leenathan.com) webhook, or
anything else you describe in the config. A switchboard is simple because the power plant is elsewhere.

```toml
# configs/app.toml
[app]
name = "Contacts"
theme = "corporate"

[state]
page = 1

[sources.contacts]
call = "crm.contacts.list"
params = { page = "{{ app.page }}" }
select = "/rows"

[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.crud.contacts]
style = "post"

[[body]]
type = "cell"
each = "app.contacts"
key = "item.id"
empty = "No contacts yet."
grid = 12
gap = 2

[[body.children]]
type = "text"
text = "{{ item.name }}"
span = { base = 12, md = 6 }

[[body.children]]
type = "text"
text = "{{ item.email }}"
color = "muted"
span = { base = 12, md = 6 }

[[body]]
type = "button"
label = "Next page"
on_click = [{ step = "app.page", by = 1 }]
```

That's a paged list. Save a change to the file and every open window changes with it.

## Status

**Milestones one and two are built:** the event bus, the renderer, live reload, connections and sources
with server-held credentials, sessions kept on the device, and Docker Compose with dufs for agents.

**Milestone three is built:** `serve` runs one app, the configs folder, at `/`, with every atom (cells
with style keys and `each`, text, images, links, buttons, dividers, svg, audio, video, iframes, the
controls, forms, file pickers, dialogs, Markdown, and the outlet that `go` fills); components with props
and local state; display logic (`show`, `disabled`, checks, and formats); `toast` and `confirm`; load
checks for wiring; and what agents need from `serve`: the guide at `/agents`, one log of every failure
at `/api/log`, and pushing events in. The screen redraws only what changed. `samples/crm/` is a whole
CRM built on it, with its own API and a check in a real browser. What's left is the milestone's last
test: a fresh agent, given only the guide and the token, builds the CRM on its own.

The design is in [`specs/`](specs/000_overview.md); start with `000_overview.md`. What's planned and
still open is in [`specs/999_todo.md`](specs/999_todo.md).

Genesis runs as a server (`serve`, plus dufs for the config files) and shows its one app, the configs
folder, in a browser or a Tauri desktop window, with one renderer (Leptos + daisyUI 5).

## Running it

Set up the toolchain once ([`docs/setup.md`](docs/setup.md)), then:

```bash
(cd apps/web && trunk build)                       # the browser bundle
cargo run -p genesis-serve -- --configs samples/hello    # then open http://127.0.0.1:8080/
```

`scripts/check-hello.sh` does all that headless and clicks the button for you. `samples/crm/` is a whole
CRM, with its own small API; the top of its `app.toml` says how to run it. The whole stack, with
dufs for agents at `/fs/`:

```bash
cp .env.example .env    # then set GENESIS_AGENT_TOKEN
docker compose up -d --build
```

`scripts/check-compose.sh` checks that stack's boundaries. To run an app's
events with no screen at all:

```bash
echo '{"fire": "hello_btn.click"}' | cargo run -q -p genesis-bench -- samples/hello
```

## Deploying it

On a server with Docker, from a clone of this repo:

```bash
cp .env.example .env                       # then set GENESIS_AGENT_TOKEN: openssl rand -hex 24
mkdir -p configs state logs
cp docs/authoring.md configs/AGENTS.md     # the guide; without it, /agents is a 404
docker compose up -d --build
```

- **The token is required.** Compose refuses to start without `GENESIS_AGENT_TOKEN`; it's what agents send
  for `/fs/` and the `/api/` routes.
- **The guide has to be in the configs folder.** `serve` hands out the configs folder's `AGENTS.md` at
  `/agents`, so copy `docs/authoring.md` there (and again when it changes).
- **`serve` listens on `127.0.0.1:8080` only** (`GENESIS_PORT` changes the port). Put a reverse proxy with
  HTTPS in front, passing the `Host` (or `X-Forwarded-Host`) and `X-Forwarded-Proto` headers, so the guide's
  `BASE` is your public address. With Caddy, `genesis.example.com { reverse_proxy 127.0.0.1:8080 }` does it.
- **The folders belong to user 1000** by default; `PUID` and `PGID` in `.env` change that.

Then point the agent at `https://genesis.example.com/agents` and hand it the token. It writes the app
into the configs folder through `/fs/`, and every open screen follows.

## For agents

Working on Genesis: read [`AGENTS.md`](AGENTS.md) and [`OP-PARAMS.md`](OP-PARAMS.md).
Writing Genesis apps: [`docs/authoring.md`](docs/authoring.md), with the pieces, patterns, and whole
example apps. Put a copy of it at the top of the configs folder as `AGENTS.md`,
and `serve` hands it out, open, at `/agents`, with each `BASE` replaced by the address the agent came to.
With `GENESIS_AGENT_TOKEN`, an agent reads every failure at `/api/log` and fires an event in every open
screen with `POST /api/push/{name}` ([spec 110](specs/110_agents.md)).

## License

MIT, in [`LICENSE`](LICENSE).
