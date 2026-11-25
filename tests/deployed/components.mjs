// The components deployed check (spec 090): in a real browser, a component in a folder of its own
// draws once per element of an each with its own props, draws again when the state its props come
// from changes, keeps each instance's local state apart (and across a reload of the page), fires
// its events under the use node for the app to hear, runs the steps its caller passes in, and every
// place that uses it redraws when its file is edited. The app's outlet draws its default, go swaps
// in another component whose nodes fire under its own name, and a reload opens on the default again.
//
// It runs genesis-serve on a temporary app of its own. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 26000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-components-"));
const configs = join(work, "configs");
mkdirSync(join(configs, "views"), { recursive: true });
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `# The components deployed check's app: two contact cards from one component, and a button that renames one.
[app]
name = "Components"

[state]
contacts = [{ name = "Ada", email = "ada@example.com" }, { name = "Lin", email = "lin@example.com" }]
picked = "nobody"
chosen = "no one"
tapped = "no"

[on]
"card.more.click" = [{ set = "app.picked", to = "{{ event.item.name }}" }]
"notes.tap.click" = [{ set = "app.tapped", to = "yes" }]

[[body]]
type = "button"
label = "Go to notes"
on_click = { go = "notes" }

[[body]]
type = "button"
label = "Go home"
on_click = { go = "home" }

[[body]]
type = "text"
text = "Tapped {{ app.tapped }}"

[[body]]
type = "outlet"
default = "home"

[[body]]
type = "button"
label = "Rename"
on_click = [{ set = "app.contacts", to = [{ name = "Grace", email = "grace@example.com" }, { name = "Lin", email = "lin@example.com" }] }]

[[body]]
type = "text"
text = "Picked {{ app.picked }}"

[[body]]
type = "text"
text = "Chose {{ app.chosen }}"

[[body]]
type = "cell"
each = "app.contacts"
gap = 2

[[body.children]]
type = "use"
id = "card"
component = "contact_card"
props = { contact = "{{ item }}", on_choose = [{ set = "app.chosen", to = "{{ event.item.name }}" }] }
`,
);

const card = (greeting) => `# A contact's card: its name and email, used once per contact.
[component]
props = ["contact", "on_choose"]

[state]
clicks = 0

[[body]]
type = "text"
text = "${greeting} {{ props.contact.name }} at {{ props.contact.email }}, {{ local.clicks }} clicks"

[[body]]
type = "button"
id = "more"
label = "More on {{ props.contact.name }}"
on_click = [{ step = "local.clicks" }]

[[body]]
type = "button"
id = "choose"
label = "Choose {{ props.contact.name }}"
on_click = "{{ props.on_choose }}"
`;
writeFileSync(join(configs, "views", "contact_card.toml"), card("Card:"));
writeFileSync(
  join(configs, "views", "home.toml"),
  `# The outlet's default.
[component]

[[body]]
type = "text"
text = "Home page"
`,
);
writeFileSync(
  join(configs, "views", "notes.toml"),
  `# What go puts in the outlet: a tap counter of its own, heard by the app.
[component]

[state]
taps = 0

[[body]]
type = "text"
text = "Notes, {{ local.taps }} taps"

[[body]]
type = "button"
id = "tap"
label = "Tap"
on_click = [{ step = "local.taps" }]
`,
);

async function until(what, check, ms = 10_000) {
  const deadline = Date.now() + ms;
  for (;;) {
    let value;
    try {
      value = await check();
    } catch {
      value = undefined;
    }
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

const serve = spawn(
  join(root, "target/debug/genesis-serve"),
  ["--configs", configs, "--state", join(work, "state"), "--dist", join(root, "apps/web/dist"), "--listen", `127.0.0.1:${port}`],
  { stdio: ["ignore", "inherit", "inherit"] },
);

let browser;
try {
  await until("serve's /health", async () => (await fetch(`${base}/health`)).ok);
  browser = await chromium.launch({ executablePath: chrome });
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${base}/`);
  await page.getByText("Card: Ada at ada@example.com, 0 clicks").waitFor();
  await page.getByText("Card: Lin at lin@example.com, 0 clicks").waitFor();
  // Each instance has its own local state, and the app hears its event with its item.
  await page.getByRole("button", { name: "More on Lin" }).click();
  await page.getByRole("button", { name: "More on Lin" }).click();
  await page.getByText("Card: Lin at lin@example.com, 2 clicks").waitFor();
  await page.getByText("Card: Ada at ada@example.com, 0 clicks").waitFor();
  await page.getByText("Picked Lin").waitFor();
  // Steps the caller passed in run in the app, with the event from inside.
  await page.getByRole("button", { name: "Choose Ada" }).click();
  await page.getByText("Chose Ada").waitFor();
  // The outlet draws its default; go swaps in a component that fires under its own name.
  await page.getByText("Home page").waitFor();
  await page.getByRole("button", { name: "Go to notes" }).click();
  await page.getByText("Notes, 0 taps").waitFor();
  await until("home to go", async () => !(await page.getByText("Home page").isVisible()));
  await page.getByRole("button", { name: "Tap" }).click();
  await page.getByText("Notes, 1 taps").waitFor();
  await page.getByText("Tapped yes").waitFor();
  // Its locals stay while it's away.
  await page.getByRole("button", { name: "Go home" }).click();
  await page.getByText("Home page").waitFor();
  await page.getByRole("button", { name: "Go to notes" }).click();
  await page.getByText("Notes, 1 taps").waitFor();
  // Locals are kept on the device, so they're back after a reload of the page.
  await page.waitForTimeout(500);
  await page.reload();
  await page.getByText("Card: Lin at lin@example.com, 2 clicks").waitFor();
  // sys.outlet isn't kept: a reload opens on the default.
  await page.getByText("Home page").waitFor();
  // Its props come from the app's state where it's used, so a change there draws it again.
  await page.getByRole("button", { name: "Rename" }).click();
  await page.getByText("Card: Grace at grace@example.com, 0 clicks").waitFor();
  // Editing the component's file redraws every place that uses it, and its locals stay.
  writeFileSync(join(configs, "views", "contact_card.toml"), card("Contact:"));
  await page.getByText("Contact: Grace at grace@example.com, 0 clicks").waitFor();
  await page.getByText("Contact: Lin at lin@example.com, 2 clicks").waitFor();

  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: a component drew per element with its props and its own locals, fired under its use node, ran its caller's steps, and redrew when its file changed; the outlet swapped components with go");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
