// The layout deployed check (spec 040, "Style keys"): in a real browser, a 12-column grid lays
// children of span 4 three to a row, offset skips columns, "fit" fits as many 6rem columns as there's
// room for, and a span by screen size changes with the window's width.
//
// It runs genesis-serve on a temporary app of its own. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 21000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-layout-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

const cells = (prefix, n, keys) =>
  Array.from({ length: n }, (_, i) => `[[body.children]]\ntype = "text"\ntext = "${prefix}${i}"\n${keys}\n`).join("\n");
// One top-level grid per case, each child a text named so it can be found.
const app = `[app]
name = "Layout"

[[body]]
type = "cell"
grid = 12
${cells("third-", 6, "span = 4")}
[[body]]
type = "cell"
grid = 12
${cells("offset-", 1, "span = 4\noffset = 4")}
[[body]]
type = "cell"
grid = "fit"
${cells("fit-", 20, "")}
[[body]]
type = "cell"
grid = 12
${cells("sized-", 2, "span = { base = 12, md = 6 }")}
`;
writeFileSync(join(configs, "app.toml"), app);

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
  const page = await browser.newPage({ viewport: { width: 1200, height: 900 } });
  await page.goto(`${base}/`);
  await page.getByText("sized-1").waitFor();
  const box = async (text) => page.getByText(text, { exact: true }).boundingBox();
  const near = (a, b) => Math.abs(a - b) < 2;

  const thirds = await Promise.all([0, 1, 2, 3].map((i) => box(`third-${i}`)));
  if (!(near(thirds[0].y, thirds[1].y) && near(thirds[1].y, thirds[2].y) && thirds[3].y > thirds[2].y + 1))
    throw new Error(`span 4 in a 12-column grid isn't three to a row: ${JSON.stringify(thirds)}`);
  if (!(thirds[0].x < thirds[1].x && thirds[1].x < thirds[2].x && near(thirds[3].x, thirds[0].x)))
    throw new Error(`span 4 children aren't side by side, left to right: ${JSON.stringify(thirds)}`);

  const offset = await box("offset-0");
  if (!near(offset.x, thirds[1].x)) throw new Error(`offset 4 doesn't start at the fifth column: ${offset.x} vs ${thirds[1].x}`);

  const fits = await Promise.all(Array.from({ length: 20 }, (_, i) => box(`fit-${i}`)));
  const firstRow = fits.filter((b) => near(b.y, fits[0].y)).length;
  if (firstRow < 2 || fits.some((b) => b.width < 96 - 1))
    throw new Error(`"fit" didn't fit 6rem columns: ${firstRow} in the first row, widths ${fits.map((b) => b.width)}`);
  if (fits[firstRow].y <= fits[0].y) throw new Error(`"fit" didn't wrap to a second row`);

  const wide = await Promise.all([box("sized-0"), box("sized-1")]);
  if (!near(wide[0].y, wide[1].y)) throw new Error(`at 1200px, span md = 6 isn't two to a row`);
  await page.setViewportSize({ width: 500, height: 900 });
  await until("the narrow layout", async () => (await box("sized-1")).y > (await box("sized-0")).y + 1);

  console.log(`ok: span 4 three to a row, offset skips columns, "fit" made ${firstRow} columns, and span changes by screen size`);
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
