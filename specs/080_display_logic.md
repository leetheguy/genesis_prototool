# 080 — Display logic

Status: draft
Created: 2026-11-21

Genesis has no logic about data. It does have display logic: whether something shows, whether
input is acceptable, and how a value looks. This logic stays small on purpose. It's written as
TOML tables, never as expressions, so it can't grow into a language. Anything beyond it is the API's job.

## Conditions

A condition is a table. It names a variable and makes one test:

```toml
show = { var = "sys.view", is = "contacts" }
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
| `status = "loading"` | is a source with that status ([050](050_state.md)) |

`all = [...]`, `any = [...]`, and `not = {...}` combine conditions. `var` may also be `row.*`,
`form.*`, `local.*`, `props.*`, or `sys.*`. A test on the wrong type (a `gt` on text) is false, never an error.

Conditions go in `show` (any node) and `disabled` (buttons and inputs), and nowhere else. Handlers
and sources never test anything; they branch by event name instead ([100](100_events.md)).

## Checks on input

Inputs take checks. They run as the user types, and a `form` won't submit while any check on an
input inside it fails. Each failing check shows its message under its input:

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

A check is a condition on the input's own value (so it has no `var`), plus a `message`. `required = true` is
shorthand for `{ empty = false, message = "Required." }`. Length tests are `min_len` and `max_len`.

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
- a condition anywhere but `show`, `disabled`, or an input's checks is a config error
- a failing check shows its message and blocks submit; fixing the value clears it
- `required`, `min_len`, and `max_len` act as their shorthand
- each format turns its example into the value shown; the raw value in state is unchanged
- `locale` changes number and date output

## Tasks

Milestone three ([999](999_todo.md)):

- [ ] conditions, and `show` on every node through `Wired`
- [ ] `disabled` on buttons and inputs
- [ ] checks on inputs, with their messages
- [ ] formats, and `locale`
