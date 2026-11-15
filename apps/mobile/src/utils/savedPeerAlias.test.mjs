import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./savedPeerAlias.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { isSavedPeerAppAlias } = await import(moduleUrl);

const identity = "11111111111111111111111111111111";
const appHash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const lxmfHash = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

test("app hash of a peer saved under its LXMF hash is an alias", () => {
  assert.equal(
    isSavedPeerAppAlias(appHash, { [identity]: appHash }, { [identity]: lxmfHash }, { [lxmfHash]: {} }),
    true,
  );
});

test("unknown destinations and unsaved owners are not aliases", () => {
  assert.equal(isSavedPeerAppAlias(appHash, {}, {}, { [lxmfHash]: {} }), false);
  assert.equal(isSavedPeerAppAlias(appHash, { [identity]: appHash }, { [identity]: lxmfHash }, {}), false);
  assert.equal(isSavedPeerAppAlias(appHash, { [identity]: appHash }, {}, { [lxmfHash]: {} }), false);
});
