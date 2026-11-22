// The calls deployed check (spec 030, its last task): a real browser, a real serve, a mock API that
// wants a credential only serve holds, and a real PurpleFlow webhook when one is reachable.
//
// It runs serve on a temporary copy of samples/ with the contacts app pointed at a fresh mock API,
// and GENESIS_CREDS_CONTACTS_TOKEN in serve's environment only. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";
import { startMockApi } from "./mock-api.mjs";

const root = new URL("../..", import.meta.url).pathname;
const pick = () => 20000 + Math.floor(Math.random() * 5000);
const [port, apiPort] = [pick(), pick() + 5000];
const base = `http://127.0.0.1:${port}`;
const pfHooks = process.env.GENESIS_PF_HOOKS ?? "http://127.0.0.1:4002/hooks";
const token = "check-token-" + Math.random().toString(36).slice(2);
const work = mkdtempSync(join(tmpdir(), "genesis-calls-"));
const configs = join(work, "configs");
cpSync(join(root, "samples"), configs, { recursive: true });
const appToml = join(configs, "contacts/app.toml");
writeFileSync(
  appToml,
  readFileSync(appToml, "utf8").replace("http://127.0.0.1:4010", `http://127.0.0.1:${apiPort}`).replace("http://127.0.0.1:4002/hooks", pfHooks),
);
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

const api = await startMockApi(apiPort, token);
const serve = spawn(
  join(root, "target/debug/genesis-serve"),
  ["--configs", configs, "--state", join(work, "state"), "--dist", join(root, "apps/web/dist"), "--listen", `127.0.0.1:${port}`],
  { stdio: ["ignore", "inherit", "inherit"], env: { ...process.env, GENESIS_CREDS_CONTACTS_TOKEN: token } },
);
let browser;
try {
  await until("serve's /health", async () => (await fetch(`${base}/health`)).ok);
  browser = await chromium.launch({ executablePath: chrome });
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${base}/contacts/`);
  await page.getByText("Contacts: []").waitFor();

  await page.getByRole("button", { name: "Add Ada" }).click();
  await page.getByText('Contacts: [{"id":1,"name":"Ada"}]').waitFor();

  const html = await page.content();
  if (html.includes(token)) throw new Error("the credential reached the browser");

  const pfUp = await fetch(`${pfHooks}/ping`, { method: "POST", headers: { "content-type": "application/json" }, body: '{"text":"ping"}' })
    .then((r) => r.ok)
    .catch(() => false);
  if (pfUp) {
    await page.getByRole("button", { name: "Ping PurpleFlow" }).click();
    await page.getByText("PurpleFlow says: pong").waitFor();
  } else {
    console.log(`note: no PurpleFlow at ${pfHooks}; skipped the ping`);
  }
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log(`ok: listed and added contacts through serve with its credential${pfUp ? ", and PurpleFlow said pong" : ""}`);
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  api.close();
  rmSync(work, { recursive: true, force: true });
}
