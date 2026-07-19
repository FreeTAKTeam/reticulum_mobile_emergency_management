import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./detachedStoreTask.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { runDetachedStoreTask } = await import(moduleUrl);

test("detached store task failures retain their scope and cause", async () => {
  const errors = [];
  const logs = [];
  const sink = {
    setLastError: (message) => errors.push(message),
    logUi: (level, message) => logs.push({ level, message }),
  };

  runDetachedStoreTask(sink, "events", "projection refresh", async () => {
    throw new Error("database unavailable");
  });
  await new Promise((resolve) => setImmediate(resolve));

  const expected = "[events] projection refresh failed: database unavailable";
  assert.deepEqual(errors, [expected]);
  assert.deepEqual(logs, [{ level: "Warn", message: expected }]);
});

test("successful detached store tasks do not report an error", async () => {
  let reported = false;
  runDetachedStoreTask(
    {
      setLastError: () => {
        reported = true;
      },
      logUi: () => {
        reported = true;
      },
    },
    "checklists",
    "projection refresh",
    async () => undefined,
  );
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(reported, false);
});
