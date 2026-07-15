import {
  type InstalledPluginRecord,
  type OperationalSummary,
  type PluginCapabilityRecord,
  type PluginSensorRecord,
  type ReticulumNodeClient,
  type SavedPeerRecord,
  type WatchStatusServerSettings,
  type WatchStatusServerState,
} from "@reticulum/node-client";
import type { Ref, ShallowRef } from "vue";

import type {
  DiscoveredPeer,
  NodeUiSettings,
  SavedPeer,
} from "../types/domain";
import {
  buildLegacyProjectionState,
  clearLegacyProjectionStorage,
  loadUiSettingsProjection,
  persistUiSettingsProjection as storeUiSettingsProjection,
  persistWebLegacySavedPeers,
  persistWebLegacySettings,
  type NodeUiPreferences,
} from "../utils/legacyState";
import { projectionRefreshCoordinator } from "../utils/projectionRefreshCoordinator";
import { normalizeDestinationHex } from "../utils/peers";
import { normalizeRnodeSettings } from "../utils/rnodeProfiles";
import { runtimeProfile } from "../utils/runtimeProfile";
import {
  fromSavedPeerRecords,
  DEFAULT_SETTINGS,
  normalizeAppSettingsRecord,
  normalizeClientMode,
  settingsRecordsEqual,
  settingsRecordWasNormalized,
  toAppSettingsRecord,
  toSavedPeerRecords,
  toUiSettingsProjection,
} from "./nodeSettingsModel";
import {
  OPERATIONAL_SUMMARY_REFRESH_MIN_INTERVAL_MS,
  PROJECTION_REFRESH_DEBOUNCE_MS,
  DEFAULT_WATCH_STATUS_SERVER,
  EMPTY_OPERATIONAL_SUMMARY,
  nowMs,
} from "./nodeStoreCore";

interface NodeProjectionContext {
  appendLog: (level: string, message: string) => void;
  client: ShallowRef<ReticulumNodeClient | null>;
  defaultsWithTcpFallback: () => string[];
  discoveredByDestination: Record<string, DiscoveredPeer>;
  errorMessage: (error: unknown) => string;
  init: () => Promise<void>;
  lastError: Ref<string>;
  operationalSummary: Ref<OperationalSummary>;
  plugins: Ref<InstalledPluginRecord[]>;
  pluginSensors: Ref<PluginSensorRecord[]>;
  savedByDestination: Record<string, SavedPeer>;
  settings: NodeUiSettings;
  status: Ref<import("@reticulum/node-client").NodeStatus>;
  upsertDiscovered: (
    destination: string,
    patch: Partial<DiscoveredPeer>,
    source?: "announce" | "hub" | "import",
  ) => void;
  watchStatusServer: WatchStatusServerState;
}

