import {
  type AppSettingsRecord,
  DEFAULT_NODE_CONFIG,
  type NodeConfig,
  type SavedPeerRecord,
  generateDefaultCallSign,
  YELLOW_TEAM_UID,
} from "@reticulum/node-client";
import { Capacitor } from "@capacitor/core";

import type {
  NodeUiSettings,
  SavedPeer,
} from "../types/domain";
import type { NodeUiPreferences } from "../utils/legacyState";
import {
  ensureRequiredAnnounceCapabilities,
  formatAnnounceAppData,
  isValidDestinationHex,
  normalizeDisplayName,
  normalizeDestinationHex,
} from "../utils/peers";
import {
  DEFAULT_RNODE_SETTINGS,
  normalizeRnodeSettings,
} from "../utils/rnodeProfiles";
import { normalizeTeamPreferences } from "../utils/teamSettings";
import {
  DEFAULT_TCP_COMMUNITY_ENDPOINTS,
  normalizeTcpCommunityClients,
} from "../utils/tcpCommunityServers";
import {
  NODE_CONFIG_RESTART_REQUIRED_STORAGE_KEY,
  REMOVED_PEERS_STORAGE_KEY,
  nowMs,
} from "./nodeStoreCore";

export const LEGACY_DEFAULT_DISPLAY_NAME = "emergency-ops-mobile";

export const DEFAULT_SETTINGS: NodeUiSettings = {
  displayName: DEFAULT_NODE_CONFIG.name,
  clientMode: "auto",
  autoConnectSaved: false,
  announceCapabilities: ensureRequiredAnnounceCapabilities("R3AKT,EMergencyMessages"),
  tcpClients: [...DEFAULT_TCP_COMMUNITY_ENDPOINTS],
  broadcast: DEFAULT_NODE_CONFIG.broadcast,
  transportNodeEnabled: DEFAULT_NODE_CONFIG.transportNodeEnabled,
  announceIntervalSeconds: DEFAULT_NODE_CONFIG.announceIntervalSeconds,
  telemetry: {
    enabled: false,
    publishIntervalSeconds: 360,
    accuracyThresholdMeters: undefined,
    staleAfterMinutes: 30,
    expireAfterMinutes: 180,
  },
  checklists: {
    defaultTaskDueStepMinutes: 30,
  },
  rnode: { ...DEFAULT_RNODE_SETTINGS },
  hub: {
    mode: "Autonomous",
    identityHash: "",
    apiBaseUrl: "",
    apiKey: "",
    refreshIntervalSeconds: 3600,
  },
  teams: {
    activeTeamUid: YELLOW_TEAM_UID,
    aliases: [],
    localTeams: [],
    localTeamsInitialized: false,
  },
};
export const RCH_HUB_DIRECTORY_ENABLED = true;
export function normalizeClientMode(value: unknown): NodeUiSettings["clientMode"] {
  const requested = value === "capacitor" ? "capacitor" : "auto";
  if (requested === "capacitor" && Capacitor.getPlatform() === "web") {
    return "auto";
  }
  return requested;
}

export function normalizeStoredDisplayName(value: unknown): string {
  const normalized = normalizeDisplayName(typeof value === "string" ? value : "");
  if (!normalized || normalized.toLowerCase() === LEGACY_DEFAULT_DISPLAY_NAME) {
    return generateDefaultCallSign();
  }
  return normalized;
}

export function normalizeTelemetrySettings(
  telemetry: Partial<NodeUiSettings["telemetry"]> | undefined,
  base: NodeUiSettings["telemetry"] = DEFAULT_SETTINGS.telemetry,
): NodeUiSettings["telemetry"] {
  const staleAfterMinutes = Math.max(
    1,
    Number(telemetry?.staleAfterMinutes ?? base.staleAfterMinutes),
  );
  const expireAfterMinutes = Math.max(
    staleAfterMinutes,
    Number(telemetry?.expireAfterMinutes ?? base.expireAfterMinutes),
  );

  return {
    ...base,
    ...telemetry,
    publishIntervalSeconds: Math.max(
      1,
      Number(telemetry?.publishIntervalSeconds ?? base.publishIntervalSeconds),
    ),
    accuracyThresholdMeters:
      telemetry?.accuracyThresholdMeters === undefined || telemetry?.accuracyThresholdMeters === null
        ? undefined
        : Math.max(0, Number(telemetry.accuracyThresholdMeters)),
    staleAfterMinutes,
    expireAfterMinutes,
  };
}

