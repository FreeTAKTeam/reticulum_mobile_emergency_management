import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(new URL("./telemetryMapModel.ts", import.meta.url), "utf8");
const output = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(output).toString("base64")}`;
const {
  buildSosPopupHtml,
  labelPlacementsFor,
  parseTelemetryRouteTarget,
  telemetryRenderGroups,
} = await import(moduleUrl);

test("telemetry route targets require finite coordinates", () => {
  assert.deepEqual(
    parseTelemetryRouteTarget({ callsign: "Alpha", lat: "44.6", lon: "-63.6" }),
    { callsign: "Alpha", lat: 44.6, lon: -63.6 },
  );
  assert.equal(parseTelemetryRouteTarget({ lat: "invalid", lon: "-63.6" }), null);
});

test("SOS popup content escapes untrusted payloads and hides GPS metadata", () => {
  const html = buildSosPopupHtml(
    {
      incidentId: "incident-1",
      sourceHex: "<source>",
      lat: 44.6,
      lon: -63.6,
      recordedAtMs: 1_700_000_000_000,
    },
    [{
      incidentId: "incident-1",
      sourceHex: "<source>",
      bodyUtf8: "Need <help>\nGPS:44.6,-63.6",
    }],
    [],
  );

  assert.match(html, /Need &lt;help&gt;/);
  assert.match(html, /Source &lt;source&gt;/);
  assert.doesNotMatch(html, /GPS:/);
});

test("1,000 telemetry positions cluster and label within the local latency budget", () => {
  const positions = Array.from({ length: 1_000 }, (_, index) => ({
    callsign: `unit-${index}`,
    lat: 44.6 + (index % 20) * 0.0001,
    lon: -63.6 + Math.floor(index / 20) * 0.0001,
    updatedAt: Date.now(),
  }));
  const map = {
    getZoom: () => 8,
    project: ([lon, lat]) => ({ x: lon * 100, y: lat * 100 }),
  };

  const startedAt = performance.now();
  const groups = telemetryRenderGroups(positions, map, 60_000);
  const placements = labelPlacementsFor(groups.individuals);
  const elapsedMs = performance.now() - startedAt;

  assert.equal(groups.clusters.length, 1);
  assert.equal(groups.clusters[0].count, 1_000);
  assert.equal(placements.size, 0);
  assert.ok(elapsedMs < 50, `clustering created a ${elapsedMs.toFixed(1)}ms long task`);
});

test("1,000 sparse positions avoid quadratic cluster scans", () => {
  const positions = Array.from({ length: 1_000 }, (_, index) => ({
    callsign: `sparse-${index}`,
    lat: index,
    lon: index,
    updatedAt: Date.now(),
  }));
  const map = {
    getZoom: () => 8,
    project: ([lon, lat]) => ({ x: lon * 100, y: lat * 100 }),
  };

  const startedAt = performance.now();
  const groups = telemetryRenderGroups(positions, map, 60_000);
  const elapsedMs = performance.now() - startedAt;

  assert.equal(groups.clusters.length, 0);
  assert.equal(groups.individuals.length, 1_000);
  assert.ok(elapsedMs < 50, `sparse clustering created a ${elapsedMs.toFixed(1)}ms long task`);
});
