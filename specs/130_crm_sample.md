# 130 — The CRM sample

Status: implemented
Created: 2026-11-25

Milestone three's goal is an agent building a CRM on its own from the guide ([999](999_todo.md)). This spec
is the CRM built that way once, as a sample: a small API of its own, the app's files, and a deployed check
that drives it in a real browser. It's the yardstick for the agent's run, and it shows that the atoms,
components, and steps that exist are enough for a real app without new features.

## The API

`samples/crm/api.mjs` is a small HTTP API in Node, with no dependencies, for the sample and its check only:

```
node samples/crm/api.mjs PORT DATA_FILE TOKEN
```

- **Two tables.** A company has an `id`, a `name`, and a `website`. A contact has an `id`, a `name`, an
  `email`, a `phone`, and a `company_id`: each contact is at one company.
- **Full CRUD on each, as REST**, the shape Genesis's `crud` makes ([030](030_connections.md)):
  `GET /companies`, `GET /companies/{id}`, `POST /companies`, `PATCH /companies/{id}`, `DELETE /companies/{id}`,
  and the same under `/contacts`. A list answers `{ "rows": [...] }`, sorted by name.
- **Searching is the API's.** `GET /companies?q=` keeps the companies whose name has `q` in it, and
  `GET /contacts?q=` the contacts whose name, email, or company has it, ignoring case. `GET /contacts?company_id=`
  keeps one company's contacts. Each company row says how many contacts it has (`contacts`), and each contact
  row names its company (`company`).
- **It decides what's valid**: a name is required on both, an email has an `@`, two contacts can't share an
  email, a contact's company exists, and a company with contacts can't be deleted. Each refusal is a `4xx`
  whose body is one plain sentence (`Acme still has 2 contacts.`), so the failure Genesis shows reads well.
  An unknown id is a `404`.
- **Its data survives restarts.** Every change is written to `DATA_FILE` (a temporary file, then renamed). A
  missing file starts from a few companies and contacts.
- Every request needs `Authorization: Bearer TOKEN`, or it's a `401`, so the sample shows a credential
  `serve` holds ([070](070_secrets.md)).

## The app

`samples/crm/configs/` is the configs folder, built only from atoms, its own components, and steps that exist:

- **`app.toml`**: a bar that stays (the app's name, Companies, Contacts), the `outlet`, which opens on
  `companies`, and the four forms, each in a dialog: `new_company`, `company_changes`, `new_contact`,
  `contact_changes`. Edit forms start from the record (`data`). New and edit are separate forms, so each
  submit has its own event and handler: no branching in the wiring.
- **Pages**, each a component in the outlet (`pages/`):
  - `companies`: a search box, the list, each row with its contact count and Open, Edit and Delete, and an
    Add company button.
  - `company`: one company, with Edit, and its contacts, with Add contact (starting at that company), Edit and Delete.
  - `contacts`: a search box and the list, each row naming its company, with Edit and Delete, and Add contact.
- **Components** (`parts/`): `company_fields` and `contact_fields`, the named controls with their checks, shared
  by the new and edit forms.
- **Wiring**, by subject: `crm.toml` (the connection, the sources, and `reload_all`), `company_handlers.toml`,
  `contact_handlers.toml`.
- **Search follows what's typed.** A page can't bind a control to `app.*` (it's a component), so each search
  box binds a `local` variable, and a handler on its `change` copies it into `app.company_query` or
  `app.contact_query`, which the source's params read.
- **Forms check before they send.** A name is required, an email looks like one, and a contact needs a company
  (a `select` whose options come from every company). The API still decides.
- **Deletes ask first** with `confirm`, naming who. **Saves toast**: Saved, Added, Deleted. **Failures are loud**:
  the default failure toast says what the API said.
- The company being looked at is kept as the whole row in `app.company`, and its contacts are a source whose
  params read `app.company.id`. So the app reopens on the page it was left on, with its data.

## The deployed check

`tests/deployed/crm.mjs` starts the API on a temporary data file and `serve` on `samples/crm/configs`, and drives it in
headless Chromium, as part of `scripts/check-hello.sh`:

- the companies list shows the starting data, and typing in the search box narrows it
- adding a company with no name shows the form's check and sends nothing; with a name, it's added, toasted, and listed
- opening a company shows its contacts; adding a contact there starts at that company, and saves
- editing a contact saves the change and the list shows it
- deleting a company that has contacts fails loudly with the API's sentence; deleting a contact asks first,
  Keep deletes nothing, and Delete deletes it and toasts
- the API's data file holds the changes, and a restarted API still has them

## Tests

- the API: each CRUD route on both tables; `q` and `company_id` filter; each refusal is its status with a plain
  sentence; an unknown id is a `404`; no token is a `401`; the data file survives a restart, and a missing one
  starts from the seed
- the sample loads, and every page and component in it is used
- the deployed check above, in a real browser

## Tasks

- [x] the API, with its own checks (`samples/crm/api.test.mjs`, run with `node --test`)
- [x] the sample's files
- [x] the deployed check, in `scripts/check-hello.sh`
- [x] docs: 000, 999, the README's samples, and the guide's notes on what building it showed

## Log

- 2026-11-25: built. The forms ended up as dialogs in `app.toml`, not pages: a page can't bind a dialog's `open`
  or a control's `var` to `app.*`, since those write back. Each search box binds `local.query`, and a handler
  copies it into the `app.*` variable the source reads. The API answers a refusal as plain text, so the default
  failure toast reads as a sentence. Two gaps found are in [999](999_todo.md): a form's draft that doesn't start
  over when its `data` changes, and the message for two files with one name.
- 2026-11-25: both gaps are fixed ([040](040_catalog.md), [120](120_one_app.md)), and the deployed check now edits one
  contact, closes unsaved, and checks the next one's form starts from that contact.
