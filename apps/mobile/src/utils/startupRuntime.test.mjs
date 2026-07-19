import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./startupRuntime.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { reconcileStartupRuntime } = await import(moduleUrl);

function actions(calls) {
  return {
    start: async () => calls.push("start"),
    restart: async () => calls.push("restart"),
  };
}

test("starts a stopped runtime", async () => {
  const calls = [];
  await reconcileStartupRuntime(
    { running: false, restartRequired: false },
    actions(calls),
  );
  assert.deepEqual(calls, ["start"]);
});

test("reconciles an already-running restored runtime through idempotent start", async () => {
  const calls = [];
  await reconcileStartupRuntime(
    { running: true, restartRequired: false },
    actions(calls),
  );
  assert.deepEqual(calls, ["start"]);
});

test("restarts a running runtime when settings explicitly require it", async () => {
  const calls = [];
  await reconcileStartupRuntime(
    { running: true, restartRequired: true },
    actions(calls),
  );
  assert.deepEqual(calls, ["restart"]);
});
