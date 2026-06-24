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
  DEFAULT_TCP_COMMUNITY_ENDPOINTS,
  normalizeTcpCommunityClients,
} = await import(moduleUrl);

test("missing TCP community clients fall back to the default bootstrap endpoint", () => {
  assert.deepEqual(normalizeTcpCommunityClients(undefined), DEFAULT_TCP_COMMUNITY_ENDPOINTS);
});

test("explicitly empty TCP community clients remain empty", () => {
  assert.deepEqual(normalizeTcpCommunityClients([], DEFAULT_TCP_COMMUNITY_ENDPOINTS, true), []);
});
