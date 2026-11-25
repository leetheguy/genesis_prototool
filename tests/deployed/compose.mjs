// The Docker Compose deployed check (spec 010): the real stack, serve and dufs, run as a project of
// its own on temporary configs and state folders with a throwaway agent token. It checks the
// boundaries that only the deployed stack can show:
//
// - agents edit the app through /fs/ only, with the token; what they write there is live at once
// - the mirror is readable at /fs/.state/ and not writable through it
// - the guide an agent puts at the top of the configs folder is served, open, at /agents
// - serve can't write the configs folder, and the files service holds no credentials
// - dufs isn't published on any host port
//
// Run it with scripts/check-compose.sh, which builds the image first.

import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = new URL("../..", import.meta.url).pathname;
const work = mkdtempSync(join(tmpdir(), "genesis-compose-"));
const [configs, state] = [join(work, "configs"), join(work, "state")];
cpSync(join(root, "samples/hello"), configs, { recursive: true });
mkdirSync(state);
const port = 21000 + Math.floor(Math.random() * 1000);
const token = "compose-check-" + Math.random().toString(36).slice(2);
const project = `genesis-check-${process.pid}`;
const base = `http://127.0.0.1:${port}`;
const env = {
  ...process.env,
  CONFIGS_PATH: configs,
  STATE_PATH: state,
  GENESIS_PORT: String(port),
  GENESIS_AGENT_TOKEN: token,
  GENESIS_CREDS_CHECK: "credential-value",
  PUID: String(process.getuid()),
  PGID: String(process.getgid()),
};
const compose = (...args) =>
  execFileSync("docker", ["compose", "-p", project, "-f", join(root, "docker-compose.yml"), ...args], { env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const docker = (...args) => execFileSync("docker", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const agent = { authorization: `Bearer ${token}` };

async function until(what, check, ms = 60_000) {
  const deadline = Date.now() + ms;
  for (;;) {
    try {
      if (await check()) return;
    } catch {}
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

function expect(what, ok) {
  if (!ok) throw new Error(what);
}

try {
  compose("up", "-d", "--no-build");
  await until("serve's /health", async () => (await fetch(`${base}/health`)).ok);

  // /fs/ needs the token.
  expect("/fs/ without the token isn't 401", (await fetch(`${base}/fs/`)).status === 401);

  // What an agent writes through /fs/ is live at once: a broken app.toml fails, a good one loads.
  const write = async (text) => {
    const put = await fetch(`${base}/fs/app.toml`, { method: "PUT", headers: agent, body: text });
    expect(`writing app.toml through /fs/ answered ${put.status}`, put.ok);
  };
  const status = async () => (await (await fetch(`${base}/status`)).json()).status;
  await write('[app]\nname = "Made by an agent"\ncolour = 1\n');
  await until("/status to say the broken app failed", async () => (await status()) === "failed", 10_000);
  await write('[app]\nname = "Made by an agent"\n[[body]]\ntype = "text"\ntext = "Hi from dufs"\nlevel = 1\n');
  await until("/status to say the fixed app loaded", async () => (await status()) === "loaded", 10_000);
  expect("the app's page isn't served", (await fetch(`${base}/`)).ok);

  // The guide: open at /agents once AGENTS.md is in the configs folder, with BASE filled in.
  expect("/agents without AGENTS.md isn't 404", (await fetch(`${base}/agents`)).status === 404);
  const guide = await fetch(`${base}/fs/AGENTS.md`, { method: "PUT", headers: agent, body: "Start at BASE/status.\n" });
  expect(`writing AGENTS.md through /fs/ answered ${guide.status}`, guide.ok);
  const served = await fetch(`${base}/agents`);
  const text = await served.text();
  expect(`/agents answered ${served.status}: ${text}`, served.ok && text === `Start at ${base}/status.\n`);

  // The mirror is readable through /fs/.state/, and not writable.
  const mirrored = await fetch(`${base}/api/mirror`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ client: "check", state: { app: { clicks: 7 } }, events: [] }),
  });
  expect(`the mirror answered ${mirrored.status}`, mirrored.status === 204);
  const read = await fetch(`${base}/fs/.state/check.json`, { headers: agent });
  expect("the mirror isn't readable at /fs/.state/", read.ok && (await read.json()).app.clicks === 7);
  const overwrite = await fetch(`${base}/fs/.state/check.json`, { method: "PUT", headers: agent, body: "{}" });
  expect(`writing the mirror through /fs/ answered ${overwrite.status}`, !overwrite.ok);

  // serve can't write the configs folder; the files service holds no credentials.
  const serveId = compose("ps", "-q", "serve").trim();
  const filesId = compose("ps", "-q", "files").trim();
  let wrote = true;
  try {
    docker("exec", serveId, "sh", "-c", "touch /app/configs/oops");
  } catch {
    wrote = false;
  }
  expect("serve could write the configs folder", !wrote);
  const filesEnv = docker("inspect", "--format", "{{json .Config.Env}}", filesId);
  expect("the files service has a credential in its environment", !filesEnv.includes("GENESIS_CREDS") && !filesEnv.includes("credential-value"));

  // dufs isn't published on the host.
  // `compose port` fails when the port isn't published, which is what's wanted here.
  let published;
  try {
    published = compose("port", "files", "5000").trim();
  } catch {
    published = "";
  }
  expect(`dufs is published on the host at ${published}`, published === "");

  console.log("ok: /fs/ needs the token; what an agent wrote there went live; /agents serves the guide; the mirror reads but doesn't write; serve can't write configs; files holds no credentials and isn't published");
} catch (error) {
  console.error(`FAIL: ${error.message}${error.stderr ? `\n${error.stderr}` : ""}`);
  process.exitCode = 1;
} finally {
  try {
    compose("down", "-v", "--remove-orphans");
  } catch {}
  rmSync(work, { recursive: true, force: true });
}
