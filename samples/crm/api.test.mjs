// The CRM sample API's checks (spec 130, "Tests"). Run with: node --test samples/crm/

import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startCrmApi } from "./api.mjs";

const TOKEN = "crm-test-token";

/** Starts an API on a fresh data file, and hands back a way to ask it things. */
async function api(dataFile = join(mkdtempSync(join(tmpdir(), "crm-api-")), "data.json")) {
  const server = await startCrmApi(0, dataFile, TOKEN);
  const base = `http://127.0.0.1:${server.address().port}`;
  const ask = async (method, path, body, token = TOKEN) => {
    const response = await fetch(`${base}${path}`, {
      method,
      headers: { authorization: `Bearer ${token}`, ...(body ? { "content-type": "application/json" } : {}) },
      body: body ? JSON.stringify(body) : undefined,
    });
    const text = await response.text();
    const json = (response.headers.get("content-type") ?? "").startsWith("application/json");
    return { status: response.status, body: json ? JSON.parse(text) : text };
  };
  const close = () => new Promise((resolve) => server.close(resolve));
  return { ask, close, dataFile };
}

const names = (rows) => rows.map((r) => r.name);

test("each table lists sorted by name, with its counts and names joined in", async () => {
  const { ask, close } = await api();
  const companies = await ask("GET", "/companies");
  assert.deepEqual(names(companies.body.rows), ["Acme", "Globex", "Initech"]);
  assert.deepEqual(companies.body.rows.map((c) => c.contacts), [2, 1, 0]);
  const contacts = await ask("GET", "/contacts");
  assert.deepEqual(names(contacts.body.rows), ["Ada Lovelace", "Grace Hopper", "Lin Wei"]);
  assert.equal(contacts.body.rows[2].company, "Globex");
  await close();
});

test("q and company_id filter, ignoring case", async () => {
  const { ask, close } = await api();
  assert.deepEqual(names((await ask("GET", "/companies?q=GLO")).body.rows), ["Globex"]);
  assert.deepEqual(names((await ask("GET", "/contacts?q=acme")).body.rows), ["Ada Lovelace", "Grace Hopper"]);
  assert.deepEqual(names((await ask("GET", "/contacts?q=lin@")).body.rows), ["Lin Wei"]);
  assert.deepEqual(names((await ask("GET", "/contacts?company_id=2")).body.rows), ["Lin Wei"]);
  assert.deepEqual((await ask("GET", "/contacts?company_id=0")).body.rows, []);
  await close();
});

test("full CRUD on companies", async () => {
  const { ask, close } = await api();
  const made = await ask("POST", "/companies", { name: " Umbrella ", website: "" });
  assert.equal(made.status, 201);
  assert.deepEqual(made.body, { id: 4, name: "Umbrella", website: "", contacts: 0 });
  assert.equal((await ask("GET", "/companies/4")).body.name, "Umbrella");
  const changed = await ask("PATCH", "/companies/4", { website: "https://umbrella.example.com" });
  assert.deepEqual(changed.body, { id: 4, name: "Umbrella", website: "https://umbrella.example.com", contacts: 0 });
  assert.equal((await ask("DELETE", "/companies/4")).status, 204);
  assert.equal((await ask("GET", "/companies/4")).status, 404);
  await close();
});

test("full CRUD on contacts", async () => {
  const { ask, close } = await api();
  const made = await ask("POST", "/contacts", { name: "Katherine Johnson", email: "kj@initech.example.com", phone: "", company_id: 3 });
  assert.equal(made.status, 201);
  assert.equal(made.body.id, 4);
  assert.equal(made.body.company, "Initech");
  const changed = await ask("PATCH", "/contacts/4", { phone: "555-0104", company_id: 2 });
  assert.equal(changed.body.phone, "555-0104");
  assert.equal(changed.body.company, "Globex");
  assert.equal(changed.body.name, "Katherine Johnson");
  assert.equal((await ask("DELETE", "/contacts/4")).status, 204);
  assert.equal((await ask("GET", "/contacts/4")).status, 404);
  await close();
});

test("each refusal is its status with one plain sentence", async () => {
  const { ask, close } = await api();
  const refusals = [
    [["POST", "/companies", { name: "  " }], 422, "A company needs a name."],
    [["POST", "/contacts", { name: "", email: "x@y", company_id: 1 }], 422, "A contact needs a name."],
    [["POST", "/contacts", { name: "Bo", email: "bo", company_id: 1 }], 422, "bo isn't an email."],
    [["POST", "/contacts", { name: "Bo", email: "bo@x", company_id: 9 }], 422, "Pick a company that exists."],
    [["POST", "/contacts", { name: "Bo", email: "ADA@acme.example.com", company_id: 1 }], 409, "Ada Lovelace already has ADA@acme.example.com."],
    [["PATCH", "/contacts/3", { email: "grace@acme.example.com" }], 409, "Grace Hopper already has grace@acme.example.com."],
    [["DELETE", "/companies/1"], 409, "Acme still has 2 contacts."],
    [["DELETE", "/companies/2"], 409, "Globex still has 1 contact."],
    [["GET", "/contacts/99"], 404, "There's no contact 99."],
    [["GET", "/deals"], 404, "There's nothing at /deals."],
    [["PUT", "/contacts/1", {}], 405, "PUT doesn't work on /contacts/1."],
  ];
  for (const [[method, path, body], status, sentence] of refusals) {
    assert.deepEqual(await ask(method, path, body), { status, body: sentence }, `${method} ${path}`);
  }
  // A contact keeps its own email when it changes something else.
  assert.equal((await ask("PATCH", "/contacts/1", { phone: "555-0199" })).status, 200);
  await close();
});

test("no token is a 401", async () => {
  const { ask, close } = await api();
  assert.deepEqual(await ask("GET", "/companies", undefined, "wrong"), { status: 401, body: "Who are you? Send the token." });
  await close();
});

test("the data file survives a restart, and a missing one starts from the seed", async () => {
  const first = await api();
  assert.equal(existsSync(first.dataFile), false);
  await first.ask("POST", "/companies", { name: "Soylent" });
  await first.ask("GET", "/companies");
  await first.close();
  assert.ok(JSON.parse(readFileSync(first.dataFile, "utf8")).companies.some((c) => c.name === "Soylent"));
  const again = await api(first.dataFile);
  assert.deepEqual(names((await again.ask("GET", "/companies")).body.rows), ["Acme", "Globex", "Initech", "Soylent"]);
  // A refused change writes nothing.
  await again.ask("POST", "/companies", { name: "" });
  await again.close();
  assert.equal(JSON.parse(readFileSync(first.dataFile, "utf8")).companies.length, 4);
  rmSync(join(first.dataFile, ".."), { recursive: true, force: true });
});
