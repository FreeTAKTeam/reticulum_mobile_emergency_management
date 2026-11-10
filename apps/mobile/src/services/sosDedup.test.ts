import { createPinia, setActivePinia } from "pinia";
import { expect, test, vi } from "vitest";
import { DEFAULT_SOS_SETTINGS, DEFAULT_SOS_STATUS } from "@reticulum/node-client";

const listeners = vi.hoisted(() => new Map<string, (event: unknown) => void>());
vi.mock("../stores/nodeStore", () => ({ useNodeStore: () => ({
  initialized: true,
  onClientEvent: (name: string, callback: (event: unknown) => void) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  },
  getSosSettings: async () => ({ ...DEFAULT_SOS_SETTINGS, countdownSeconds: 0,
    triggerTapPattern: false, audioRecording: false, periodicUpdates: false, silentAutoAnswer: false }),
  getSosStatus: async () => DEFAULT_SOS_STATUS,
  listSosAlerts: async () => [],
  listSosLocations: async () => [],
  listSosAudio: async () => [],
}) }));
import { useSosStore } from "../stores/sosStore";

test("case-variant SOS updates replace the existing alert", async () => {
  setActivePinia(createPinia());
  const store = useSosStore();
  await store.init();
  const emit = listeners.get("sosAlertChanged")!;
  emit({ alert: { incidentId: "ABC", sourceHex: "DEF", state: "Active" } });
  emit({ alert: { incidentId: " abc ", sourceHex: "def", state: "Resolved" } });
  expect(store.alerts).toHaveLength(1);
  expect(store.alerts[0].state).toBe("Resolved");
});
