// The markdown deployed check (spec 040, "The atoms"): in a real browser, a markdown node draws
// its Markdown with the theme's styles, shows raw HTML as text without running it, and draws again
// when a variable in its templates changes.
//
// It runs genesis-serve on a temporary app of its own. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 26000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-markdown-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `# The markdown deployed check's app: notes with a heading, a list, a link, and some raw HTML.
[app]
name = "Markdown"

[state]
who = "Ada"

[[body]]
type = "button"
label = "Grace"
on_click = [{ set = "app.who", to = "Grace" }]

[[body]]
type = "markdown"
text = """
## Notes for {{ app.who }}

- first
- [the site](https://example.com)

<img src=x onerror="window.ran=1">
"""
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
  const heading = page.getByRole("heading", { name: "Notes for Ada" });
  await heading.waitFor();
  // Tailwind resets every element; the markdown styles give a heading its size and a list its dots.
  const size = await heading.evaluate((h) => parseFloat(getComputedStyle(h).fontSize));
  if (!(size > 20)) throw new Error(`the heading is ${size}px, unstyled`);
  const dots = await page.getByText("first").evaluate((li) => getComputedStyle(li.parentElement).listStyleType);
  if (dots !== "disc") throw new Error(`the list has ${dots}, not dots`);
  if ((await page.getByRole("link", { name: "the site" }).getAttribute("href")) !== "https://example.com") {
    throw new Error("the link went missing");
  }
  // Raw HTML is words on the page, never an element that runs.
  await page.getByText('<img src=x onerror="window.ran=1">').waitFor();
  await page.waitForTimeout(300);
  if (await page.evaluate(() => window.ran)) throw new Error("the raw HTML ran");
  // A change to a variable in its templates draws it again.
  await page.getByRole("button", { name: "Grace" }).click();
  await page.getByRole("heading", { name: "Notes for Grace" }).waitFor();

  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: markdown drew styled, showed raw HTML as text, and redrew with its variable");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
