import type { InstalledPluginRecord, PluginMapSessionRecord } from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";

import { useNodeStore } from "./nodeStore";

const MAP_SESSION_API_MINOR = 3;

/**
 * Projection of operator-started plugin map sessions. The native runtime owns
 * which plugin may read which object and ends sessions on revocation; this
 * store only mirrors them and forwards operator start and stop requests.
 */
export const usePluginMapSessionsStore = defineStore("pluginMapSessions", () => {
  const nodeStore = useNodeStore();
  const sessions = shallowRef<PluginMapSessionRecord[]>([]);
  const busy = ref(false);
  const lastError = ref("");
  let unsubscribe: (() => void) | null = null;
  let initPromise: Promise<void> | null = null;

  /** Running plugins that can take a session; native re-checks every start. */
  const eligiblePlugins = computed<InstalledPluginRecord[]>(() =>
    nodeStore.plugins.filter((plugin) =>
      plugin.trusted
      && plugin.enabled
      && plugin.state === "Running"
      && plugin.apiMajor === 1
      && plugin.apiMinor >= MAP_SESSION_API_MINOR
      && plugin.declaredCapabilities.mapSelectionRead
      && plugin.grantedCapabilities.mapSelectionRead),
  );

  const pluginNames = computed(
    () => new Map(nodeStore.plugins.map((plugin) => [plugin.pluginId, plugin.displayName])),
  );

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  async function refresh(): Promise<void> {
    try {
      sessions.value = await (await nodeStore.pluginMapSessionClient()).listPluginMapSessions();
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = errorMessage(error);
    }
  }

  async function init(): Promise<void> {
    if (!initPromise) {
      unsubscribe?.();
      // Session changes bump the Plugins projection; deleted objects bump MapMarkers.
      unsubscribe = nodeStore.onClientEvent("projectionInvalidated", (event) => {
        if (event.scope === "Plugins" || event.scope === "MapMarkers") {
          void refresh();
        }
      });
      initPromise = refresh();
    }
    return initPromise;
  }

  async function run(operation: () => Promise<unknown>): Promise<void> {
    busy.value = true;
    try {
      await operation();
      lastError.value = "";
    } catch (error: unknown) {
      lastError.value = errorMessage(error);
    } finally {
      busy.value = false;
      await refresh();
    }
  }

  function start(pluginId: string, objectDestinationHashes: string[]): Promise<void> {
    return run(async () =>
      (await nodeStore.pluginMapSessionClient()).startPluginMapSession(pluginId, objectDestinationHashes));
  }

  function stop(sessionId: string): Promise<void> {
    return run(async () => (await nodeStore.pluginMapSessionClient()).stopPluginMapSession(sessionId));
  }

  function sessionsFor(objectDestinationHash: string): PluginMapSessionRecord[] {
    return sessions.value.filter((session) =>
      session.objectDestinationHashes.includes(objectDestinationHash));
  }

  function dispose(): void {
    unsubscribe?.();
    unsubscribe = null;
    initPromise = null;
  }

  return {
    sessions,
    busy,
    lastError,
    eligiblePlugins,
    pluginNames,
    init,
    refresh,
    start,
    stop,
    sessionsFor,
    dispose,
  };
});
