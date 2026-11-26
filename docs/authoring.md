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
though its own `on_click` still works. Every node also takes **`show`**, and buttons and controls take
**`disabled`** ([Show, hide, and disable](#show-hide-and-disable)).

| Type | What it is | It takes |
|---|---|---|
| `cell` | a box that lays out its children | `children`, `each`, `empty`, `error`, and the layout, box, type and placement keys |
| `text` | a line of text, or a heading with `level` 1 to 3 | `text`, `level`, `format` |
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
key = "item.id"
empty = "No contacts yet."
gap = 2

[[body.children]]
type = "text"
text = "{{ item.name }} at {{ item.company }}"
```

`key` says what tells the rows apart, usually the record's id (`item` itself for a list of text). Give
one whenever the elements have an id: then each row belongs to its record, so when a row is added or
taken out, what's typed in the others stays with the right record. Every element needs its own key; a
missing or shared one is a loud error in the list's place. Without `key`, rows are kept by their place in
the list, which is fine for a list that only grows or changes in place.

The cell looks after the list's other states on its own: a loading state while it's first fetched, a
loud error in its place if the fetch failed, and `empty` when there's nothing. When the list loads again,
its rows stay on screen until the new ones come, and a row whose values changed is updated in place, so
what's typed or focused in it stays put.

### Controls

A control works one of two ways.

- **With `var`, anywhere:** it reads and writes that variable as the person types or clicks. Use it for
  a search box, a filter, a switch.
- **With `name`, inside a `form`:** it fills the form's draft, and nothing changes until the form is
  submitted. The `submit` event's payload holds every named control's value as `event.form`. A form with
  `data` starts from that record, and when the `app.*` or `local.*` variable its `data` reads changes, the
  draft starts over: open the edit form on another row and it shows that row, not what was typed before.

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

**Checks** say what a control's value has to be. Every control takes `required` (`true`, or the message
to show) and `checks`: a list of tests on its own value, each with the `message` shown while it fails.
The tests are the ones a [condition](#show-hide-and-disable) uses, without `var`, plus `min_len` and
`max_len` (characters in text, items in a list). Tests in one check all have to pass.

```toml
[[body.children]]
type = "input"
name = "seats"
kind = "number"
label = "Seats"
required = true
checks = [{ gte = 1, lte = 500, message = "1 to 500 seats." }]

[[body.children]]
type = "checkbox"
name = "terms"
label = "I agree to the terms"
required = "Please agree to the terms."     # on a checkbox or toggle, required means ticked
```

- **A form with a failing check doesn't submit.** It fires `.invalid` instead, whose payload has `form`
  and `invalid`, each failing field's message by name. The draft stays, so nothing typed is lost.
- **A control speaks once the person changes it,** or once a submit fails. A fresh form is quiet, and a
  good submit makes it quiet again.
- **An empty value passes every check but `required`,** so an optional field can be left blank.
- **The browser's own checks are off,** so a `kind = "email"` box takes anything until you add a check.
  These checks help the person typing; the API still decides what it accepts.

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

### Show, hide, and disable

`show` draws a node, and its children, only while a **condition** holds. `disabled` greys out a button
or control while one holds, so it can't be clicked or typed in. Both follow state as it changes. There's
no list of pages inside a screen: it's all show and hide.

A condition is a small table, never an expression: a variable, and one test on it.

| Test | Holds when the variable |
|---|---|
| `is = X` | equals `X`, with the same type (`"1"` isn't `1`) |
| `in = [X, Y]` | equals one of them |
| `empty = true` | is missing, `null`, `""`, `[]` or `{}`; `empty = false` is the opposite |
| `gt`, `gte`, `lt`, `lte = N` | is a number above, at least, below, or at most `N` |
| `matches = "regex"` | is text the regular expression finds a match in |
| `status = "loading"` | is a source with that status: `idle`, `loading`, `ok` or `failed` |

`all = [...]`, `any = [...]` and `not = {...}` combine conditions. A test on the wrong type, like `gt`
on text, is false rather than an error.

`var` can read `app.*`, `sys.*`, `item.*` (in a row of an `each`), `form.*` (what's typed into the
form it's in, else its `data`), and in a component `local.*` and `props.*`. A condition that reads
something its place can't see is a load problem.

```toml
# app.toml
[app]
name = "Tabs"

[state]
view = "people"

[[body]]
type = "cell"
flow = "row"
gap = 2

[[body.children]]
type = "button"
id = "people_tab"
label = "People"
variant = "ghost"
disabled = { var = "app.view", is = "people" }      # the tab you're on can't be clicked

[[body.children]]
type = "button"
id = "join_tab"
label = "Join"
variant = "ghost"
disabled = { var = "app.view", is = "join" }

[[body]]
type = "text"
text = "Everyone who's joined."
show = { var = "app.view", is = "people" }

[[body]]
type = "form"
id = "join"
show = { var = "app.view", is = "join" }

[[body.children]]
type = "cell"
gap = 2

[[body.children.children]]
type = "input"
id = "email_box"
name = "email"
kind = "email"
label = "Email"

[[body.children.children]]
type = "button"
label = "Join"
submit = true
disabled = { not = { var = "form.email", matches = "^[^@ ]+@[^@ ]+$" } }   # until it looks like an email

[on]
"people_tab.click" = [{ set = "app.view", to = "people" }]
"join_tab.click" = [{ set = "app.view", to = "join" }]
"join.submit" = [{ set = "app.view", to = "people" }]
```

- **On a cell with `each`,** `show` is tested once, for the whole list. To hide some rows, put `show`
  on the row's own children, where `item.*` is that row: `show = { var = "item.archived", is = false }`.
- **To show "Loading…" or a failure** somewhere other than the list itself, test the source's status:
  `show = { var = "app.contacts", status = "failed" }`. A cell with `each` already does this in its own place.
- **A hidden node takes no room,** and a hidden form keeps what was typed into it.

### How values look: `format`

A `text` node's `format` changes how the values its templates fill in look, never what they are. The
variable keeps its raw value, so a handler still sends `1234.5`, not `"$1,234.50"`.

| Format | `1234.5` or `"2026-11-21T09:30:00Z"` shows as |
|---|---|
| `{ number = 2 }` | `1,234.50`: grouped, with that many decimals |
| `{ currency = "USD" }` | `$1,234.50`, with the currency's own decimals |
| `{ percent = 1 }` | the number × 100: `0.123` is `12.3%` |
| `{ date = "%b %-d, %Y" }` | `Nov 21, 2026`, by a strftime pattern |
| `{ ago = true }` | `3 hours ago`, or `in 2 days` |
| `{ bytes = true }` | `1.2 KB` |
| `{ case = "title" }` | text in `"upper"`, `"lower"`, or `"title"` case |
| `{ truncate = 80 }` | text cut to 80 characters, `…` included |
| `{ map = { won = "Won", lost = "Lost" } }` | the label for each value; others as they are |

```toml
[[body.children]]
type = "text"
text = "{{ item.amount }}"
format = { currency = "USD" }
text_align = "right"

[[body.children]]
type = "text"
text = "Closed {{ item.closed_at }}"
format = { date = "%b %-d" }
```

- **One format per text.** It applies to every value in the text, so put a date and an amount in
  their own `text` nodes, side by side in a cell.
- **Dates are ISO 8601 text.** One with a zone (`Z`, `+01:00`) shows in the person's time zone; one
  without (`2026-11-21`) shows as written, so a birthday never slips a day. A date pattern takes `%Y`,
  `%y`, `%m`, `%B`, `%b`, `%d`, `%e`, `%A`, `%a`, `%H`, `%I`, `%p`, `%M`, `%S` and `%%`; `%-d` and
  the like drop the leading zero.
- **The language is the person's browser's,** unless `[app] locale = "en-GB"` names one. A German
  browser shows `1.234,50 €`.
- **A value a format can't take** (text given to `currency`) shows as it is, and the browser console
  says why.

## State and templates

| Name | What it is | Written by |
|---|---|---|
| `app.*` | the app's variables, from `[state]` and `[sources]`, read in `app.toml` and any component it draws | steps, and sources |
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
first), `events` (the recent event trail), `bus` (every handler, and every event the app can fire),
`sources` (each source's status: `idle`, `loading`, `ok` or `failed`), `toasts` (the toasts on screen)
and `asking` (the questions waiting for an answer).

## Events, handlers and steps

Things happen as events, and **every event has a name**:

| Fired by | Name | Payload |
|---|---|---|
| a node, by its `id` | `save.click`, `search.change`, `new_contact.submit`, `new_contact.invalid`, `confirm_delete.close` | `value` for a control, `form` for a form (and `invalid` on `.invalid`), and `item` inside an `each` |
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
| `{ toast = "Saved.", variant = "success" }` | shows a short message in the corner, gone on its own | |
| `{ confirm = "Delete Ada?", yes = "Delete", no = "Keep" }` | asks in a dialog, and waits: yes carries on, no stops the list | |
| `{ log = "..." }` | writes a line in the event trail | |
| `"sign_up"` | runs the custom action `sign_up` | `sign_up.done` or `.failed` |

The steps of a list run in order. A step that fails stops its list, and the failure is logged and shown.
A `call` pauses its list, not the app: other events keep flowing, and the list carries on when the
answer comes.

**`toast` and `confirm`** talk to the person without any nodes of your own:

- A **toast** goes away after `[app] toast_ms` (5000 by default), or its own `ms`, and a tap closes it
  sooner. `variant` is `info` (the default), `success`, `warning` or `error`. Its text may hold templates;
  what's missing shows as nothing, so a toast never fails.
- A **confirm** waits like a `call` does: the rest of the list runs only on yes. No (the `no` button,
  Escape, or a click outside) stops the list quietly: nothing fails and nothing fires. The buttons say
  "Yes" and "No" unless you name them.
- `sys.toasts` and `sys.asking` show what's up right now. A component's own steps can't use either; it
  fires an event, and the app's handler toasts or asks.

**Custom actions** name a list of steps so you can run it from many places:

```toml
[actions.clear_search]
do = [{ set = "app.typed", to = "" }, { set = "app.query", to = "" }]

[on]
"clear.click" = ["clear_search"]
"app.opened" = ["clear_search"]
```

**Failures are loud.** Every `.failed` event goes into `sys.errors`, the event trail and the server's log
before any handler runs. If no handler matches it, the screen shows a short message too, with the API's
answer in it (`crm.companies.delete failed: HTTP 409: Acme still has 2 contacts.`), so an API that answers a
refusal with one plain sentence reads well with no handler at all. A handler can say it better; it can't make a
failure disappear.

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

- **What `app.toml` draws gets `app.*`. Nothing else does.** A component drawn by `app.toml`, by a `use` node
  in its body or in its outlet, reads the app's variables just as `app.toml` does. A component drawn inside
  another component gets only what it's handed: its props, its locals, and `sys.*`. To give it something
  from `app.*`, pass it down as a prop (`props = { contacts = "{{ app.contacts }}" }`). That's the whole rule,
  and it's about where a component is drawn, not what it is: any component can be drawn either way. Drawn
  inside another, its `{{ app.* }}` shows nothing, and a step of its own that reads `app.*` fails loudly,
  saying why. Making one component read well both ways is up to you.
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

### Delete, asking first

```toml
# app.toml: each contact has a Delete button; the app asks, deletes, and says so.
[app]
name = "Delete"

[[body]]
type = "cell"
each = "app.contacts"

[[body.children]]
type = "cell"
flow = "row"
gap = 2

[[body.children.children]]
type = "text"
text = "{{ item.name }}"

[[body.children.children]]
type = "button"
id = "delete"
label = "Delete"
variant = "error"

[connections.crm]
base_url = "https://pf.example.com/webhook/crm"
headers = { Authorization = "Bearer {{ creds.PF_TOKEN }}" }

[connections.crm.crud.contacts]
style = "post"

[sources.contacts]
call = "crm.contacts.list"
select = "/rows"

[on]
"delete.click" = [
  { confirm = "Delete {{ event.item.name }}?", yes = "Delete", no = "Keep" },
  { call = "crm.contacts.delete", params = { id = "{{ event.item.id }}" } },
  { reload = ["app.contacts"] },
  { toast = "Deleted {{ event.item.name }}.", variant = "success" },
]
```

- The button inside the `each` fires `delete.click` with the row's `item`, so the question names who.
- Keep stops the list before the call: nothing is deleted, and nothing is said.
- A failed delete stops the list at the call, and the failure is toasted as an error on its own, so
  there's no `"Deleted"` to take back.

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
required = true

[[body.children.children]]
type = "input"
name = "email"
kind = "email"
label = "Email"
required = true
checks = [{ matches = "^[^@ ]+@[^@ ]+$", message = "That doesn't look like an email." }]

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

The form's checks keep a blank name or a broken email from ever reaching the API: until they pass,
the form fires `new_contact.invalid`, not `.submit`. The list after the call runs only when the call
works: a failed call stops its list. So the success message needs no condition. A failure fires `crm.contacts.create.failed` and then the status,
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
- **A page reads the app's variables**, `app.*`, just as `app.toml` does, with no wiring: in text, in an `each`, as a form's
  `data`. It follows them as they change, so when the app reloads `app.contacts`, `home` shows the new list.
  It reads them and never writes them: to change one, it fires an event and the app sets it.
- The page is drawn by `app.toml`, so it gets `app.*`. A component the page draws doesn't: the page hands it
  what it needs as props (see [Components](#components)).
- **What writes back stays out of a page's `app.*`.** A control's `var` and a dialog's `open` write to their
  variable, so in a page they're `local.*`. A search box on a page binds `local.query`, and the app's handler
  for `PAGE.search.change` copies `{{ event.value }}` into the `app.*` variable a source's params read. A
  dialog the app opens and closes, like a form to add or edit, goes in `app.toml`'s body, over whatever page
  is open.

The CRM sample (`samples/crm/` in the Genesis repo) is a whole app built this way: three pages, forms in
dialogs, searching, deletes that ask first, and its own small API.

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
  problem as `FILE:LINE:COLUMN: what's wrong`. Check it after every write. Wiring is checked too: a step
  naming an action, timer, or operation that isn't there, a `set` on a variable with no starting value in
  `[state]`, and a template reading something that isn't there where it's written (`item` outside an `each`,
  `event` outside a step) are refused here, not when they run. A `go` to a page that isn't there isn't: it
  fails when it runs, so you can add the page later.
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
- What `app.toml` draws reads `app.*`; a component drawn inside another gets only its props.
- Credentials are `{{ creds.NAME }}`, never values, and only in a connection.
- Don't write in `.state/`.
- Check `BASE/status` after every write, and `BASE/api/log` when something doesn't work.
