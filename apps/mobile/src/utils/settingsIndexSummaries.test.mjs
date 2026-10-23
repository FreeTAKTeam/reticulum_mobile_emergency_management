import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./settingsIndexSummaries.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const summaries = await import(moduleUrl);

test("identity summary adds the household only when named", () => {
  assert.equal(summaries.identitySummary("CORVO", "Harbour House"), 'CORVO · Household "Harbour House"');
  assert.equal(summaries.identitySummary("CORVO", "  "), "CORVO");
  assert.equal(summaries.identitySummary("", ""), "No call sign");
});

test("network and routing summaries", () => {
  assert.equal(summaries.networkSummary(2, false), "2 TCP · LoRa off");
  assert.equal(summaries.networkSummary(0, true), "0 TCP · LoRa on");
  assert.equal(summaries.routingSummary("Autonomous", "abc"), "Autonomous");
  assert.equal(summaries.routingSummary("SemiAutonomous", ""), "Semi-autonomous · no hub selected");
  assert.equal(
    summaries.routingSummary("Connected", "0123456789abcdef0123456789abcdef"),
    "Connected · hub 01234567…",
  );
});

test("location summary reports the loop state only when sharing", () => {
  assert.equal(summaries.locationSummary(false, 60, "running"), "Off");
  assert.equal(summaries.locationSummary(true, 60, "running"), "Every 60 s · acquiring location");
  assert.equal(summaries.locationSummary(true, 120, "permission_denied"), "Every 120 s · permission denied");
});

test("SOS summary lists active triggers", () => {
  const none = { shake: false, powerButton: false, floatingButton: false };
  assert.equal(summaries.sosSummary(false, { ...none, shake: true }), "Off");
  assert.equal(summaries.sosSummary(true, none), "On · manual only");
  assert.equal(
    summaries.sosSummary(true, { ...none, shake: true, powerButton: true }),
    "On · Shake + power button",
  );
  assert.equal(summaries.sosSummary(true, { ...none, powerButton: true }), "On · Power button");
});

test("device summaries", () => {
  assert.equal(summaries.powerSummary(true, 20), "On at 20 %");
  assert.equal(summaries.powerSummary(false, 20), "Off");
  assert.equal(summaries.pluginsSummary(0, 0), "None installed");
  assert.equal(summaries.pluginsSummary(2, 1), "2 installed · 1 enabled");
  assert.equal(summaries.nodeSummary(false, 3), "Stopped");
  assert.equal(summaries.nodeSummary(true, 1), "Running · 1 interface");
});

test("folded section summary skips empty parts", () => {
  assert.equal(summaries.foldSummary(["Off", " ", "Base map"]), "Off · Base map");
});
