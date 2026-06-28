import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./tcpCommunityServers.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  DEFAULT_TCP_COMMUNITY_ENDPOINT,
  DEFAULT_TCP_COMMUNITY_ENDPOINTS,
  TCP_COMMUNITY_SERVERS,
  normalizeTcpCommunityClients,
} = await import(moduleUrl);

test("R3AKT server is the default TCP community endpoint", () => {
  assert.equal(TCP_COMMUNITY_SERVERS[0].name, "R3AKT Server");
  assert.equal(DEFAULT_TCP_COMMUNITY_ENDPOINT, "134.122.46.48:37428");
  assert.deepEqual(DEFAULT_TCP_COMMUNITY_ENDPOINTS, ["134.122.46.48:37428"]);
});

test("rmap is available as a TCP community server", () => {
  assert.ok(
    TCP_COMMUNITY_SERVERS.some(
      (server) => server.name === "rmap" && server.host === "rmap.world" && server.port === 4242,
    ),
  );
});

test("missing TCP community clients fall back to the default bootstrap endpoint", () => {
  assert.deepEqual(normalizeTcpCommunityClients(undefined), DEFAULT_TCP_COMMUNITY_ENDPOINTS);
});

test("explicitly empty TCP community clients remain empty", () => {
  assert.deepEqual(normalizeTcpCommunityClients([], DEFAULT_TCP_COMMUNITY_ENDPOINTS, true), []);
});

test("rmap TCP community clients are preserved", () => {
  assert.deepEqual(normalizeTcpCommunityClients(["rmap.world:4242"]), ["rmap.world:4242"]);
});
