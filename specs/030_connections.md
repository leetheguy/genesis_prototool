# 030 — Connections

Status: draft
Created: 2026-11-21

A connection is an HTTP API: a name, a base URL, its headers, and its operations. There's only one
kind. A PurpleFlow webhook is an endpoint like any other. The name is how the rest of the config
says which API it means: `crm.contacts.list`, `market.prices`.

Genesis is fully decoupled from PurpleFlow and from any other API. It knows nothing about any
API's conventions beyond what the config says.

## A connection

```toml
[connections.market]
base_url = "https://api.example.com/v1"
headers = { Authorization = "Bearer {{ creds.MARKET_KEY }}", "X-User-Token" = "{{ session.token }}" }
timeout = 30                 # seconds; 0 waits forever. Default 30

[connections.market.ops.search]
method = "GET"
path = "/symbols/search"

[connections.market.ops.prices]
method = "GET"
path = "/prices/{symbol}"
stream = "sse"               # "sse", "ndjson", or "lines"
```

- `method` (required) is any HTTP method. `path` is joined onto `base_url`.
- `{name}` in `path` takes the param `name`, URL-encoded, and that param isn't sent again. The
  rest of the params go in the query string for `GET`, `HEAD`, and `DELETE`, and in a JSON body otherwise.
- When any param holds a file reference from a `file` control ([040](040_catalog.md)), the request is
  `multipart/form-data` instead, whatever the method: each file is a part with its name and type, and every
  other param is a field. That's how a PurpleFlow webhook takes an upload.
- `headers` may hold `{{ creds.NAME }}` and `{{ session.token }}` ([070](070_secrets.md)), which `serve`
  fills in, and `{{ app.* }}` or `{{ sys.* }}`, which the client sends.
- An operation may set its own `headers` (merged over the connection's) and `timeout`.

## `crud`

```toml
[connections.crm.crud.contacts]
key = "id"         # the param naming one record; default "id"
style = "rest"     # or "post"; default "rest"
```

It expands into five operations, named `crm.contacts.list` and so on:

| Action | `style = "rest"` | `style = "post"` |
|---|---|---|
| `list` | `GET /contacts` | `POST /contacts-list` |
| `get` | `GET /contacts/{id}` | `POST /contacts-get` |
| `create` | `POST /contacts` | `POST /contacts-create` |
| `update` | `PATCH /contacts/{id}` | `POST /contacts-update` |
| `delete` | `DELETE /contacts/{id}` | `POST /contacts-delete` |

`style = "post"` fits endpoints that take everything as a POST body, such as PurpleFlow webhooks
(`base_url = ".../hooks"`). `path = "/people"` changes the base name, and a
`[connections.crm.crud.contacts.update]` table replaces any one operation. Once the connection is defined,
`crud` makes the rest of the work boring.

## Calls

The client sends `POST /api/call` with the operation's name, its params, and the values of
any `app.*` or `sys.*` templates in its headers. `serve` adds the credentials, makes the request,
and replies with the response. For a streaming operation, `serve` reads the response with the
named protocol (the same three as PurpleFlow's HTTP node) and passes each message on as it arrives:

| `stream` | One message per |
|---|---|
| `"sse"` | server-sent event: `{ event, data, id }`, with `data` decoded if it's JSON |
| `"ndjson"` | line of JSON |
| `"lines"` | line of text |

## Selecting

`select` is a [JSON Pointer](https://www.rfc-editor.org/rfc/rfc6901) into a response, or into each stream
message: `"/rows"`, `"/data/items"`, `"/0"`. Without it, the whole body is used. Selecting is the
only shaping Genesis does to data. Filtering, joining, sorting, and computing are the API's job.

## Fail loud

Genesis doesn't second-guess responses. It handles no special shapes: no partial results, no
retries, no `202` conventions. A response is either good or a failure, and every failure is loud:

| What happened | Result |
|---|---|
| 2xx with a JSON body | the body |
| 2xx with no body | `null` |
| non-2xx | failure: status and body. With `[app.auth]`, a `401` also sets `sys.auth` to `out` ([050](050_state.md)) |
| a 2xx body that isn't JSON (outside a stream) | failure: `response isn't JSON` |
| `select` finds nothing | failure: `nothing at /path` |
| a timeout, DNS failure, or refused connection | failure: what went wrong |
| a missing credential or param | failure naming it; no request is made |

Every call fires its result as events: `crm.contacts.list.ok` or `.failed`, plus the status (`.200`,
`.404`), and handlers pick what they care about ([100](100_events.md)). A failure goes into `sys.errors` and is shown where it happened ([050](050_state.md)). Each one
includes the operation, the status, and the body, cut to 500 characters, with credentials redacted.
If an API needs to signal a problem, it does so with a non-2xx status. PurpleFlow can do that
before it hands off bad data.

## Tests

- a `GET` puts params in the query, a `POST` in a JSON body, and `{id}` in the path (URL-encoded, not repeated)
- `crud` expands to both styles' tables; `key`, `path`, and a per-operation override each change what they should
- headers resolve `creds`, `app`, and `sys` templates; an operation's headers merge over its connection's
- each row of the fail-loud table produces its result, against a mock server
- `select` picks nested values, array indexes, and escaped keys (`~1`, `~0`)
- each stream protocol passes on one message per event or line, as they arrive
- `timeout` makes a slow request a failure; `0` waits; an operation's timeout beats its connection's
- no failure message contains a credential's value
- params holding a file reference go out as `multipart/form-data`, the file as a part with its name and type, the rest as fields

## Tasks

Milestone two ([999](999_todo.md)):

- [x] core: `[connections.*]` config types: `base_url`, `headers`, `timeout`, `ops`, and `crud` (both styles) expanded into operations
- [x] core: the `call` step: a call effect for the host, the reply resuming the list, `.ok`/`.failed`/`.{status}` events, `into`, `select`, and `event`
- [x] `serve`: `POST /api/call` makes the request (`reqwest`), with `{{ creds.NAME }}` from `GENESIS_CREDS_NAME`, redacted from failures; only declared operations
- [x] the browser: send calls to `serve` and hand replies back; `genesis-bench --mock`
- [x] deployed check: the contacts sample lists and creates records against a mock API, and pings a PurpleFlow webhook when one is reachable (`tests/deployed/calls.mjs`)
