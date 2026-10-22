import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { createPinia, disposePinia } from "pinia";
import ts from "typescript";

function dataModule(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`;
}

const nodeClientMock = dataModule(`
  export const DEFAULT_SOS_SETTINGS = {
    countdownSeconds: 0,
    triggerTapPattern: false,
    shakeSensitivity: 2.5,
    audioRecording: false,
    audioDurationSeconds: 30,
    periodicUpdates: false,
    updateIntervalSeconds: 120,
    silentAutoAnswer: false,
  };
  export const DEFAULT_SOS_STATUS = { state: "Idle", updatedAtMs: 0 };
`);
const nodeStoreMock = dataModule(`
  let store;
  export function setNodeStore(next) { store = next; }
  export function useNodeStore() { return store; }
`);
const { DEFAULT_SOS_SETTINGS, DEFAULT_SOS_STATUS } = await import(nodeClientMock);
const { setNodeStore } = await import(nodeStoreMock);
const source = await readFile(new URL("../stores/sosStore.ts", import.meta.url), "utf8");
let transpiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
for (const [specifier, replacement] of [
  ["@reticulum/node-client", nodeClientMock],
  ["./nodeStore", nodeStoreMock],
  ["pinia", import.meta.resolve("pinia")],
  ["vue", import.meta.resolve("vue")],
]) {
  transpiled = transpiled.replaceAll(`"${specifier}"`, `"${replacement}"`);
}
const { useSosStore } = await import(dataModule(transpiled));

function alertRecord(overrides = {}) {
  return {
    incidentId: "SOS-ABC-1",
    sourceHex: "ab".repeat(16),
    conversationId: "ab".repeat(16),
    state: "Active",
    active: true,
    bodyUtf8: "SOS! I need help...",
    receivedAtMs: 1000,
    updatedAtMs: 1000,
    ...overrides,
  };
}

for (const [name, identity] of [
  ["identical IDs", {}],
  ["incident case", { incidentId: "sos-abc-1" }],
  ["source case", { sourceHex: "AB".repeat(16) }],
  ["incident whitespace", { incidentId: " SOS-ABC-1 " }],
  ["source whitespace", { sourceHex: ` ${"ab".repeat(16)} ` }],
  ["case and whitespace", { incidentId: " sos-abc-1 ", sourceHex: ` ${"AB".repeat(16)} ` }],
]) {
  test(`SOS cancellation replaces the alert with ${name} while refresh is pending`, async (t) => {
    const original = alertRecord();
    const otherIncident = alertRecord({ incidentId: "SOS-ABC-2" });
    const otherSource = alertRecord({ sourceHex: "cd".repeat(16), conversationId: "cd".repeat(16) });
    const initialAlerts = [original, otherIncident, otherSource];
    const locations = initialAlerts.map(({ incidentId, sourceHex }, index) => ({
      incidentId,
      sourceHex,
      lat: 44.6 + index / 100,
      lon: -63.5,
      recordedAtMs: 1000,
    }));
    const handlers = new Map();
    let listCalls = 0;
    // Keep event-triggered refreshes pending so they cannot hide a stale active alert.
    const refreshPending = new Promise(() => {});
    setNodeStore({
      initialized: true,
      onClientEvent(name, handler) {
        handlers.set(name, handler);
        return () => handlers.delete(name);
      },
      async getSosSettings() { return { ...DEFAULT_SOS_SETTINGS }; },
      async getSosStatus() { return { ...DEFAULT_SOS_STATUS }; },
      async listSosAlerts() {
        listCalls += 1;
        return listCalls === 1 ? initialAlerts : refreshPending;
      },
      async listSosLocations() { return locations; },
      async listSosAudio() { return []; },
    });
    const pinia = createPinia();
    t.after(() => disposePinia(pinia));
    const store = useSosStore(pinia);
    await store.init();
    assert.deepEqual(store.activeAlerts, initialAlerts);
    assert.deepEqual(store.activeLocations, locations);

    const cancelled = alertRecord({
      ...identity,
      state: "Cancelled",
      active: false,
      bodyUtf8: "SOS Cancelled - I am safe.",
      updatedAtMs: 2000,
    });
    const onAlertChanged = handlers.get("sosAlertChanged");
    assert.equal(typeof onAlertChanged, "function");
    onAlertChanged({ alert: cancelled });

    assert.equal(listCalls, 2);
    assert.equal(store.alerts.length, 3);
    assert.deepEqual(store.alerts, [cancelled, otherIncident, otherSource]);
    assert.deepEqual(store.activeAlerts, [otherIncident, otherSource]);
    assert.deepEqual(store.activeLocations, locations.slice(1));

    // Repeated delivery must not duplicate the cancellation or disturb unrelated alerts.
    onAlertChanged({ alert: cancelled });
    assert.equal(listCalls, 3);
    assert.deepEqual(store.alerts, [cancelled, otherIncident, otherSource]);
    assert.deepEqual(store.activeAlerts, [otherIncident, otherSource]);
  });
}
