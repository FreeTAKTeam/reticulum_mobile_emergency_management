import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./householdDefaults.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { withNativeHouseholdDefaults } = await import(moduleUrl);

test("blank household fields accept the defaults the runtime filled in", () => {
  const requested = { displayName: "Pixelcorvo", community: { householdId: " ", householdName: "", adults: 1 } };
  const persisted = { community: { householdId: "6521979f1165965b", householdName: "Household Pixelcorvo" } };
  assert.deepEqual(withNativeHouseholdDefaults(requested, persisted), {
    displayName: "Pixelcorvo",
    community: { householdId: "6521979f1165965b", householdName: "Household Pixelcorvo", adults: 1 },
  });
});

test("operator-provided household fields are never replaced by persisted values", () => {
  const requested = { community: { householdId: "0123456789abcdef", householdName: "Harbour House" } };
  const persisted = { community: { householdId: "ffffffffffffffff", householdName: "Household X" } };
  assert.deepEqual(withNativeHouseholdDefaults(requested, persisted).community, requested.community);
});
