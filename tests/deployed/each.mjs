// The each deployed check (spec 040, "Repeating: each"): in a real browser, a cell repeats its
// children per element, and a click inside one carries that element to its handler as event.item.
// Nothing on the page changes while nothing happens, and a change to the list keeps its rows on
// the page: only the changed text changes, and a new element adds a row at the end. With a key, a
// row is its record's: taking one out of the middle leaves the others' elements alone.
//
// It runs genesis-serve on a temporary app of its own. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 23000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-each-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Each"

[state]
picked = "nobody"
rows = [{ name = "Ada", id = 1 }, { name = "Bob", id = 2 }, { name = "Cy", id = 3 }]
team = [{ name = "Al", id = "a" }, { name = "Bo", id = "b" }, { name = "Cid", id = "c" }]

[[body]]
type = "text"
text = "picked: {{ app.picked }}"

[[body]]
type = "cell"
each = "app.rows"
gap = 2

[[body.children]]
type = "cell"
flow = "row"
gap = 2

[[body.children.children]]
type = "text"
text = "{{ item.name }}"

[[body.children.children]]
type = "button"
id = "pick"
label = "Pick {{ item.name }}"

[[body]]
type = "button"
id = "rename"
label = "Rename Bob"
on_click = { set = "app.rows", to = [{ name = "Ada", id = 1 }, { name = "Bea", id = 2 }, { name = "Cy", id = 3 }] }

[[body]]
type = "button"
id = "grow"
label = "Add Dee"
on_click = { set = "app.rows", to = [{ name = "Ada", id = 1 }, { name = "Bea", id = 2 }, { name = "Cy", id = 3 }, { name = "Dee", id = 4 }] }

[[body]]
type = "cell"
each = "app.team"
key = "item.id"

[[body.children]]
type = "button"
id = "member"
label = "Member {{ item.name }}"

[[body]]
type = "button"
id = "drop_bo"
label = "Drop Bo"
on_click = { set = "app.team", to = [{ name = "Al", id = "a" }, { name = "Cid", id = "c" }] }

[on]
"pick.click" = [{ set = "app.picked", to = "{{ event.item.name }} ({{ event.item.id }})" }]
`,
);

async function until(what, check, ms = 10_000) {
  const deadline = Date.now() + ms;
  for (;;) {
    try {
      if (await check()) return;
    } catch {}
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
  await page.getByText("picked: nobody").waitFor();
  const picks = page.getByRole("button", { name: /^Pick / });
  const buttons = await picks.allTextContents();
  if (buttons.join(",") !== "Pick Ada,Pick Bob,Pick Cy") throw new Error(`the rows' buttons are ${buttons}`);

  // While nothing happens, nothing on the page changes: the clock ticking redraws nothing.
  const changes = await page.evaluate(
    () =>
      new Promise((resolve) => {
        let seen = 0;
        const watch = new MutationObserver((records) => (seen += records.length));
        watch.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
        setTimeout(() => (watch.disconnect(), resolve(seen)), 1000);
      }),
  );
  if (changes) throw new Error(`the page changed ${changes} times while nothing happened`);

  // Each row's button is marked, so it's plain whether a change kept it or drew a new one.
  await picks.evaluateAll((all) => all.forEach((b, i) => (b.dataset.row = String(i))));
  const marks = () => picks.evaluateAll((all) => all.map((b) => `${b.dataset.row ?? "new"}:${b.textContent}`).join(","));

  await page.getByRole("button", { name: "Pick Bob" }).click();
  await page.getByText("picked: Bob (2)").waitFor();
  await page.getByRole("button", { name: "Pick Cy" }).click();
  await page.getByText("picked: Cy (3)").waitFor();
  if ((await marks()) !== "0:Pick Ada,1:Pick Bob,2:Pick Cy") throw new Error(`a change elsewhere redrew the rows: ${await marks()}`);

  // A changed element changes its row's text in place; a new one adds a row at the end.
  await page.getByRole("button", { name: "Rename Bob" }).click();
  await until("Bob's row to read Bea", async () => (await marks()) === "0:Pick Ada,1:Pick Bea,2:Pick Cy");
  await page.getByRole("button", { name: "Add Dee" }).click();
  await until("Dee's row", async () => (await marks()) === "0:Pick Ada,1:Pick Bea,2:Pick Cy,new:Pick Dee");
  await page.getByRole("button", { name: "Pick Dee" }).click();
  await page.getByText("picked: Dee (4)").waitFor();

  // With a key, a row is its record's: taking out the middle one removes its own element, and the
  // ones after it keep theirs.
  const members = page.getByRole("button", { name: /^Member / });
  await members.evaluateAll((all) => all.forEach((b) => (b.dataset.row = b.textContent)));
  const kept = () => members.evaluateAll((all) => all.map((b) => `${b.dataset.row ?? "new"}=${b.textContent}`).join(","));
  await page.getByRole("button", { name: "Drop Bo" }).click();
  await until("Bo's row to go", async () => (await members.count()) === 2);
  if ((await kept()) !== "Member Al=Member Al,Member Cid=Member Cid") throw new Error(`a keyed row moved records: ${await kept()}`);
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: each drew a row per element, a click in a row carried its item, nothing redrew while idle, a changed list kept its rows, and a keyed row stayed its record's");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
