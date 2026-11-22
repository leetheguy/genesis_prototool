// The Docker Compose deployed check (spec 010): the real stack, serve and dufs, run as a project of
// its own on temporary configs and state folders with a throwaway agent token. It checks the
// boundaries that only the deployed stack can show:
//
// - agents edit apps through /fs/ only, with the token; a new app written there is live at once
// - the mirror is readable at /fs/.state/ and not writable through it
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
cpSync(join(root, "samples"), configs, { recursive: true });
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

  // An app written through /fs/ is live at once.
  const app = '[app]\nname = "Made by an agent"\n[[body]]\ntype = "heading"\ntext = "Hi from dufs"\n';
  const put = await fetch(`${base}/fs/agentapp/app.toml`, { method: "PUT", headers: agent, body: app });
  expect(`writing an app through /fs/ answered ${put.status}`, put.ok);
  await until("/status to list the new app as loaded", async () => {
    const report = await (await fetch(`${base}/status`)).json();
    return report.apps.agentapp?.status === "loaded";
  }, 10_000);
  expect("the new app's page isn't served", (await fetch(`${base}/agentapp/`)).ok);

  // The mirror is readable through /fs/.state/, and not writable.
  const mirrored = await fetch(`${base}/api/hello/mirror`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ client: "check", state: { app: { clicks: 7 } }, events: [] }),
  });
  expect(`the mirror answered ${mirrored.status}`, mirrored.status === 204);
  const read = await fetch(`${base}/fs/.state/hello/check.json`, { headers: agent });
  expect("the mirror isn't readable at /fs/.state/", read.ok && (await read.json()).app.clicks === 7);
  const write = await fetch(`${base}/fs/.state/hello/check.json`, { method: "PUT", headers: agent, body: "{}" });
  expect(`writing the mirror through /fs/ answered ${write.status}`, !write.ok);

  // serve can't write the configs folder; the files service holds no credentials.
  const serveId = compose("ps", "-q", "serve").trim();
  const filesId = compose("ps", "-q", "files").trim();
  let wrote = true;
  try {
    docker("exec", serveId, "sh", "-c", "touch /app/configs/hello/oops");
  } catch {
    wrote = false;
  }
  expect("serve could write the configs folder", !wrote);
  const filesEnv = docker("inspect", "--format", "{{json .Config.Env}}", filesId);
  expect("the files service has a credential in its environment", !filesEnv.includes("GENESIS_CREDS") && !filesEnv.includes("credential-value"));

  // dufs isn't published on the host.
  expect("dufs is published on a host port", compose("port", "files", "5000").trim() === "");

  console.log("ok: /fs/ needs the token; an app written there went live; the mirror reads but doesn't write; serve can't write configs; files holds no credentials and isn't published");
} catch (error) {
  console.error(`FAIL: ${error.message}${error.stderr ? `\n${error.stderr}` : ""}`);
  process.exitCode = 1;
} finally {
  try {
    compose("down", "-v", "--remove-orphans");
  } catch {}
  rmSync(work, { recursive: true, force: true });
}
