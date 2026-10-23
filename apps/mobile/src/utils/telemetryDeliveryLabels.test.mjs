import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import ts from "typescript";
const source = readFileSync(new URL("./telemetryDeliveryLabels.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } });
const { telemetryDeliveryLabel } = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
test("telemetry delivery labels distinguish receipts, accepted results and failures", () => {
  assert.match(telemetryDeliveryLabel("TransportDelivered"), /Awaiting application/);
  assert.equal(telemetryDeliveryLabel("Acknowledged"), "Receiver accepted");
  assert.equal(telemetryDeliveryLabel("QueuedNoRoute"), "Waiting for route");
  assert.equal(telemetryDeliveryLabel("Rejected"), "Receiver rejected");
  assert.equal(telemetryDeliveryLabel("TimedOut"), "Acknowledgement timed out");
});
