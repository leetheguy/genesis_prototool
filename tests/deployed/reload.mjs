// The hot reload deployed check (spec 060, its last task): edit an app while it's open in a real
// browser, and watch it change, keep its state, show a bad edit's problems, and recover.
//
// It runs genesis-serve on a temporary copy of samples/hello, never on samples/ itself. Run it with
// scripts/check-hello.sh. Waiting is always on a condition with a deadline (OP-PARAMS section 4).

import { spawn } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 19000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-reload-"));
const configs = join(work, "configs");
cpSync(join(root, "samples/hello"), configs, { recursive: true });
const appToml = join(configs, "app.toml");
const original = readFileSync(appToml, "utf8");
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

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
  await page.goto(`${base}/`);
  await page.getByRole("button", { name: "Click me" }).click();
  await page.getByText("Clicked 1 times").waitFor();

  writeFileSync(appToml, original.replace('text = "Hello, Genesis"', 'text = "Hello, live edit"'));
  await page.getByRole("heading", { name: "Hello, live edit" }).waitFor();
  await page.getByText("Clicked 1 times").waitFor();

  writeFileSync(appToml, original + '\n[[body]]\ntype = "text"\ncolour = "red"\n');
  await page.getByText("This app's config has problems").waitFor();
  await page.getByText("colour").waitFor();
  await page.getByRole("button", { name: "Click me" }).waitFor();

  writeFileSync(appToml, original);
  await page.getByRole("heading", { name: "Hello, Genesis" }).waitFor();
  await until("the banner to clear", async () => (await page.getByText("This app's config has problems").count()) === 0);

  console.log("ok: a live edit redrew the page and kept its state; a bad edit showed its problems; fixing it cleared them");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