export function normalizeChecklistSettings(
  checklists: Partial<NodeUiSettings["checklists"]> | undefined,
  base: NodeUiSettings["checklists"] = DEFAULT_SETTINGS.checklists,
): NodeUiSettings["checklists"] {
  const parsed = Math.trunc(Number(checklists?.defaultTaskDueStepMinutes ?? base.defaultTaskDueStepMinutes));
  return {
    ...base,
    ...checklists,
    defaultTaskDueStepMinutes: Number.isFinite(parsed) ? Math.max(1, parsed) : base.defaultTaskDueStepMinutes,
  };
}

export function normalizeHubMode(value: unknown): NodeUiSettings["hub"]["mode"] {
  if (!RCH_HUB_DIRECTORY_ENABLED) {
    return "Autonomous";
  }

  switch (String(value ?? "").trim()) {
    case "Connected":
      return "Connected";
    case "SemiAutonomous":
    case "RchLxmf":
    case "RchHttp":
      return "SemiAutonomous";
    case "Autonomous":
    case "Disabled":
    default:
      return "Autonomous";
  }
}

export function hubModeUsesRch(mode: NodeUiSettings["hub"]["mode"]): boolean {
  return mode !== "Autonomous";
}

export function hasSelectedHubIdentity(hubIdentityHash = ""): boolean {
  return isValidDestinationHex(normalizeDestinationHex(hubIdentityHash));
}

export function cloneDefaultSettings(): NodeUiSettings {
  return {
    ...DEFAULT_SETTINGS,
    telemetry: { ...DEFAULT_SETTINGS.telemetry },
    checklists: { ...DEFAULT_SETTINGS.checklists },
    hub: { ...DEFAULT_SETTINGS.hub },
    teams: {
      activeTeamUid: DEFAULT_SETTINGS.teams.activeTeamUid,
      aliases: DEFAULT_SETTINGS.teams.aliases.map((alias) => ({ ...alias })),
      localTeams: DEFAULT_SETTINGS.teams.localTeams.map((team) => ({
        ...team,
        memberDestinations: [...team.memberDestinations],
      })),
      localTeamsInitialized: DEFAULT_SETTINGS.teams.localTeamsInitialized,
    },
    rnode: { ...DEFAULT_SETTINGS.rnode },
  };
}

export function toAppSettingsRecord(settings: NodeUiSettings): AppSettingsRecord {
  return {
    displayName: settings.displayName,
    autoConnectSaved: settings.autoConnectSaved,
    announceCapabilities: settings.announceCapabilities,
    tcpClients: [...settings.tcpClients],
    broadcast: settings.broadcast,
    transportNodeEnabled: settings.transportNodeEnabled,
    announceIntervalSeconds: settings.announceIntervalSeconds,
    telemetry: {
      enabled: settings.telemetry.enabled,
      publishIntervalSeconds: settings.telemetry.publishIntervalSeconds,
      accuracyThresholdMeters: settings.telemetry.accuracyThresholdMeters,
      staleAfterMinutes: settings.telemetry.staleAfterMinutes,
      expireAfterMinutes: settings.telemetry.expireAfterMinutes,
    },
    checklists: {
      defaultTaskDueStepMinutes: settings.checklists.defaultTaskDueStepMinutes,
    },
    hub: {
      mode: settings.hub.mode,
      identityHash: settings.hub.identityHash,
      apiBaseUrl: settings.hub.apiBaseUrl,
      apiKey: settings.hub.apiKey,
      refreshIntervalSeconds: settings.hub.refreshIntervalSeconds,
    },
    teams: {
      activeTeamUid: settings.teams.activeTeamUid,
      aliases: settings.teams.aliases.map((alias) => ({ ...alias })),
      localTeams: settings.teams.localTeams.map((team) => ({
        ...team,
        memberDestinations: [...team.memberDestinations],
      })),
      localTeamsInitialized: settings.teams.localTeamsInitialized,
    },
    rnode: normalizeRnodeSettings(settings.rnode),
  };
}

