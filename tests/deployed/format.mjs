// The formats deployed check (spec 080, "Formats"): in a real browser, formats write in the language
// `[app] locale` names (German here), through the browser's own Intl: grouping, currency, month and
// weekday names, and how long ago. The variables keep their raw values.
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
const work = mkdtempSync(join(tmpdir(), "genesis-format-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

// Three hours ago, as the config is written.
const earlier = new Date(Date.now() - 3 * 3600 * 1000).toISOString();

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Formats"
locale = "de-DE"

[state]
total = 1234.5
seen = "${earlier}"
day = "2026-11-21"

[[body]]
type = "text"
text = "number={{ app.total }}"
format = { number = 2 }

[[body]]
type = "text"
text = "money={{ app.total }}"
format = { currency = "EUR" }

[[body]]
type = "text"
text = "day={{ app.day }}"
format = { date = "%A, %-d. %B %Y" }

[[body]]
type = "text"
text = "seen={{ app.seen }}"
format = { ago = true }

[[body]]
type = "text"
text = "raw={{ app.total }}"
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
  // Intl puts a no-break space between the amount and the euro sign.
  const text = async (start) => (await page.getByText(start).first().textContent()).replace(/\u00a0/g, " ");
  await page.getByText("raw=1234.5").waitFor();
  const want = {
    "number=": "number=1.234,50",
    "money=": "money=1.234,50 €",
    "day=": "day=Samstag, 21. November 2026",
    "seen=": "seen=vor 3 Stunden",
  };
  for (const [start, shown] of Object.entries(want)) {
    const got = await text(start);
    if (got !== shown) throw new Error(`${start} shows ${JSON.stringify(got)}, not ${JSON.stringify(shown)}`);
  }
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: each format wrote in German, and the variables kept their raw values");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
