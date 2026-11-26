// The toast and confirm deployed check (spec 100): in a real browser, a `confirm` asks in a modal
// dialog and its list waits; Escape answers no and the list stops quietly; the yes button carries
// it on, and the `toast` it shows goes away on its own, or sooner when tapped.
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
const work = mkdtempSync(join(tmpdir(), "genesis-confirm-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Confirm"
toast_ms = 1500

[state]
wiped = 0

[[body]]
type = "text"
text = "Wiped {{ app.wiped }} times"

[[body]]
type = "button"
id = "wipe"
label = "Wipe"

[[body]]
type = "button"
id = "hello"
label = "Say hello"

[on]
"wipe.click" = [
  { confirm = "Wipe everything?", yes = "Wipe it", no = "Keep it" },
  { step = "app.wiped" },
  { toast = "Wiped.", variant = "success" },
]
"hello.click" = [{ toast = "Hello there.", ms = 60000 }]
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
  await page.getByText("Wiped 0 times").waitFor();

  // Escape is a no: the dialog goes, and nothing after the confirm runs.
  await button("Wipe").click();
  const dialog = page.getByRole("dialog");
  await dialog.getByText("Wipe everything?").waitFor();
  await button("Keep it").waitFor();
  await page.keyboard.press("Escape");
  await until("the dialog to go", async () => (await page.getByText("Wipe everything?").count()) === 0);
  await page.waitForTimeout(300);
  if (!(await page.getByText("Wiped 0 times").count())) throw new Error("a no still ran the rest of the list");

  // The yes button carries on: the count goes up and a success toast shows, then goes on its own.
  await button("Wipe").click();
  await button("Wipe it").click();
  await page.getByText("Wiped 1 times").waitFor();
  const toast = page.getByText("Wiped.", { exact: true });
  await toast.waitFor();
  if (!(await toast.evaluate((el) => el.classList.contains("alert-success")))) throw new Error("the toast isn't green");
  await until("the toast to go on its own", async () => (await toast.count()) === 0);

  // A tap takes a long toast away at once.
  await button("Say hello").click();
  const hello = page.getByText("Hello there.");
  await hello.waitFor();
  await hello.click();
  await until("the tapped toast to go", async () => (await hello.count()) === 0);

  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: confirm waited, Escape said no, yes carried on, and toasts went on their own and on a tap");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
