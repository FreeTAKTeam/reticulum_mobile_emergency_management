import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./communityStatus.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { COMMUNITY_STATUS_OPTIONS, freshnessLabel, householdComposition } = await import(moduleUrl);

test("community status exposes the exact four operator actions", () => {
  assert.deepEqual(COMMUNITY_STATUS_OPTIONS.map(({ label }) => label), [
    "All Home", "1 Missing", "Evacuated", "Needs Help",
  ]);
});

test("native-projected B04 records are presentation-only inputs", () => {
  const updatedAt = 1_710_000_000_000;
  const projection = {
    householdId: "0123456789abcdef", householdName: "Harbour House",
    adults: 2, children: 1, pets: 2, roleBadges: ["medic"],
    status: "needs_help", saverActive: true, updatedAtMs: updatedAt,
    sourceIdentity: "signed-source",
  };
  assert.equal(householdComposition(projection), "3 people · 2 pets");
  assert.equal(freshnessLabel(updatedAt, updatedAt + 3_600_000), "updated 1h ago");
});
