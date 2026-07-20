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

test("synchronous detached task failures are contained", async () => {
  const errors = [];
  runDetachedStoreTask(
    {
      setLastError: (message) => errors.push(message),
      logUi: () => undefined,
    },
    "navigation",
    "route update",
    () => {
      throw new Error("router unavailable");
    },
  );
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(errors, ["[navigation] route update failed: router unavailable"]);
});

test("known fallible detached call sites use the shared containment helper", async () => {
  const sourceUrls = [
    new URL("../components/sos/SosOverlay.vue", import.meta.url),
    new URL("../composables/useChecklistDetail.ts", import.meta.url),
    new URL("../composables/useChecklistList.ts", import.meta.url),
    new URL("../views/DashboardView.vue", import.meta.url),
    new URL("../stores/nodeAnnounceController.ts", import.meta.url),
    new URL("../views/InboxView.vue", import.meta.url),
    new URL("../views/ActionMessagesView.vue", import.meta.url),
  ];
  const sources = await Promise.all(sourceUrls.map((url) => readFile(url, "utf8")));
  for (const callSite of sources) {
    assert.match(callSite, /runDetachedStoreTask/);
  }
  assert.doesNotMatch(sources[0], /void persistPosition/);
  assert.doesNotMatch(sources[1], /void checklistsStore\.refreshDetail/);
  assert.doesNotMatch(sources[2], /void ensureChecklistData/);
  assert.doesNotMatch(sources[3], /void checklistsStore\.refreshLive/);
  assert.doesNotMatch(sources[4], /void persistSavedPeersProjection/);
  assert.doesNotMatch(sources[5], /void messagingStore/);
  assert.doesNotMatch(sources[6], /deleteLocal\(callsign\)\.catch/);
});
