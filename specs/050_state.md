# 050 — State

Status: draft
Created: 2026-11-21

Every value on screen comes from one place: the state machine. Components hold no data of their
own. They read immutable snapshots of state, and what they do fires events. Handlers turn events into
actions, and only actions change state ([100](100_events.md)). The screen always shows the current state.
Components never know where a value came from: a starting value, a click, a call, or a stream.

## What state holds

| Part | Declared by | Example |
|---|---|---|
| app variables | `[state]` | `page = 1` |
| sources | `[sources.NAME]` | app variables an operation or stream fills; any action can also write one, and its next load replaces it |
| local variables | `state = { ... }` on a node or partial | `open = false` |
| form drafts | each Form | the values typed and not yet submitted |
| system variables | Genesis | `sys.view`, `sys.theme`, `sys.auth`, `sys.errors`, `sys.events`, `sys.bus` |

### System variables

- **`sys.view`**: which screen is showing. It starts at `[app] start` and is kept in the URL's
  `#` part, so the back button and links work. The `go` action sets it.
- **`sys.theme`**: the daisyUI theme. It starts at `[app] theme`, can be set like any variable (a
  theme picker is a Select bound to it), and is kept on this device.
- **`sys.errors`**: every loud failure, newest first: `{ at, op, status, message }`. It's read-only,
  and a Table can show it.
- **`sys.events`** and **`sys.bus`**: the event trail and who listens to what ([100](100_events.md)).

## Sources

```toml
[sources.contacts]
call = "crm.contacts.list"
params = { page = "{{ app.page }}" }
select = "/rows"
load_on = ["auth.in"]       # optional: load on these events instead of app.opened
```

A source is a shortcut for handlers ([100](100_events.md)), Rails-style: the convention does the wiring.

- **By default** it calls on `app.opened`, and again on the `.changed` event of every variable in its
  `params`. Set `app.page` and the list follows.
- **`load_on`** lists the events to call on instead of `app.opened` (`["auth.in"]`, so it waits for a
  sign-in). Param changes still reload it. `load_on = []` means it calls only on a `reload` action.
- Its calls fire events like any call: `crm.contacts.list.ok`, `.failed`, and so on.
- **Streams:** with a `stream` operation, each message updates the source. `mode = "replace"` (the
  default) keeps the latest, which suits a ticker. `mode = "append"` adds to it: text is joined onto
  text, which suits a streamed AI answer, and anything else is pushed onto a list. A stream reconnects
  when its params change, and stops on a `cancel` naming the source.

Every source has a status: `idle`, `loading`, `ok`, or `failed`. A node showing a source shows
loading and failure in its own place on its own ([040](040_catalog.md)). Conditions can test it with `status`.

## Persisting

A session persists. Close the tab or restart the device, open the app again, and it's where you
left it. The rule is simple: **what the user did is kept; what an API said is fetched fresh.**

| Kept, on this device | Fetched fresh on every open |
|---|---|
| `app.*` variables changed by `set`, `step`, `toggle`, or an `input` | every source |
| `sys.*`: `view`, `theme`, `errors`, and `events` | every variable an `into` filled (call results) |
| local variables and form drafts (except `password` fields) | `sys.auth`, which only `check` can answer |

Credentials and session tokens are never in the browser at all ([070](070_secrets.md)).

- Kept state lives in local storage, keyed by app, so each device has its own.
- `[app] forget = ["scratch"]` marks app variables never to keep, for anything an app would rather start fresh.
- A kept value whose variable no longer exists, or whose starting value now has a different JSON
  type, is dropped and the variable starts fresh. A note goes to the browser console.
- Two tabs of one app share the kept state; the last write wins.

## Signing in

Genesis has no users of its own. The API does the actual authentication. `serve` holds the session, so
the browser never sees a token, and every open asks the API afresh whether you're still signed in.

```toml
[app.auth]
login = "auth.login"        # takes the login form's params
token = "/token"            # where the token sits in login's reply
check = "auth.me"           # called on every open, and after login
user = "/id"                # optional: who is signed in, in check's reply
logout = "auth.logout"      # optional: called on sign-out
```

