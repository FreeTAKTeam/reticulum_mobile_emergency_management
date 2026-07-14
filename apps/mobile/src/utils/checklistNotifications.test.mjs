import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const notificationMock = `
  export const calls = [];
  export function primeOperationalNotificationScope(scope, keys) {
    calls.push({ type: "prime", scope, keys: [...keys] });
  }
  export async function notifyOperationalUpdateOnce(scope, key, title, body, extra) {
    calls.push({ type: "notify", scope, key, title, body, extra });
    return true;
  }
  export function truncateNotificationBody(value) { return value.trim(); }
`;
const mockUrl = `data:text/javascript;base64,${Buffer.from(notificationMock).toString("base64")}`;
const source = await readFile(new URL("./checklistNotifications.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText.replaceAll('"../services/operationalNotifications"', `"${mockUrl}"`);
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { createChecklistNotificationCoordinator } = await import(moduleUrl);
const { calls } = await import(mockUrl);

function checklist(overrides = {}) {
  return {
    uid: "checklist-1",
    name: "Field Ops",
    updatedAt: "2026-07-14T20:00:00Z",
    counts: { pendingCount: 2, completeCount: 1, lateCount: 1 },
    tasks: [{}, {}, {}],
    ...overrides,
  };
}

test("checklist notifications prime existing records and debounce remote updates", async () => {
  let scheduled;
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  globalThis.setTimeout = (callback) => {
    scheduled = callback;
    return 1;
  };
  globalThis.clearTimeout = () => undefined;
  try {
    const coordinator = createChecklistNotificationCoordinator(() => "local-id");
    await coordinator.notifyForChanges([checklist()]);
    await coordinator.notifyForChanges([
      checklist({
        updatedAt: "2026-07-14T20:01:00Z",
        lastChangedByTeamMemberRnsIdentity: "remote-id",
      }),
    ]);
    scheduled();
    await Promise.resolve();

    assert.equal(calls[0].type, "prime");
    assert.deepEqual(calls[0].keys, ["checklist-1:2026-07-14T20:00:00Z"]);
    assert.deepEqual(calls[1], {
      type: "notify",
      scope: "checklist",
      key: "checklist-1:2026-07-14T20:01:00Z",
      title: "Checklist updated: Field Ops",
      body: "2 pending, 1 complete, 1 late across 3 tasks",
      extra: { route: "/checklists/checklist-1" },
    });
  } finally {
    globalThis.setTimeout = originalSetTimeout;
    globalThis.clearTimeout = originalClearTimeout;
  }
});
