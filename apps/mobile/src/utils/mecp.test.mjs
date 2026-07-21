import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./mecp.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { decodeMecpMessage, encodeMecpMessage } = await import(moduleUrl);

test("MECP encoding round-trips every portable boundary field", () => {
  const encoded = encodeMecpMessage({
    severity: 1,
    codes: ["R03"],
    mode: "portable",
    extras: {
      pax: 9999,
      coordinates: { latitude: 90, longitude: -180 },
      references: ["CASE_1"],
      etaMinutes: 9999,
      language: "eng",
      timestamp: "2359",
      callsign: "MEDIC-1",
    },
  });
  const decoded = decodeMecpMessage(encoded);

  assert.equal(decoded.valid, true);
  assert.deepEqual(decoded.extras, {
    callsign: "MEDIC-1",
    etaMinutes: 9999,
    language: "eng",
    pax: 9999,
    references: ["#CASE_1"],
    coordinates: { latitude: 90, longitude: -180 },
    timestamp: "2359",
  });
});

test("MECP encoding rejects values that its decoder cannot represent", () => {
  const invalidInputs = [
    { severity: 4, codes: ["P01"] },
    { severity: 2, codes: ["invalid"] },
    { severity: 2, codes: ["P01"], extras: { coordinates: { latitude: Number.NaN, longitude: 0 } } },
    { severity: 2, codes: ["P01"], extras: { coordinates: { latitude: 91, longitude: 0 } } },
    { severity: 2, codes: ["P01"], extras: { pax: 10_000 } },
    { severity: 2, codes: ["R03"], extras: { etaMinutes: 10_000 } },
    { severity: 2, codes: ["P01"], extras: { language: "english" } },
    { severity: 2, codes: ["P01"], extras: { references: ["bad reference"] } },
    { severity: 2, codes: ["P01"], mode: "portable", extras: { timestamp: "2400" } },
    { severity: 2, codes: ["P01"], mode: "portable", extras: { callsign: "bad callsign" } },
  ];

  for (const input of invalidInputs) {
    assert.throws(() => encodeMecpMessage(input), /Invalid MECP/);
  }
});

test("MECP decoding reports out-of-range coordinates without accepting them", () => {
  const decoded = decodeMecpMessage("MECP/2/P01 91,0");

  assert.equal(decoded.valid, true);
  assert.equal(decoded.extras.coordinates, null);
  assert.deepEqual(decoded.warnings, ['Coordinates outside valid range: "91,0".']);
});

test("MECP decoding accepts compact wire bodies with additional text", () => {
  const decoded = decodeMecpMessage("H01 Bolle");

  assert.equal(decoded.valid, true);
  assert.equal(decoded.severity, 2);
  assert.equal(decoded.category, "H");
  assert.deepEqual(decoded.codes, ["H01"]);
  assert.equal(decoded.codeDetails[0]?.label, "Have water available");
  assert.equal(decoded.details, "Bolle");
  assert.equal(decoded.raw, "H01 Bolle");
});
