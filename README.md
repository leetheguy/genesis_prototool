# Genesis Prototool

A universal, simple GUI generator. You write TOML; Genesis lays it out, shows it, reloads it live as
the files change, and wires it to your data through APIs.

It's a dumb interface on purpose. Genesis holds display logic and nothing more. Every read and
write of data goes through an HTTP API: a [PurpleFlow](https://purple.leenathan.com) webhook, or
anything else you describe in the config. A switchboard is simple because the power plant is elsewhere.

```toml
# configs/contacts/app.toml
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
type = "table"
data = "app.contacts"
empty = "No contacts yet."
columns = [
  { label = "Name", field = "/name" },
  { label = "Email", field = "/email" },
]

[[body]]
type = "button"
label = "Next page"
on_click = [{ step = "app.page", by = 1 }]
```

That's a paged list screen. Save a change to the file and every open window changes with it.

## Status

**Not built yet.** The design is in [`specs/`](specs/000_overview.md); start with `000_overview.md`.
Genesis runs as a server (`serve`, plus dufs for the config files) and shows apps in a browser or a
Tauri desktop window, with one renderer (Leptos + daisyUI 5).

## For agents

Working on Genesis: read [`AGENTS.md`](AGENTS.md) and [`OP-PARAMS.md`](OP-PARAMS.md).
Writing Genesis apps: `docs/authoring.md` (coming with the first build).