export function createNodeProjectionController(context: NodeProjectionContext) {
  const {
    appendLog,
    client,
    defaultsWithTcpFallback,
    discoveredByDestination,
    errorMessage,
    init,
    lastError,
    operationalSummary,
    plugins,
    pluginSensors,
    savedByDestination,
    settings,
    status,
    upsertDiscovered,
    watchStatusServer,
  } = context;
  let refreshOperationalSummaryTimerId: number | null = null;
  let refreshOperationalSummaryQueued = false;
  let refreshOperationalSummaryLastRunAt = 0;

  function applyUiSettingsProjection(next: NodeUiPreferences): void {
    settings.clientMode = normalizeClientMode(next.clientMode);
  }

  function applySettingsProjection(next: NodeUiSettings): void {
    settings.displayName = next.displayName;
    settings.autoConnectSaved = next.autoConnectSaved;
    settings.announceCapabilities = next.announceCapabilities;
    settings.tcpClients = [...next.tcpClients];
    settings.broadcast = next.broadcast;
    settings.transportNodeEnabled = next.transportNodeEnabled;
    settings.announceIntervalSeconds = next.announceIntervalSeconds;
    settings.telemetry = { ...next.telemetry };
    settings.checklists = { ...next.checklists };
    settings.hub = { ...next.hub };
    settings.rnode = normalizeRnodeSettings(next.rnode);
    applyUiSettingsProjection(toUiSettingsProjection(next));
  }

  function applySavedPeersProjection(records: SavedPeerRecord[]): void {
    const nextSavedPeers = fromSavedPeerRecords(records);
    const previousDestinations = new Set(Object.keys(savedByDestination));

    for (const [destination, peer] of Object.entries(nextSavedPeers)) {
      savedByDestination[destination] = peer;
      upsertDiscovered(
        destination,
        {
          label: peer.label,
          saved: true,
          lastSeenAt: discoveredByDestination[destination]?.lastSeenAt ?? 0,
          stale: discoveredByDestination[destination]?.stale ?? false,
          activeLink: discoveredByDestination[destination]?.activeLink ?? false,
        },
        "import",
      );
      previousDestinations.delete(destination);
    }

    for (const destination of previousDestinations) {
      delete savedByDestination[destination];
      const peer = discoveredByDestination[destination];
      if (!peer) {
        continue;
      }
      peer.sources = peer.sources.filter((source) => source !== "import");
      peer.saved = false;
      peer.activeLink = false;
      peer.state = "disconnected";
      peer.lastError = undefined;
      peer.lastResolutionError = undefined;
    }
  }

  function savedPeerProjectionDelta(records: SavedPeerRecord[]): {
    added: string[];
    removed: string[];
  } {
    const nextDestinations = new Set(records.map((peer) => normalizeDestinationHex(peer.destination)));
    const previousDestinations = new Set(Object.keys(savedByDestination));
    const added = [...nextDestinations].filter((destination) => !previousDestinations.has(destination));
    const removed = [...previousDestinations].filter((destination) => !nextDestinations.has(destination));
    return { added, removed };
  }

  function logSavedPeerProjectionDelta(
    reason: string,
    records: SavedPeerRecord[],
  ): void {
    const { added, removed } = savedPeerProjectionDelta(records);
    if (added.length === 0 && removed.length === 0) {
      return;
    }
    appendLog(
      "Debug",
      `[saved-peers] ${reason} added=[${added.join(",") || "-"}] removed=[${removed.join(",") || "-"}] total=${records.length}.`,
    );
  }

  async function refreshSettingsProjection(): Promise<void> {
    if (!client.value) {
      return;
    }
    await projectionRefreshCoordinator.run("node:settings", async () => {
      const record = await client.value!.getAppSettings();
      if (record) {
        const normalizedSettings = normalizeAppSettingsRecord(
          record,
          loadUiSettingsProjection(DEFAULT_SETTINGS),
          defaultsWithTcpFallback(),
          true,
        );
        applySettingsProjection(normalizedSettings);
        const normalizedRecord = toAppSettingsRecord(normalizedSettings);
        if (settingsRecordWasNormalized(record, normalizedRecord)) {
          await client.value!.setAppSettings(normalizedRecord);
        }
      }
    }).catch((error: unknown) => {
      appendLog("Debug", `Settings projection refresh skipped: ${errorMessage(error)}`);
    });
  }

  async function refreshSavedPeersProjection(): Promise<void> {
    if (!client.value) {
      return;
    }
    await projectionRefreshCoordinator.run("node:saved-peers", async () => {
      const peers = await client.value!.getSavedPeers();
      logSavedPeerProjectionDelta("native projection", peers);
      applySavedPeersProjection(peers);
    }).catch((error: unknown) => {
      appendLog("Debug", `Saved-peer projection refresh skipped: ${errorMessage(error)}`);
    });
  }

  async function refreshOperationalSummaryProjection(): Promise<void> {
    if (!client.value) {
      operationalSummary.value = { ...EMPTY_OPERATIONAL_SUMMARY };
      return;
    }
    await projectionRefreshCoordinator.run("node:operational-summary", async () => {
      operationalSummary.value = await client.value!.getOperationalSummary();
    }).catch((error: unknown) => {
      appendLog("Debug", `Operational summary refresh skipped: ${errorMessage(error)}`);
    });
  }

  async function refreshPluginProjection(discover = false): Promise<void> {
    if (!client.value) {
      plugins.value = [];
      return;
    }
    await projectionRefreshCoordinator.run("node:plugins", async () => {
      plugins.value = discover
        ? await client.value!.refreshPlugins()
        : await client.value!.listPlugins();
    }).catch((error: unknown) => {
      appendLog("Debug", `Plugin projection refresh skipped: ${errorMessage(error)}`);
    });
  }

  async function refreshPluginSensors(): Promise<void> {
    if (!client.value) {
      pluginSensors.value = [];
      return;
    }
    await projectionRefreshCoordinator.run("node:plugin-sensors", async () => {
      pluginSensors.value = await client.value!.listPluginSensors();
    }).catch((error: unknown) => {
      appendLog("Debug", `Plugin sensor refresh skipped: ${errorMessage(error)}`);
    });
  }

  async function approvePluginPublisher(pluginId: string): Promise<void> {
    if (!client.value) return;
    await client.value.approvePluginPublisher(pluginId);
    await refreshPluginProjection();
  }

  async function revokePluginPublisher(fingerprint: string): Promise<void> {
    if (!client.value) return;
    await client.value.revokePluginPublisher(fingerprint);
    await refreshPluginProjection();
  }

  async function setPluginEnabled(pluginId: string, enabled: boolean): Promise<void> {
    if (!client.value) return;
    await client.value.setPluginEnabled(pluginId, enabled);
    await refreshPluginProjection();
  }

  async function grantPluginCapabilities(
    pluginId: string,
    capabilities: PluginCapabilityRecord,
  ): Promise<void> {
    if (!client.value) return;
    await client.value.grantPluginCapabilities(pluginId, capabilities);
    await refreshPluginProjection();
  }

  async function openPluginConfiguration(pluginId: string): Promise<void> {
    if (!client.value) return;
    await client.value.openPluginConfiguration(pluginId);
  }

  async function refreshWatchStatusServerSettings(): Promise<void> {
    if (!client.value) {
      Object.assign(watchStatusServer, DEFAULT_WATCH_STATUS_SERVER);
      return;
    }
    await projectionRefreshCoordinator.run("node:watch-status-server", async () => {
      Object.assign(watchStatusServer, await client.value!.getWatchStatusServerSettings());
    }).catch((error: unknown) => {
      appendLog("Debug", `Watch status server settings refresh skipped: ${errorMessage(error)}`);
    });
  }

  async function updateWatchStatusServerSettings(settingsRecord: WatchStatusServerSettings): Promise<void> {
    await init();
    if (!client.value) {
      return;
    }
    await client.value.setWatchStatusServerSettings(settingsRecord);
    Object.assign(watchStatusServer, await client.value.getWatchStatusServerState());
  }

  function scheduleOperationalSummaryRefresh(delayMs = PROJECTION_REFRESH_DEBOUNCE_MS): void {
    refreshOperationalSummaryQueued = true;
    if (refreshOperationalSummaryTimerId !== null) {
      return;
    }

    const elapsed = nowMs() - refreshOperationalSummaryLastRunAt;
    const nextDelay = Math.max(
      delayMs,
      Math.max(0, OPERATIONAL_SUMMARY_REFRESH_MIN_INTERVAL_MS - elapsed),
    );

    refreshOperationalSummaryTimerId = window.setTimeout(() => {
      refreshOperationalSummaryTimerId = null;
      if (!refreshOperationalSummaryQueued) {
        return;
      }
      refreshOperationalSummaryQueued = false;
      refreshOperationalSummaryLastRunAt = nowMs();
      void refreshOperationalSummaryProjection()
        .catch(() => undefined)
        .finally(() => {
          if (refreshOperationalSummaryQueued) {
            scheduleOperationalSummaryRefresh(delayMs);
          }
        });
    }, nextDelay);
  }

  async function persistSettingsProjection(nextSettings: NodeUiSettings = settings): Promise<void> {
    const normalizedUiSettings = toUiSettingsProjection(nextSettings);
    storeUiSettingsProjection(normalizedUiSettings);
    applyUiSettingsProjection(normalizedUiSettings);

    if (runtimeProfile === "web") {
      persistWebLegacySettings(nextSettings);
      applySettingsProjection(nextSettings);
      return;
    }
    if (!client.value) {
      return;
    }
    applySettingsProjection(nextSettings);
    const requestedRecord = toAppSettingsRecord(nextSettings);
    await client.value.setAppSettings(requestedRecord);
    const persistedRecord = await client.value.getAppSettings();
    if (!persistedRecord) {
      throw new Error("Native app settings save did not return persisted settings.");
    }
    const persistedSettings = normalizeAppSettingsRecord(
      persistedRecord,
      normalizedUiSettings,
      defaultsWithTcpFallback(),
      true,
    );
    const normalizedPersistedRecord = toAppSettingsRecord(persistedSettings);
    if (!settingsRecordsEqual(requestedRecord, normalizedPersistedRecord)) {
      throw new Error("Native app settings save verification failed.");
    }
    applySettingsProjection(persistedSettings);
    await refreshOperationalSummaryProjection();
  }

  async function persistSavedPeersProjection(
    nextSavedPeers: Record<string, SavedPeer>,
    reason = "projection update",
  ): Promise<void> {
    const records = toSavedPeerRecords(nextSavedPeers);
    logSavedPeerProjectionDelta(reason, records);
    if (runtimeProfile === "web") {
      if (client.value) {
        await client.value.setSavedPeers(records);
      }
      persistWebLegacySavedPeers(records);
      applySavedPeersProjection(records);
      return;
    }
    if (!client.value) {
      return;
    }
    await client.value.setSavedPeers(records);
    applySavedPeersProjection(records);
    await refreshOperationalSummaryProjection();
  }

  async function importLegacyProjectionState(): Promise<void> {
    const legacyState = buildLegacyProjectionState(DEFAULT_SETTINGS);
    if (!legacyState) {
      return;
    }

    storeUiSettingsProjection(legacyState.uiSettings);
    applyUiSettingsProjection(legacyState.uiSettings);

    if (runtimeProfile === "web") {
      if (legacyState.payload.settings) {
        applySettingsProjection(
          normalizeAppSettingsRecord(
            legacyState.payload.settings,
            legacyState.uiSettings,
            defaultsWithTcpFallback(),
          ),
        );
      }
      if (legacyState.payload.savedPeers.length > 0) {
        if (client.value) {
          await client.value.setSavedPeers(legacyState.payload.savedPeers);
        }
        applySavedPeersProjection(legacyState.payload.savedPeers);
      }
      return;
    }

    if (!client.value) {
      return;
    }

    const legacyCounts = {
      savedPeers: legacyState.payload.savedPeers.length,
      eams: legacyState.payload.eams.length,
      events: legacyState.payload.events.length,
      messages: legacyState.payload.messages.length,
      telemetryPositions: legacyState.payload.telemetryPositions.length,
    };
    const nativeHasImportedLegacyState = async (): Promise<boolean> => {
      const [summary, savedPeers] = await Promise.all([
        client.value!.getOperationalSummary(),
        client.value!.getSavedPeers(),
      ]);
      return savedPeers.length >= legacyCounts.savedPeers
        && summary.eamCount >= legacyCounts.eams
        && summary.eventCount >= legacyCounts.events
        && summary.messageCount >= legacyCounts.messages
        && summary.telemetryCount >= legacyCounts.telemetryPositions;
    };

    const completed = await client.value.legacyImportCompleted();
    if (!completed || !(await nativeHasImportedLegacyState())) {
      await client.value.importLegacyState(legacyState.payload);
    }
    if (await nativeHasImportedLegacyState()) {
      clearLegacyProjectionStorage();
    } else {
      appendLog(
        "Warn",
        "[startup] legacy projection import left WebView storage intact because native verification did not match.",
      );
    }
  }

  return {
    applySavedPeersProjection,
    applySettingsProjection,
    applyUiSettingsProjection,
    approvePluginPublisher,
    grantPluginCapabilities,
    importLegacyProjectionState,
    openPluginConfiguration,
    persistSavedPeersProjection,
    persistSettingsProjection,
    refreshOperationalSummaryProjection,
    refreshPluginProjection,
    refreshPluginSensors,
    refreshSavedPeersProjection,
    refreshSettingsProjection,
    refreshWatchStatusServerSettings,
    revokePluginPublisher,
    scheduleOperationalSummaryRefresh,
    setPluginEnabled,
    updateWatchStatusServerSettings,
  };
}