- **Signing in:** a Form's `on_submit = [{ sign_in = { email = "{{ event.form.email }}", password = "{{ event.form.password }}" } }]`
  calls `login`, and fires `auth.in` or `sign_in.failed`. `serve` keeps the token in a session of its own, tied to the browser by an `httpOnly`
  cookie, and connections use it as `{{ session.token }}` in their headers. The token never reaches the state.
- **`sys.auth`** is `checking`, `in`, or `out`, and each change fires `auth.in` or `auth.out`. On every open, `check` runs: a 2xx reply is `in`, and a
  `401` is `out`. Anything else is a loud failure. Screens show a sign-in form with
  `show = { var = "sys.auth", is = "out" }`.
- **Signed out mid-use:** any call that gets a `401` sets `sys.auth` to `out`. The call still fails
  loud, the sign-in form shows, and the state stays put. Sign in again and carry on where you were.
- **Signing out:** the `sign_out` action calls `logout`, ends the session on `serve`, and sets
  `sys.auth` to `out`. To also clear the screen, wire `"auth.out" = ["wipe_state"]`.
- **A different person:** when `user` is set and `check` names a different user than the one the
  kept state belongs to, the kept state is cleared before anything shows. So a shared device never shows one
  person's screen to another.
- **Required:** with `[app.auth]` set, `serve` refuses every call but `login` and `check` while the session
  isn't signed in. A credential-bearing operation can't be fired by someone who hasn't signed in.

A `password` field's value is sent with its call and then cleared. It's never kept in state, stored, or logged.
Hiding a screen is cosmetic; the API enforces access.

## The mirror

So an agent can debug a running app without a browser, each open client mirrors its state to `serve`,
which writes plain files that dufs serves read-only at `/fs/.state/` ([010](010_architecture.md)):

```
state/{app}/{client}.json        the whole state: variables, sources, sys.* (sys.bus, sys.events, sys.errors)
state/{app}/{client}.events.ndjson   the event trail, one event per line, kept to the last 10,000
```

- The client sends changes at most every 250 ms. `serve` writes each file whole (write, then rename),
  so a reader never sees half a file.
- A client is a random ID the browser keeps. Files for clients gone for a day are deleted.
- Credentials and session tokens are never in client state, so they're never in the mirror. Neither are `password` fields.
- **The mirror holds whatever app data is on screen**, and anyone with the agent token can read it.
  `[app] mirror = false` turns it off for an app. It's on by default.

## Across a config reload

The state stays ([060](060_hot_reload.md)). Variables still declared keep their values, and new ones
start at their starting values. Removed variables are dropped. A source whose `call`, `params`, or
`select` changed calls again. A form draft keeps its values if its Form is still in the same place.

## Tests

- components draw from state; only actions change it
- `[state]` starting values; `sys.view` starts at `start` and follows the URL's `#`; the back button goes back a view
- a source calls on `app.opened` and on its params' `.changed` events; `load_on` replaces `app.opened`; `load_on = []` waits for `reload`
- a stream in `replace` mode keeps the latest message; in `append` mode joins text and pushes others
- the mirror files appear under `state/{app}/`, whole, within a second of a change, with no credentials or passwords; `mirror = false` writes nothing
- after closing and reopening: user-set `app.*`, `sys.view`, `sys.theme`, `sys.errors`, locals, and drafts are back; sources and `into` results are fetched fresh; `forget` variables start fresh
- a kept value of a removed variable, or of a changed type, is dropped
- login keeps the token on `serve` only: it's in no state, storage, or response the browser gets
- `check` on open: 2xx is `in`, `401` is `out`, anything else is a loud failure
- a `401` mid-use sets `sys.auth` to `out` and keeps the state; signing in again resumes
- a different `user` clears the kept state before anything shows
- with `[app.auth]`, `serve` refuses every call but `login` and `check` until signed in
- a `password` field's value never appears in state, storage, or logs
- after a config reload: kept, new, and removed variables; a changed source calls again

## Tasks

Milestone two ([999](999_todo.md)):

- [ ] core: `[sources.NAME]` as handlers: a call on `app.opened` (or `load_on`) and on its params' `.changed`, filling `app.NAME`; the `reload` action
- [ ] core and the browser: persisting: what the user did kept in local storage and restored on open; sources and `into` results never kept; `[app] forget`
