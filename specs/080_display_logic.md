# 080 — Display logic

Status: draft
Created: 2026-11-21

Genesis has no logic about data. It does have display logic: whether something shows, whether
input is acceptable, and how a value looks. This logic stays small on purpose. It's written as
TOML tables, never as expressions, so it can't grow into a language. Anything beyond it is the API's job.

## Conditions

A condition is a table. It names a variable and makes one test:

```toml
show = { var = "app.view", is = "contacts" }
show = { var = "sys.auth", is = "out" }
show = { var = "app.contacts", status = "failed" }
disabled = { var = "local.saving", is = true }
show = { all = [{ var = "app.role", in = ["admin", "owner"] }, { not = { var = "app.archived", is = true } }] }
```

| Test | Holds when the variable |
|---|---|
| `is = X` | equals `X` (same JSON type and value) |
| `in = [X, Y]` | equals one of them |
| `empty = true` | is missing, `null`, `""`, `[]`, or `{}` (`empty = false` is the opposite) |
| `gt`, `gte`, `lt`, `lte = N` | is a number on that side of `N` |
| `matches = "regex"` | is text the regular expression finds a match in |
| `min_len`, `max_len = N` | is text with at least (or at most) `N` characters, or a list with that many items |
| `status = "loading"` | is a source with that status ([050](050_state.md)) |

`all = [...]`, `any = [...]`, and `not = {...}` combine conditions. `var` may also be `item.*`,
`form.*`, `local.*`, `props.*`, or `sys.*`. A test on the wrong type (a `gt` on text) is false, never an error.

Conditions go in `show` (any node) and `disabled` (buttons and controls), and nowhere else. Handlers
and sources never test anything; they branch by event name instead ([100](100_events.md)).

- **`show`**: the node and its children are drawn only while it holds, and take no place while hidden.
  On a node with `each`, it's tested once, for the whole list; to hide some rows, put `show` on the
  row's own children, where `item.*` is that row's element.
- **`disabled`**: the button or control is drawn disabled while it holds, so it can't be clicked,
  typed in, or picked from. A disabled button fires nothing.
- **What `var` can read** is checked when the app loads: `local.*` and `props.*` only inside a
  component, `form.*` only inside a `form` (or a component, which may be drawn in one), and
  `status` only on a source. `form.*` reads what's typed into the form so far, else its `data`.
  A component whose condition reads `app.*` is a page ([090](090_components.md)).

## Checks on input

Controls take checks. They run as the user types, and a `form` won't submit while any check on a
control inside it fails. Each failing check shows its message under its control:

```toml
[[body.children]]
type = "input"
name = "email"
kind = "email"
required = true
checks = [{ matches = "^[^@]+@[^@]+$", message = "That doesn't look like an email." }]

[[body.children]]
type = "input"
name = "seats"
kind = "number"
checks = [{ gte = 1, lte = 500, message = "1 to 500 seats." }]
```

A check is one or more tests on the control's own value (so it has no `var`, `all`, `any`, or `not`),
which all have to pass, plus a `message`. `required = true` is shorthand for
`{ empty = false, message = "Required." }`; `required = "Pick a plan."` says its own message. On a
checkbox or toggle, `required` means ticked. Length tests are `min_len` and `max_len`, which count
characters in text and items in a list (a `file` with `multiple`). Every control takes checks.

- **An empty value passes** every check but `required` (and one that tests `empty`, `is`, or `in`), so
  an optional field left blank isn't wrong.
- **When a control speaks:** its first failing check's message shows under it once the person has
  changed it, or once its form's submit failed a check. Fixing the value clears it, and a good submit
  starts the form over quietly.
- **A submit with a failing check** fires `.invalid` in place of `.submit`. Its payload has `form`, as
  `.submit`'s would, and `invalid`: each failing field's first message, by name. The draft stays.
- **Only the form's checks decide.** The form is drawn `novalidate`, so the browser's own bubbles (for
  a `kind = "email"` box, say) never stop a submit before `.invalid` can fire. Write the check you mean.
- **A control with `var`** shows its messages the same way, but has no form to stop.

These checks are for the person typing. They aren't a guarantee: the API checks what it accepts,
and its refusal is a loud failure ([030](030_connections.md)).

## Formats

`format` changes how a value looks, never what it is. The variable keeps the raw value.

| Format | Shows `1234.5` or `"2026-11-21T09:30:00Z"` as |
|---|---|
| `{ number = 2 }` | `1,234.50` (that many decimals, grouped) |
| `{ currency = "USD" }` | `$1,234.50` |
| `{ percent = 1 }` | the number × 100, with that many decimals and `%` |
| `{ date = "%b %d, %Y" }` | `Nov 21, 2026` (strftime patterns) |
| `{ ago = true }` | `3 hours ago` |
| `{ bytes = true }` | `1.2 KB` |
| `{ case = "upper" }` | text in `upper`, `lower`, or `title` case |
| `{ truncate = 80 }` | text cut to 80 characters with `…` |
| `{ map = { active = "🟢 Active", lost = "⚪ Lost" } }` | the label for each value; others as they are |

Formats follow the browser's locale unless `[app] locale = "en-GB"` sets one. A value a format can't
handle (text given to `currency`) is shown as it is, and the problem goes to the browser console.

## Tests

- each test in the table holds and fails where it should, including type mismatches being false
- `all`, `any`, and `not` combine, and nest
- `show` hides a node and its children; `disabled` disables
- `show` and `disabled` follow state, a row's `item`, and what's typed into a form, without a reload
- a condition's `var` that its place can't read (`local.*` outside a component, `form.*` outside a
  form, `status` on a variable that isn't a source) is a config error
- a condition anywhere but `show`, `disabled`, or a control's checks is a config error
- a failing check shows its message and blocks submit; fixing the value clears it
- `required`, `min_len`, and `max_len` act as their shorthand
- a control says nothing until it's changed or its form's submit fails a check (in a real browser:
  `tests/deployed/checks.mjs`)
- `.invalid` carries `form` and each failing field's message, and keeps the draft; a row's form is
  checked against its own item
- a check without `message`, or with `var`, `status`, or a combinator, is a config error
- each format turns its example into the value shown; the raw value in state is unchanged
- `locale` changes number and date output

## Tasks

Milestone three ([999](999_todo.md)):

- [x] conditions, and `show` on every node through `Wired`
- [x] `disabled` on buttons and controls
- [x] checks on controls, with their messages
- [ ] formats, and `locale`

## Log

- 2026-11-25: the first example shows on `app.view`, a variable the app declares, since `sys.view` is gone ([050](050_state.md)). Showing and hiding is all the navigation there is.
- 2026-11-25: conditions, `show`, and `disabled` are built. `show` sits with the placement keys every
  node takes. `matches` uses the `regex-lite` crate, chosen over `regex` because it adds far less to
  the browser bundle; it has no Unicode classes like `\p{L}`, which a display check doesn't need.
  Numbers compare by value, so `is = 1` holds for `1.0`.
- 2026-11-25: checks on controls are built. A check may hold several tests, all of which must pass,
  as the seats example does. `min_len` and `max_len` are tests like the others, so conditions can use
  them too. An empty value passes every check but `required`, so optional fields stay optional.
  Messages wait until the person changes the control or a submit fails, so a fresh form isn't
  covered in red. Forms are drawn `novalidate` so Genesis's checks are the only ones, and `.invalid`
  always fires.
