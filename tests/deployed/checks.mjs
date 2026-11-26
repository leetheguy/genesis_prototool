// The checks deployed check (spec 080, "Checks on input"): in a real browser, a control says what's
// wrong under itself as the person types, a submit with a failing check fires `.invalid` and every
// failing control speaks, and fixing the values lets the form submit.
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
const work = mkdtempSync(join(tmpdir(), "genesis-checks-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Checks"

[state]
joined = ""
tries = 0

[[body]]
type = "text"
text = "joined={{ app.joined }} tries={{ app.tries }}"

[[body]]
type = "form"
id = "join"

[[body.children]]
type = "input"
id = "email_box"
name = "email"
label = "Email"
required = true
checks = [{ matches = "^[^@ ]+@[^@ ]+$", message = "That doesn't look like an email." }]

[[body.children]]
type = "checkbox"
id = "terms_box"
name = "terms"
label = "I agree"
required = "Please agree to the terms."

[[body.children]]
type = "button"
label = "Sign up"
submit = true

[on]
"join.submit" = [{ set = "app.joined", to = "{{ event.form.email }}" }]
"join.invalid" = [{ step = "app.tries", by = 1 }]
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
  await page.getByText("joined= tries=0").waitFor();
  if (await page.getByRole("alert").count()) throw new Error("a control spoke before anything was done");

  // Typed a key at a time, the box keeps its focus while its message comes and goes.
  const email = page.getByLabel("Email");
  await email.pressSequentially("ada");
  await page.getByText("That doesn't look like an email.").waitFor();
  await email.pressSequentially("@example.com");
  await until("the message to go", async () => (await page.getByText("That doesn't look like").count()) === 0);
  if (!(await email.evaluate((el) => el === document.activeElement))) throw new Error("the box lost focus");
  if ((await email.inputValue()) !== "ada@example.com") throw new Error(`the box holds ${await email.inputValue()}`);

  // Submitting with the terms unticked fires .invalid, and the checkbox says why.
  await page.getByRole("button", { name: "Sign up" }).click();
  await page.getByText("joined= tries=1").waitFor();
  await page.getByText("Please agree to the terms.").waitFor();
  if ((await email.inputValue()) !== "ada@example.com") throw new Error("a failed submit lost what was typed");

  await page.getByLabel("I agree").check();
  await until("the terms message to go", async () => (await page.getByText("Please agree").count()) === 0);
  await page.getByRole("button", { name: "Sign up" }).click();
  await page.getByText("joined=ada@example.com tries=1").waitFor();
  if (await page.getByRole("alert").count()) throw new Error("the form didn't start over quietly");
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: each control said what was wrong as the person typed, and the form submitted once it was fixed");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
