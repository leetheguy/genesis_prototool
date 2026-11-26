// The file deployed check (spec 040, "Controls"): in a real browser, a `file` in a form refuses a
// file over its max_size and fires nothing, takes one that fits, and on submit the call goes out
// through serve as multipart, so the API gets the file as a part beside the form's text. The
// picker empties once the form starts over. A file kept from before a reload is gone, and the call
// that sends it fails loudly.
//
// It runs genesis-serve on a temporary app of its own, with a mock API that echoes the parts it
// gets. Run it with scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const root = new URL("../..", import.meta.url).pathname;
const port = 24000 + Math.floor(Math.random() * 1000);
const apiPort = port + 1000;
const base = `http://127.0.0.1:${port}`;
const work = mkdtempSync(join(tmpdir(), "genesis-file-"));
const configs = join(work, "configs");
mkdirSync(configs);
const chrome =
  process.env.GENESIS_CHROME ??
  join(process.env.HOME, ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell");

// The mock API: POST /upload answers with each part it got, in one line of text.
const api = createServer(async (request, response) => {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  const form = await new Response(Buffer.concat(chunks), { headers: { "content-type": request.headers["content-type"] ?? "" } })
    .formData()
    .catch(() => null);
  const parts = [];
  for (const [name, value] of form ?? []) {
    parts.push(typeof value === "string" ? `${name}=${value}` : `${name}=${value.name}(${value.type}):${await value.text()}`);
  }
  response.writeHead(200, { "content-type": "application/json" });
  response.end(JSON.stringify({ got: parts.join(" ") || "no parts" }));
});
await new Promise((resolve) => api.listen(apiPort, "127.0.0.1", resolve));

writeFileSync(
  join(configs, "app.toml"),
  `# The file deployed check's app: a form that sends a note and a file to a mock API.
[app]
name = "File"

[state]
got = "nothing yet"

[connections.docs]
base_url = "http://127.0.0.1:${apiPort}"

[connections.docs.ops.upload]
method = "POST"
path = "/upload"

[[body]]
type = "text"
text = "got={{ app.got }}"

[[body]]
type = "form"
id = "upload"
on_submit = [{ call = "docs.upload", params = { note = "{{ event.form.note }}", doc = "{{ event.form.doc }}" }, into = { "app.got" = "/got" } }]

[[body.children]]
type = "input"
name = "note"
label = "Note"

[[body.children]]
type = "file"
name = "doc"
label = "Document"
max_size = 100

[[body.children]]
type = "button"
label = "Send"
submit = true

[on]
"docs.upload.failed" = [{ set = "app.got", to = "failed: {{ event.error }}" }]
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
  const shows = (text) => page.getByText(text).waitFor();
  const picker = page.getByLabel("Document");
  await shows("got=nothing yet");

  // Too big: a note under the picker says so, the picker lets go, and nothing goes out.
  await picker.setInputFiles({ name: "big.txt", mimeType: "text/plain", buffer: Buffer.from("x".repeat(200)) });
  await shows("big.txt is 200 bytes; this takes at most 100 bytes.");
  if ((await picker.inputValue()) !== "") throw new Error("the picker kept a file it refused");

  // One that fits goes out with the note, as a file part with its name, type, and bytes.
  await picker.setInputFiles({ name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("made-up file text") });
  if (await page.getByText("this takes at most").count()) throw new Error("the refusal stayed after a good pick");
  await page.getByLabel("Note").fill("for the record");
  await page.getByRole("button", { name: "Send" }).click();
  await shows("got=note=for the record doc=notes.txt(text/plain):made-up file text");
  await until("the picker to empty", async () => (await picker.inputValue()) === "");

  // A file picked before a reload is gone after it: its kept reference fails the call loudly.
  await picker.setInputFiles({ name: "later.txt", mimeType: "text/plain", buffer: Buffer.from("made-up later text") });
  await page.waitForTimeout(300);
  await page.reload();
  await shows("got=nothing yet");
  await page.getByRole("button", { name: "Send" }).click();
  await shows("its file is gone");

  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);
  console.log("ok: the file picker refused a big file, sent one that fits as a multipart part, and failed loudly on a gone one");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  api.close();
  rmSync(work, { recursive: true, force: true });
}
