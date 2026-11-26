# Building screens with Genesis

Welcome. This guide is for you, the agent, and for the person who works with you. It says what Genesis
is for, how to write an app, and the patterns that make apps work well.

Genesis is a screen you build for a person, on the fly. Some things land better as a screen than as a
paragraph: a list to pick from, a form to fill in, a number that updates, a choice between three
options. When a screen would help, write one here. The person clicks, chooses, and types; you read what
they did, and the two of you carry on together.

An app is a folder of TOML files. You write the files, Genesis checks them and draws the screen, and
every open screen updates within a moment of each save. There's no build step and no code to write.

In this guide, **BASE** is the address of this Genesis server (it's filled in for you), and **TOKEN** is
the agent token the person gave you. Every request below, except this guide, sends the token in a header:

```
Authorization: Bearer TOKEN
```

## Reading and writing files

The app's files are at `BASE/fs/`.

| To do this | Send this |
|---|---|
| List a folder | `GET BASE/fs/FOLDER/?json` |
| Read a file | `GET BASE/fs/PATH` |
| Write a file (create or replace) | `PUT BASE/fs/PATH` with the file's text as the body |
| Delete a file or folder | `DELETE BASE/fs/PATH` |
| Move or rename | `MOVE BASE/fs/PATH` with a `Destination: BASE/fs/NEWPATH` header |
| Make a folder | `MKCOL BASE/fs/FOLDER/` |

For example, with curl:

```
curl -H "Authorization: Bearer TOKEN" "BASE/fs/?json"
curl -H "Authorization: Bearer TOKEN" BASE/fs/app.toml
curl -X PUT -H "Authorization: Bearer TOKEN" --data-binary @app.toml BASE/fs/app.toml
```

Start by listing the folder and reading what's there. If there's an app already, the person is probably
using it, so change it with care.

## What an app is

```
app.toml              the app: [app], and the screen in [[body]]
timers.toml           the app's timers, if it has any
crm.toml              anything else: connections, sources, handlers, actions
views/
  home.toml           a component (it has a [component] table)
  contact_row.toml    another component
```

- **`app.toml`** holds `[app]` (the app's `name`, and its `theme`) and the screen, as a list of
  `[[body]]` nodes.
- **Any other file adds to the app.** Put connections, `[state]`, sources, handlers and actions in
  whichever files make sense to you. Folders only organize; they add nothing to names.
- **A file with a `[component]` table is a component**, a piece of screen you can use again. It's
  named by its file name: `views/contact_row.toml` is `contact_row`.
- **`timers.toml`** holds the app's timers.
- **Every file name is unique across the whole app**, in any folder. That's how a name always means
  one file.
- `.state/` is where Genesis mirrors what each open screen holds, for you to read (see
  [Check that your change worked](#check-that-your-change-worked)). Don't write there.

## Comments for people

Start every file with a `#` comment that says what it's for, written for a person skimming the folder,
not for a machine. Add a comment wherever the why isn't obvious: why a timer is 300 ms, why a handler
reloads a list.

```toml
# contact_row.toml: one contact in the list, with a button to open it.
```

## A first app

```toml
# app.toml: a greeting, and a button that counts.
[app]
name = "Hello"

[state]
clicks = 0

[[body]]
type = "cell"
max_width = "md"
margin_x = "auto"
pad = 6
gap = 4

[[body.children]]
type = "text"
text = "Hello there."
level = 1

[[body.children]]
type = "text"
text = "You've clicked {{ app.clicks }} times."

[[body.children]]
type = "button"
label = "Click me"
variant = "primary"
on_click = { step = "app.clicks" }
```

Write that to `BASE/fs/app.toml`, check `BASE/status`, and open `BASE` in a browser.

Three ideas carry the whole of Genesis:

1. **The screen is a tree of nodes** from a fixed catalog: cells, text, buttons, inputs, and so on.
2. **State is a set of variables** the screen shows through templates: `{{ app.clicks }}`.
3. **Everything that happens is an event**, and handlers answer events with steps that change state,
   call APIs, or fire more events. A button's `on_click` is a handler written on the button.

There are no `if`s, no loops, and no expressions. What would be an `if` is a different event, and what
would be a computation is the API's job. The patterns below show how far that goes.

## The screen

### Nodes

Each node has a `type`. A `cell` holds other nodes in `children`, and lays them out.

```toml
[[body]]
type = "cell"
flow = "row"
gap = 2

[[body.children]]
type = "button"
id = "save"
label = "Save"

[[body.children]]
type = "button"
id = "cancel"
label = "Cancel"
variant = "ghost"
```

Every node takes an **`id`**: the name it fires its events under (`save.click`), and the name its
local state and form draft are kept by. A node without an `id` fires nothing you can handle in `[on]`,
though its own `on_click` still works.

| Type | What it is | It takes |
|---|---|---|
| `cell` | a box that lays out its children | `children`, `each`, `empty`, `error`, and the layout, box, type and placement keys |
| `text` | a line of text, or a heading with `level` 1 to 3 | `text`, `level` |
| `markdown` | longer prose, drawn safely (no raw HTML) | `text` |
| `image` | a picture | `src`, `alt` (required, `""` for decoration) |
| `svg` | an inline drawing, checked when it loads | `svg` (the markup), `label` |
| `link` | a link; one to another site opens in a new tab | `text`, `href` |
| `divider` | a line between sections | `color` |
| `audio`, `video` | a player | `src`, `controls`, `autoplay`, `loop`, `muted`; `video` also `poster` |
| `iframe` | another page, sandboxed | `src`, `title` (required), `sandbox` |
| `button` | a button | `label`, `variant`, `size`, `submit`, `on_click` |
| `input` | a text box | `var` or `name`, `kind` (`text`, `email`, `password`, `number`, `date`, `search`), `label`, `placeholder` |
| `textarea` | a bigger text box | as `input`, without `kind`, plus `rows` |
| `select` | a dropdown | as `textarea`, plus `options` |
| `checkbox`, `toggle` | on or off | `var` or `name`, `label` |
| `radio` | one of a group | `var` or `name` (the group), `value`, `label` |
| `file` | a file picker, inside a form | `name`, `label`, `accept`, `multiple`, `max_size` |
| `form` | a group of controls that submits together (put a cell inside to lay them out) | `children`, `data`, `on_submit`, and the placement keys |
| `dialog` | a pop-up over the screen | `children`, `open`, `title`, `on_close` |
| `use` | a component | `component`, `props` |
| `outlet` | where pages swap, in `app.toml` only | `default` |

`button` takes `variant` (`"primary"`, `"secondary"`, `"accent"`, `"neutral"`, `"ghost"`, `"link"`,
`"error"`) and `size` (`"xs"`, `"sm"`, `"md"`, `"lg"`). Templates work in `text`, `src`, `href`, and
labels.

### Layout and style

Style is a closed set of keys, each with a short list of values. A value outside its list is a load
problem that names the values it takes, so you can't get it quietly wrong.

| Group | Keys |
|---|---|
| layout (cells) | `flow` (`"column"`, `"row"`), `grid` (`1` to `12`, or `"fit"`), `gap`, `align`, `justify`, `wrap` |
| placement (any node) | `span`, `offset`, `grow`, `width` (`"full"`, `"fit"`, `"1/2"`, …), `max_width` (`"sm"` to `"6xl"`, `"prose"`) |
| box | `pad`, `pad_x`, `pad_y`, `margin`, `margin_x`, `margin_y`, `bg`, `border`, `radius`, `shadow`, `height`, `scroll` |
| type | `size` (`"xs"` to `"4xl"`), `weight`, `color`, `text_align`, `font` (`"sans"`, `"mono"`) |

- **Spacing** (`gap`, `pad`, `margin`) is `0`, `1`, `2`, `3`, `4`, `5`, `6`, `8`, `10`, `12` or `16`,
  each step a quarter of a rem. `margin_x = "auto"` centers.
- **Colors** are the theme's: `"base-100"`, `"base-200"`, `"base-300"`, `"base-content"`, `"neutral"`,
  `"primary"`, `"secondary"`, `"accent"`, `"info"`, `"success"`, `"warning"`, `"error"`, and each one's
  `-content`, the text color that reads on it. Text also takes `color = "muted"`.
- **A cell takes `flow` or `grid`, not both.** `grid = "fit"` fits as many columns as there's room for.
- **Any style key can change with the screen's width.** Give a table, smallest first: `base` for every
  screen, then `sm`, `md`, `lg`, `xl`.

```toml
[[body]]
type = "cell"
grid = 12
gap = 4

[[body.children]]
type = "cell"
span = { base = 12, md = 6, lg = 4 }    # full width on a phone, half on a tablet, a third on a desktop
pad = 4
bg = "base-200"
radius = "box"
```

The look comes from the theme, `[app] theme` (a daisyUI theme name, `"light"` by default). The person
can change it if you bind a select to `sys.theme`.

### Lists: `each`

A cell with `each` draws its children once for each element of a list, with `item` set to that element.

```toml
[[body]]
type = "cell"
each = "app.contacts"
empty = "No contacts yet."
gap = 2

[[body.children]]
type = "text"
text = "{{ item.name }} at {{ item.company }}"
```

The cell looks after the list's other states on its own: a loading state while it's being fetched, a
loud error in its place if the fetch failed, and `empty` when there's nothing.

### Controls

A control works one of two ways.

- **With `var`, anywhere:** it reads and writes that variable as the person types or clicks. Use it for
  a search box, a filter, a switch.
- **With `name`, inside a `form`:** it fills the form's draft, and nothing changes until the form is
  submitted. The `submit` event's payload holds every named control's value as `event.form`.

```toml
[[body]]
type = "select"
var = "sys.theme"
label = "Theme"
options = [{ label = "Light", value = "light" }, { label = "Dark", value = "dark" }]
```

A `select` can also take its options from a variable: `options = "app.companies"`, with
`option_label` and `option_value` pointers (`"/label"` and `"/value"` unless you say otherwise). A value
keeps its type: a number from a `number` input, `true` or `false` from a checkbox.

### Dialogs

A dialog is open while its `open` variable is anything but `false` or `null`. Set the variable to open
it, and set it `false` to close it. When the person closes it themselves (Escape, or a click outside),
it fires `.close`, sets the variable `false`, and runs `on_close`.

```toml
[[body]]
type = "dialog"
id = "confirm_delete"
open = "app.deleting"
title = "Delete this contact?"

[[body.children]]
type = "button"
id = "delete_yes"
label = "Delete"
variant = "error"
```

## State and templates

| Name | What it is | Written by |
|---|---|---|
| `app.*` | the app's variables, from `[state]` and `[sources]`, read in `app.toml` and the page in the outlet | steps, and sources |
| `local.*` | a component's own variables, from its `[state]` | steps inside it |
| `sys.*` | Genesis's own (below) | steps can set `sys.theme`; the rest are read-only |
| `event.*` | inside a handler: the event's `name` and payload | read-only |
| `item.*` | inside a cell with `each`: the element | read-only |
| `props.*` | inside a component: what it was given | read-only |

Declare app variables, with their starting values, in a `[state]` table in any file:

```toml
[state]
page = 1
selected = ""
contacts_open = false
```

A **template** is `{{ name.path }}`: a lookup, never an expression. A string that's only a template
keeps the value's type, so `to = "{{ event.form }}"` sets a whole table. Inside longer text the value
becomes text. In text on screen, a missing value shows as nothing; in a call's params, it's a loud error
and the call isn't made.

**What the person does is kept.** Variables they change, the page they're on, and their form drafts
stay on their device, so the app reopens as they left it. What an API said is fetched fresh each time.
List anything that should always start fresh in `[app] forget = ["scratch"]`.

**`sys.*`** holds `theme`, `outlet` (the page in the outlet), `errors` (every loud failure, newest
first), `events` (the recent event trail), `bus` (every handler, and every event the app can fire) and
`sources` (each source's status: `idle`, `loading`, `ok` or `failed`).

## Events, handlers and steps

Things happen as events, and **every event has a name**:

| Fired by | Name | Payload |
|---|---|---|
| a node, by its `id` | `save.click`, `search.change`, `new_contact.submit`, `confirm_delete.close` | `value` for a control, `form` for a form, and `item` inside an `each` |
| an audio or video player | `.play`, `.pause`, `.ended` | |
| a variable changing | `app.page.changed`, `sys.outlet.changed` | `value`, `was` |
| a call | `crm.contacts.list.ok` or `.failed`, and the status, `.200` or `.404` | `status`, `body` |
| a custom action | `sign_up.done`, or `sign_up.failed` | the failure, if any |
| a timer | `poll.start`, `poll.complete`, `poll.stop` | |
| a `fire` step | any name you give it | its `with` table |
| an agent or workflow, through `BASE/api/push/NAME` | `push.NAME` | the JSON it sent |
| Genesis | `app.opened` when the screen opens, `config.loaded` each time it takes a new version of the files | |

**Handlers** live in an `[on]` table in any file except a component. Each maps an event pattern to a
list of steps:

```toml
[on]
"save.click" = [{ call = "crm.contacts.update", params = { id = "{{ app.selected }}" } }]
"*.failed" = [{ log = "{{ event.name }}" }]
"crm.contacts.4??" = [{ set = "app.problem", to = "Check that contact and try again." }]
```

A pattern matches like a file name in a shell: `*` is any run of characters, `?` is one. Every handler
that matches runs, in the order the files list them (`app.toml` first). `on_click`, `on_submit` and
`on_close` on a node are the same thing written in place, and they take one step or a list.

**Steps:**

| Step | What it does | Then fires |
|---|---|---|
| `{ set = "app.x", to = "..." }` | sets a variable | `app.x.changed` |
| `{ step = "app.page", by = 1, min = 1, max = 9 }` | adds to a number (`by` is 1 unless you say), within optional bounds | `app.page.changed` |
| `{ toggle = "app.open" }` | flips `true` and `false` | `app.open.changed` |
| `{ fire = "contact.picked", with = { id = "..." } }` | fires an event | that event |
| `{ call = "crm.contacts.get", params = {...}, into = {...}, event = "/ptr" }` | calls an API, and waits for its answer | `.ok` or `.failed`, and the status |
| `{ reload = ["app.contacts"] }` | fetches those sources again | their calls' events |
| `{ go = "contacts" }` | puts a page in the outlet | `sys.outlet.changed` |
| `{ start = "poll" }`, `{ stop = "poll" }` | starts a timer (or starts it over), or stops it | `poll.start`, `poll.stop` |
| `{ log = "..." }` | writes a line in the event trail | |
| `"sign_up"` | runs the custom action `sign_up` | `sign_up.done` or `.failed` |

The steps of a list run in order. A step that fails stops its list, and the failure is logged and shown.
A `call` pauses its list, not the app: other events keep flowing, and the list carries on when the
answer comes.

**Custom actions** name a list of steps so you can run it from many places:

```toml
[actions.clear_search]
do = [{ set = "app.typed", to = "" }, { set = "app.query", to = "" }]

[on]
"clear.click" = ["clear_search"]
"app.opened" = ["clear_search"]
```

**Failures are loud.** Every `.failed` event goes into `sys.errors`, the event trail and the server's log
before any handler runs. If no handler matches it, the screen shows a short message too. A handler can
say it better; it can't make a failure disappear.

**Chains have a limit.** An event fired by a handler is one deeper than the event that ran it. Past
`[app] max_chain` (100 by default) the chain stops, loudly. A timer starts a fresh chain each time it
completes, so a repeating timer can run forever while a loop with no timer in it is stopped.

## Timers

All the app's timers are in `timers.toml`, one table each:

```toml
# timers.toml: every timer the app uses, by name.
[poll]
ms = 30000        # from start to .complete
repeat = true     # starts again each time, until stopped

[save_slow]
ms = 5000
```

`{ start = "poll" }` starts one and fires `poll.start`; starting one that's running starts it over.
When its time is up it fires `poll.complete`. `{ stop = "poll" }` stops it and fires `poll.stop`. The
[timer patterns](#timers-wait-repeat-and-give-up) show what to do with them.

## Components

A component is a file with a `[component]` table. It says which **props** it takes, and may have its
own **local** state:

```toml
# contact_card.toml: a contact, with more detail on request.
[component]
props = ["contact"]
optional = ["compact"]

[state]
expanded = false

[[body]]
type = "cell"
pad = 4
gap = 2
bg = "base-100"
border = true
radius = "box"

[[body.children]]
type = "text"
text = "{{ props.contact.name }}"
weight = "semibold"

[[body.children]]
type = "button"
id = "more"
label = "More"
variant = "ghost"
size = "sm"
on_click = { toggle = "local.expanded" }
```

Use it with a `use` node, passing each prop as a template:

```toml
[[body]]
type = "cell"
each = "app.contacts"
grid = "fit"
gap = 4

[[body.children]]
type = "use"
id = "card"
component = "contact_card"
props = { contact = "{{ item }}" }
```

- **A component sees only what it's given:** its props, its locals, and `sys.*`. It can't read `app.*`.
  That's what lets the same component work anywhere. The one exception is a page in the outlet, which
  reads `app.*` too (see [Pages](#pages-go-and-the-outlet)).
- **Its events are named under the `use` node's `id`:** the `more` button above fires `card.more.click`,
  and every card in the list fires that same name, with `item` in the payload to tell them apart.
- **Its own steps only change its locals and fire events.** It can't call an API, `go`, start a timer,
  `log`, or write `app.*`. It says what happened; the app decides what to do about it. See
  [Components fire events, the app decides](#components-fire-events-the-app-decides).
- Ten cards have ten `expanded`s, each kept on the device.

## Connections and calls

A connection is an HTTP API, a PurpleFlow webhook included:

```toml
[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }
timeout = 30                          # seconds; 0 waits forever

[connections.crm.ops.search]
method = "POST"
path = "/contacts-search"

[connections.crm.crud.contacts]       # list, get, create, update and delete in one go
style = "post"                        # PurpleFlow webhooks take everything as a POST
```

- An **operation** has a `method` and a `path`, and may add its own `headers` and `timeout`. `{id}` in
  a path takes the param `id`. The other params go in the query string for `GET`, `HEAD` and `DELETE`,
  and in a JSON body otherwise.
- **`crud`** makes five operations, `crm.contacts.list`, `.get`, `.create`, `.update` and `.delete`.
  `style = "rest"` (the default) makes them `GET /contacts`, `GET /contacts/{id}`, and so on;
  `style = "post"` makes them `POST /contacts-list`, `POST /contacts-get`, and so on. `key` names the
  record's param (`"id"`), and `path` changes the base name.
- **A call** sends params and can copy parts of the answer into variables with `into`, by
  [JSON Pointer](https://www.rfc-editor.org/rfc/rfc6901) (`""` is the whole answer):

```toml
[on]
"refresh.click" = [{ call = "crm.contacts.list", params = { page = "{{ app.page }}" }, into = { "app.contacts" = "/rows" } }]
```

Any `2xx` answer is a success, and anything else is a failure. Genesis doesn't reshape data beyond picking
a part of it: filtering, sorting, joining and counting are the API's job.

### Sources

A source is an app variable an API fills, with the wiring done for you:

```toml
[sources.contacts]
call = "crm.contacts.list"
params = { page = "{{ app.page }}" }
select = "/rows"
```

It calls when the app opens, and again whenever a variable in its params changes: set `app.page` and the
list follows. `load_on = ["some.event"]` calls on those events instead of when the app opens, and
`load_on = []` waits for a `{ reload = ["app.contacts"] }`. Its status is in `sys.sources.contacts`, and
a cell showing it handles loading and failure in its own place.

### Credentials

API keys never go in the files. Write `{{ creds.NAME }}` in a connection's headers or `base_url`, and the
person running the server sets `GENESIS_CREDS_NAME` in its environment. The server fills it in as it
makes the call; the value never reaches the files, the browser, or you. `BASE/status` doesn't list
missing ones yet, but a call needing one that isn't set fails saying `credential NAME isn't set`.

## Patterns

This is where Genesis comes together. Each pattern is a complete app: every block that starts with a
file name is a whole file, and together they load as they are.

### Show a list from an API, and open one

```toml
# app.toml: contacts from the CRM, and the one the person opened.
[app]
name = "Contacts"

[state]
selected = 0               # the id of the contact that's open, 0 for none

[[body]]
type = "cell"
grid = 12
gap = 6
pad = 6

[[body.children]]
type = "cell"
span = { base = 12, md = 5 }
each = "app.contacts"
empty = "No contacts yet."
gap = 2

[[body.children.children]]
type = "use"
id = "row"
component = "contact_row"
props = { contact = "{{ item }}" }

[[body.children]]
type = "cell"
span = { base = 12, md = 7 }
pad = 4
gap = 2
bg = "base-200"
radius = "box"

[[body.children.children]]
type = "text"
text = "{{ app.contact.name }}"
level = 2

[[body.children.children]]
type = "text"
text = "{{ app.contact.email }}"
color = "muted"

[[body.children.children]]
type = "markdown"
text = "{{ app.contact.notes }}"
```

```toml
# crm.toml: the CRM's webhooks, and what the app reads from them.
[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.crud.contacts]
style = "post"

[sources.contacts]
call = "crm.contacts.list"
select = "/rows"

# Fetched when a contact is opened, not when the app opens: there's nothing to fetch until then.
[sources.contact]
call = "crm.contacts.get"
params = { id = "{{ app.selected }}" }
load_on = []

[on]
"contact.picked" = [{ set = "app.selected", to = "{{ event.id }}" }]
```

```toml
# contact_row.toml: one contact in the list. It says it was picked; the app decides what that means.
[component]
props = ["contact"]

[[body]]
type = "cell"
flow = "row"
justify = "between"
align = "center"
pad = 3
bg = "base-100"
border = true
radius = "box"

[[body.children]]
type = "text"
text = "{{ props.contact.name }}"

[[body.children]]
type = "button"
id = "open"
label = "Open"
size = "sm"
on_click = { fire = "contact.picked", with = { id = "{{ props.contact.id }}" } }
```

Opening a contact sets `app.selected`. The `contact` source has `app.selected` in its params, so it
fetches the contact, and the right-hand side follows. No handler mentions the detail view at all.

### Components fire events, the app decides

In the list above, `contact_row` doesn't know what opening a contact means. It fires `contact.picked`
with the contact's id, and the app's handler sets `app.selected`. Tomorrow the same row could open a
dialog, start a call, or go to another page, and the row wouldn't change.

That's the rule for every component: **its own steps change its locals and fire events, and nothing
else.** Events are universal, so the name a component fires is the name the app hears. Genesis puts no
structure on names; `contact.picked` is just a convention you choose.

You don't always need `fire`. Every node in a component already fires events under its `use` node's
`id`, so the app could handle `row.open.click` directly, with the contact in `event.item`. Fire your own
event when the name should say what happened (`contact.picked`) rather than which button was pressed.

When the caller should choose the steps, pass them in as a prop. The component runs them with
`"{{ props.NAME }}"`, and they run in the app, where they were written:

```toml
# In the caller:
[[body.children]]
type = "use"
component = "contact_row"
props = { contact = "{{ item }}", on_open = [{ set = "app.selected", to = "{{ event.item.id }}" }] }

# In contact_row.toml, with props = ["contact", "on_open"]:
[[body.children]]
type = "button"
label = "Open"
on_click = "{{ props.on_open }}"
```

### Save a form, and branch on the answer

```toml
# app.toml: add a contact, and say plainly what happened.
[app]
name = "New contact"

[state]
problem = ""
added = ""

[[body]]
type = "form"
id = "new_contact"
max_width = "md"

[[body.children]]
type = "cell"
gap = 3
pad = 6

[[body.children.children]]
type = "input"
name = "name"
label = "Name"

[[body.children.children]]
type = "input"
name = "email"
kind = "email"
label = "Email"

[[body.children.children]]
type = "button"
label = "Add contact"
variant = "primary"
submit = true

[[body.children.children]]
type = "text"
text = "{{ app.problem }}"
color = "error"

[[body.children.children]]
type = "text"
text = "{{ app.added }}"
color = "success"
```

```toml
# contacts.toml: what adding a contact does, and what each answer means.
[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.crud.contacts]
style = "post"

[on]
"new_contact.submit" = [
  { set = "app.problem", to = "" },
  { call = "crm.contacts.create", params = { name = "{{ event.form.name }}", email = "{{ event.form.email }}" } },
  { set = "app.added", to = "Added {{ event.form.name }}." },
]

# Any failure gets a plain message. A 409 gets a better one, because it runs after.
"crm.contacts.create.failed" = [{ set = "app.problem", to = "That contact didn't save. Try again in a moment." }]
"crm.contacts.create.409" = [{ set = "app.problem", to = "Someone already has that email." }]
```

The list after the call runs only when the call works: a failed call stops its list. So the success
message needs no condition. A failure fires `crm.contacts.create.failed` and then the status,
`crm.contacts.create.409`, and each has its own handler.

### Branch by event name

Handlers never test anything. **To branch, fire different events**, and give each its own handler. The
answer to a call already does this, and there are three more ways.

- **Different buttons fire different events.** An Approve button and a Reject button are two handlers,
  not one handler with an `if`.
- **Let the API pick the branch.** `event = "/outcome"` on a call fires an event named by the answer's
  value at that pointer: an answer of `{"outcome": "needs_review"}` fires `jobs.score.needs_review`.
  Deciding is the API's job, and naming the branch is how it tells the screen.
- **Custom actions report how they went**, with `.done` or `.failed`.

```toml
# app.toml: score a job; the API says what happens next.
[app]
name = "Review"

[state]
verdict = ""
reviewing = false

[[body]]
type = "cell"
flow = "row"
gap = 2
pad = 6

[[body.children]]
type = "button"
id = "score"
label = "Score it"
variant = "primary"

[[body.children]]
type = "text"
text = "{{ app.verdict }}"

[[body]]
type = "dialog"
open = "app.reviewing"
title = "This one needs a person"

[[body.children]]
type = "button"
id = "approve"
label = "Approve"
variant = "primary"

[[body.children]]
type = "button"
id = "reject"
label = "Reject"
variant = "error"
```

```toml
# review.toml: one handler for each way it can go.
[connections.jobs]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.jobs.ops.score]
method = "POST"
path = "/score-job"

[connections.jobs.ops.decide]
method = "POST"
path = "/decide-job"

[on]
"score.click" = [{ call = "jobs.score", event = "/outcome" }]
"jobs.score.approved" = [{ set = "app.verdict", to = "Approved on its own." }]
"jobs.score.rejected" = [{ set = "app.verdict", to = "Rejected on its own." }]
"jobs.score.needs_review" = [{ set = "app.reviewing", to = true }]

"approve.click" = [{ call = "jobs.decide", params = { verdict = "approve" } }, "close_review"]
"reject.click" = [{ call = "jobs.decide", params = { verdict = "reject" } }, "close_review"]

[actions.close_review]
do = [{ set = "app.reviewing", to = false }, { set = "app.verdict", to = "Decided." }]
```

### Pages: `go` and the outlet

An app can swap whole pages. `app.toml` is the top-level page: put one `outlet` in its body, and
everything outside the outlet stays on screen, like a menu. The outlet shows one component at a time,
its `default` first, and `{ go = "NAME" }` swaps in another.

```toml
# app.toml: a menu that stays, and the page below it.
[app]
name = "Pages"

[[body]]
type = "cell"
flow = "row"
gap = 2
pad = 4
bg = "base-200"

[[body.children]]
type = "button"
label = "Home"
variant = "ghost"
on_click = { go = "home" }

[[body.children]]
type = "button"
label = "New contact"
variant = "ghost"
on_click = { go = "new_contact" }

[[body]]
type = "cell"
pad = 6

[[body.children]]
type = "outlet"
default = "home"
```

```toml
# home.toml: the first page, with everyone in the CRM.
[component]

[[body]]
type = "text"
text = "Welcome back. You have {{ app.count }} contacts."
level = 1

[[body]]
type = "cell"
each = "app.contacts"
empty = "No contacts yet."

[[body.children]]
type = "text"
text = "{{ item.name }}"

[[body]]
type = "button"
id = "start"
label = "Add someone"
on_click = { fire = "nav.new_contact" }
```

```toml
# new_contact.toml: a page with a form. The app does the saving.
[component]

[[body]]
type = "form"
id = "form"

[[body.children]]
type = "cell"
gap = 3

[[body.children.children]]
type = "input"
name = "name"
label = "Name"

[[body.children.children]]
type = "button"
label = "Save"
variant = "primary"
submit = true
```

```toml
# pages.toml: what each page's events do.
[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.crud.contacts]
style = "post"

[sources.contacts]
call = "crm.contacts.list"
select = "/rows"

[state]
count = 0

[on]
"crm.contacts.list.ok" = [{ set = "app.count", to = "{{ event.body.total }}" }]
"nav.new_contact" = [{ go = "new_contact" }]
"new_contact.form.submit" = [
  { call = "crm.contacts.create", params = { name = "{{ event.form.name }}" } },
  { reload = ["app.contacts"] },
  { go = "home" },
]
"sys.outlet.changed" = [{ log = "page: {{ event.was }} to {{ event.value }}" }]
```

- A page in the outlet is drawn like a component used with no props, named after itself: the form in
  `new_contact` fires `new_contact.form.submit`. So a page needs no props, and fires events for the app
  to act on. Even moving to another page is the app's: `home` fires `nav.new_contact`, and the app goes.
- `go` takes a page's name written out, not a template.
- `sys.outlet` holds the current page, and it's kept, so the app reopens on the page it was closed on.
  A page's local state stays while it's out of the outlet, so going back finds it as it was left.
- **A page reads the app's variables**, `app.*`, just as `app.toml` does: in text, in an `each`, as a form's
  `data`. It follows them as they change, so when the app reloads `app.contacts`, `home` shows the new list.
  It reads them and never writes them: to change one, it fires an event and the app sets it.
- A component that reads `app.*` is a page, so a `use` node can't draw it. A partial inside a page gets what
  it needs as props, as anywhere else.

### Timers: wait, repeat, and give up

There's no built-in debounce, poll, timeout or auto-dismiss. Each is a timer and two or three
handlers, and you can see every part of it in `sys.bus`.

```toml
# app.toml: a search that waits for a pause, an inbox that keeps itself fresh,
# a save that says when it's slow, and a note that goes away on its own.
[app]
name = "Timers"

[[body]]
type = "cell"
gap = 4
pad = 6
max_width = "2xl"

[[body.children]]
type = "input"
id = "search"
var = "app.typed"
kind = "search"
placeholder = "Search contacts"

[[body.children]]
type = "cell"
each = "app.matches"
empty = "Type to search."

[[body.children.children]]
type = "text"
text = "{{ item.name }}"

[[body.children]]
type = "text"
text = "Inbox"
level = 2

[[body.children]]
type = "cell"
each = "app.inbox"
empty = "Nothing new."

[[body.children.children]]
type = "text"
text = "{{ item.subject }}"

[[body.children]]
type = "textarea"
var = "app.note"
label = "Note"

[[body.children]]
type = "button"
id = "save"
label = "Save note"
variant = "primary"

[[body.children]]
type = "text"
text = "{{ app.slow }}"
color = "muted"

[[body]]
type = "dialog"
open = "app.saved"
title = "Saved"

[[body.children]]
type = "text"
text = "Your note is in."
```

```toml
# timers.toml: every timer the app uses.
[search_pause]
ms = 300          # long enough to finish a word, short enough to feel instant

[inbox_poll]
ms = 30000
repeat = true

[save_slow]
ms = 5000

[saved_note]
ms = 2000
```

```toml
# crm.toml: the CRM's webhooks.
[connections.crm]
base_url = "https://purple.example.com/hooks"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.ops.search]
method = "POST"
path = "/contacts-search"

[connections.crm.ops.inbox]
method = "POST"
path = "/inbox"

[connections.crm.ops.save_note]
method = "POST"
path = "/save-note"
```

**Debounce: act when typing pauses.** Each keystroke starts the timer over, so it completes only once
the person stops. Then the query is set, and the source follows it.

```toml
# search.toml: search once typing pauses, not on every key.
[state]
typed = ""
query = ""

[sources.matches]
call = "crm.search"
params = { q = "{{ app.query }}" }
load_on = []

[on]
"app.typed.changed" = [{ start = "search_pause" }]
"search_pause.complete" = [{ set = "app.query", to = "{{ app.typed }}" }]
```

**Polling: fetch again every so often.** A repeating timer, started when the app opens.

```toml
# inbox.toml: the inbox, fetched when the app opens and every 30 seconds after.
[sources.inbox]
call = "crm.inbox"
select = "/messages"

[on]
"app.opened" = [{ start = "inbox_poll" }]
"inbox_poll.complete" = [{ reload = ["app.inbox"] }]
```

**Timeout: say when something's slow.** Start a timer with the call and stop it when the call ends,
however it ends. If the timer completes first, the call is taking too long. The call itself carries on:
the connection's own `timeout` is what gives up on it.

**Auto-dismiss: show something, then take it away.** Open a dialog, and close it when a timer completes.

```toml
# note.toml: save the note, say so if it's slow, and show "Saved" for two seconds.
[state]
note = ""
slow = ""
saved = false

[on]
"save.click" = [
  { start = "save_slow" },
  { call = "crm.save_note", params = { note = "{{ app.note }}" } },
  { stop = "save_slow" },
  { set = "app.slow", to = "" },
  { set = "app.saved", to = true },
  { start = "saved_note" },
]
"save_slow.complete" = [{ set = "app.slow", to = "Still saving. The server is slow right now." }]
"crm.save_note.failed" = [{ stop = "save_slow" }, { set = "app.slow", to = "" }]
"saved_note.complete" = [{ set = "app.saved", to = false }]
```

### `log`: leave yourself notes

`{ log = "..." }` writes a line into the event trail, `sys.events`, which the mirror shows you. It's a
step like any other, so put it where you want to see what happened, with templates for the detail:

```toml
[on]
"new_contact.submit" = [{ log = "adding {{ event.form.email }}" }, { call = "crm.contacts.create", params = { email = "{{ event.form.email }}" } }]
"*.failed" = [{ log = "{{ event.name }} failed" }]
```

Failures are already logged for you, in `sys.errors` and the server's log. Use `log` for the things that
aren't failures: which branch ran, what was sent, when a timer fired.

### Hear from a workflow: push

A workflow or an agent can fire an event in every open screen:

```
curl -X POST -H "Authorization: Bearer TOKEN" -H "Content-Type: application/json" \
  -d '{"contact": {"id": 42, "name": "Ada"}, "note": "Ready for review"}' \
  BASE/api/push/review_ready
```

Every open screen gets the event `push.review_ready`, with the JSON as its payload. The answer is
`{"delivered": N}`, the number of open screens; zero means nobody has it open, which isn't an error. The
name takes lowercase letters, digits and `_`, and the body must be a JSON object.

```toml
# app.toml: shows what a workflow sends, the moment it sends it.
[app]
name = "Inbox"

[state]
contact = {}
note = ""
reviewing = false

[[body]]
type = "dialog"
open = "app.reviewing"
title = "Ready for review"

[[body.children]]
type = "text"
text = "{{ app.contact.name }}: {{ app.note }}"

[on]
"push.review_ready" = [
  { set = "app.contact", to = "{{ event.contact }}" },
  { set = "app.note", to = "{{ event.note }}" },
  { set = "app.reviewing", to = true },
]
```

The push lands in the event trail and the mirror like any event, so the sender can check what it set off.

## Check that your change worked

Every save reloads every open screen. If the new version has a problem, **the old one keeps running**
and the problem is reported, so the person never sees a broken screen.

- **`GET BASE/status`** says whether the app loads: `{"status": "loaded"}`, or `"failed"` with each
  problem as `FILE:LINE:COLUMN: what's wrong`. Check it after every write.
- **`GET BASE/api/log`** is the server's log of every failure, in order: problems loading, calls that
  failed (with the API's answer and the event that started them), steps that failed in a screen, and
  pushes it refused. `?tail=50` gives the last 50 lines, and `?since=2026-11-24T10:00:00Z` the lines
  from then on.

  ```
  2026-11-24T10:02:11Z load contact_row.toml:12:5: unknown field `colour`
  2026-11-24T10:02:40Z load ok
  2026-11-24T10:05:03Z call crm.contacts.update 422 {"error":"email taken"} (client 7f3a, from save.click)
  ```

- **The mirror**, at `BASE/fs/.state/`, has a file for each open screen: `CLIENT.json` holds its whole
  state, `sys.bus` and `sys.errors` included, and `CLIENT.events.ndjson` its recent events, one a line,
  with your `log` lines among them.
- **`sys.bus`**, in the mirror, lists every handler and every event the app can fire. When nothing
  happens, look there first: "nothing listens to `crm.contacts.list.ok`" is the usual answer.

If you have the Genesis repo, `genesis-bench FOLDER` runs an app with no screen: it reads lines like
`{"fire": "save.click"}` and `{"advance": 300}` and prints every event, call and log line.

## Guidelines

- **This is a power tool.** You can build almost anything with events, timers and handlers. Build what
  helps the person, and keep it as simple as the job allows.
- **When the wiring starts to compute, move it behind the API.** Counting, deciding, looping over data,
  formatting numbers: that belongs in a PurpleFlow workflow or whatever API you call, not in handlers.
  The screen shows and asks; the API thinks.
- **Name events for what happened.** `contact.picked` reads better in a year than `row.open.click`.
- **Keep components blind.** Pass what they need as props, let them fire events, and keep the app's
  decisions in the app's handlers. Put related handlers in one file, named for what they do.
- **Let failures be loud.** Handle `.failed` to say something better, not to hide it.
- **Write for the person.** Short labels, plain words, a clear first thing to click.
- **Change a running app gently.** The person may be in the middle of something. Variables they changed
  are kept across your edits, as long as each keeps its name and the type of its starting value.

## Not built yet

These are designed but not yet in Genesis. Don't use them: the app won't load, or they'll do nothing.

- Showing and hiding by condition (`show`), `disabled`, input checks such as `required`, and `format`
  for numbers and dates.
- The `confirm` and `toast` steps. Use a dialog for both for now.
- Signing in (`[app.auth]`, `sign_in`, `sign_out`, `{{ session.token }}`) and `wipe_state`.
- Streaming answers (`stream` on an operation, and a source's `mode`).
- `on_click` on a cell, and `state` on a node outside a component's `[state]`.
- `on_change` on a control: handle `ID.change` in `[on]` instead.
- The events `server.connected`, `server.disconnected`, `config.failed`, `auth.in`, `auth.out` and
  `state.wiped`.

## Try it

1. `GET BASE/fs/?json` to see what's there.
2. Write the [first app](#a-first-app) to `BASE/fs/app.toml`.
3. `GET BASE/status` until it says `loaded`.
4. Ask the person to open `BASE`, and click the button.
5. Read `BASE/fs/.state/` to see the click count in their state.

## Rules

- `app.toml` holds `[app]` and the screen. A component holds only `[component]`, `[state]` and
  `[[body]]`. Timers live only in `timers.toml`.
- Every file name is unique across the app, and each variable, source, connection and action is
  defined once.
- Templates are lookups, never expressions.
- Credentials are `{{ creds.NAME }}`, never values, and only in a connection.
- Don't write in `.state/`.
- Check `BASE/status` after every write, and `BASE/api/log` when something doesn't work.
