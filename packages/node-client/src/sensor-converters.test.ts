import { describe, expect, it } from "vitest";
import { toSensorOrigins } from "./sensor-converters";
import { toPluginCapabilities } from "./runtime-converters";

describe("sensor projection boundary", () => {
  it("preserves independent identity, unknown altitude and native freshness", () => {
    const originId = "ab".repeat(16);
    const writerId = "cd".repeat(16);
    const snapshot = { observedAtMs: 2000, ttlMs: 1000, items: [{ originId, writerId, label: "Drone B", originKind: "uav", position: { lat: 45, lon: -63, altM: null }, sensors: [{ state: { sensorId: "eo", sampleAtMs: 1000 }, display: { stale: true, geojson: { type: "FeatureCollection", features: [] } } }] }] };
    expect(toSensorOrigins(snapshot)).toEqual(snapshot);
    expect(() => toSensorOrigins({ items: [{}], observedAtMs: 1, ttlMs: 1000 })).toThrow("sensor origin");
  });
  it("does not grant new capabilities to legacy plugins", () => {
    const capabilities = toPluginCapabilities({ sensorsPublish: true });
    expect(capabilities.sensorsPublish).toBe(true);
    expect(capabilities.sensorGeometryPublish).toBe(false);
    expect(capabilities.originTelemetryPublish).toBe(false);
    expect(capabilities.sensorGeometryShare).toBe(false);
    expect(capabilities.sensorVideoShare).toBe(false);
  });
});
