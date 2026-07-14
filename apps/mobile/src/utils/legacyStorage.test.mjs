import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./legacyStorage.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { asRecord, asTrimmedString, optionalNumber, readJson } = await import(moduleUrl);

test("legacy storage decoding rejects malformed and missing JSON", () => {
  globalThis.localStorage = {
    getItem(key) {
      return key === "valid" ? '{"value":1}' : key === "invalid" ? "{" : null;
    },
  };

  assert.deepEqual(readJson("valid"), { value: 1 });
  assert.equal(readJson("invalid"), null);
  assert.equal(readJson("missing"), null);
});

test("legacy scalar normalization preserves supported values", () => {
  assert.deepEqual(asRecord({ value: 1 }), { value: 1 });
  assert.equal(asRecord([]), null);
  assert.equal(asTrimmedString("  Alpha  "), "Alpha");
  assert.equal(asTrimmedString(42), "");
  assert.equal(optionalNumber("12.5"), 12.5);
  assert.equal(optionalNumber(""), undefined);
  assert.equal(optionalNumber("invalid"), undefined);
});
