import type { SensorOriginsSnapshot } from "./contracts-sensors";
import { pluginRecord } from "./runtime-converters";

/** Decode the native projection boundary; never calculate domain geometry in the client. */
export function toSensorOrigins(value: unknown): SensorOriginsSnapshot {
  const raw = pluginRecord(value);
  if (!Array.isArray(raw.items) || !Number.isFinite(raw.observedAtMs) || !Number.isFinite(raw.ttlMs)) {
    throw new Error("Invalid native sensor snapshot.");
  }
  for (const item of raw.items) {
    const origin = pluginRecord(item);
    if (typeof origin.originId !== "string" || typeof origin.writerId !== "string" || typeof origin.label !== "string" || !Array.isArray(origin.sensors)) {
      throw new Error("Invalid native sensor origin.");
    }
    for (const item of origin.sensors) {
      const sensor = pluginRecord(item);
      const state = pluginRecord(sensor.state);
      const display = pluginRecord(sensor.display);
      const geometry = pluginRecord(display.geojson);
      if (typeof state.sensorId !== "string" || !Number.isFinite(state.sampleAtMs) || geometry.type !== "FeatureCollection" || !Array.isArray(geometry.features)) {
        throw new Error("Invalid native sensor projection.");
      }
    }
  }
  return raw as unknown as SensorOriginsSnapshot;
}
