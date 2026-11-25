// The media deployed check (spec 040, "Media and embeds"): in a real browser, a named player fires
// .play, .pause, and .ended onto the bus; one without an id fires nothing; an iframe is sandboxed.
//
// It runs genesis-serve on a temporary app of its own. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 22000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-media-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Media"

[state]
heard = "nothing"

[[body]]
type = "text"
text = "heard: {{ app.heard }}"

[[body]]
type = "audio"
id = "song"
src = "/missing.mp3"

[[body]]
type = "video"
src = "/missing.mp4"

[[body]]
type = "iframe"
src = "/health"
title = "Health"

[on]
"song.play" = [{ set = "app.heard", to = "play" }]
"song.pause" = [{ set = "app.heard", to = "pause" }]
"song.ended" = [{ set = "app.heard", to = "ended" }]
"body.2.ended" = [{ set = "app.heard", to = "the unnamed video" }]
`,
);

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
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${base}/`);
  await page.getByText("heard: nothing").waitFor();

  // The media files don't exist; the check is the wiring, so it fires the element's own events.
  for (const event of ["play", "pause", "ended"]) {
    await page.locator("audio").dispatchEvent(event);
    await page.getByText(`heard: ${event}`).waitFor();
  }
  await page.locator("video").dispatchEvent("ended");
  await new Promise((resolve) => setTimeout(resolve, 300));
  if ((await page.getByText("heard: the unnamed video").count()) > 0) throw new Error("a video with no id fired .ended");

  const sandbox = await page.locator("iframe").getAttribute("sandbox");
  if (sandbox !== "allow-scripts allow-forms allow-popups") throw new Error(`the iframe's sandbox is ${sandbox}`);
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: a named player fired play, pause, and ended; an unnamed one fired nothing; the iframe is sandboxed");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
