// The CRM deployed check (spec 130): the CRM sample, driven in a real browser against its own API.
// It searches, adds a company (the form's check first), opens one and adds a contact there, edits a
// contact, fails loudly deleting a company that has contacts, deletes a contact after asking, and
// checks the API's data file holds it all, even after a restart.
//
// It runs samples/crm/api.mjs on a temporary data file, and serve on a temporary copy of
// samples/crm/configs pointed at it, with the token in serve's environment only. Run it with
// scripts/check-hello.sh.

import { spawn } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";
import { startCrmApi } from "../../samples/crm/api.mjs";

const root = new URL("../..", import.meta.url).pathname;
const pick = () => 24000 + Math.floor(Math.random() * 4000);
const [port, apiPort, againPort] = [pick(), pick() + 4000, pick() + 8000];
const base = `http://127.0.0.1:${port}`;
const token = "crm-check-" + Math.random().toString(36).slice(2);
const work = mkdtempSync(join(tmpdir(), "genesis-crm-"));
const configs = join(work, "configs");
const dataFile = join(work, "crm-data.json");
cpSync(join(root, "samples/crm/configs"), configs, { recursive: true });
const crmToml = join(configs, "crm.toml");
writeFileSync(crmToml, readFileSync(crmToml, "utf8").replace("http://127.0.0.1:4020", `http://127.0.0.1:${apiPort}`));
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

