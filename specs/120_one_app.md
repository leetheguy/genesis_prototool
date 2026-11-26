# 120 — One app, built on the fly

Status: implemented
Created: 2026-11-24

A pivot, in two parts: Genesis runs one app, and its catalog is atoms. Both bring the design back to
what Genesis was meant to be from the start.

## Why

Genesis isn't a tool for building apps. It's a screen an AI builds on the fly: whatever an agent
wants to show a person, in whatever form it judges most useful, it shows at once, and the person
answers by clicking, choosing, and typing, not only by writing prose. It's a way for people and AI
to work together without everything going through language.

So:

- **There's one app, and it's whatever is on screen now.** The configs folder is that app. An agent
  rebuilds, rearranges, and redesigns it by writing files, and the screen follows live. Folders inside
  it are for organizing files, nothing more. There are no separate apps to choose between.
- **The catalog is atoms**, as small as plain HTML: a cell for layout, text, an image, a link, a button,
  each form control, a form, a dialog. A stack, a row, a grid, a card, and a table are all cells laid out
  a certain way, so they're composites, built from atoms by an agent or as components. An agent can
  build anything from parts; it isn't stuck with someone else's idea of a card.

## One app

- **There's no list of pages.** `app.toml` is the top-level page, by convention. Its body may hold one
  `outlet`, which draws one component at a time, and `go` swaps in another ([100](100_events.md), "Going");
  every other component is a partial. Within a screen it's all show and hide ([080](080_display_logic.md)).
- **The configs folder is the app.** `app.toml` at its top holds `[app]` and the screen's `[[body]]`.
  Every other `.toml` file, in any folder, is a component ([090](090_components.md)) or adds to the app
  (state, connections, sources, actions, handlers) ([020](020_config.md)).
- **Every file name is unique across the whole app.** Folders don't make a name unique:
  `views/review.toml` and `old/review.toml` clash, and a clash is a load problem naming both paths
  (`views/review.toml: this file and old/review.toml share the name review.toml`); a file at the top says so. A file
  name then always points at exactly one file, for an agent, a log line, and a person reading the files,
  and a component's name, which is its file name, means one component.
- **Names inside files may repeat.** Two nodes may share an `id`, and two files may handle the same event
  (both run, in file order). Only a *definition* can't be made twice: the same `[state]` variable,
  source, connection, or action in two files is a load problem naming both places, as it is within one
  file, since the app couldn't say which one holds.
- **One address.** `serve` draws the app at `/`; its routes lose the app's name (`/api/call`,
  `/api/push/{name}`, [010](010_architecture.md)). A second app is a second Genesis, with its own
  configs folder.

## Atoms

The catalog is in [040](040_catalog.md). Cells carry layout and look through a closed set of style
keys, mapped to Tailwind classes that daisyUI's themes color, with Tailwind's screen sizes for
responsive layout. There are still no CSS classes in the TOML: a style key is a word Genesis knows,
like `span = 4`, and Genesis picks the class. The grid keys follow Atomic Grid
(github.com/leetheguy/atomic-grid): a grid of columns, cells that span and offset, an auto-fit grid,
and a different span at each screen size.

Composites (a data table, a sign-in form, a card, tabs) are components built from atoms, in a library
planned in [999](999_todo.md).

## Tests

The tests for each part are in the spec that holds it: [010](010_architecture.md) for routes,
[020](020_config.md) for loading many files and unique names, [040](040_catalog.md) for the atoms and
style keys, and [090](090_components.md) for components.

## Log

- 2026-11-25: there are no pages, so `go`, `sys.view`, and `[app] start` are gone ([100](100_events.md)). Lee: "Yeah, there is no go. It doesn't exist. Not yet anyway. We may create it as a convenience down the road, but for now it's not necessary." There are no pages: "It's all show and hide everywhere. If you want to navigate from one page to another, you just hide one stack of components and show another one."
- 2026-11-25: `app.toml` is the top-level page, and `go` fills its one `outlet` with a component
  ([040](040_catalog.md), [100](100_events.md)). Lee: "We don't need a list of pages. We need a top-level page
  and a partial. The top level page should have a special name. Convention over configuration, 'cause this app
  already has enough configuration." And: "Yes to app.toml and outlet."
- 2026-11-25: the clash message names both paths plainly. It read "the file name companies.toml is also
  companies.toml" when one of the two was at the top; now it's "this file and companies.toml (at the top of the
  configs folder) share the name companies.toml". Lee: "Fix both".
