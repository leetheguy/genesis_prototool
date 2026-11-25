// The dialog deployed check (spec 040, "The atoms"): in a real browser, a dialog opens when its
// variable says so, as a modal; Escape closes it, which fires .close, sets the variable false, and
// runs on_close; and a button inside it that sets the variable false closes it without .close.
//
// It runs genesis-serve on a temporary app of its own. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 25000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-dialog-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `# The dialog deployed check's app: a button that asks in a dialog before it deletes.
[app]
name = "Dialog"

[state]
asking = false
said = "nothing"

[[body]]
type = "text"
text = "said={{ app.said }} asking={{ app.asking }}"

[[body]]
type = "button"
label = "Delete"
on_click = [{ set = "app.asking", to = true }]

[[body]]
type = "dialog"
id = "ask"
open = "app.asking"
title = "Delete it?"
on_close = [{ set = "app.said", to = "never mind" }]

[[body.children]]
type = "button"
label = "Yes, delete"
on_click = [{ set = "app.said", to = "deleted" }, { set = "app.asking", to = false }]
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
  const dialog = page.getByRole("dialog");
  const modal = () => page.evaluate(() => document.querySelector("dialog")?.matches(":modal") ?? false);
  await shows("said=nothing asking=false");
  if (await dialog.isVisible()) throw new Error("the dialog showed before its variable opened it");

  // The variable opens it, as a modal.
  await page.getByRole("button", { name: "Delete" }).click();
  await dialog.getByText("Delete it?").waitFor();
  await until("the dialog to be modal", modal);

  // Escape closes it: .close sets the variable false, then on_close runs.
  await page.keyboard.press("Escape");
  await shows("said=never mind asking=false");
  await until("the dialog to close", async () => !(await dialog.isVisible()));

  // A button inside that sets the variable false closes it, and .close doesn't fire.
  await page.getByRole("button", { name: "Delete" }).click();
  await until("the dialog to open again", modal);
  await dialog.getByRole("button", { name: "Yes, delete" }).click();
  await shows("said=deleted asking=false");
  await until("the dialog to close", async () => !(await dialog.isVisible()));

  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: the dialog opened with its variable as a modal, and closed with Escape and from inside");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