export function hubModeWasCoerced(left: AppSettingsRecord, right: AppSettingsRecord): boolean {
  return left.hub.mode !== right.hub.mode;
}

export function settingsRecordWasNormalized(left: AppSettingsRecord, right: AppSettingsRecord): boolean {
  return left.displayName !== right.displayName || hubModeWasCoerced(left, right);
}

export function settingsRecordsEqual(left: AppSettingsRecord, right: AppSettingsRecord): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

export function nodeConfigsEqual(left: NodeConfig, right: NodeConfig): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

export function loadNodeConfigRestartRequired(): boolean {
  try {
    return window.localStorage.getItem(NODE_CONFIG_RESTART_REQUIRED_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function storeNodeConfigRestartRequired(required: boolean): void {
  try {
    if (required) {
      window.localStorage.setItem(NODE_CONFIG_RESTART_REQUIRED_STORAGE_KEY, "1");
    } else {
      window.localStorage.removeItem(NODE_CONFIG_RESTART_REQUIRED_STORAGE_KEY);
    }
  } catch {
    // Local storage can be unavailable in restricted webviews; the in-memory flag still applies.
  }
}

export function toUiSettingsProjection(
  next: Pick<NodeUiSettings, "clientMode">,
): NodeUiPreferences {
  return {
    clientMode: normalizeClientMode(next.clientMode),
  };
}

export function normalizeAppSettingsRecord(
  runtimeSettings: AppSettingsRecord,
  uiSettings: NodeUiPreferences,
  tcpFallback: string[] = DEFAULT_TCP_COMMUNITY_ENDPOINTS,
  allowEmptyTcpClients = false,
): NodeUiSettings {
  return {
    ...cloneDefaultSettings(),
    ...runtimeSettings,
    displayName: normalizeStoredDisplayName(runtimeSettings.displayName),
    clientMode: normalizeClientMode(uiSettings.clientMode),
    autoConnectSaved: false,
    announceCapabilities: ensureRequiredAnnounceCapabilities(runtimeSettings.announceCapabilities),
    tcpClients: normalizeTcpCommunityClients(
      runtimeSettings.tcpClients,
      tcpFallback,
      allowEmptyTcpClients,
    ),
    transportNodeEnabled: runtimeSettings.transportNodeEnabled ?? DEFAULT_SETTINGS.transportNodeEnabled,
    telemetry: normalizeTelemetrySettings(runtimeSettings.telemetry),
    checklists: normalizeChecklistSettings(runtimeSettings.checklists),
    rnode: normalizeRnodeSettings(runtimeSettings.rnode),
    hub: {
      ...DEFAULT_SETTINGS.hub,
      ...runtimeSettings.hub,
      mode: normalizeHubMode(runtimeSettings.hub?.mode),
    },
    teams: normalizeTeamPreferences(runtimeSettings.teams),
  };
}

export function toSavedPeerRecords(savedPeers: Record<string, SavedPeer>): SavedPeerRecord[] {
  return Object.values(savedPeers).map((peer) => ({
    destination: normalizeDestinationHex(peer.destination),
    label: peer.label?.trim() || undefined,
    savedAt: Number(peer.savedAt ?? nowMs()),
    identityHex: isValidDestinationHex(normalizeDestinationHex(peer.identityHex ?? ""))
      ? normalizeDestinationHex(peer.identityHex ?? "")
      : undefined,
    lxmfDestinationHex: isValidDestinationHex(normalizeDestinationHex(peer.lxmfDestinationHex ?? ""))
      ? normalizeDestinationHex(peer.lxmfDestinationHex ?? "")
      : undefined,
    appData: peer.appData?.trim() || undefined,
    displayName: peer.displayName?.trim() || undefined,
    lastRouteSeenAtMs: typeof peer.lastRouteSeenAtMs === "number" && Number.isFinite(peer.lastRouteSeenAtMs)
      ? peer.lastRouteSeenAtMs
      : undefined,
    lastHops: typeof peer.lastHops === "number" && Number.isFinite(peer.lastHops)
      ? peer.lastHops
      : undefined,
  }));
}

export function fromSavedPeerRecords(records: SavedPeerRecord[]): Record<string, SavedPeer> {
  const out: Record<string, SavedPeer> = {};
  for (const peer of records) {
    const destination = normalizeDestinationHex(peer.destination ?? "");
    if (!isValidDestinationHex(destination)) {
      continue;
    }
    out[destination] = {
      destination,
      label: peer.label?.trim() || undefined,
      savedAt: Number(peer.savedAt ?? nowMs()),
      identityHex: isValidDestinationHex(normalizeDestinationHex(peer.identityHex ?? ""))
        ? normalizeDestinationHex(peer.identityHex ?? "")
        : undefined,
      lxmfDestinationHex: isValidDestinationHex(normalizeDestinationHex(peer.lxmfDestinationHex ?? ""))
        ? normalizeDestinationHex(peer.lxmfDestinationHex ?? "")
        : undefined,
      appData: peer.appData?.trim() || undefined,
      displayName: peer.displayName?.trim() || undefined,
      lastRouteSeenAtMs: typeof peer.lastRouteSeenAtMs === "number" && Number.isFinite(peer.lastRouteSeenAtMs)
        ? peer.lastRouteSeenAtMs
        : undefined,
      lastHops: typeof peer.lastHops === "number" && Number.isFinite(peer.lastHops)
        ? peer.lastHops
        : undefined,
    };
  }
  return out;
}

export function loadRemovedPeerDestinations(): Record<string, number> {
  try {
    const raw = window.localStorage.getItem(REMOVED_PEERS_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const out: Record<string, number> = {};
    for (const [destinationRaw, removedAtRaw] of Object.entries(parsed)) {
      const destination = normalizeDestinationHex(destinationRaw);
      if (!isValidDestinationHex(destination)) {
        continue;
      }
      const removedAt = Number(removedAtRaw);
      out[destination] = Number.isFinite(removedAt) ? removedAt : nowMs();
    }
    return out;
  } catch {
    return {};
  }
}

export function storeRemovedPeerDestinations(destinations: Record<string, number>): void {
  try {
    window.localStorage.setItem(REMOVED_PEERS_STORAGE_KEY, JSON.stringify(destinations));
  } catch {
    // Local storage can be unavailable in restricted webviews; native removal still applies.
  }
}

export function toNodeConfig(settings: NodeUiSettings): NodeConfig {
  const displayName = normalizeStoredDisplayName(settings.displayName);
  return {
    name: displayName,
    storageDir: "reticulum-mobile",
    tcpClients: normalizeTcpCommunityClients(settings.tcpClients, DEFAULT_TCP_COMMUNITY_ENDPOINTS, true),
    broadcast: settings.broadcast,
    transportNodeEnabled: settings.transportNodeEnabled,
    announceIntervalSeconds: settings.announceIntervalSeconds,
    staleAfterMinutes: settings.telemetry.staleAfterMinutes,
    announceCapabilities: formatAnnounceAppData(
      ensureRequiredAnnounceCapabilities(settings.announceCapabilities),
      displayName,
    ),
    hubMode: settings.hub.mode,
    hubIdentityHash: settings.hub.identityHash || undefined,
    hubApiBaseUrl: settings.hub.apiBaseUrl || undefined,
    hubApiKey: settings.hub.apiKey || undefined,
    hubRefreshIntervalSeconds: settings.hub.refreshIntervalSeconds,
    rnode: normalizeRnodeSettings(settings.rnode),
  };
}
