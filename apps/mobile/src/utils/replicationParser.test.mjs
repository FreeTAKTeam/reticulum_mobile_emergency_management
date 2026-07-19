import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./replicationParser.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { asNumber, parseReplicationEnvelope } = await import(moduleUrl);

test("replication envelopes reject malformed shapes and blank kinds", () => {
  for (const raw of [
    "not-json",
    "null",
    "[]",
    "{}",
    '{"kind":7}',
    '{"kind":"   "}',
  ]) {
    assert.equal(parseReplicationEnvelope(raw), null, raw);
  }
});

test("replication envelope kinds are normalized without losing payload fields", () => {
  assert.deepEqual(
    parseReplicationEnvelope('{"kind":"  event.upsert  ","sequence":7}'),
    {
      kind: "event.upsert",
      payload: { kind: "event.upsert", sequence: 7 },
    },
  );
});

test("replication numbers accept finite scalars and reject coercion traps", () => {
  assert.equal(asNumber(" 12.5 ", 3), 12.5);
  for (const value of ["", "   ", false, true, [], [7], null, undefined, "Infinity"]) {
    assert.equal(asNumber(value, 3), 3, String(value));
  }
});
