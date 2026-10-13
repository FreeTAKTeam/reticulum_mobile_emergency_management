import type {
  MapMarkerCreateRequest,
  MapMarkerRecord,
  MapMarkerSymbolRecord,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";

import { readJson } from "../utils/legacyStorage";
import { pushRecentSymbol, recentSymbols } from "../utils/mapMarkerPresentation";
import { useNodeStore } from "./nodeStore";

const RECENT_SYMBOLS_STORAGE_KEY = "reticulum.mobile.mapMarkerRecentSymbols.v1";
const MARKERS_VISIBLE_STORAGE_KEY = "reticulum.mobile.mapMarkersVisible.v1";

function writePreference(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Preferences are a convenience; markers themselves live in the native store.
  }
}

/**
 * Projection cache for shared map markers plus marker UI preferences.
 * Creation, validation and persistence are owned by the native runtime.
 */
export const useMapMarkersStore = defineStore("mapMarkers", () => {
  const nodeStore = useNodeStore();
  const symbols = shallowRef<MapMarkerSymbolRecord[]>([]);
  const markers = shallowRef<MapMarkerRecord[]>([]);
  const loading = ref(false);
  const busy = ref(false);
  const lastError = ref("");
  const recentSymbolIds = ref<string[]>(readJson<string[]>(RECENT_SYMBOLS_STORAGE_KEY) ?? []);
  const visible = ref(readJson<boolean>(MARKERS_VISIBLE_STORAGE_KEY) ?? true);
  let unsubscribe: (() => void) | null = null;
  let initPromise: Promise<void> | null = null;

  const symbolsById = computed(() => new Map(symbols.value.map((symbol) => [symbol.id, symbol])));
  const recent = computed(() => recentSymbols(recentSymbolIds.value, symbols.value));

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  async function refresh(): Promise<void> {
    loading.value = true;
    try {
      const client = await nodeStore.mapMarkerClient();
      const [nextSymbols, nextMarkers] = await Promise.all([
        symbols.value.length > 0 ? Promise.resolve(symbols.value) : client.listMapMarkerSymbols(),
        client.listMapMarkers(),
      ]);
      symbols.value = nextSymbols;
      markers.value = nextMarkers;
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = errorMessage(error);
    } finally {
      loading.value = false;
    }
  }

  async function init(): Promise<void> {
    if (!initPromise) {
      unsubscribe?.();
      unsubscribe = nodeStore.onClientEvent("projectionInvalidated", (event) => {
        if (event.scope === "MapMarkers") {
          void refresh();
        }
      });
      initPromise = refresh();
    }
    return initPromise;
  }

  async function run<T>(operation: () => Promise<T>): Promise<T> {
    busy.value = true;
    try {
      const result = await operation();
      lastError.value = "";
      return result;
    } catch (error: unknown) {
      lastError.value = errorMessage(error);
      throw error;
    } finally {
      busy.value = false;
    }
  }

  async function create(request: MapMarkerCreateRequest): Promise<MapMarkerRecord> {
    const result = await run(async () => (await nodeStore.mapMarkerClient()).createMapMarker(request));
    recentSymbolIds.value = pushRecentSymbol(recentSymbolIds.value, result.marker.symbol);
    writePreference(RECENT_SYMBOLS_STORAGE_KEY, recentSymbolIds.value);
    await refresh();
    return result.marker;
  }

  async function rename(key: string, name: string): Promise<MapMarkerRecord> {
    const marker = await run(async () => (await nodeStore.mapMarkerClient()).renameMapMarker(key, name));
    await refresh();
    return marker;
  }

  async function move(key: string, lat: number, lon: number): Promise<MapMarkerRecord> {
    const marker = await run(async () => (await nodeStore.mapMarkerClient()).moveMapMarker(key, lat, lon));
    await refresh();
    return marker;
  }

  async function remove(key: string): Promise<boolean> {
    const deleted = await run(async () => (await nodeStore.mapMarkerClient()).deleteMapMarker(key));
    await refresh();
    return deleted;
  }

  function setVisible(next: boolean): void {
    visible.value = next;
    writePreference(MARKERS_VISIBLE_STORAGE_KEY, next);
  }

  function dispose(): void {
    unsubscribe?.();
    unsubscribe = null;
    initPromise = null;
  }

  return {
    symbols,
    markers,
    loading,
    busy,
    lastError,
    visible,
    symbolsById,
    recent,
    init,
    refresh,
    create,
    rename,
    move,
    remove,
    setVisible,
    dispose,
  };
});
