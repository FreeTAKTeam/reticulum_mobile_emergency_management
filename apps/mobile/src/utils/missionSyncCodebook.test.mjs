import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

async function transpiledUrl(relativeUrl, replacements = new Map()) {
  const source = await readFile(new URL(relativeUrl, import.meta.url), "utf8");
  let output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  for (const [specifier, replacement] of replacements) {
    output = output.replaceAll(`"${specifier}"`, `"${replacement}"`);
  }
  return `data:text/javascript;base64,${Buffer.from(output).toString("base64")}`;
}

const recordsUrl = await transpiledUrl("./records.ts");
const codebookUrl = await transpiledUrl(
  "./missionSyncCodebook.ts",
  new Map([["./records", recordsUrl]]),
);
const {
  canonicalCommandType,
  commandWireValue,
  compactMissionCommandArgs,
  expandChecklistCommandArgs,
} = await import(codebookUrl);

test("mission command codes remain wire-compatible", () => {
  assert.equal(commandWireValue("mission.registry.eam.upsert"), "M1");
  assert.equal(canonicalCommandType("M1"), "mission.registry.eam.upsert");
  assert.equal(commandWireValue("custom.command"), "custom.command");
});

test("checklist arguments compact and expand recursively", () => {
  const expanded = {
    checklist_uid: "checklist-1",
    task_uid: "task-1",
    patch: { name: "Updated", due_relative_minutes: 15 },
  };
  const compact = compactMissionCommandArgs("checklist.update", expanded);

  assert.deepEqual(compact, {
    cl: "checklist-1",
    tsk: "task-1",
    pa: { n: "Updated", dr: 15 },
  });
  assert.deepEqual(expandChecklistCommandArgs(compact), expanded);
});

test("non-checklist command arguments retain their public shape", () => {
  const args = { mission_uid: "mission-1", content: "Report" };
  assert.equal(compactMissionCommandArgs("mission.registry.log_entry.upsert", args), args);
});
