import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./startupInterfaces.ts", import.meta.url), "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const {
  buildStartupInterfaceItems,
  statusHasRuntimeStartupReadiness,
  statusNeedsStartupInterfaceGrace,
} = await import(moduleUrl);

function status(state, interfaces, running = true) {
  return {
    running,
    readiness: { state, interfaces },
  };
}

function record(id, state, detail, lastError) {
  return {
    id,
    label: id === "rnode" ? "LoRa" : id === "tcp" ? "TCP community" : "Reticulum Net",
    state,
    detail,
    lastError,
  };
}

function itemById(items, id) {
  const item = items.find((entry) => entry.id === id);
  assert.ok(item, `missing startup interface item ${id}`);
  return item;
}

test("renders typed runtime interface readiness without inferring from traffic", () => {
  const items = buildStartupInterfaceItems(
    status("Pending", [
      record("rnode", "Pending", "Waiting for RNode"),
      record("tcp", "Disabled", "No TCP interface configured"),
      record("local", "Ready", "Runtime is ready"),
    ]),
    {},
  );

  assert.equal(itemById(items, "rnode").state, "loading");
  assert.equal(itemById(items, "tcp").state, "disabled");
  assert.equal(itemById(items, "local").state, "ready");
});

test("surfaces failed and unsupported interface details", () => {
  const items = buildStartupInterfaceItems(
    status("Failed", [
      record("rnode", "Unsupported", "Classic is unsupported"),
      record("tcp", "Failed", "Interface failed", "connection refused"),
    ]),
    {},
  );

  assert.equal(itemById(items, "rnode").state, "unsupported");
  assert.equal(itemById(items, "tcp").state, "failed");
  assert.equal(itemById(items, "tcp").detail, "connection refused");
});

test("runtime readiness uses the typed aggregate state", () => {
  assert.equal(statusHasRuntimeStartupReadiness(status("Pending", [], true)), false);
  assert.equal(statusHasRuntimeStartupReadiness(status("Failed", [], true)), false);
  assert.equal(statusHasRuntimeStartupReadiness(status("Ready", [], false)), false);
  assert.equal(statusHasRuntimeStartupReadiness(status("Ready", [], true)), true);
});

test("failed network interface does not block a running ready runtime", () => {
  const runtimeStatus = status("Ready", [
    record("tcp", "Failed", "Configured endpoint unavailable", "connection refused"),
    record("local", "Ready", "Runtime is ready"),
  ]);

  assert.equal(statusHasRuntimeStartupReadiness(runtimeStatus), true);
  assert.equal(itemById(buildStartupInterfaceItems(runtimeStatus), "tcp").state, "failed");
});

test("stopped runtime never reopens the interface loading screen", () => {
  const stoppedStatus = status("Pending", [
    record("tcp", "Pending", "Waiting for configured interface"),
    record("local", "Pending", "Runtime is stopped"),
  ], false);
  const items = buildStartupInterfaceItems(stoppedStatus);

  assert.equal(statusNeedsStartupInterfaceGrace(stoppedStatus, items), false);
});

test("running pending runtime keeps configured interfaces in startup grace", () => {
  const startingStatus = status("Pending", [
    record("tcp", "Pending", "Waiting for configured interface"),
    record("local", "Ready", "Runtime is ready"),
  ]);
  const items = buildStartupInterfaceItems(startingStatus);

  assert.equal(statusNeedsStartupInterfaceGrace(startingStatus, items), true);
});

test("corrupt native readiness records are ignored instead of breaking startup", () => {
  const items = buildStartupInterfaceItems(status("Ready", [
    null,
    {},
    { id: "tcp", label: 7, state: "Ready", detail: "invalid" },
    record("local", "Ready", "Runtime is ready"),
  ]));

  assert.deepEqual(items, [{
    id: "local",
    label: "Reticulum Net",
    detail: "Runtime is ready",
    state: "ready",
  }]);
});