let api = await startCrmApi(apiPort, dataFile, token);
const serve = spawn(
  join(root, "target/debug/genesis-serve"),
  ["--configs", configs, "--state", join(work, "state"), "--dist", join(root, "apps/web/dist"), "--listen", `127.0.0.1:${port}`],
  { stdio: ["ignore", "inherit", "inherit"], env: { ...process.env, GENESIS_CREDS_CRM_TOKEN: token } },
);
let browser;
try {
  await until("serve's /health", async () => (await fetch(`${base}/health`)).ok);
  browser = await chromium.launch({ executablePath: chrome });
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${base}/`);

  const button = (name, within = page) => within.getByRole("button", { name, exact: true });
  // Only what shows: a closed dialog's company picker holds every company's name too.
  const named = (text) => page.getByText(text, { exact: true }).filter({ visible: true });
  const gone = (what, locator) => until(what, async () => (await locator.count()) === 0);
  // A list's row: the innermost block that holds the name and the row's buttons.
  const row = (name, has = "Edit") =>
    page.locator("div").filter({ has: named(name) }).filter({ has: button(has) }).last();
  const dialog = (title) => page.getByRole("dialog").filter({ hasText: title });
  // A toast says what it should in its color; a tap takes it away, so the next one is easy to find.
  async function toast(text, color) {
    const shown = page.getByRole("status").filter({ hasText: text });
    await shown.waitFor();
    if (!(await shown.evaluate((el, c) => el.classList.contains(`alert-${c}`), color))) throw new Error(`the toast "${text}" isn't ${color}`);
    await shown.click();
    await gone(`the toast "${text}" to go`, shown);
  }

  // The list shows the starting data, and the search box asks the API.
  await row("Acme", "Open").getByText("2 contacts").waitFor();
  await named("Globex").waitFor();
  await page.getByPlaceholder("Search companies").fill("glo");
  await gone("Acme to leave the search", named("Acme"));
  await named("Globex").waitFor();
  await page.getByPlaceholder("Search companies").fill("");
  await named("Acme").waitFor();

  // A company with no name shows the form's check and sends nothing; with one, it's added.
  await button("Add company").click();
  const adding = dialog("Add a company");
  await button("Add company", adding).click();
  await adding.getByText("A company needs a name.").waitFor();
  const before = await (await fetch(`http://127.0.0.1:${apiPort}/companies`, { headers: { authorization: `Bearer ${token}` } })).json();
  if (before.rows.length !== 3) throw new Error(`a failing form still sent: ${before.rows.length} companies`);
  await adding.getByLabel("Name").fill("Umbrella");
  await adding.getByLabel("Website").fill("https://umbrella.example.com");
  await button("Add company", adding).click();
  await toast("Added Umbrella.", "success");
  await gone("the add dialog to close", adding);
  await row("Umbrella", "Open").getByText("0 contacts").waitFor();

  // Opening a company shows its contacts; a contact added there starts at that company.
  await button("Open", row("Acme", "Open")).click();
  await page.getByRole("heading", { name: "Acme" }).waitFor();
  await named("Ada Lovelace").waitFor();
  await named("Grace Hopper").waitFor();
  await button("Add contact").click();
  const newContact = dialog("Add a contact");
  const company = newContact.getByLabel("Company");
  await until("the new contact to start at Acme", async () => (await company.evaluate((el) => el.options[el.selectedIndex]?.text)) === "Acme");
  await newContact.getByLabel("Name").fill("Katherine Johnson");
  await newContact.getByLabel("Email").fill("katherine@acme.example.com");
  await button("Add contact", newContact).click();
  await toast("Added Katherine Johnson.", "success");
  await named("Katherine Johnson").waitFor();

  // Editing a contact saves it, and the list shows the change.
  await button("Edit", row("Grace Hopper")).click();
  const editing = dialog("Edit Grace Hopper");
  await editing.getByLabel("Phone").fill("555-0199");
  await button("Save", editing).click();
  await toast("Saved Grace Hopper.", "success");
  await row("Grace Hopper").getByText("555-0199").waitFor();

  // Deleting a company that has contacts asks, then fails loudly with the API's sentence.
  await button("All companies").click();
  await button("Delete", row("Acme", "Open")).click();
  const ask = dialog("Delete Acme?");
  await button("Delete", ask).click();
  await toast("Acme still has 3 contacts.", "error");
  await row("Acme", "Open").getByText("3 contacts").waitFor();

  // Deleting a contact asks first: Keep deletes nothing, Delete deletes it and says so.
  await button("Contacts").click();
  await page.getByRole("heading", { name: "Contacts" }).waitFor();
  await row("Lin Wei").getByText("lin@globex.example.com at Globex").waitFor();
  await button("Delete", row("Lin Wei")).click();
  await button("Keep", dialog("Delete Lin Wei?")).click();
  await gone("the question to go", dialog("Delete Lin Wei?"));
  await page.waitForTimeout(300);
  if (!(await named("Lin Wei").count())) throw new Error("Keep still deleted the contact");
  await button("Delete", row("Lin Wei")).click();
  await button("Delete", dialog("Delete Lin Wei?")).click();
  await toast("Deleted Lin Wei.", "success");
  await gone("Lin Wei to leave the list", named("Lin Wei"));

  const html = await page.content();
  if (html.includes(token)) throw new Error("the token reached the browser");
  if (errors.length) throw new Error(`the page threw: ${errors.join(" / ")}`);

  // The data file holds every change, and a restarted API still has them.
  const saved = JSON.parse(readFileSync(dataFile, "utf8"));
  const names = (rows) => rows.map((r) => r.name).sort().join(", ");
  if (names(saved.companies) !== "Acme, Globex, Initech, Umbrella") throw new Error(`the file's companies: ${names(saved.companies)}`);
  if (names(saved.contacts) !== "Ada Lovelace, Grace Hopper, Katherine Johnson") throw new Error(`the file's contacts: ${names(saved.contacts)}`);
  await new Promise((resolve) => api.close(resolve));
  api = await startCrmApi(againPort, dataFile, token);
  const again = await (await fetch(`http://127.0.0.1:${againPort}/contacts?q=grace`, { headers: { authorization: `Bearer ${token}` } })).json();
  if (again.rows[0]?.phone !== "555-0199") throw new Error(`the restarted API lost the edit: ${JSON.stringify(again)}`);

  console.log("ok: the CRM searched, added, checked, edited, asked before deleting, failed loudly, and kept its data");
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  serve.kill();
  api?.close();
  rmSync(work, { recursive: true, force: true });
}
