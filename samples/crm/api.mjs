// The CRM sample's API (spec 130): companies and contacts, each with full CRUD as REST, searching,
// and a JSON data file that survives restarts. It decides what's valid; a refusal is a 4xx whose
// body is one plain sentence. Every request needs `Authorization: Bearer TOKEN`.
//
//   node samples/crm/api.mjs PORT DATA_FILE TOKEN      (or import { startCrmApi })

import { createServer } from "node:http";
import { existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";

/** What a missing data file starts from. */
export function seed() {
  return {
    companies: [
      { id: 1, name: "Acme", website: "https://acme.example.com" },
      { id: 2, name: "Globex", website: "https://globex.example.com" },
      { id: 3, name: "Initech", website: "" },
    ],
    contacts: [
      { id: 1, name: "Ada Lovelace", email: "ada@acme.example.com", phone: "555-0101", company_id: 1 },
      { id: 2, name: "Grace Hopper", email: "grace@acme.example.com", phone: "", company_id: 1 },
      { id: 3, name: "Lin Wei", email: "lin@globex.example.com", phone: "555-0103", company_id: 2 },
    ],
  };
}

/** A refusal: an HTTP status and the sentence that says why. */
class Refused extends Error {
  constructor(status, sentence) {
    super(sentence);
    this.status = status;
  }
}

const has = (text, q) => String(text ?? "").toLowerCase().includes(q);
const byName = (a, b) => a.name.localeCompare(b.name);
const text = (value) => (value === undefined || value === null ? "" : String(value).trim());

export function startCrmApi(port, dataFile, token) {
  const data = existsSync(dataFile) ? JSON.parse(readFileSync(dataFile, "utf8")) : seed();
  const save = () => {
    writeFileSync(`${dataFile}.tmp`, JSON.stringify(data, null, 2));
    renameSync(`${dataFile}.tmp`, dataFile);
  };
  const nextId = (rows) => rows.reduce((top, row) => Math.max(top, row.id), 0) + 1;
  const company = (id) => data.companies.find((c) => c.id === Number(id));
  const companyRow = (c) => ({ ...c, contacts: data.contacts.filter((p) => p.company_id === c.id).length });
  const contactRow = (p) => ({ ...p, company: company(p.company_id)?.name ?? "" });
  const find = (rows, id, what) => {
    const row = rows.find((r) => r.id === Number(id));
    if (!row) throw new Refused(404, `There's no ${what} ${id}.`);
    return row;
  };

  // Each table's rules: what a company or contact may hold once `fields` are merged into `row`.
  const companyFields = (fields, row = {}) => {
    const next = { ...row };
    if ("name" in fields || !row.id) next.name = text(fields.name);
    if ("website" in fields || !row.id) next.website = text(fields.website);
    if (!next.name) throw new Refused(422, "A company needs a name.");
    return next;
  };
  const contactFields = (fields, row = {}) => {
    const next = { ...row };
    for (const key of ["name", "email", "phone"]) {
      if (key in fields || !row.id) next[key] = text(fields[key]);
    }
    if ("company_id" in fields || !row.id) next.company_id = Number(fields.company_id);
    if (!next.name) throw new Refused(422, "A contact needs a name.");
    if (!next.email.includes("@")) throw new Refused(422, `${next.email || "An empty email"} isn't an email.`);
    if (!company(next.company_id)) throw new Refused(422, "Pick a company that exists.");
    const twin = data.contacts.find((p) => p.email.toLowerCase() === next.email.toLowerCase() && p.id !== row.id);
    if (twin) throw new Refused(409, `${twin.name} already has ${next.email}.`);
    return next;
  };

  const routes = {
    companies: {
      list: (query) => {
        const q = text(query.get("q")).toLowerCase();
        return { rows: data.companies.filter((c) => has(c.name, q)).sort(byName).map(companyRow) };
      },
      get: (id) => companyRow(find(data.companies, id, "company")),
      create: (fields) => {
        const row = { id: nextId(data.companies), ...companyFields(fields) };
        data.companies.push(row);
        return companyRow(row);
      },
      update: (id, fields) => {
        const row = find(data.companies, id, "company");
        Object.assign(row, companyFields(fields, row));
        return companyRow(row);
      },
      delete: (id) => {
        const row = find(data.companies, id, "company");
        const count = data.contacts.filter((p) => p.company_id === row.id).length;
        if (count) throw new Refused(409, `${row.name} still has ${count} contact${count === 1 ? "" : "s"}.`);
        data.companies.splice(data.companies.indexOf(row), 1);
        return null;
      },
    },
    contacts: {
      list: (query) => {
        const q = text(query.get("q")).toLowerCase();
        const at = query.get("company_id");
        return {
          rows: data.contacts
            .map(contactRow)
            .filter((p) => at === null || p.company_id === Number(at))
            .filter((p) => has(p.name, q) || has(p.email, q) || has(p.company, q))
            .sort(byName),
        };
      },
      get: (id) => contactRow(find(data.contacts, id, "contact")),
      create: (fields) => {
        const row = { id: nextId(data.contacts), ...contactFields(fields) };
        data.contacts.push(row);
        return contactRow(row);
      },
      update: (id, fields) => {
        const row = find(data.contacts, id, "contact");
        Object.assign(row, contactFields(fields, row));
        return contactRow(row);
      },
      delete: (id) => {
        const row = find(data.contacts, id, "contact");
        data.contacts.splice(data.contacts.indexOf(row), 1);
        return null;
      },
    },
  };

  /** Runs one request, and says what to answer: a status and a body. */
  const handle = (method, url, body) => {
    const [, table, id, extra] = url.pathname.split("/");
    const route = routes[table];
    if (!route || extra !== undefined) throw new Refused(404, `There's nothing at ${url.pathname}.`);
    if (id === undefined || id === "") {
      if (method === "GET") return [200, route.list(url.searchParams)];
      if (method === "POST") return [201, route.create(body)];
    } else {
      if (method === "GET") return [200, route.get(id)];
      if (method === "PATCH") return [200, route.update(id, body)];
      if (method === "DELETE") return [204, route.delete(id)];
    }
    throw new Refused(405, `${method} doesn't work on ${url.pathname}.`);
  };

  const server = createServer((request, response) => {
    const answer = (status, body) => {
      if (body === null) {
        response.writeHead(status);
        return response.end();
      }
      const plain = typeof body === "string";
      response.writeHead(status, { "content-type": plain ? "text/plain; charset=utf-8" : "application/json" });
      response.end(plain ? body : JSON.stringify(body));
    };
    if (request.headers.authorization !== `Bearer ${token}`) return answer(401, "Who are you? Send the token.");
    let raw = "";
    request.on("data", (chunk) => (raw += chunk));
    request.on("end", () => {
      try {
        let body = {};
        try {
          body = raw ? JSON.parse(raw) : {};
        } catch {
          throw new Refused(400, "That body isn't JSON.");
        }
        const changes = request.method !== "GET";
        const [status, out] = handle(request.method, new URL(request.url, "http://crm"), body);
        if (changes) save();
        answer(status, out);
      } catch (error) {
        if (!(error instanceof Refused)) throw error;
        answer(error.status, error.message);
      }
    });
  });
  return new Promise((resolve) => server.listen(port, "127.0.0.1", () => resolve(server)));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [port = "4020", dataFile = "crm-data.json", token = "crm-test-token"] = process.argv.slice(2);
  await startCrmApi(Number(port), dataFile, token);
  console.log(`CRM API on http://127.0.0.1:${port} (data in ${dataFile})`);
}
