import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("../composables/setupWizardConfig.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { SETUP_STEPS } = await import(moduleUrl);

test("setup wizard steps follow the Settings index order", () => {
  assert.deepEqual(
    SETUP_STEPS.map((step) => step.id),
    ["welcome", "identity", "network", "routing", "location", "sos", "review"],
  );
});

test("only the routing step is optional", () => {
  assert.deepEqual(
    SETUP_STEPS.filter((step) => step.optional).map((step) => step.id),
    ["routing"],
  );
});
