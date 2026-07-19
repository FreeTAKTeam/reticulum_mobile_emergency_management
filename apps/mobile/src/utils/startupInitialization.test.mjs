import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./startupInitialization.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { isRecoveredChatHydrationError, runRecoverableStartupStep } = await import(moduleUrl);

test("recoverable startup failures are reported without rejecting startup", async () => {
  const failures = [];
  const result = await runRecoverableStartupStep(
    "event projection hydration",
    async () => {
      throw new Error("timeout");
    },
    (message, error) => failures.push({ message, error }),
  );

  assert.equal(result, false);
  assert.equal(failures.length, 1);
  assert.equal(failures[0].message, "event projection hydration failed: timeout");
  assert.equal(failures[0].error.message, "timeout");
});

test("successful startup steps return true without reporting a failure", async () => {
  let reported = false;
  const result = await runRecoverableStartupStep(
    "chat history hydration",
    async () => undefined,
    () => {
      reported = true;
    },
  );

  assert.equal(result, true);
  assert.equal(reported, false);
});

test("only recovered chat hydration errors are eligible for clearing", () => {
  assert.equal(isRecoveredChatHydrationError("chat history hydration failed: timeout"), true);
  assert.equal(isRecoveredChatHydrationError("chat history hydration retry failed: timeout"), true);
  assert.equal(isRecoveredChatHydrationError("[chat] history hydration retry failed: timeout"), true);
  assert.equal(isRecoveredChatHydrationError("Timeout: RCH TEAM peer directory refresh failed"), false);
  assert.equal(isRecoveredChatHydrationError("transport startup failed"), false);
});

test("App initializes projections before recoverable chat initialization", async () => {
  const appSource = await readFile(new URL("../App.vue", import.meta.url), "utf8");
  assert.equal(appSource.includes("messagingStore.hydrateStartupHistory"), false);
  assert.equal((appSource.match(/messagingStore\.init\(\)/g) ?? []).length, 2);
  assert.match(appSource, /chatHistoryHydrated/);
  assert.ok(appSource.indexOf("eventsStore.init();") < appSource.indexOf("messagingStore.init()"));
  assert.ok(appSource.indexOf("eventsStore.initReplication();") < appSource.indexOf("messagingStore.init()"));
});
