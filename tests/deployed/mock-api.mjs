// A tiny contacts API for the deployed checks and the contacts sample: GET /contacts lists them,
// POST /contacts adds one. Every request needs `Authorization: Bearer TOKEN`, or it's a 401.
//
//   node tests/deployed/mock-api.mjs PORT TOKEN      (or import { startMockApi })

import { createServer } from "node:http";

export function startMockApi(port, token) {
  const contacts = [];
  const server = createServer((request, response) => {
    const send = (status, body) => {
      response.writeHead(status, { "content-type": "application/json" });
      response.end(JSON.stringify(body));
    };
    if (request.headers.authorization !== `Bearer ${token}`) return send(401, { error: "who are you?" });
    let text = "";
    request.on("data", (chunk) => (text += chunk));
    request.on("end", () => {
      const path = request.url.split("?")[0];
      if (request.method === "GET" && path === "/contacts") return send(200, { rows: contacts });
      if (request.method === "POST" && path === "/contacts") {
        const contact = { id: contacts.length + 1, ...JSON.parse(text || "{}") };
        contacts.push(contact);
        return send(201, contact);
      }
      send(404, { error: `no ${request.method} ${path}` });
    });
  });
  return new Promise((resolve) => server.listen(port, "127.0.0.1", () => resolve(server)));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [port = "4010", token = "test-token"] = process.argv.slice(2);
  await startMockApi(Number(port), token);
  console.log(`mock contacts API on http://127.0.0.1:${port}/contacts (token ${token})`);
}
