import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./eamReadiness.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { buildWebEamReadinessSummary, computeWebEamTeamSummary } = await import(moduleUrl);

const green = {
  callsign: "Alpha-1",
  securityStatus: "Green",
  capabilityStatus: "Yellow",
  preparednessStatus: "Red",
  medicalStatus: "Unknown",
  mobilityStatus: "Green",
  commsStatus: "Yellow",
  updatedAt: 100,
};
const mixed = {
  callsign: "Bravo-2",
  securityStatus: "Red",
  capabilityStatus: "Green",
  preparednessStatus: "Green",
  medicalStatus: "Yellow",
  mobilityStatus: "Unknown",
  commsStatus: "Red",
  updatedAt: 200,
};

test("web EAM readiness matches the native aggregate scoring contract", () => {
  const summary = buildWebEamReadinessSummary([green, mixed]);

  assert.equal(summary.activeTotal, 2);
  assert.equal(summary.updatedAt, 200);
  assert.equal(summary.statusMetrics[0].score, 63);
  assert.equal(summary.statusMetrics[1].score, 75);
  assert.equal(summary.statusMetrics[5].score, 38);
  assert.deepEqual(summary.messages.map((message) => message.callsign), ["Alpha-1", "Bravo-2"]);
});

test("web EAM readiness excludes deleted records but preserves latest update time", () => {
  const summary = buildWebEamReadinessSummary([
    green,
    { ...mixed, deletedAt: 300, updatedAt: 300 },
  ]);

  assert.equal(summary.activeTotal, 1);
  assert.equal(summary.updatedAt, 300);
  assert.equal(summary.statusMetrics[0].score, 100);
});

test("web EAM readiness returns six neutral metrics when empty", () => {
  const summary = buildWebEamReadinessSummary([]);

  assert.equal(summary.activeTotal, 0);
  assert.equal(summary.statusMetrics.length, 6);
  assert.ok(summary.statusMetrics.every((metric) => metric.score === 0));
});

test("web EAM team summary excludes deleted records and selects the worst status", () => {
  const summary = computeWebEamTeamSummary([
    { ...green, teamUid: "team-1", overallStatus: "Green" },
    { ...mixed, teamUid: "team-1", overallStatus: "Red" },
    { ...mixed, callsign: "Deleted", teamUid: "team-1", overallStatus: "Yellow", deletedAt: 300 },
  ], "team-1");

  assert.equal(summary.active_total, 2);
  assert.equal(summary.deleted_total, 1);
  assert.equal(summary.overall_status, "Red");
});
