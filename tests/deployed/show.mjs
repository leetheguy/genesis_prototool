// The display-logic deployed check (spec 080): in a real browser, `show` and `disabled` follow state
// as it changes, with no reload. Tabs swap views; a form's submit button stays disabled until what's
// typed looks like an email, and the input keeps focus and what's typed while the button changes.
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
const work = mkdtempSync(join(tmpdir(), "genesis-show-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Show"

[state]
view = "people"
joined = ""

[[body]]
type = "button"
id = "people_tab"
label = "People"
disabled = { var = "app.view", is = "people" }

[[body]]
type = "button"
id = "join_tab"
label = "Join"
disabled = { var = "app.view", is = "join" }

[[body]]
type = "text"
text = "Everyone who's joined: {{ app.joined }}"
show = { var = "app.view", is = "people" }

[[body]]
type = "form"
id = "join"
show = { var = "app.view", is = "join" }

[[body.children]]
type = "input"
id = "email_box"
name = "email"
label = "Email"

[[body.children]]
type = "button"
label = "Sign up"
submit = true
disabled = { not = { var = "form.email", matches = "^[^@ ]+@[^@ ]+$" } }

[on]
"people_tab.click" = [{ set = "app.view", to = "people" }]
"join_tab.click" = [{ set = "app.view", to = "join" }]
"join.submit" = [{ set = "app.joined", to = "{{ event.form.email }}" }, { set = "app.view", to = "people" }]
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
  const button = (name) => page.getByRole("button", { name, exact: true });
  await page.getByText("Everyone who's joined:").waitFor();
  if (!(await button("People").isDisabled())) throw new Error("the tab you're on isn't disabled");
  if (await page.getByLabel("Email").count()) throw new Error("the hidden form is drawn");

  await button("Join").click();
  await page.getByLabel("Email").waitFor();
  if (await page.getByText("Everyone who's joined:").count()) throw new Error("the hidden view is still drawn");
  await until("the People tab to come back", async () => !(await button("People").isDisabled()));
  if (!(await button("Sign up").isDisabled())) throw new Error("Sign up isn't disabled with nothing typed");

  // Typed a key at a time, the box keeps its focus and text as the button changes beside it.
  const email = page.getByLabel("Email");
  await email.pressSequentially("ada@example");
  await until("Sign up to be enabled", async () => !(await button("Sign up").isDisabled()));
  await email.pressSequentially(" x");
  await until("Sign up to be disabled again", () => button("Sign up").isDisabled());
  await email.press("Backspace");
  await email.press("Backspace");
  await until("Sign up to be enabled again", async () => !(await button("Sign up").isDisabled()));
  if ((await email.inputValue()) !== "ada@example") throw new Error(`the box holds ${await email.inputValue()}`);
  if (!(await email.evaluate((el) => el === document.activeElement))) throw new Error("the box lost focus");

  await button("Sign up").click();
  await page.getByText("Everyone who's joined: ada@example").waitFor();
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: show and disabled followed state, a form draft, and typing, with no reload");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
