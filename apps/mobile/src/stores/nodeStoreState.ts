import type {
  AnnounceRecord,
  InstalledPluginRecord,
  NodeStatus,
  PluginSensorRecord,
  ReticulumNodeClient,
  SyncStatus,
  WatchStatusServerState,
} from "@reticulum/node-client";
import { reactive, ref, shallowRef } from "vue";

import { loadHubRegistryLinkage } from "../services/hubRegistryBootstrap";
import type { DiscoveredPeer, HubDirectorySnapshot, NodeUiSettings, SavedPeer } from "../types/domain";
import {
  cloneDefaultSettings,
  hubModeUsesRch,
  loadNodeConfigRestartRequired,
  loadRemovedPeerDestinations,
} from "./nodeSettingsModel";
import {
  DEFAULT_WATCH_STATUS_SERVER,
  EMPTY_OPERATIONAL_SUMMARY,
  EMPTY_STATUS,
  EMPTY_SYNC_STATUS,
  type HubRegistrationSnapshot,
  type UiLogLine,
  nowMs,
} from "./nodeStoreCore";

export function createNodeStoreState() {
  const settings = reactive<NodeUiSettings>(cloneDefaultSettings());
  const status = ref<NodeStatus>({ ...EMPTY_STATUS });
  const nodeConfigRestartRequired = ref(loadNodeConfigRestartRequired());
  const announceByDestination = reactive<Record<string, AnnounceRecord>>({});
  const discoveredByDestination = reactive<Record<string, DiscoveredPeer>>({});
  const savedByDestination = reactive<Record<string, SavedPeer>>({});
  const removedByDestination = reactive<Record<string, number>>(loadRemovedPeerDestinations());
  const appDestinationByIdentity = reactive<Record<string, string>>({});
  const lxmfDestinationByIdentity = reactive<Record<string, string>>({});
  const livePresenceByDestination = reactive<Record<string, number>>({});
  const liveLxmfPresenceByIdentity = reactive<Record<string, number>>({});
  const logs = ref<UiLogLine[]>([]);
  const nodeControlEntries = ref<UiLogLine[]>([]);
  const lastError = ref("");
  const readinessError = ref("");
  const lastHubRefreshAt = ref(0);
  const syncStatus = ref<SyncStatus>({ ...EMPTY_SYNC_STATUS });
  const operationalSummary = ref({ ...EMPTY_OPERATIONAL_SUMMARY });
  const watchStatusServer = reactive<WatchStatusServerState>({ ...DEFAULT_WATCH_STATUS_SERVER });
  const hubDirectorySnapshot = ref<HubDirectorySnapshot | null>(null);
  const telemetryDestinations = ref<string[]>([]);
  const plugins = ref<InstalledPluginRecord[]>([]);
  const pluginSensors = ref<PluginSensorRecord[]>([]);
  const linkage = loadHubRegistryLinkage() ?? undefined;
  const hubRegistration = reactive<HubRegistrationSnapshot>({
    status: hubModeUsesRch(settings.hub.mode) ? "pending" : "disabled",
    linkage,
    lastReadyAt: linkage?.updatedAt,
  });
  const initialized = ref(false);
  const presenceNow = ref(nowMs());
  const client = shallowRef<ReticulumNodeClient | null>(null);
  const unsubscribeClientEvents = ref<Array<() => void>>([]);
  const startupSettling = ref(false);

  return {
    announceByDestination,
    appDestinationByIdentity,
    client,
    discoveredByDestination,
    hubDirectorySnapshot,
    hubRegistration,
    initialized,
    lastError,
    lastHubRefreshAt,
    liveLxmfPresenceByIdentity,
    livePresenceByDestination,
    logs,
    lxmfDestinationByIdentity,
    nodeConfigRestartRequired,
    nodeControlEntries,
    operationalSummary,
    pluginSensors,
    plugins,
    presenceNow,
    readinessError,
    removedByDestination,
    savedByDestination,
    settings,
    startupSettling,
    status,
    syncStatus,
    telemetryDestinations,
    unsubscribeClientEvents,
    watchStatusServer,
  };
}

export type NodeStoreState = ReturnType<typeof createNodeStoreState>;
