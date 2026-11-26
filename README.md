# Genesis Prototool

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

**Milestones one and two are built:** the event bus, the renderer (cell, text, button), live
reload, connections and sources with server-held credentials, sessions kept on the device, and Docker
Compose with dufs for agents. **Milestone three is under way:** `serve` runs one app, the configs
folder, at `/`, and most of the atoms are built (cells with style keys and `each`, text, images, links,
buttons, dividers, svg, audio, video, iframes, the controls (inputs, textareas, selects, checkboxes,
toggles, and radios), forms, file pickers, whose files go out with a call as multipart, dialogs, and Markdown). Next come components, and what
agents need to build a screen on their own. The design is in
[`specs/`](specs/000_overview.md); start with `000_overview.md`.

Genesis runs as a server (`serve`, plus dufs for the config files) and shows its one app, the configs
folder, in a browser or a Tauri desktop window, with one renderer (Leptos + daisyUI 5).

## Running it

Set up the toolchain once ([`docs/setup.md`](docs/setup.md)), then:

```bash
(cd apps/web && trunk build)                       # the browser bundle
cargo run -p genesis-serve -- --configs samples/hello    # then open http://127.0.0.1:8080/
```

`scripts/check-hello.sh` does all that headless and clicks the button for you. The whole stack, with
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

## For agents

Working on Genesis: read [`AGENTS.md`](AGENTS.md) and [`OP-PARAMS.md`](OP-PARAMS.md).
Writing Genesis apps: `docs/authoring.md` (coming soon); until then, `samples/hello/app.toml` and
[`specs/100_events.md`](specs/100_events.md).
