import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./actionMessageStatus.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  applyActionMessageStatusCycle,
} = await import(moduleUrl);

const baseMessage = {
  callsign: "ALPHA",
  groupName: "Blue",
  securityStatus: "Unknown",
  capabilityStatus: "Green",
  preparednessStatus: "Yellow",
  medicalStatus: "Red",
  mobilityStatus: "Unknown",
  commsStatus: "Unknown",
  updatedAt: 100,
};

test("cycling an EAM status returns the next local message immediately", () => {
  const updated = applyActionMessageStatusCycle(baseMessage, "securityStatus", 150);

  assert.equal(updated.securityStatus, "Green");
  assert.equal(updated.updatedAt, 150);
  assert.equal(baseMessage.securityStatus, "Unknown");
});

test("non-status EAM fields are ignored by the status cycle helper", () => {
  const updated = applyActionMessageStatusCycle(baseMessage, "callsign", 150);

  assert.equal(updated, undefined);
});
