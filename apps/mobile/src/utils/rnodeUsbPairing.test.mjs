import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./rnodeUsbPairing.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { selectUsbBondedRnodeCandidate } = await import(moduleUrl);

test("selects the newly bonded Bluetooth RNode after USB pairing mode", () => {
  const previous = [
    { id: "AA:AA:AA:AA:AA:AA", address: "AA:AA:AA:AA:AA:AA", name: "TOYOTA CAMRY", paired: true },
  ];
  const current = [
    ...previous,
    { id: "48:CA:43:3F:14:11", address: "48:CA:43:3F:14:11", name: "RNode 5E2D", paired: true },
  ];

  assert.equal(selectUsbBondedRnodeCandidate(previous, current)?.address, "48:CA:43:3F:14:11");
});

test("does not select a Bluetooth device when USB pairing leaves multiple new bonded candidates", () => {
  const current = [
    { id: "48:CA:43:3F:14:11", address: "48:CA:43:3F:14:11", name: "", paired: true },
    { id: "48:CA:43:3F:14:12", address: "48:CA:43:3F:14:12", name: "", paired: true },
  ];

  assert.equal(selectUsbBondedRnodeCandidate([], current), undefined);
});
