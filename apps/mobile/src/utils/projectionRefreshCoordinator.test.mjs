import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./projectionRefreshCoordinator.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { ProjectionRefreshCoordinator } = await import(moduleUrl);

function deferred() {
  let resolve;
  const promise = new Promise((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

test("coalesces concurrent refreshes for the same key", async () => {
  const coordinator = new ProjectionRefreshCoordinator();
  const gate = deferred();
  let calls = 0;
  const operation = async () => {
    calls += 1;
    await gate.promise;
  };

  const first = coordinator.run("events", operation);
  const second = coordinator.run("events", operation);

  assert.strictEqual(second, first);
  assert.equal(calls, 1);
  gate.resolve();
  await Promise.all([first, second]);
  assert.equal(calls, 1);
});

test("runs one trailing refresh with the latest requested operation", async () => {
  const coordinator = new ProjectionRefreshCoordinator();
  const gate = deferred();
  const calls = [];

  const first = coordinator.run("messages", async () => {
    calls.push("initial");
    await gate.promise;
  }, { trailing: true });
  const second = coordinator.run("messages", async () => {
    calls.push("superseded");
  }, { trailing: true });
  const third = coordinator.run("messages", async () => {
    calls.push("latest");
  }, { trailing: true });

  assert.strictEqual(second, first);
  assert.strictEqual(third, first);
  gate.resolve();
  await Promise.all([first, second, third]);
  assert.deepEqual(calls, ["initial", "latest"]);
});

test("keeps different projection keys independent", async () => {
  const coordinator = new ProjectionRefreshCoordinator();
  const firstGate = deferred();
  const calls = [];

  const first = coordinator.run("events", async () => {
    calls.push("events");
    await firstGate.promise;
  });
  const second = coordinator.run("telemetry", async () => {
    calls.push("telemetry");
  });

  await second;
  assert.deepEqual(calls, ["events", "telemetry"]);
  firstGate.resolve();
  await first;
});

test("clears failed refreshes so the key can be retried", async () => {
  const coordinator = new ProjectionRefreshCoordinator();

  await assert.rejects(
    coordinator.run("settings", async () => {
      throw new Error("failed");
    }),
    /failed/,
  );

  let retried = false;
  await coordinator.run("settings", async () => {
    retried = true;
  });
  assert.equal(retried, true);
});
