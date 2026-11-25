// The controls deployed check (spec 040, "Controls"): in a real browser, typing in an input or a
// textarea, ticking a checkbox or a toggle, picking a radio, and choosing in a select each write
// their variable, with its JSON type, and the screen follows. A select on sys.theme changes the theme.
//
// It runs genesis-serve on a temporary app of its own. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 23000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-controls-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Controls"

[state]
query = ""
qty = 1
note = ""
agree = false
dark = false
size = 1
company = 0
companies = [{ name = "Acme", id = 1 }, { name = "Globex", id = 2 }]

[[body]]
type = "text"
text = "query={{ app.query }} qty={{ app.qty }} note={{ app.note }} agree={{ app.agree }} dark={{ app.dark }} size={{ app.size }} company={{ app.company }}"

[[body]]
type = "input"
var = "app.query"
kind = "search"
label = "Search"

[[body]]
type = "input"
var = "app.qty"
kind = "number"
label = "Quantity"

[[body]]
type = "textarea"
var = "app.note"
label = "Note"

[[body]]
type = "checkbox"
var = "app.agree"
label = "Agree"

[[body]]
type = "toggle"
var = "app.dark"
label = "Dark"

[[body]]
type = "radio"
var = "app.size"
value = 1
label = "Small"

[[body]]
type = "radio"
var = "app.size"
value = 3
label = "Large"

[[body]]
type = "select"
var = "app.company"
label = "Company"
placeholder = "Pick one"
options = "app.companies"
option_label = "/name"
option_value = "/id"

[[body]]
type = "select"
var = "sys.theme"
label = "Theme"
options = [{ label = "Light", value = "light" }, { label = "Dracula", value = "dracula" }]
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
  const shows = (text) => page.getByText(text).waitFor();
  await shows("query= qty=1 note= agree=false dark=false size=1 company=0");

  await page.getByLabel("Search").fill("ada");
  await shows("query=ada qty=1");
  // Typed a key at a time, a number keeps "-" and "2." on the way, and writes null, then numbers.
  await page.getByLabel("Quantity").fill("");
  await shows("qty= note=");
  await page.getByLabel("Quantity").pressSequentially("-2.5");
  await shows("qty=-2.5 ");
  if ((await page.getByLabel("Quantity").inputValue()) !== "-2.5") throw new Error("the number box lost what was typed");
  // A number box writes a number; the mirror's state below checks the type.
  await page.getByLabel("Quantity").fill("12");
  await shows("qty=12 ");
  await page.getByLabel("Note").fill("hello there");
  await shows("note=hello there agree=false");
  await page.getByLabel("Agree").check();
  await shows("agree=true dark=false");
  await page.getByLabel("Dark").check();
  await shows("agree=true dark=true");
  await page.getByLabel("Large").check();
  await shows("size=3 ");
  // The arrow keys move within a radio group, as in plain HTML.
  await page.getByLabel("Large").press("ArrowUp");
  await shows("size=1 ");
  await page.getByLabel("Company").selectOption({ label: "Globex" });
  await shows("company=2");
  await page.getByLabel("Theme").selectOption({ label: "Dracula" });
  await page.locator('[data-theme="dracula"]').waitFor();

  // The values keep their JSON types: the mirror holds this client's state as serve last saw it.
  const dir = join(work, "state");
  const client = await until("the mirror's state file", () => readdirSync(dir).find((f) => f.endsWith(".json")));
  const app = await until("the mirror to hold the last pick", () => {
    const snapshot = JSON.parse(readFileSync(join(dir, client), "utf8"));
    return snapshot.app.company === 2 && snapshot.app;
  });
  const want = { query: "ada", qty: 12, note: "hello there", agree: true, dark: true, size: 1, company: 2 };
  for (const [key, value] of Object.entries(want)) {
    if (app[key] !== value) throw new Error(`the mirror's app.${key} is ${JSON.stringify(app[key])}, not ${JSON.stringify(value)}`);
  }
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: each control wrote its variable as the person typed, ticked, and picked");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
