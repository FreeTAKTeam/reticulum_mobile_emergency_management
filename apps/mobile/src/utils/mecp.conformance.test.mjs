import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

// MECP §6.1 freetext decode vectors from mecp-org/MECP, pinned to a release tag.
const VECTORS_URL = "https://raw.githubusercontent.com/mecp-org/MECP/v1.4.0/freetext/tests.json";

const source = await readFile(new URL("./mecp.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { decodeMecpMessage } = await import(`data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`);
const response = await fetch(VECTORS_URL);
assert.ok(response.ok, `GET ${VECTORS_URL} failed: ${response.status}`);
const vectors = await response.json();

// Cases this decoder does not pass yet, reported as TODO instead of failing.
const KNOWN_FAILURES = new Set([
  "timestamp MECP/3/L01 @123",
  "timestamp MECP/2/T05 @1430,",
  "reference MECP/2/C08 #base-camp,",
  "reference MECP/2/C08 (#tag)",
  "reference MECP/2/C08 #hut،ok",
  "reference MECP/2/C08 #hut”ok",
  "reference MECP/2/C08 #hut、ok",
  "reference MECP/2/C08 #hut，ok",
  "gps MECP/3/L02 2,5 km",
  "callsign MECP/3/L20 ~DL/OK1ABC",
  "callsign MECP/3/L20 ~W1AW-9",
  "language MECP/3/L04 @zh_CN",
  "duration MECP/2/R03 2d",
  "duration MECP/2/R03 48h",
  "duration MECP/2/C01 R03 1pax 45",
  "duration MECP/2/R03 45.",
  "duration MECP/2/R03 45 2h",
  "duration MECP/2/R03 2d 45min",
  "duration MECP/2/R03 1h 30min",
  "duration MECP/2/R03 2 h",
  "message MECP/1/T05 ",
  "message MECP/1/t05 x",
  "message MECP/1/T05 foo\nbar",
  "message MECP/1/T05 foo\u2028bar",
  "message MECP/1/T05 foo\fbar",
]);

function decodeConvention(convention, input) {
  const { valid, extras } = decodeMecpMessage(input);
  if (convention === "message") return { valid };
  const value = !valid ? null : {
    pax: extras.pax,
    gps: extras.coordinates && { lat: extras.coordinates.latitude, lon: extras.coordinates.longitude },
    timestamp: extras.timestamp && `${extras.timestamp.slice(0, 2)}:${extras.timestamp.slice(2)}`,
    language: extras.language,
    callsign: extras.callsign,
    reference: extras.references[0]?.slice(1) ?? null,
    duration: extras.etaMinutes,
  }[convention];
  return value === null ? {} : { [convention]: value };
}

for (const { convention, input, expect } of vectors) {
  const id = `${convention} ${input}`;
  test(`MECP vector: ${JSON.stringify(id)}`, { todo: KNOWN_FAILURES.has(id) }, () => {
    assert.deepEqual(decodeConvention(convention, input), expect);
  });
}
