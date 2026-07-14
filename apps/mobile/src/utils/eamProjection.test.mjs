import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

async function importTypeScriptModule(relativeUrl, replacements = new Map()) {
  const source = await readFile(new URL(relativeUrl, import.meta.url), "utf8");
  let transpiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  for (const [specifier, replacement] of replacements) {
    transpiled = transpiled.replaceAll(`"${specifier}"`, `"${replacement}"`);
  }
  const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
  return import(moduleUrl);
}

const r3aktSource = await readFile(new URL("./r3akt.ts", import.meta.url), "utf8");
const r3aktOutput = ts.transpileModule(r3aktSource, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const r3aktUrl = `data:text/javascript;base64,${Buffer.from(r3aktOutput).toString("base64")}`;
const {
  countRedStatuses,
  nextLocalUpdatedAt,
  normalizeMessage,
  toProjectionRecord,
  toStoredMessages,
  toTeamSummary,
} = await importTypeScriptModule("./eamProjection.ts", new Map([["./r3akt", r3aktUrl]]));

test("EAM records normalize and round-trip through the native projection shape", () => {
  const message = normalizeMessage({
    callsign: "  Alpha-1  ",
    groupName: "invalid",
    securityStatus: "Red",
    capabilityStatus: "invalid",
    updatedAt: 100,
    source: { rns_identity: " identity ", display_name: "  Alpha  " },
  });

  assert.equal(message.callsign, "Alpha-1");
  assert.equal(message.groupName, "YELLOW");
  assert.equal(message.capabilityStatus, "Unknown");
  assert.deepEqual(message.source, { rns_identity: "identity", display_name: "Alpha" });
  assert.equal(countRedStatuses(message), 1);

  const stored = toStoredMessages([toProjectionRecord(message)]);
  assert.deepEqual(stored["alpha-1"], message);
});

test("local update timestamps remain monotonic when the wall clock stalls", () => {
  const before = Date.now();
  assert.ok(nextLocalUpdatedAt(before + 1000) > before + 1000);
});

test("native team summaries preserve status totals and ISO timestamps", () => {
  assert.deepEqual(
    toTeamSummary({
      teamUid: "team-1",
      total: 4,
      activeTotal: 3,
      deletedTotal: 1,
      greenTotal: 1,
      yellowTotal: 0,
      redTotal: 2,
      overallStatus: "Red",
      updatedAt: 1000,
    }),
    {
      team_uid: "team-1",
      total: 4,
      active_total: 3,
      deleted_total: 1,
      overall_status: "Red",
      by_status: { Green: 1, Red: 2 },
      updated_at: "1970-01-01T00:00:01.000Z",
    },
  );
});
