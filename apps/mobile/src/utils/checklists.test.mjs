import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./checklists.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  runtimeChecklistTaskToUi,
} = await import(moduleUrl);

function runtimeTask(overrides = {}) {
  return {
    taskUid: "task-1",
    number: 1,
    userStatus: "PENDING",
    taskStatus: "PENDING",
    isLate: false,
    updatedAt: undefined,
    deletedAt: undefined,
    customStatus: undefined,
    dueRelativeMinutes: 10,
    dueDtg: undefined,
    notes: undefined,
    rowBackgroundColor: undefined,
    lineBreakEnabled: false,
    completedAt: undefined,
    completedByTeamMemberRnsIdentity: undefined,
    legacyValue: "Check comms",
    cells: [],
    ...overrides,
  };
}

test("submitted task override renders before runtime completion refresh", () => {
  const task = runtimeChecklistTaskToUi(runtimeTask(), [], {
    submittedTaskIds: new Set(["task-1"]),
  });

  assert.equal(task.status, "submitted");
  assert.equal(task.metaLabel, "Submitted");
  assert.equal(task.metaTone, "submitted");
});
