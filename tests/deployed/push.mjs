// The push deployed check (spec 110): an agent's curl to POST /api/push/{name} changes a screen that
// is open in a real browser, and the pushed event lands in that client's mirror.
//
// It runs genesis-serve on a temporary app with a throwaway agent token. Run it with
// scripts/check-hello.sh. Waiting is always on a condition with a deadline (OP-PARAMS section 4).

import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 20000 + Math.floor(Math.random() * 1000);
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-push-"));
const [configs, state] = [join(work, "configs"), join(work, "state")];
mkdirSync(configs);
writeFileSync(
  join(configs, "app.toml"),
  `[app]
name = "Push"

[state]
note = "Nothing yet"

[[body]]
type = "text"
text = "{{ app.note }}"

[on]
"push.review_ready" = [{ set = "app.note", to = "{{ event.note }}: {{ event.contact.name }}" }]
`,
);
const token = "push-check-" + Math.random().toString(36).slice(2);
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

function expect(what, ok) {
  if (!ok) throw new Error(what);
}

const serve = spawn(
  join(root, "target/debug/genesis-serve"),
  [
    "--configs", configs, "--state", state, "--logs", join(work, "logs"),
    "--dist", join(root, "apps/web/dist"), "--listen", `127.0.0.1:${port}`,
  ],
  { stdio: ["ignore", "inherit", "inherit"], env: { ...process.env, GENESIS_AGENT_TOKEN: token } },
);
let browser;
try {
  await until("serve's /health", async () => (await fetch(`${base}/health`)).ok);
  browser = await chromium.launch({ executablePath: chrome });
  const page = await browser.newPage();
  await page.goto(`${base}/`);
  await page.getByText("Nothing yet").waitFor();

  // As an agent would: curl, with the token.
  const curl = (...args) =>
    execFileSync("curl", ["-sS", "-X", "POST", "-H", `Authorization: Bearer ${token}`, ...args], { encoding: "utf8" });
  const body = JSON.stringify({ contact: { id: 42, name: "Ada" }, note: "Ready for review" });
  let answer;
  // The page's config stream may still be connecting: push until it counts.
  await until("a push to reach the open page", async () => {
    answer = JSON.parse(curl("-H", "content-type: application/json", "-d", body, `${base}/api/push/review_ready`));
    return answer.delivered === 1;
  });
  await page.getByText("Ready for review: Ada").waitFor();

  // The pushed event is in the client's mirror, so the sender can read what it set off.
  await until("the push in the mirror's trail", async () => {
    const trail = readdirSync(state).filter((f) => f.endsWith(".events.ndjson"));
    return trail.some((f) => readFileSync(join(state, f), "utf8").includes('"push.review_ready"'));
  });

  // Without the token, it's refused, and the screen doesn't change.
  const refused = await fetch(`${base}/api/push/review_ready`, { method: "POST", body: '{"note": "No"}' });
  expect(`a push without the token answered ${refused.status}`, refused.status === 401);

  console.log("ok: an agent's curl to the push route changed the open screen; the push is in the mirror; no token is refused");
} catch (error) {
  console.error(`FAIL: ${error.message}${error.stderr ? `\n${error.stderr}` : ""}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  rmSync(work, { recursive: true, force: true });
}
