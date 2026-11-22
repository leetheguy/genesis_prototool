// The hello deployed check (spec 010, its last task): a real genesis-serve, a real browser, real clicks.
//
// It starts genesis-serve on samples/ with a fresh state folder, opens /hello/ in headless
// Chromium, clicks the button three times, and checks that the screen counts them and that the
// mirror on disk holds the state and every click. Run it with scripts/check-hello.sh, which
// builds what it needs first.
//
// Waiting is always on a condition with a deadline, never a bare sleep (OP-PARAMS section 4).

import { spawn } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 18000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const state = mkdtempSync(join(tmpdir(), "genesis-hello-state-"));
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

/** Polls `check` until it returns something truthy, or fails after `ms` saying what it waited for. */
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

function fail(why) {
  console.error(`FAIL: ${why}`);
  process.exitCode = 1;
}

const serve = spawn(
  join(root, "target/debug/genesis-serve"),
  ["--configs", join(root, "samples"), "--state", state, "--dist", join(root, "apps/web/dist"), "--listen", `127.0.0.1:${port}`],
  { stdio: ["ignore", "inherit", "inherit"] },
);
let browser;
try {
  await until("serve's /health", async () => (await fetch(`${base}/health`)).ok);
  browser = await chromium.launch({ executablePath: chrome });
  const page = await browser.newPage();
  const consoleErrors = [];
  page.on("console", (message) => message.type() === "error" && consoleErrors.push(message.text()));
  page.on("pageerror", (error) => consoleErrors.push(String(error)));

  await page.goto(`${base}/hello/`);
  await page.getByRole("heading", { name: "Hello, Genesis" }).waitFor();
  await page.getByText("Clicked 0 times").waitFor();
  const theme = await page.locator("[data-theme]").first().getAttribute("data-theme");
  if (theme !== "corporate") fail(`the screen wears theme ${theme}, not corporate`);

  const button = page.getByRole("button", { name: "Click me" });
  for (let i = 0; i < 3; i++) await button.click();
  await page.getByText("Clicked 3 times").waitFor();

  const dir = join(state, "hello");
  const client = await until("the mirror's state file", () => readdirSync(dir).find((f) => f.endsWith(".json")));
  const mirrored = await until("the mirror to count 3 clicks", () => {
    const snapshot = JSON.parse(readFileSync(join(dir, client), "utf8"));
    return snapshot.app.clicks === 3 && snapshot;
  });
  if (mirrored.sys.view !== "main") fail(`the mirror's sys.view is ${mirrored.sys.view}`);

  const events = readFileSync(join(dir, client.replace(/\.json$/, ".events.ndjson")), "utf8")
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  const clicks = events.filter((e) => e.name === "hello_btn.click").length;
  const logs = events.filter((e) => e.log).map((e) => e.log);
  if (clicks !== 3) fail(`the mirror's trail has ${clicks} clicks, not 3`);
  if (logs.join("|") !== "click number 1|click number 2|click number 3") fail(`the mirror's logs are ${JSON.stringify(logs)}`);
  if (!events.some((e) => e.name === "app.opened")) fail("the mirror's trail has no app.opened");
  // The session is kept on this device: a reload of the page comes back where it left off.
  await page.reload();
  await page.getByText("Clicked 3 times").waitFor();

  if (consoleErrors.length) fail(`the browser logged errors: ${consoleErrors.join(" / ")}`);

  if (!process.exitCode) console.log(`ok: 3 clicks counted on screen, mirrored to ${dir}, and kept across a reload`);
} catch (error) {
  fail(error.message);
} finally {
  await browser?.close();
  serve.kill();
  rmSync(state, { recursive: true, force: true });
}
