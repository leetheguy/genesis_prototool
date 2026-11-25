// The each deployed check (spec 040, "Repeating: each"): in a real browser, a cell repeats its
// children per element, and a click inside one carries that element to its handler as event.item.
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
  const buttons = await page.getByRole("button").allTextContents();
  if (buttons.join(",") !== "Pick Ada,Pick Bob,Pick Cy") throw new Error(`the rows' buttons are ${buttons}`);

  await page.getByRole("button", { name: "Pick Bob" }).click();
  await page.getByText("picked: Bob (2)").waitFor();
  await page.getByRole("button", { name: "Pick Cy" }).click();
  await page.getByText("picked: Cy (3)").waitFor();
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: each drew a row per element, and a click in a row carried its item to the handler");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
