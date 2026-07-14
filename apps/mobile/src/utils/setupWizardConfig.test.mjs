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
const {
  SETUP_STEPS,
  normalizeWizardTcpEndpoint,
  normalizeWizardTelemetryPublishIntervalSeconds,
} = await import(moduleUrl);

test("setup wizard steps retain stable unique identifiers", () => {
  assert.deepEqual(
    SETUP_STEPS.map((step) => step.id),
    ["welcome", "callsign", "permissions", "tcp", "rnode", "telemetry", "sos", "review"],
  );
});

test("setup wizard telemetry interval uses a positive default", () => {
  assert.equal(normalizeWizardTelemetryPublishIntervalSeconds(undefined), 360);
  assert.equal(normalizeWizardTelemetryPublishIntervalSeconds(0), 1);
  assert.equal(normalizeWizardTelemetryPublishIntervalSeconds("42.9"), 42);
});

test("setup wizard validates IPv4, hostname, and bracketed IPv6 endpoints", () => {
  assert.equal(normalizeWizardTcpEndpoint(" mesh.example:4242 "), "mesh.example:4242");
  assert.equal(normalizeWizardTcpEndpoint("[2001:db8::1]:4242"), "[2001:db8::1]:4242");
  assert.equal(normalizeWizardTcpEndpoint("missing-port"), undefined);
  assert.equal(normalizeWizardTcpEndpoint("mesh.example:70000"), undefined);
});
