// The form deployed check (spec 040, "Controls"): in a real browser, a form's named controls start
// from its data, keep what's typed across a reload (except passwords), and go out on .submit, from
// the submit button or Enter in a box, but not from any other button. Then the form starts over.
// A form repeated by an each keeps one draft per row.
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
const work = mkdtempSync(join(tmpdir(), "genesis-form-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Form"

[state]
saved = "nothing yet"
contact = { name = "Ada", email = "ada@example.com" }
people = [{ name = "Ada" }, { name = "Grace" }]

[[body]]
type = "text"
text = "saved={{ app.saved }}"

[[body]]
type = "form"
id = "contact"
data = "app.contact"
on_submit = [{ set = "app.saved", to = "{{ event.form.name }} <{{ event.form.email }}> size {{ event.form.size }} agree {{ event.form.agree }} pass {{ event.form.password }}" }]

[[body.children]]
type = "input"
name = "name"
label = "Name"

[[body.children]]
type = "input"
name = "email"
kind = "email"
label = "Email"

[[body.children]]
type = "input"
name = "password"
kind = "password"
label = "Password"

[[body.children]]
type = "checkbox"
name = "agree"
label = "Agree"

[[body.children]]
type = "radio"
name = "size"
value = 1
label = "Small"

[[body.children]]
type = "radio"
name = "size"
value = 3
label = "Large"

[[body.children]]
type = "button"
label = "Save"
submit = true

[[body.children]]
type = "button"
label = "Cancel"
on_click = [{ set = "app.saved", to = "cancelled" }]

[[body]]
type = "cell"
each = "app.people"

[[body.children]]
type = "form"
id = "edit"
data = "item"
on_submit = [{ set = "app.saved", to = "row {{ event.form.name }}" }]

[[body.children.children]]
type = "input"
name = "name"
label = "Who: {{ item.name }}"

[[body.children.children]]
type = "button"
label = "Keep {{ item.name }}"
submit = true
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
  const holds = async (label, want) => {
    await until(`${label} to hold ${JSON.stringify(want)}`, async () => (await page.getByLabel(label).inputValue()) === want);
  };
  await shows("saved=nothing yet");
  await holds("Name", "Ada");
  await holds("Email", "ada@example.com");

  // What's typed is the form's draft: nothing changes state, and a reload keeps it, but not a password.
  await page.getByLabel("Name").fill("Grace");
  await page.getByLabel("Password").fill("s3cret");
  await page.getByLabel("Agree").check();
  await page.getByLabel("Large").check();
  await shows("saved=nothing yet");
  await page.waitForTimeout(300);
  await page.reload();
  await holds("Name", "Grace");
  await holds("Password", "");
  if (!(await page.getByLabel("Large").isChecked())) throw new Error("the draft's radio didn't come back");
  if (!(await page.getByLabel("Agree").isChecked())) throw new Error("the draft's checkbox didn't come back");

  // A button that isn't `submit` doesn't submit.
  await page.getByRole("button", { name: "Cancel" }).click();
  await shows("saved=cancelled");

  // Enter in a box submits: the form goes out with each named control, and starts over from data.
  await page.getByLabel("Password").fill("s3cret");
  await page.getByLabel("Email").press("Enter");
  await shows("saved=Grace <ada@example.com> size 3 agree true pass s3cret");
  await holds("Name", "Ada");
  await holds("Password", "");

  // So does the submit button.
  await page.getByLabel("Name").fill("Linus");
  await page.getByRole("button", { name: "Save" }).click();
  await shows("saved=Linus <ada@example.com> size  agree  pass");

  // Inside an each, each row's form has its own draft, kept across a reload, and submits alone.
  await page.getByLabel("Who: Ada").fill("Ada L.");
  await holds("Who: Grace", "Grace");
  await page.waitForTimeout(300);
  await page.reload();
  await holds("Who: Ada", "Ada L.");
  await holds("Who: Grace", "Grace");
  await page.getByLabel("Who: Grace").fill("Grace H.");
  await page.getByRole("button", { name: "Keep Grace" }).click();
  await shows("saved=row Grace H.");
  await holds("Who: Grace", "Grace");
  await holds("Who: Ada", "Ada L.");

  // The mirror never holds what a password input held, in a draft or on the form's own events.
  // (This app's on_submit copies the password into app.saved; that's the config's own choice.)
  const dir = join(work, "state");
  const client = await until("the mirror's state file", () => readdirSync(dir).find((f) => f.endsWith(".json")));
  const events = await until("the mirror's trail to hold the last submit", () => {
    const lines = readFileSync(join(dir, client.replace(/\.json$/, ".events.ndjson")), "utf8").trim().split("\n");
    const events = lines.map((line) => JSON.parse(line));
    return events.filter((e) => e.name === "contact.submit").length === 2 && events;
  });
  const own = events.filter((e) => e.name.endsWith(".change") || e.name === "contact.submit");
  if (JSON.stringify(own).includes("s3cret")) throw new Error("the mirror's trail holds a password");
  const state = JSON.parse(readFileSync(join(dir, client), "utf8"));
  if (JSON.stringify(state.drafts ?? {}).includes("s3cret")) throw new Error("the mirror's drafts hold a password");
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: the form started from data, kept its draft (one per row in an each), and submitted only when asked");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
