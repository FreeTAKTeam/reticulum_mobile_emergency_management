import type { SensorOriginProjection } from "@reticulum/node-client";
import { defineStore } from "pinia";
import { ref, shallowRef } from "vue";
import { readJson } from "../utils/legacyStorage";
import { useNodeStore } from "./nodeStore";

const PREFERENCE_KEY = "reticulum.mobile.sensorVisibility.v1";
/** Runtime projections and local display choices only. */
export const useSensorGeometryStore = defineStore("sensorGeometry", () => {
  const node = useNodeStore();
  const origins = shallowRef<SensorOriginProjection[]>([]);
  const observedAtMs = ref(0);
  const error = ref("");
  const hidden = ref<string[]>(readJson<string[]>(PREFERENCE_KEY) ?? []);
  let loading = false;
  async function refresh(): Promise<void> {
    if (loading) return;
    loading = true;
    try {
      const snapshot = await (await node.sensorGeometryClient()).getSensorOrigins();
      origins.value = snapshot.items;
      observedAtMs.value = snapshot.observedAtMs;
      error.value = "";
    } catch (cause) {
      origins.value = [];
      error.value = cause instanceof Error ? cause.message : String(cause);
    }
    finally { loading = false; }
  }
  function visible(key: string): boolean { return !hidden.value.includes(key); }
  function toggle(key: string): void {
    hidden.value = visible(key) ? [...hidden.value, key] : hidden.value.filter((item) => item !== key);
    try { localStorage.setItem(PREFERENCE_KEY, JSON.stringify(hidden.value)); } catch { /* Display preferences are optional. */ }
  }
  async function approveVideo(originId: string, sensorId: string, approved: boolean): Promise<void> {
    try {
      await (await node.sensorGeometryClient()).setSensorVideoSharing(originId, sensorId, approved);
      await refresh();
    } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause); }
  }
  return { origins, observedAtMs, error, refresh, visible, toggle, approveVideo };
});
