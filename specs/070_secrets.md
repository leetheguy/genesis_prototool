# 070 — Secrets

Status: draft
Created: 2026-11-21

A credential is a named value, such as an API key, that a connection needs. Credentials live on the
server, are set by a person, and never reach the configs folder, an agent, or a browser.

There are two kinds of secret in a Genesis app, kept apart:

| | Credentials | A user's session |
|---|---|---|
| What | the app's own keys for the APIs it calls | a token a login API returned for one person ([050](050_state.md)) |
| Lives | in `serve`'s env file | in `serve`'s memory, tied to the browser by an `httpOnly` cookie |
| Set by | whoever runs the server | the person signing in |
| In the config | `{{ creds.NAME }}` | `{{ session.token }}` |

## Credentials

- The TOML says `{{ creds.NAME }}`, the same name PurpleFlow uses, and only in a connection's or an
  operation's `headers` (or `base_url`). Anywhere else is a config error ([020](020_config.md)).
- The value is the environment variable `GENESIS_CREDS_NAME`, from `serve`'s env file. The
  `files` service never mounts that file, so nothing that can write configs can read it ([010](010_architecture.md)).
- `serve` substitutes the value into the request at the moment it makes the call, and nowhere else. The
  config sent to clients holds the reference, never the value.
- A credential that isn't set fails that call loudly (`credential NAME isn't set`), with no request
  made. `/status` lists every credential an app references that isn't set, so the person running the
  server knows what to add.
- Values are redacted (`[redacted]`) from logs, failures, and `sys.errors`.

A config can still send a credential to any URL its connection names. As in PurpleFlow, whoever writes
configs is trusted with the credentials' use, though never with their values. A credentials UI like PurpleFlow's, and
per-credential host locks, are later items in [999](999_todo.md).

## Sessions

- `serve` keeps each session's token in memory, for as long as the API keeps accepting it. A restart
  of `serve` ends every session: people sign in again, and their state is still there ([050](050_state.md)).
- The cookie is `httpOnly`, `Secure`, and `SameSite=Strict`. `serve` also refuses a call whose `Origin`
  names another site, as PurpleFlow's `/mcp` does, so another site can't fire calls with someone's session.
- Signing out, or a `401` from the API, ends the session on `serve`.

## Remaining risk

Anyone who can open an app can fire the operations it declares, with its credentials attached.
They can't read the credentials, and they can only fire what the config declares. An app with
`[app.auth]` closes this: `serve` refuses everything but signing in until someone has ([050](050_state.md)).
An app without it is open to anyone who can reach it, like a PurpleFlow webhook without `auth`.

## Tests

- `{{ creds.NAME }}` in headers or `base_url` resolves from `GENESIS_CREDS_NAME`; anywhere else is a config error
- an unset credential fails the call loudly, no request reaches the mock server, and `/status` lists it
- no credential value appears in logs, a failure, `sys.errors`, `/status`, or the config a client gets
- the `files` container has no access to the env file (deployed check)
- `{{ session.token }}` resolves to the signed-in session's token; the cookie is `httpOnly`, `Secure`, `SameSite=Strict`
- a call with another site's `Origin` is refused
- restarting `serve` ends sessions; the next `check` is `out`
