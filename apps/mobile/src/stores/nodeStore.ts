import {
  type AppSettingsRecord,
  DEFAULT_NODE_CONFIG,
  type AnnounceRecord,
  type PeerRecord,
  type ProjectionInvalidationEvent,
  type ProjectionScope,
  type SendMode,
  type SavedPeerRecord,
  type SyncStatus,
  createReticulumNodeClient,
  type AnnounceReceivedEvent,
  type HubDirectoryUpdatedEvent,
  type LogLevel,
  type NodeConfig,
  type NodeClientEvents,
  type NodeErrorEvent,
  type NodeLogEvent,
  type NodeStatus,
  type PeerChangedEvent,
  type ReticulumNodeClient,
  type StatusChangedEvent,
} from "@reticulum/node-client";
import { Capacitor } from "@capacitor/core";
import { defineStore } from "pinia";
import { computed, reactive, ref, shallowRef } from "vue";

import {
  bootstrapHubRegistry,
  buildHubRegistryBootstrapProfile,
  clearHubRegistryLinkage,
  loadHubRegistryLinkage,
  matchesHubRegistryProfile,
  saveHubRegistryLinkage,
  type HubRegistrationStatus,
  type HubRegistryBootstrapProfile,
  type HubRegistryCommandTransport,
  type HubRegistryLinkage,
} from "../services/hubRegistryBootstrap";
import { buildMissionCommandFieldsBase64 } from "../utils/missionSync";
import {
  buildLegacyProjectionState,
  clearLegacyProjectionStorage,
  loadUiSettingsProjection,
  persistUiSettingsProjection as storeUiSettingsProjection,
  persistWebLegacySavedPeers,
  persistWebLegacySettings,
  type NodeUiPreferences,
} from "../utils/legacyState";
import type {
  DiscoveredPeer,
  HubDirectorySnapshot,
  NodeUiSettings,
  PeerConnectionState,
  PeerListV1,
  SavedPeer,
} from "../types/domain";
import {
  createPeerListV1,
  ensureRequiredAnnounceCapabilities,
  extractAnnounceCapabilityText,
  extractAnnouncedName,
  formatAnnounceAppData,
  hasCapability,
  isValidDestinationHex,
  normalizeDisplayName,
  normalizeDestinationHex,
  parseCapabilityTokens,
  parsePeerListV1,
} from "../utils/peers";
import { runtimeProfile } from "../utils/runtimeProfile";
import {
  DEFAULT_TCP_COMMUNITY_ENDPOINTS,
  normalizeTcpCommunityClients,
} from "../utils/tcpCommunityServers";

const PEER_ONLINE_FRESHNESS_MS = 10 * 60_000;
const PEER_VISIBLE_UNSAVED_MAX_AGE_MS = 30 * 60_000;
const PEER_PRESENCE_TICK_MS = 15_000;
const EMPTY_BYTES = new Uint8Array(0);
const STARTUP_ANNOUNCE_SETTLE_MS = 2_500;
const STARTUP_AUTO_CONNECT_FRESHNESS_MS = 30_000;
const AUTO_CONNECT_SERIAL_DELAY_MS = 300;
const PROJECTION_REFRESH_DEBOUNCE_MS = 200;
const OPERATIONAL_SUMMARY_REFRESH_MIN_INTERVAL_MS = 2_000;

const EMPTY_STATUS: NodeStatus = {
  running: false,
  name: "",
  identityHex: "",
  appDestinationHex: "",
  lxmfDestinationHex: "",
};

const EMPTY_SYNC_STATUS: SyncStatus = {
  phase: "Idle",
  messagesReceived: 0,
};

const EMPTY_OPERATIONAL_SUMMARY = {
  running: false,
  peerCountTotal: 0,
  savedPeerCount: 0,
  connectedPeerCount: 0,
  conversationCount: 0,
  messageCount: 0,
  eamCount: 0,
  eventCount: 0,
  telemetryCount: 0,
  updatedAtMs: 0,
};

interface HubRegistrationSnapshot {
  status: HubRegistrationStatus;
  linkage?: HubRegistryLinkage;
  lastAttemptAt?: number;
  lastReadyAt?: number;
  lastError?: string;
}

interface HubAnnounceCandidate {
  destination: string;
  label: string;
}

const DEFAULT_SETTINGS: NodeUiSettings = {
  displayName: DEFAULT_NODE_CONFIG.name,
  clientMode: "auto",
  autoConnectSaved: true,
  announceCapabilities: ensureRequiredAnnounceCapabilities("R3AKT,EMergencyMessages"),
  tcpClients: [...DEFAULT_TCP_COMMUNITY_ENDPOINTS],
  broadcast: DEFAULT_NODE_CONFIG.broadcast,
  announceIntervalSeconds: DEFAULT_NODE_CONFIG.announceIntervalSeconds,
  telemetry: {
    enabled: false,
    publishIntervalSeconds: 60,
    accuracyThresholdMeters: undefined,
    staleAfterMinutes: 30,
    expireAfterMinutes: 180,
  },
  hub: {
    mode: "Autonomous",
    identityHash: "",
    apiBaseUrl: "",
    apiKey: "",
    refreshIntervalSeconds: 3600,
  },
};

interface UiLogLine {
  at: number;
  level: string;
  message: string;
}

type DedicatedFields = Record<string, string>;
type EventPeerRoute = {
  appDestinationHex: string;
  lxmfDestinationHex: string;
  identityHex?: string;
  label?: string;
  announcedName?: string;
  sendMode: SendMode;
};
type PacketSendOptions = {
  dedicatedFields?: DedicatedFields;
  fieldsBase64?: string;
  sendMode?: SendMode;
};

function shouldDisplayDiscoveredPeer(peer: DiscoveredPeer): boolean {
  if (peer.saved) {
    return true;
  }

  if (!peer.sources.includes("announce") && !peer.sources.includes("hub")) {
    return false;
  }

  const seenAt = Math.max(peer.announceLastSeenAt ?? 0, peer.lxmfLastSeenAt ?? 0, peer.lastSeenAt ?? 0);
  return seenAt > 0 && (nowMs() - seenAt) <= PEER_VISIBLE_UNSAVED_MAX_AGE_MS;
}

function nowMs(): number {
  return Date.now();
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function asTrimmedString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function normalizeNodeStatus(value?: Partial<NodeStatus> | null): NodeStatus {
  return {
    running: Boolean(value?.running),
    name: typeof value?.name === "string" ? value.name : "",
    identityHex: typeof value?.identityHex === "string" ? value.identityHex : "",
    appDestinationHex: typeof value?.appDestinationHex === "string" ? value.appDestinationHex : "",
    lxmfDestinationHex: typeof value?.lxmfDestinationHex === "string" ? value.lxmfDestinationHex : "",
  };
}

function activePropagationNodeHex(status: SyncStatus): string | undefined {
  const candidate = asTrimmedString(status.activePropagationNodeHex);
  return candidate ? candidate : undefined;
}

function toUiPeerState(
  state: PeerRecord["state"] | PeerChangedEvent["change"]["state"] | undefined,
): PeerConnectionState {
  if (state === "Connected") {
    return "connected";
  }
  if (state === "Connecting") {
    return "connecting";
  }
  return "disconnected";
}

function peerSortRank(peer: Pick<DiscoveredPeer, "saved" | "activeLink" | "lastSeenAt">): number {
  let rank = 0;
  if (peer.saved) {
    rank += 2;
  }
  if (peer.activeLink) {
    rank += 4;
  }
  if (peer.lastSeenAt > 0) {
    rank += 1;
  }
  return rank;
}

function peerExposesPropagationCapability(appData?: string): boolean {
  return parseCapabilityTokens(appData ?? "").some(
    (token) => token === "hub" || token.endsWith("hub"),
  );
}

function connectionRank(state: PeerConnectionState): number {
  switch (state) {
    case "connected":
      return 2;
    case "connecting":
      return 1;
    default:
      return 0;
  }
}

function comparePropagationCandidates(
  left: DiscoveredPeer,
  right: DiscoveredPeer,
  preferredDestination?: string,
): number {
  const leftPreferred = preferredDestination && left.destination === preferredDestination ? 1 : 0;
  const rightPreferred = preferredDestination && right.destination === preferredDestination ? 1 : 0;
  if (leftPreferred !== rightPreferred) {
    return rightPreferred - leftPreferred;
  }

  const byConnection = connectionRank(right.state) - connectionRank(left.state);
  if (byConnection !== 0) {
    return byConnection;
  }

  const leftHops = typeof left.hops === "number" ? left.hops : Number.MAX_SAFE_INTEGER;
  const rightHops = typeof right.hops === "number" ? right.hops : Number.MAX_SAFE_INTEGER;
  if (leftHops !== rightHops) {
    return leftHops - rightHops;
  }

  const leftSeenAt = Math.max(left.announceLastSeenAt ?? 0, left.lxmfLastSeenAt ?? 0);
  const rightSeenAt = Math.max(right.announceLastSeenAt ?? 0, right.lxmfLastSeenAt ?? 0);
  if (leftSeenAt !== rightSeenAt) {
    return rightSeenAt - leftSeenAt;
  }

  return left.destination.localeCompare(right.destination);
}

function normalizeClientMode(value: unknown): NodeUiSettings["clientMode"] {
  const requested = value === "capacitor" ? "capacitor" : "auto";
  if (requested === "capacitor" && Capacitor.getPlatform() === "web") {
    return "auto";
  }
  return requested;
}

function normalizeStoredDisplayName(value: unknown): string {
  return normalizeDisplayName(typeof value === "string" ? value : "") ?? DEFAULT_SETTINGS.displayName;
}

function normalizeTelemetrySettings(
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
    publishIntervalSeconds: Math.min(
      60,
      Math.max(5, Number(telemetry?.publishIntervalSeconds ?? base.publishIntervalSeconds)),
    ),
    accuracyThresholdMeters:
      telemetry?.accuracyThresholdMeters === undefined || telemetry?.accuracyThresholdMeters === null
        ? undefined
        : Math.max(0, Number(telemetry.accuracyThresholdMeters)),
    staleAfterMinutes,
    expireAfterMinutes,
  };
}

function normalizeHubMode(value: unknown): NodeUiSettings["hub"]["mode"] {
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

function hubModeUsesRch(mode: NodeUiSettings["hub"]["mode"]): boolean {
  return mode !== "Autonomous";
}

function hasSelectedHubIdentity(hubIdentityHash = ""): boolean {
  return isValidDestinationHex(normalizeDestinationHex(hubIdentityHash));
}

function cloneDefaultSettings(): NodeUiSettings {
  return {
    ...DEFAULT_SETTINGS,
    telemetry: { ...DEFAULT_SETTINGS.telemetry },
    hub: { ...DEFAULT_SETTINGS.hub },
  };
}

function toAppSettingsRecord(settings: NodeUiSettings): AppSettingsRecord {
  return {
    displayName: settings.displayName,
    autoConnectSaved: settings.autoConnectSaved,
    announceCapabilities: settings.announceCapabilities,
    tcpClients: [...settings.tcpClients],
    broadcast: settings.broadcast,
    announceIntervalSeconds: settings.announceIntervalSeconds,
    telemetry: {
      enabled: settings.telemetry.enabled,
      publishIntervalSeconds: settings.telemetry.publishIntervalSeconds,
      accuracyThresholdMeters: settings.telemetry.accuracyThresholdMeters,
      staleAfterMinutes: settings.telemetry.staleAfterMinutes,
      expireAfterMinutes: settings.telemetry.expireAfterMinutes,
    },
    hub: {
      mode: settings.hub.mode,
      identityHash: settings.hub.identityHash,
      apiBaseUrl: settings.hub.apiBaseUrl,
      apiKey: settings.hub.apiKey,
      refreshIntervalSeconds: settings.hub.refreshIntervalSeconds,
    },
  };
}

function toUiSettingsProjection(
  next: Pick<NodeUiSettings, "clientMode">,
): NodeUiPreferences {
  return {
    clientMode: normalizeClientMode(next.clientMode),
  };
}

function normalizeAppSettingsRecord(
  runtimeSettings: AppSettingsRecord,
  uiSettings: NodeUiPreferences,
  tcpFallback: string[] = DEFAULT_TCP_COMMUNITY_ENDPOINTS,
): NodeUiSettings {
  return {
    ...cloneDefaultSettings(),
    ...runtimeSettings,
    displayName: normalizeStoredDisplayName(runtimeSettings.displayName),
    clientMode: normalizeClientMode(uiSettings.clientMode),
    announceCapabilities: ensureRequiredAnnounceCapabilities(runtimeSettings.announceCapabilities),
    tcpClients: normalizeTcpCommunityClients(
      runtimeSettings.tcpClients,
      tcpFallback,
    ),
    telemetry: normalizeTelemetrySettings(runtimeSettings.telemetry),
    hub: {
      ...DEFAULT_SETTINGS.hub,
      ...runtimeSettings.hub,
      mode: normalizeHubMode(runtimeSettings.hub?.mode),
    },
  };
}

function toSavedPeerRecords(savedPeers: Record<string, SavedPeer>): SavedPeerRecord[] {
  return Object.values(savedPeers).map((peer) => ({
    destination: normalizeDestinationHex(peer.destination),
    label: peer.label?.trim() || undefined,
    savedAt: Number(peer.savedAt ?? nowMs()),
  }));
}

function fromSavedPeerRecords(records: SavedPeerRecord[]): Record<string, SavedPeer> {
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
    };
  }
  return out;
}

function toNodeConfig(settings: NodeUiSettings): NodeConfig {
  const displayName = normalizeDisplayName(settings.displayName) ?? DEFAULT_NODE_CONFIG.name;
  return {
    name: displayName,
    storageDir: "reticulum-mobile",
    tcpClients: normalizeTcpCommunityClients(settings.tcpClients),
    broadcast: settings.broadcast,
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
  };
}

export const useNodeStore = defineStore("node", () => {
  const settings = reactive<NodeUiSettings>(cloneDefaultSettings());
  const status = ref<NodeStatus>({ ...EMPTY_STATUS });
  const announceByDestination = reactive<Record<string, AnnounceRecord>>({});
  const discoveredByDestination = reactive<Record<string, DiscoveredPeer>>({});
  const savedByDestination = reactive<Record<string, SavedPeer>>({});
  const appDestinationByIdentity = reactive<Record<string, string>>({});
  const lxmfDestinationByIdentity = reactive<Record<string, string>>({});
  const livePresenceByDestination = reactive<Record<string, number>>({});
  const liveLxmfPresenceByIdentity = reactive<Record<string, number>>({});
  const logs = ref<UiLogLine[]>([]);
  const nodeControlEntries = ref<UiLogLine[]>([]);
  const lastError = ref<string>("");
  const lastHubRefreshAt = ref<number>(0);
  const syncStatus = ref<SyncStatus>({ ...EMPTY_SYNC_STATUS });
  const operationalSummary = ref({ ...EMPTY_OPERATIONAL_SUMMARY });
  const hubDirectorySnapshot = ref<HubDirectorySnapshot | null>(null);
  const telemetryDestinations = ref<string[]>([]);
  const hubRegistration = reactive<HubRegistrationSnapshot>({
    status: hubModeUsesRch(settings.hub.mode) ? "pending" : "disabled",
    linkage: loadHubRegistryLinkage() ?? undefined,
    lastReadyAt: loadHubRegistryLinkage()?.updatedAt,
  });
  const initialized = ref(false);
  const presenceNow = ref(nowMs());

  const client = shallowRef<ReticulumNodeClient | null>(null);
  const unsubscribeClientEvents = ref<Array<() => void>>([]);
  const identityResolutionInFlight = new Set<string>();
  const autoConnectInFlight = new Set<string>();
  const autoConnectQueue: string[] = [];
  let hubRegistryBootstrapInFlight: Promise<void> | null = null;
  let propagationSelectionInFlight = false;
  let presenceTickerId: number | null = null;
  let refreshMessagingStatePromise: Promise<void> | null = null;
  let refreshSettingsPromise: Promise<void> | null = null;
  let refreshSavedPeersPromise: Promise<void> | null = null;
  let refreshOperationalSummaryPromise: Promise<void> | null = null;
  let refreshOperationalSummaryTimerId: number | null = null;
  let refreshOperationalSummaryQueued = false;
  let refreshOperationalSummaryLastRunAt = 0;
  let initPromise: Promise<void> | null = null;
  const startupSettling = ref(false);
  const autoConnectQueueActive = ref(false);

  applyUiSettingsProjection(loadUiSettingsProjection(DEFAULT_SETTINGS));

  function defaultsWithTcpFallback(): string[] {
    return DEFAULT_SETTINGS.tcpClients.length > 0
      ? [...DEFAULT_SETTINGS.tcpClients]
      : [...DEFAULT_TCP_COMMUNITY_ENDPOINTS];
  }

  function appendLog(level: string, message: string): void {
    logs.value = [{ at: nowMs(), level, message }, ...logs.value].slice(0, 120);
  }

  function appendNodeControlEntry(level: string, message: string, at = nowMs()): void {
    nodeControlEntries.value = [{ at, level, message }, ...nodeControlEntries.value].slice(0, 120);
  }

  function toPluginLogLevel(level: string): LogLevel {
    switch (asTrimmedString(level).toLowerCase()) {
      case "trace":
        return "Trace";
      case "debug":
        return "Debug";
      case "warn":
        return "Warn";
      case "error":
        return "Error";
      case "info":
      default:
        return "Info";
    }
  }

  function mirrorUiLogToNative(level: string, message: string): void {
    if (!client.value || runtimeProfile === "web") {
      return;
    }
    const normalizedLevel = asTrimmedString(level).toLowerCase();
    if (normalizedLevel !== "warn" && normalizedLevel !== "error") {
      return;
    }
    void client.value.logMessage(toPluginLogLevel(level), message).catch(() => undefined);
  }

  function logUi(level: string, message: string): void {
    appendLog(level, message);
    mirrorUiLogToNative(level, message);
    const normalizedLevel = asTrimmedString(level).toLowerCase();
    if (normalizedLevel === "error") {
      lastError.value = message;
      console.error(`[ui][${level}] ${message}`);
      return;
    }
    if (normalizedLevel === "debug" || normalizedLevel === "trace") {
      console.debug(`[ui][${level}] ${message}`);
      return;
    }
    if (normalizedLevel === "warn") {
      console.warn(`[ui][${level}] ${message}`);
      return;
    }
    console.info(`[ui][${level}] ${message}`);
  }

  function setLastError(message: string): void {
    lastError.value = asTrimmedString(message);
  }

  function clearLastError(): void {
    lastError.value = "";
  }

  function errorMessage(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }
    return String(error);
  }

  function captureActionError(action: string, error: unknown): Error {
    const message = `${action}: ${errorMessage(error)}`;
    lastError.value = message;
    mirrorUiLogToNative("Error", message);
    console.error(`[ui][Error] ${message}`);
    appendLog("Error", message);
    return error instanceof Error ? error : new Error(message);
  }

  function upsertDiscovered(
    destinationRaw: string,
    patch: Partial<DiscoveredPeer>,
    source?: "announce" | "hub" | "import",
  ): void {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return;
    }

    const existing = discoveredByDestination[destination];
    const sources = existing ? [...existing.sources] : [];
    if (source && !sources.includes(source)) {
      sources.push(source);
    }

    const base: DiscoveredPeer = existing ?? {
      destination,
      lastSeenAt: nowMs(),
      sources,
      state: "disconnected",
      saved: false,
      stale: false,
      activeLink: false,
    };

    discoveredByDestination[destination] = {
      ...base,
      ...patch,
      destination,
      sources,
      identityHex: patch.identityHex ?? base.identityHex,
      lxmfDestinationHex: patch.lxmfDestinationHex ?? base.lxmfDestinationHex,
      announceLastSeenAt: patch.announceLastSeenAt ?? base.announceLastSeenAt,
      lxmfLastSeenAt: patch.lxmfLastSeenAt ?? base.lxmfLastSeenAt,
      announcedName: patch.announcedName ?? base.announcedName,
      label: patch.label ?? base.label,
      appData: patch.appData ?? base.appData,
      hops: patch.hops ?? base.hops,
      interfaceHex: patch.interfaceHex ?? base.interfaceHex,
      saved: patch.saved ?? base.saved,
      stale: patch.stale ?? base.stale,
      activeLink: patch.activeLink ?? base.activeLink,
      lastError: Object.prototype.hasOwnProperty.call(patch, "lastError")
        ? patch.lastError
        : base.lastError,
      lastResolutionError: Object.prototype.hasOwnProperty.call(patch, "lastResolutionError")
        ? patch.lastResolutionError
        : base.lastResolutionError,
      lastResolutionAttemptAt: patch.lastResolutionAttemptAt ?? base.lastResolutionAttemptAt,
      lastSeenAt: patch.lastSeenAt ?? base.lastSeenAt,
    };
  }

  function upsertNativeAnnounceRecord(
    record: AnnounceReceivedEvent | AnnounceRecord,
  ): void {
    const destination = normalizeDestinationHex(record.destinationHex);
    if (!isValidDestinationHex(destination)) {
      return;
    }
    const existing = announceByDestination[destination];
    if (existing && existing.receivedAtMs > record.receivedAtMs) {
      return;
    }
    announceByDestination[destination] = {
      destinationHex: destination,
      identityHex: normalizeDestinationHex(record.identityHex),
      destinationKind: record.destinationKind,
      announceClass: record.announceClass,
      appData: record.appData,
      displayName: record.displayName ?? existing?.displayName,
      hops: record.hops,
      interfaceHex: record.interfaceHex,
      receivedAtMs: record.receivedAtMs,
    };
  }

  function isLocalPeerDestination(destinationRaw: string): boolean {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return false;
    }

    const localAppDestination = normalizeDestinationHex(status.value.appDestinationHex ?? "");
    const localLxmfDestination = normalizeDestinationHex(status.value.lxmfDestinationHex ?? "");
    return destination === localAppDestination || destination === localLxmfDestination;
  }

  function isLocalPeer(peer: Pick<DiscoveredPeer, "destination" | "identityHex">): boolean {
    if (isLocalPeerDestination(peer.destination)) {
      return true;
    }

    const localIdentity = normalizeDestinationHex(status.value.identityHex ?? "");
    const peerIdentity = normalizeDestinationHex(peer.identityHex ?? "");
    return isValidDestinationHex(localIdentity) && peerIdentity === localIdentity;
  }

  function isLocalDestinationIdentityPair(
    destinationRaw: string,
    identityRaw?: string,
  ): boolean {
    if (isLocalPeerDestination(destinationRaw)) {
      return true;
    }
    const localIdentity = normalizeDestinationHex(status.value.identityHex ?? "");
    const peerIdentity = normalizeDestinationHex(identityRaw ?? "");
    return isValidDestinationHex(localIdentity) && peerIdentity === localIdentity;
  }

  function resolvePeerLxmfDestinationByIdentity(identityRaw?: string): string | undefined {
    const identityHex = normalizeDestinationHex(identityRaw ?? "");
    if (!isValidDestinationHex(identityHex) || identityHex === normalizeDestinationHex(status.value.identityHex ?? "")) {
      return undefined;
    }

    const mapped = normalizeDestinationHex(lxmfDestinationByIdentity[identityHex] ?? "");
    if (isValidDestinationHex(mapped)) {
      return mapped;
    }

    return Object.values(discoveredByDestination)
      .find((peer) => normalizeDestinationHex(peer.identityHex ?? "") === identityHex)
      ?.lxmfDestinationHex;
  }

  function setPeerState(
    destinationRaw: string,
    stateValue: PeerConnectionState,
    lastErrorValue?: string,
  ): void {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return;
    }

    upsertDiscovered(destination, {
      state: stateValue,
      lastError: lastErrorValue,
    });
  }

  function clearHubDirectoryState(): void {
    hubDirectorySnapshot.value = null;
    lastHubRefreshAt.value = 0;
  }

  function clearAnnounceState(): void {
    for (const destination of Object.keys(announceByDestination)) {
      delete announceByDestination[destination];
    }
  }

  function upsertResolvedPeer(peer: PeerRecord): void {
    const destination = normalizeDestinationHex(peer.destinationHex);
    if (!isValidDestinationHex(destination) || isLocalDestinationIdentityPair(destination, peer.identityHex)) {
      return;
    }

    const identityHex = normalizeDestinationHex(peer.identityHex ?? "");
    const lxmfDestinationHex = normalizeDestinationHex(peer.lxmfDestinationHex ?? "");
    if (isValidDestinationHex(identityHex)) {
      appDestinationByIdentity[identityHex] = destination;
    }
    if (isValidDestinationHex(identityHex) && isValidDestinationHex(lxmfDestinationHex)) {
      lxmfDestinationByIdentity[identityHex] = lxmfDestinationHex;
    }

    const saved = savedByDestination[destination];
    upsertDiscovered(
      destination,
      {
        identityHex: isValidDestinationHex(identityHex) ? identityHex : undefined,
        lxmfDestinationHex: isValidDestinationHex(lxmfDestinationHex)
          ? lxmfDestinationHex
          : undefined,
        announcedName: peer.displayName?.trim() || undefined,
        label: saved?.label ?? undefined,
        appData: peer.appData,
        announceLastSeenAt: peer.announceLastSeenAtMs,
        lxmfLastSeenAt: peer.lxmfLastSeenAtMs,
        lastSeenAt: peer.lastSeenAtMs,
        state: toUiPeerState(peer.state),
        saved: peer.saved,
        stale: peer.stale,
        activeLink: peer.activeLink,
        lastError: peer.lastResolutionError,
        lastResolutionError: peer.lastResolutionError,
        lastResolutionAttemptAt: peer.lastResolutionAttemptAtMs,
      },
      peer.hubDerived ? "hub" : "announce",
    );
  }

  function applyPeerChanged(change: PeerChangedEvent["change"]): void {
    const destination = normalizeDestinationHex(change.destinationHex);
    if (!isValidDestinationHex(destination) || isLocalDestinationIdentityPair(destination, change.identityHex)) {
      return;
    }

    const saved = savedByDestination[destination];
    upsertDiscovered(
      destination,
      {
        identityHex: isValidDestinationHex(change.identityHex ?? "")
          ? normalizeDestinationHex(change.identityHex ?? "")
          : undefined,
        lxmfDestinationHex: isValidDestinationHex(change.lxmfDestinationHex ?? "")
          ? normalizeDestinationHex(change.lxmfDestinationHex ?? "")
          : undefined,
        announcedName: change.displayName?.trim() || undefined,
        label: saved?.label ?? discoveredByDestination[destination]?.label,
        appData: change.appData ?? discoveredByDestination[destination]?.appData,
        state: change.state ? toUiPeerState(change.state) : undefined,
        saved: change.saved,
        stale: change.stale,
        activeLink: change.activeLink,
        lastError: change.lastError,
        lastResolutionError: change.lastResolutionError,
        lastResolutionAttemptAt: change.lastResolutionAttemptAtMs,
        lastSeenAt: change.lastSeenAtMs,
        announceLastSeenAt: change.announceLastSeenAtMs,
        lxmfLastSeenAt: change.lxmfLastSeenAtMs,
      },
      change.hubDerived ? "hub" : "announce",
    );
  }

  function reconcileNativePeerSnapshot(peers: PeerRecord[]): void {
    const nativeDestinations = new Set(
      peers
        .map((peer) => normalizeDestinationHex(peer.destinationHex))
        .filter((destination) => isValidDestinationHex(destination)),
    );

    for (const [destination, peer] of Object.entries(discoveredByDestination)) {
      if (nativeDestinations.has(destination)) {
        continue;
      }

      const retainedSources = peer.sources.filter((source) => source === "import");
      if (retainedSources.length === 0) {
        delete discoveredByDestination[destination];
        continue;
      }

      discoveredByDestination[destination] = {
        ...peer,
        sources: retainedSources,
        identityHex: undefined,
        lxmfDestinationHex: undefined,
        announceLastSeenAt: undefined,
        lxmfLastSeenAt: undefined,
        state: peer.saved ? "connecting" : "disconnected",
        stale: false,
        activeLink: false,
        lastError: undefined,
        lastResolutionError: undefined,
      };
    }
  }

  function markPeerManagedState(destinationRaw: string, managed: boolean): void {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination) || isLocalPeerDestination(destination)) {
      return;
    }
    upsertDiscovered(destination, {
      saved: managed,
      state: managed ? "connecting" : "disconnected",
      activeLink: managed ? discoveredByDestination[destination]?.activeLink : false,
      lastError: undefined,
      lastResolutionError: undefined,
    });
  }

  async function settlePeerConnectionState(
    destinationRaw: string,
    target: "connected" | "disconnected",
  ): Promise<void> {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination) || !status.value.running) {
      return;
    }

    const deadline = nowMs() + 6_000;
    do {
      await refreshMessagingState();
      const peer = discoveredByDestination[destination];
      if (!peer) {
        return;
      }
      if (target === "connected" && peer.activeLink) {
        return;
      }
      if (target === "disconnected" && !peer.activeLink) {
        return;
      }
      await sleep(400);
    } while (nowMs() < deadline);
  }

  function describePeerState(destinationRaw: string): string {
    const destination = normalizeDestinationHex(destinationRaw);
    const peer = discoveredByDestination[destination];
    if (!peer) {
      return `destination=${destination} state=missing`;
    }

    return [
      `destination=${destination}`,
      `state=${peer.state}`,
      `saved=${peer.saved}`,
      `stale=${peer.stale}`,
      `activeLink=${peer.activeLink}`,
      `label=${peer.label ?? "-"}`,
      `announced=${peer.announcedName ?? "-"}`,
      `identity=${peer.identityHex ?? "-"}`,
      `lxmf=${peer.lxmfDestinationHex ?? "-"}`,
      `sources=${peer.sources.join("+") || "-"}`,
    ].join(" ");
  }

  function applyUiSettingsProjection(next: NodeUiPreferences): void {
    settings.clientMode = normalizeClientMode(next.clientMode);
  }

  function applySettingsProjection(next: NodeUiSettings): void {
    settings.displayName = next.displayName;
    settings.autoConnectSaved = next.autoConnectSaved;
    settings.announceCapabilities = next.announceCapabilities;
    settings.tcpClients = [...next.tcpClients];
    settings.broadcast = next.broadcast;
    settings.announceIntervalSeconds = next.announceIntervalSeconds;
    settings.telemetry = { ...next.telemetry };
    settings.hub = { ...next.hub };
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
    if (refreshSettingsPromise) {
      return refreshSettingsPromise;
    }
    refreshSettingsPromise = (async () => {
      const record = await client.value!.getAppSettings();
      if (record) {
        applySettingsProjection(
          normalizeAppSettingsRecord(
            record,
            loadUiSettingsProjection(DEFAULT_SETTINGS),
            defaultsWithTcpFallback(),
          ),
        );
      }
    })()
      .catch((error: unknown) => {
        appendLog("Debug", `Settings projection refresh skipped: ${errorMessage(error)}`);
      })
      .finally(() => {
        refreshSettingsPromise = null;
      });
    return refreshSettingsPromise;
  }

  async function refreshSavedPeersProjection(): Promise<void> {
    if (!client.value) {
      return;
    }
    if (refreshSavedPeersPromise) {
      return refreshSavedPeersPromise;
    }
    refreshSavedPeersPromise = (async () => {
      const peers = await client.value!.getSavedPeers();
      logSavedPeerProjectionDelta("native projection", peers);
      applySavedPeersProjection(peers);
    })()
      .catch((error: unknown) => {
        appendLog("Debug", `Saved-peer projection refresh skipped: ${errorMessage(error)}`);
      })
      .finally(() => {
        refreshSavedPeersPromise = null;
      });
    return refreshSavedPeersPromise;
  }

  async function refreshOperationalSummaryProjection(): Promise<void> {
    if (!client.value) {
      operationalSummary.value = { ...EMPTY_OPERATIONAL_SUMMARY };
      return;
    }
    if (refreshOperationalSummaryPromise) {
      return refreshOperationalSummaryPromise;
    }
    refreshOperationalSummaryPromise = (async () => {
      operationalSummary.value = await client.value!.getOperationalSummary();
    })()
      .catch((error: unknown) => {
        appendLog("Debug", `Operational summary refresh skipped: ${errorMessage(error)}`);
      })
      .finally(() => {
        refreshOperationalSummaryPromise = null;
      });
    return refreshOperationalSummaryPromise;
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
    await client.value.setAppSettings(toAppSettingsRecord(nextSettings));
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

    const completed = await client.value.legacyImportCompleted();
    if (!completed) {
      await client.value.importLegacyState(legacyState.payload);
    }
    clearLegacyProjectionStorage();
  }

  function recordLivePresence(
    destinationKind: "app" | "lxmf_delivery" | "lxmf_propagation" | "other",
    destinationHex: string,
    identityHex: string | undefined,
    receivedAtMs: number,
  ): void {
    if (destinationKind === "lxmf_propagation") {
      return;
    }

    if (destinationKind === "lxmf_delivery") {
      if (isValidDestinationHex(identityHex ?? "")) {
        const normalizedIdentity = normalizeDestinationHex(identityHex ?? "");
        liveLxmfPresenceByIdentity[normalizedIdentity] = Math.max(
          liveLxmfPresenceByIdentity[normalizedIdentity] ?? 0,
          receivedAtMs,
        );
        const appDestinationHex = appDestinationByIdentity[normalizedIdentity];
        if (isValidDestinationHex(appDestinationHex)) {
          livePresenceByDestination[appDestinationHex] = Math.max(
            livePresenceByDestination[appDestinationHex] ?? 0,
            receivedAtMs,
          );
        }
      }
      return;
    }

    if (!isValidDestinationHex(destinationHex)) {
      return;
    }
    livePresenceByDestination[destinationHex] = Math.max(
      livePresenceByDestination[destinationHex] ?? 0,
      receivedAtMs,
    );
    if (isValidDestinationHex(identityHex ?? "")) {
      const normalizedIdentity = normalizeDestinationHex(identityHex ?? "");
      const lxmfSeenAt = liveLxmfPresenceByIdentity[normalizedIdentity];
      if (typeof lxmfSeenAt === "number") {
        livePresenceByDestination[destinationHex] = Math.max(
          livePresenceByDestination[destinationHex],
          lxmfSeenAt,
        );
      }
    }
  }

  function shouldAutoConnectSavedPeer(destination: string): boolean {
    return autoConnectSavedPeerSkipReason(destination) === undefined;
  }

  function autoConnectSavedPeerSkipReason(destination: string): string | undefined {
    const normalizedDestination = normalizeDestinationHex(destination);
    if (!settings.autoConnectSaved || !status.value.running) {
      return "auto-connect disabled or node not running";
    }
    if (isLocalPeerDestination(normalizedDestination) || !savedByDestination[normalizedDestination]) {
      return "peer is local or not saved";
    }
    const peer = discoveredByDestination[normalizedDestination];
    if (peer?.state === "connecting") {
      return "peer is already connecting";
    }
    if (peer?.activeLink) {
      return "peer already has an active link";
    }
    if (autoConnectInFlight.has(normalizedDestination)) {
      return "connect already in flight";
    }
    return undefined;
  }

  function scheduleSavedPeerAutoConnect(destination: string, reason: string): void {
    const normalizedDestination = normalizeDestinationHex(destination);
    const skipReason = autoConnectSavedPeerSkipReason(normalizedDestination);
    if (skipReason) {
      appendLog(
        "Debug",
        `[peers] auto-connect not scheduled destination=${normalizedDestination} reason=${reason}: ${skipReason}.`,
      );
      return;
    }
    autoConnectInFlight.add(normalizedDestination);
    if (!autoConnectQueue.includes(normalizedDestination)) {
      autoConnectQueue.push(normalizedDestination);
    }
    void drainAutoConnectQueue(reason);
  }

  async function drainAutoConnectQueue(reason: string): Promise<void> {
    if (autoConnectQueueActive.value) {
      return;
    }
    autoConnectQueueActive.value = true;
    try {
      while (autoConnectQueue.length > 0) {
        const nextDestination = autoConnectQueue.shift();
        if (!nextDestination) {
          continue;
        }
        const skipReason = autoConnectSavedPeerSkipReason(nextDestination);
        if (skipReason) {
          appendLog(
            "Debug",
            `[peers] auto-connect cancelled destination=${nextDestination} reason=${reason}: ${skipReason}.`,
          );
          autoConnectInFlight.delete(nextDestination);
          continue;
        }
        await sleep(AUTO_CONNECT_SERIAL_DELAY_MS);
        try {
          await connectPeer(nextDestination);
          appendLog("Debug", `[peers] auto-connected saved peer ${nextDestination} after ${reason}.`);
        } catch (error: unknown) {
          appendLog(
            "Debug",
            `[peers] auto-connect skipped destination=${nextDestination} after ${reason}: ${errorMessage(error)}.`,
          );
        } finally {
          autoConnectInFlight.delete(nextDestination);
        }
      }
    } finally {
      autoConnectQueueActive.value = false;
    }
  }

  function queueEligibleSavedPeerAutoConnects(reason: string): void {
    for (const peer of Object.values(savedByDestination)) {
      if (savedByDestination[peer.destination]) {
        scheduleSavedPeerAutoConnect(peer.destination, reason);
      }
    }
  }

  function applyAnnounceUpdate(
    event: AnnounceReceivedEvent | AnnounceRecord,
    source: "live" | "snapshot" = "live",
  ): void {
    presenceNow.value = event.receivedAtMs;
    const identityHex = normalizeDestinationHex(event.identityHex ?? "");
    if (isLocalDestinationIdentityPair(event.destinationHex, identityHex)) {
      return;
    }
    if (source === "live") {
      recordLivePresence(
        event.destinationKind,
        normalizeDestinationHex(event.destinationHex),
        identityHex,
        event.receivedAtMs,
      );
    }
    if (event.destinationKind === "lxmf_propagation") {
      return;
    }
    if (event.destinationKind === "lxmf_delivery") {
      if (isValidDestinationHex(identityHex)) {
        lxmfDestinationByIdentity[identityHex] = event.destinationHex;
        const appDestinationHex = appDestinationByIdentity[identityHex];
        if (isValidDestinationHex(appDestinationHex)) {
          upsertDiscovered(appDestinationHex, {
            identityHex,
            lxmfDestinationHex: event.destinationHex,
            lxmfLastSeenAt: event.receivedAtMs,
            lastSeenAt: event.receivedAtMs,
          });
        }
      }
      return;
    }

    const saved = savedByDestination[event.destinationHex];
    const announcedName = extractAnnouncedName(event.appData)
      ?? ("displayName" in event && typeof event.displayName === "string"
        ? event.displayName.trim()
        : undefined);
    const capabilityText = extractAnnounceCapabilityText(event.appData);
    const knownLxmfDestination = isValidDestinationHex(identityHex)
      ? lxmfDestinationByIdentity[identityHex]
      : undefined;
    if (isValidDestinationHex(identityHex)) {
      appDestinationByIdentity[identityHex] = event.destinationHex;
    }
    upsertDiscovered(
      event.destinationHex,
      {
        identityHex: isValidDestinationHex(identityHex) ? identityHex : undefined,
        lxmfDestinationHex: isValidDestinationHex(knownLxmfDestination ?? "")
          ? knownLxmfDestination
          : undefined,
        lxmfLastSeenAt: isValidDestinationHex(knownLxmfDestination ?? "")
          ? event.receivedAtMs
          : undefined,
        announcedName,
        appData: capabilityText || undefined,
        hops: event.hops,
        interfaceHex: event.interfaceHex,
        label: saved?.label,
        announceLastSeenAt: event.receivedAtMs,
        lastSeenAt: event.receivedAtMs,
      },
      "announce",
    );
    if (source === "live") {
      scheduleSavedPeerAutoConnect(event.destinationHex, `${event.destinationKind} announce`);
    }
  }

  async function refreshAnnounceState(): Promise<void> {
    if (!client.value || !status.value.running) {
      return;
    }
    try {
      const announces = await client.value.listAnnounces();
      for (const announce of announces) {
        upsertNativeAnnounceRecord(announce);
        applyAnnounceUpdate(announce, "snapshot");
      }
    } catch (error: unknown) {
      appendLog("Debug", `Announce snapshot refresh skipped: ${errorMessage(error)}`);
    }
  }

  function scheduleDiscoveryRefresh(reason: string, delayMs = 2_000): void {
    window.setTimeout(() => {
      void refreshAnnounceState()
        .then(() => {
          appendLog("Debug", `[announce] refreshed discovery after ${reason}.`);
        })
        .catch(() => undefined);
    }, delayMs);
  }

  async function settleStartupDiscovery(reason: string): Promise<void> {
    if (!status.value.running) {
      return;
    }
    startupSettling.value = true;
    try {
      await sleep(STARTUP_ANNOUNCE_SETTLE_MS);
      await refreshMessagingState();
      queueEligibleSavedPeerAutoConnects(`${reason} settle`);
      await refreshMessagingState();
    } finally {
      startupSettling.value = false;
    }
  }

  async function resolvePeerIdentityIfNeeded(
    destinationRaw: string,
    reason: string,
  ): Promise<void> {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!client.value || !status.value.running || !isValidDestinationHex(destination)) {
      return;
    }
    if (isLocalPeerDestination(destination) || identityResolutionInFlight.has(destination)) {
      return;
    }

    const peer = discoveredByDestination[destination];
    if (
      peer
      && isValidDestinationHex(peer.identityHex ?? "")
      && isValidDestinationHex(peer.lxmfDestinationHex ?? "")
    ) {
      return;
    }

    identityResolutionInFlight.add(destination);
    try {
      logUi("Debug", `[peers] requesting identity destination=${destination} reason=${reason}.`);
      await client.value.requestPeerIdentity(destination);
      await Promise.allSettled([refreshMessagingState(), refreshAnnounceState()]);
    } catch (error: unknown) {
      appendLog(
        "Debug",
        `[peers] identity request failed destination=${destination} reason=${reason}: ${errorMessage(error)}.`,
      );
    } finally {
      identityResolutionInFlight.delete(destination);
    }
  }

  async function resolveSavedPeerIdentities(reason: string): Promise<void> {
    await Promise.allSettled(
      Object.values(savedByDestination).map((peer) =>
        resolvePeerIdentityIfNeeded(peer.destination, reason),
      ),
    );
  }

  function buildClient(): ReticulumNodeClient {
    if (runtimeProfile === "web") {
      return createReticulumNodeClient({
        mode: "web",
      });
    }
    return createReticulumNodeClient({
      mode: settings.clientMode,
    });
  }

  function currentHubBootstrapProfile(): HubRegistryBootstrapProfile | null {
    if (!hubModeUsesRch(settings.hub.mode)) {
      return null;
    }
    if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
      return null;
    }
    return buildHubRegistryBootstrapProfile({
      callsign: settings.displayName,
      localIdentityHex: status.value.identityHex,
      hubIdentityHash: settings.hub.identityHash,
    });
  }

  function setHubRegistrationPending(lastErrorValue?: string): void {
    hubRegistration.status = hubModeUsesRch(settings.hub.mode) ? "pending" : "disabled";
    if (lastErrorValue !== undefined) {
      hubRegistration.lastError = asTrimmedString(lastErrorValue);
    } else {
      hubRegistration.lastError = "";
    }
  }

  function setHubRegistrationReady(linkage: HubRegistryLinkage): void {
    hubRegistration.status = "ready";
    hubRegistration.linkage = { ...linkage };
    hubRegistration.lastReadyAt = nowMs();
    hubRegistration.lastError = "";
    saveHubRegistryLinkage(linkage);
  }

  function setHubRegistrationError(error: unknown): void {
    hubRegistration.status = hubModeUsesRch(settings.hub.mode) ? "error" : "disabled";
    hubRegistration.lastError = errorMessage(error);
    hubRegistration.lastAttemptAt = nowMs();
  }

  function clearHubRegistrationError(): void {
    if (hubRegistration.status === "error") {
      hubRegistration.status = "pending";
    }
    hubRegistration.lastError = "";
  }

  function reconcileHubRegistrationState(): void {
    if (!hubModeUsesRch(settings.hub.mode)) {
      hubRegistration.status = "disabled";
      hubRegistration.lastError = "";
      return;
    }

    if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
      setHubRegistrationPending(
        settings.hub.mode === "Connected"
          ? "Connected mode requires selecting an RCH hub before outbound traffic can be routed."
          : "Select an RCH hub to seed peer routing from the hub directory.",
      );
      return;
    }

    const storedLinkage = loadHubRegistryLinkage();
    hubRegistration.linkage = storedLinkage ?? undefined;

    if (!storedLinkage) {
      setHubRegistrationPending("Hub registry linkage has not been established yet.");
      return;
    }

    const profile = currentHubBootstrapProfile();
    if (!profile) {
      setHubRegistrationPending("Hub registry bootstrap is waiting on a node identity and hub destination.");
      return;
    }

    if (matchesHubRegistryProfile(storedLinkage, profile)) {
      hubRegistration.status = "ready";
      hubRegistration.lastError = "";
      hubRegistration.lastReadyAt = storedLinkage.updatedAt ?? nowMs();
      return;
    }

    setHubRegistrationPending("Stored hub linkage does not match the current callsign, team color, or identity.");
  }

  function buildHubRegistryTransport(): HubRegistryCommandTransport {
    return {
      sendCommand: async (destinationHex: string, command) => {
        await sendBytes(destinationHex, EMPTY_BYTES, {
          fieldsBase64: buildMissionCommandFieldsBase64([command]),
        });
      },
      onPacket: (listener) => client.value?.on("packetReceived", listener) ?? (() => undefined),
    };
  }

  async function bootstrapHubRegistration(force = false): Promise<void> {
    if (!hubModeUsesRch(settings.hub.mode)) {
      reconcileHubRegistrationState();
      return;
    }

    if (hubRegistryBootstrapInFlight && !force) {
      return hubRegistryBootstrapInFlight;
    }

    const profile = currentHubBootstrapProfile();
    if (!profile) {
      setHubRegistrationPending(
        "Hub registry bootstrap is waiting on a callsign, node identity, or hub destination.",
      );
      return;
    }

    const storedLinkage = loadHubRegistryLinkage();
    if (!force && storedLinkage && matchesHubRegistryProfile(storedLinkage, profile)) {
      setHubRegistrationReady(storedLinkage);
      return;
    }

    if (!status.value.running) {
      setHubRegistrationPending("Hub registry bootstrap will run after the node is started.");
      return;
    }

    clearHubRegistrationError();
    hubRegistration.lastAttemptAt = nowMs();
    hubRegistration.lastError = "";
    hubRegistration.status = "pending";

    const transport = buildHubRegistryTransport();
    const bootstrapPromise = bootstrapHubRegistry(profile, transport)
      .then((linkage) => {
        setHubRegistrationReady(linkage);
        appendLog(
          "Info",
          `Hub registry linkage ready: team=${linkage.teamUid} member=${linkage.teamMemberUid}.`,
        );
      })
      .catch((error: unknown) => {
        setHubRegistrationError(error);
        throw error;
      })
      .finally(() => {
        hubRegistryBootstrapInFlight = null;
      });

    hubRegistryBootstrapInFlight = bootstrapPromise;
    return bootstrapPromise;
  }

  async function refreshHubRegistrationState(attemptBootstrap = false): Promise<void> {
    reconcileHubRegistrationState();
    if (!attemptBootstrap || !hubModeUsesRch(settings.hub.mode)) {
      return;
    }

    const profile = currentHubBootstrapProfile();
    if (!profile || !status.value.running) {
      return;
    }

    const storedLinkage = loadHubRegistryLinkage();
    if (storedLinkage && matchesHubRegistryProfile(storedLinkage, profile)) {
      setHubRegistrationReady(storedLinkage);
      return;
    }

    await bootstrapHubRegistration();
  }

  async function configureClientLogging(): Promise<void> {
    if (!client.value || !status.value.running) {
      return;
    }
    try {
      await client.value.setLogLevel("Info");
      appendLog("Debug", "Node client log level set to Info.");
    } catch (error: unknown) {
      logUi("Warn", `Failed to set node log level: ${errorMessage(error)}`);
    }
  }

  function resetClientEventBindings(): void {
    for (const unsubscribe of unsubscribeClientEvents.value) {
      unsubscribe();
    }
    unsubscribeClientEvents.value = [];
  }

  function bindClientEvents(nodeClient: ReticulumNodeClient): void {
    resetClientEventBindings();
    unsubscribeClientEvents.value = [
      nodeClient.on("statusChanged", (event: StatusChangedEvent) => {
        status.value = normalizeNodeStatus(event.status);
        void refreshHubRegistrationState(event.status.running && hubModeUsesRch(settings.hub.mode));
      }),
      nodeClient.on("announceReceived", (event: AnnounceReceivedEvent) => {
        presenceNow.value = event.receivedAtMs;
        upsertNativeAnnounceRecord(event);
      }),
      nodeClient.on("peerChanged", (event: PeerChangedEvent) => {
        const destination = normalizeDestinationHex(event.change.destinationHex);
        if (isLocalDestinationIdentityPair(destination, event.change.identityHex)) {
          return;
        }
        presenceNow.value = nowMs();
        applyPeerChanged(event.change);
      }),
      nodeClient.on("peerResolved", (peer: PeerRecord) => {
        const destination = normalizeDestinationHex(peer.destinationHex);
        if (isLocalDestinationIdentityPair(destination, peer.identityHex)) {
          return;
        }
        presenceNow.value = peer.lastSeenAtMs;
        upsertResolvedPeer(peer);
      }),
      nodeClient.on("hubDirectoryUpdated", (event: HubDirectoryUpdatedEvent) => {
        presenceNow.value = event.receivedAtMs;
        hubDirectorySnapshot.value = {
          effectiveConnectedMode: event.effectiveConnectedMode,
          receivedAtMs: event.receivedAtMs,
          items: event.items.map((item) => ({
            ...item,
            announceCapabilities: [...item.announceCapabilities],
          })),
        };
        lastHubRefreshAt.value = event.receivedAtMs;
        void refreshMessagingState();
      }),
      nodeClient.on("operationalNotice", (event) => {
        appendNodeControlEntry(event.level, event.message, event.atMs);
      }),
      nodeClient.on("projectionInvalidated", (event: ProjectionInvalidationEvent) => {
        switch (event.scope) {
          case "AppSettings":
            void refreshSettingsProjection();
            break;
          case "SavedPeers":
            void refreshSavedPeersProjection();
            break;
          case "OperationalSummary":
            scheduleOperationalSummaryRefresh();
            break;
          default:
            break;
        }
      }),
      nodeClient.on("syncUpdated", (statusUpdate: SyncStatus) => {
        const previousRelay = activePropagationNodeHex(syncStatus.value);
        syncStatus.value = { ...statusUpdate };
        const nextRelay = activePropagationNodeHex(syncStatus.value);
        if (previousRelay !== nextRelay) {
          appendLog(
            "Debug",
            `[sync] propagation relay ${nextRelay ? `selected ${nextRelay}` : "cleared"}.`,
          );
        }
      }),
      nodeClient.on("log", (event: NodeLogEvent) => {
        appendLog(event.level, event.message);
      }),
      nodeClient.on("error", (event: NodeErrorEvent) => {
        lastError.value = `${event.code}: ${event.message}`;
        appendNodeControlEntry("Error", lastError.value);
      }),
    ];
  }

  async function refreshStatusSnapshot(
    retries = 1,
    delayMs = 250,
  ): Promise<NodeStatus> {
    if (!client.value) {
      return { ...EMPTY_STATUS };
    }

    let latest: NodeStatus = { ...EMPTY_STATUS };
    for (let attempt = 0; attempt < retries; attempt += 1) {
      try {
        latest = normalizeNodeStatus(await client.value.getStatus());
        status.value = { ...latest };
        if (latest.running || attempt === retries - 1) {
          return latest;
        }
      } catch {
        if (attempt === retries - 1) {
          status.value = { ...EMPTY_STATUS };
          return { ...EMPTY_STATUS };
        }
      }

      await sleep(delayMs);
    }

    return latest;
  }

  async function refreshMessagingState(): Promise<void> {
    if (!client.value || !status.value.running) {
      syncStatus.value = { ...EMPTY_SYNC_STATUS };
      telemetryDestinations.value = [];
      return;
    }

    if (refreshMessagingStatePromise) {
      return refreshMessagingStatePromise;
    }

    refreshMessagingStatePromise = (async () => {
      const [peers, nextSyncStatus, nextTelemetryDestinations] = await Promise.all([
        client.value!.listPeers(),
        client.value!.getLxmfSyncStatus(),
        client.value!.listTelemetryDestinations(),
      ]);
      reconcileNativePeerSnapshot(peers);
      for (const peer of peers) {
        upsertResolvedPeer(peer);
      }
      syncStatus.value = { ...nextSyncStatus };
      telemetryDestinations.value = [...nextTelemetryDestinations];
    })()
      .catch((error: unknown) => {
        appendLog("Debug", `Messaging projection refresh skipped: ${errorMessage(error)}`);
      })
      .finally(() => {
        refreshMessagingStatePromise = null;
      });

    return refreshMessagingStatePromise;
  }

  async function init(): Promise<void> {
    if (initPromise) {
      return initPromise;
    }
    if (initialized.value) {
      return;
    }

    initPromise = (async () => {
      client.value = buildClient();
      bindClientEvents(client.value);
      await importLegacyProjectionState();
      await Promise.all([
        refreshSettingsProjection(),
        refreshSavedPeersProjection(),
        refreshOperationalSummaryProjection(),
      ]);
      if (presenceTickerId === null) {
        presenceTickerId = window.setInterval(() => {
          presenceNow.value = nowMs();
        }, PEER_PRESENCE_TICK_MS);
      }
      await refreshHubRegistrationState(false);
      initialized.value = true;
    })()
      .finally(() => {
        initPromise = null;
      });

    return initPromise;
  }

  async function startNode(): Promise<void> {
    try {
      await init();
      if (!client.value) {
        return;
      }

      clearLastError();
      await client.value.start(toNodeConfig(settings));
      await refreshStatusSnapshot(8, 250);
      await refreshMessagingState();
      await refreshAnnounceState();
      await refreshOperationalSummaryProjection();
      await configureClientLogging();
      await settleStartupDiscovery("startup");
      await refreshHubRegistrationState(true);
      appendNodeControlEntry("Info", "Node started.");

      if (hubModeUsesRch(settings.hub.mode)) {
        await refreshHubDirectory().catch((error: unknown) => {
          appendNodeControlEntry("Warn", `Hub refresh failed after start: ${errorMessage(error)}`);
        });
      }
    } catch (error: unknown) {
      throw captureActionError("Start node failed", error);
    }
  }

  async function stopNode(): Promise<void> {
    try {
      if (!client.value) {
        return;
      }
      clearLastError();
      await client.value.stop();
      appendNodeControlEntry("Info", "Node stopped.");
      syncStatus.value = { ...EMPTY_SYNC_STATUS };
      clearAnnounceState();
      await refreshOperationalSummaryProjection();
      await refreshHubRegistrationState(false);

      for (const destination of Object.keys(discoveredByDestination)) {
        setPeerState(destination, "disconnected");
      }
    } catch (error: unknown) {
      throw captureActionError("Stop node failed", error);
    }
  }

  async function restartNode(): Promise<void> {
    try {
      await init();
      if (!client.value) {
        return;
      }
      clearLastError();
      await client.value.restart(toNodeConfig(settings));
      await refreshStatusSnapshot(8, 250);
      await refreshMessagingState();
      await refreshAnnounceState();
      await refreshOperationalSummaryProjection();
      await configureClientLogging();
      await settleStartupDiscovery("restart");
      await refreshHubRegistrationState(true);
      appendNodeControlEntry("Info", "Node restarted with updated settings.");

      if (hubModeUsesRch(settings.hub.mode)) {
        await refreshHubDirectory().catch((error: unknown) => {
          appendNodeControlEntry("Warn", `Hub refresh failed after restart: ${errorMessage(error)}`);
        });
      }
    } catch (error: unknown) {
      throw captureActionError("Restart node failed", error);
    }
  }

  async function connectPeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      const message = `Invalid peer destination: ${destinationRaw}.`;
      appendLog("Debug", `[peers] connect blocked invalid-destination raw=${destinationRaw}.`);
      throw new Error(message);
    }
    if (!client.value) {
      const message = "Node client unavailable. Reinitialize the app and try again.";
      appendLog("Debug", `[peers] connect blocked destination=${destination}: client unavailable.`);
      throw new Error(message);
    }
    if (!status.value.running) {
      const message = "Start node before connecting to a peer.";
      appendLog("Debug", `[peers] connect blocked destination=${destination}: node not running.`);
      throw new Error(message);
    }
    if (isLocalPeerDestination(destination)) {
      const message = `Cannot connect to local destination ${destination}.`;
      appendLog("Debug", `[peers] connect blocked self destination=${destination}.`);
      throw new Error(message);
    }
    const savedPeer = savedByDestination[destination];
    const existingPeer = discoveredByDestination[destination];
    if (!savedPeer && !existingPeer?.saved) {
      throw new Error(`Save peer ${destination} before connecting.`);
    }

    try {
      clearLastError();
      logUi("Debug", `[peers] connect requested ${describePeerState(destination)}.`);
      const connectPromise = client.value.connectPeer(destination);
      markPeerManagedState(destination, true);
      await connectPromise;
      void settlePeerConnectionState(destination, "connected");
    } catch (error: unknown) {
      const message = errorMessage(error);
      setPeerState(destination, "disconnected", message);
      throw captureActionError(`Connect peer failed (${destination})`, error);
    }
  }

  async function disconnectPeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      const message = `Invalid peer destination: ${destinationRaw}.`;
      appendLog("Debug", `[peers] disconnect blocked invalid-destination raw=${destinationRaw}.`);
      throw new Error(message);
    }
    if (!client.value) {
      const message = "Node client unavailable. Reinitialize the app and try again.";
      appendLog("Debug", `[peers] disconnect blocked destination=${destination}: client unavailable.`);
      throw new Error(message);
    }
    if (!status.value.running) {
      const message = "Start node before disconnecting a peer.";
      appendLog("Debug", `[peers] disconnect blocked destination=${destination}: node not running.`);
      throw new Error(message);
    }
    try {
      clearLastError();
      logUi("Debug", `[peers] disconnect requested ${describePeerState(destination)}.`);
      const disconnectPromise = client.value.disconnectPeer(destination);
      markPeerManagedState(destination, false);
      await disconnectPromise;
      await settlePeerConnectionState(destination, "disconnected");
      logUi("Debug", `[peers] disconnect applied ${describePeerState(destination)}.`);
    } catch (error: unknown) {
      throw captureActionError(`Disconnect peer failed (${destination})`, error);
    }
  }

  async function connectAllSaved(): Promise<void> {
    const results = await Promise.allSettled(
      Object.values(savedByDestination).map((peer) => connectPeer(peer.destination)),
    );
    const failures = results
      .filter((result): result is PromiseRejectedResult => result.status === "rejected")
      .map((result) => errorMessage(result.reason));
    if (failures.length > 0) {
      throw new Error(failures.join("; "));
    }
  }

  async function disconnectAllSaved(): Promise<void> {
    const results = await Promise.allSettled(
      Object.values(savedByDestination).map((peer) => disconnectPeer(peer.destination)),
    );
    const failures = results
      .filter((result): result is PromiseRejectedResult => result.status === "rejected")
      .map((result) => errorMessage(result.reason));
    if (failures.length > 0) {
      throw new Error(failures.join("; "));
    }
  }

  async function refreshHubDirectory(): Promise<void> {
    try {
      if (!hubModeUsesRch(settings.hub.mode)) {
        clearHubDirectoryState();
        return;
      }
      if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
        clearHubDirectoryState();
        if (settings.hub.mode === "Connected") {
          throw new Error("Connected mode requires selecting an RCH hub before refreshing.");
        }
        return;
      }
      if (!client.value || !status.value.running) {
        return;
      }
      clearLastError();
      await client.value.refreshHubDirectory();
    } catch (error: unknown) {
      throw captureActionError("Hub directory refresh failed", error);
    }
  }

  async function forgetHubRegistryLinkage(): Promise<void> {
    clearHubRegistryLinkage();
    hubRegistration.linkage = undefined;
    hubRegistration.lastReadyAt = undefined;
    setHubRegistrationPending("Hub registry linkage cleared.");
  }

  async function setAnnounceCapabilities(capabilityString: string): Promise<void> {
    settings.announceCapabilities = ensureRequiredAnnounceCapabilities(capabilityString);
    const nextSettings = normalizeAppSettingsRecord(
      toAppSettingsRecord(settings),
      toUiSettingsProjection(settings),
      defaultsWithTcpFallback(),
    );
    await init();
    await persistSettingsProjection(nextSettings);

    if (!client.value || !status.value.running) {
      return;
    }
    try {
      clearLastError();
      await client.value.setAnnounceCapabilities(settings.announceCapabilities);
    } catch (error: unknown) {
      throw captureActionError("Set announce capabilities failed", error);
    }
  }

  async function savePeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return;
    }
    const discovered = discoveredByDestination[destination];
    const nextSavedPeers = {
      ...savedByDestination,
      [destination]: {
        destination,
        label: discovered?.label,
        savedAt: nowMs(),
      },
    };
    await persistSavedPeersProjection(nextSavedPeers, `explicit save ${destination}`);
  }

  async function unsavePeer(destinationRaw: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    const nextSavedPeers = { ...savedByDestination };
    delete nextSavedPeers[destination];
    await persistSavedPeersProjection(nextSavedPeers, `explicit unsave ${destination}`);
  }

  async function setPeerLabel(destinationRaw: string, label: string): Promise<void> {
    await init();
    const destination = normalizeDestinationHex(destinationRaw);
    const normalizedLabel = label.trim();
    if (savedByDestination[destination]) {
      const nextSavedPeers = {
        ...savedByDestination,
        [destination]: {
          ...savedByDestination[destination],
          label: normalizedLabel || undefined,
        },
      };
      await persistSavedPeersProjection(nextSavedPeers, `label update ${destination}`);
    }
    if (discoveredByDestination[destination]) {
      discoveredByDestination[destination].label = normalizedLabel || undefined;
    }
  }

  function updateSettings(next: Partial<NodeUiSettings>): void {
    let uiSettingsChanged = false;
    let hubRoutingChanged = false;
    if (next.displayName !== undefined) {
      settings.displayName = normalizeStoredDisplayName(next.displayName);
    }
    if (next.clientMode) {
      settings.clientMode = next.clientMode;
      uiSettingsChanged = true;
    }
    if (typeof next.autoConnectSaved === "boolean") {
      settings.autoConnectSaved = next.autoConnectSaved;
    }
    if (next.announceCapabilities !== undefined) {
      settings.announceCapabilities = ensureRequiredAnnounceCapabilities(next.announceCapabilities);
    }
    if (next.tcpClients !== undefined) {
      settings.tcpClients = normalizeTcpCommunityClients(next.tcpClients, defaultsWithTcpFallback());
    }
    if (typeof next.broadcast === "boolean") {
      settings.broadcast = next.broadcast;
    }
    if (next.announceIntervalSeconds !== undefined) {
      settings.announceIntervalSeconds = next.announceIntervalSeconds;
    }
    if (next.telemetry) {
      settings.telemetry = normalizeTelemetrySettings(next.telemetry, settings.telemetry);
    }
    if (next.hub) {
      const previousHubMode = settings.hub.mode;
      const previousHubIdentityHash = settings.hub.identityHash;
      settings.hub = {
        ...settings.hub,
        ...next.hub,
        mode: normalizeHubMode(next.hub.mode ?? settings.hub.mode),
      };
      hubRoutingChanged =
        settings.hub.mode !== previousHubMode
        || settings.hub.identityHash !== previousHubIdentityHash;
      if (
        !hubModeUsesRch(settings.hub.mode)
        || settings.hub.mode !== previousHubMode
        || settings.hub.identityHash !== previousHubIdentityHash
      ) {
        clearHubDirectoryState();
      }
    }
    const nextSettings = normalizeAppSettingsRecord(
      toAppSettingsRecord(settings),
      toUiSettingsProjection(settings),
      defaultsWithTcpFallback(),
    );
    if (uiSettingsChanged) {
      storeUiSettingsProjection(toUiSettingsProjection(settings));
    }
    void init()
      .then(() => persistSettingsProjection(nextSettings))
      .then(() => {
        if (!hubRoutingChanged || !status.value.running || !hubModeUsesRch(settings.hub.mode)) {
          return;
        }
        if (!hasSelectedHubIdentity(settings.hub.identityHash)) {
          if (settings.hub.mode === "Connected") {
            const message =
              "Connected mode requires selecting an RCH hub before outbound traffic can be routed.";
            lastError.value = message;
            appendLog("Warn", message);
          }
          return;
        }
        appendLog(
          "Info",
          "Hub routing settings changed. Restart the node to apply the selected hub and refresh from the hub directory.",
        );
      })
      .catch((error: unknown) => {
        appendLog("Warn", `Settings projection persist failed: ${errorMessage(error)}`);
      });
    void refreshHubRegistrationState(hubModeUsesRch(settings.hub.mode));
  }

  function getSavedPeerList(): PeerListV1 {
    return createPeerListV1(Object.values(savedByDestination));
  }

  function importPeerList(
    peerList: PeerListV1,
    mode: "merge" | "replace" = "merge",
  ): void {
    if (mode === "replace") {
      for (const key of Object.keys(savedByDestination)) {
        delete savedByDestination[key];
      }
    }

    for (const peer of peerList.peers) {
      const destination = normalizeDestinationHex(peer.destination);
      if (!isValidDestinationHex(destination)) {
        continue;
      }
      savedByDestination[destination] = {
        destination,
        label: peer.label?.trim() || undefined,
        savedAt: nowMs(),
      };
      upsertDiscovered(
        destination,
        {
          label: peer.label?.trim() || undefined,
          saved: true,
          lastSeenAt: discoveredByDestination[destination]?.lastSeenAt ?? 0,
        },
        "import",
      );
    }
    void init()
      .then(() => persistSavedPeersProjection({ ...savedByDestination }, `peer list import (${mode})`))
      .catch((error: unknown) => {
        appendLog("Warn", `Saved-peer projection persist failed: ${errorMessage(error)}`);
      });
  }

  function parsePeerListText(text: string): ReturnType<typeof parsePeerListV1> {
    return parsePeerListV1(text);
  }

  function hasFreshPresence(lastSeenAt?: number): boolean {
    return typeof lastSeenAt === "number"
      && Number.isFinite(lastSeenAt)
      && (presenceNow.value - lastSeenAt) <= PEER_ONLINE_FRESHNESS_MS;
  }

  function peerPresenceTimestamp(
    peer: Pick<DiscoveredPeer, "lastSeenAt">,
  ): number | undefined {
    const seenAt = peer.lastSeenAt ?? 0;
    return seenAt > 0 ? seenAt : undefined;
  }

  function peerCachedPresenceTimestamp(
    peer: Pick<DiscoveredPeer, "announceLastSeenAt" | "lxmfLastSeenAt" | "lastSeenAt">,
  ): number | undefined {
    const announceSeenAt = typeof peer.announceLastSeenAt === "number" ? peer.announceLastSeenAt : 0;
    const lxmfSeenAt = typeof peer.lxmfLastSeenAt === "number" ? peer.lxmfLastSeenAt : 0;
    const seenAt = Math.max(announceSeenAt, lxmfSeenAt, peer.lastSeenAt ?? 0);
    return seenAt > 0 ? seenAt : undefined;
  }

  function peerDisplayState(peer: Pick<DiscoveredPeer, "state">): PeerConnectionState {
    return peer.state;
  }

  function peerIsSaved(
    peer: Pick<DiscoveredPeer, "destination" | "saved">,
    savedDestinations: Set<string>,
  ): boolean {
    return peer.saved || savedDestinations.has(peer.destination);
  }

  function peerHasConnectedSession(
    peer: Pick<DiscoveredPeer, "destination" | "activeLink" | "saved">,
    savedDestinations: Set<string>,
  ): boolean {
    return peerIsSaved(peer, savedDestinations) && peer.activeLink;
  }

  function peerPresenceState(
    peer: Pick<DiscoveredPeer, "activeLink">,
  ): "online" | "offline" {
    return peer.activeLink ? "online" : "offline";
  }

  function peerHasKnownLxmfRoute(
    peer: Pick<DiscoveredPeer, "destination" | "lxmfDestinationHex">,
  ): boolean {
    const appDestinationHex = normalizeDestinationHex(peer.destination);
    const lxmfDestinationHex = normalizeDestinationHex(peer.lxmfDestinationHex ?? "");
    return isValidDestinationHex(appDestinationHex)
      && isValidDestinationHex(lxmfDestinationHex)
      && appDestinationHex !== lxmfDestinationHex;
  }

  function peerByAnyKnownDestination(
    peers: Record<string, DiscoveredPeer>,
    destinationRaw: string,
  ): DiscoveredPeer | undefined {
    const destinationHex = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destinationHex)) {
      return undefined;
    }
    return Object.values(peers).find((peer) =>
      destinationHex === normalizeDestinationHex(peer.destination)
        || destinationHex === normalizeDestinationHex(peer.lxmfDestinationHex ?? "")
        || destinationHex === normalizeDestinationHex(peer.identityHex ?? ""),
    );
  }

  const discoveredPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => shouldDisplayDiscoveredPeer(peer))
      .filter((peer) => !isLocalPeer(peer))
      .sort((a, b) => {
        const byRank = peerSortRank(b) - peerSortRank(a);
        if (byRank !== 0) {
          return byRank;
        }
        return b.lastSeenAt - a.lastSeenAt;
      }),
  );
  const allPeers = discoveredPeers;

  const autoFanoutPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => !isLocalPeer(peer))
      .filter((peer) => peerIsSaved(peer, savedDestinations.value))
      .filter((peer) => peerHasKnownLxmfRoute(peer))
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const propagationEligibleEventPeerRoutes = computed<EventPeerRoute[]>(() =>
    (!bestPropagationNodeHex.value ? [] : autoFanoutPeers.value)
      .filter((peer) => !peer.activeLink)
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt)
      .map((peer) => ({
        appDestinationHex: peer.destination,
        lxmfDestinationHex: peer.lxmfDestinationHex!,
        identityHex: peer.identityHex,
        label: peer.label,
        announcedName: peer.announcedName,
        sendMode: "Auto",
      })),
  );

  const savedPeers = computed(() =>
    Object.values(savedByDestination).sort((a, b) => b.savedAt - a.savedAt),
  );

  const savedVisiblePeers = computed(() =>
    discoveredPeers.value.filter((peer) => peerIsSaved(peer, savedDestinations.value)),
  );

  const connectedPeers = computed(() =>
    savedVisiblePeers.value.filter((peer) => peer.activeLink),
  );

  const connectedDestinations = computed(() =>
    connectedPeers.value.map((peer) => peer.destination),
  );

  const intentionalPeerDestinations = computed(() =>
    savedVisiblePeers.value.map((peer) => peer.destination),
  );

  const connectedLinkDestinations = computed(() =>
    connectedPeers.value.map((peer) => peer.destination),
  );

  const connectedEventPeerRoutes = computed<EventPeerRoute[]>(() =>
    connectedPeers.value
      .filter((peer) => peerHasKnownLxmfRoute(peer))
      .map((peer) => ({
        appDestinationHex: peer.destination,
        lxmfDestinationHex: peer.lxmfDestinationHex!,
        identityHex: peer.identityHex,
        label: peer.label,
        announcedName: peer.announcedName,
        sendMode: "Auto",
      })),
  );

  const visiblePeerCount = computed(() => discoveredPeers.value.length);
  const savedPeerCount = computed(() => savedVisiblePeers.value.length);
  const connectedPeerCount = computed(() => connectedPeers.value.length);
  const propagationCandidateDestinations = computed(() =>
    activePropagationNodeHex(syncStatus.value)
      ? [activePropagationNodeHex(syncStatus.value)!]
      : [],
  );
  const bestPropagationNodeHex = computed(() => activePropagationNodeHex(syncStatus.value));
  const hubDirectoryPeers = computed(() => hubDirectorySnapshot.value?.items ?? []);
  const effectiveConnectedMode = computed(() => Boolean(hubDirectorySnapshot.value?.effectiveConnectedMode));
  const hubAnnounceCandidates = computed<HubAnnounceCandidate[]>(() => {
    const byIdentity = new Map<string, HubAnnounceCandidate & { receivedAtMs: number }>();
    for (const announce of Object.values(announceByDestination)) {
      if (announce.announceClass !== "RchHubServer") {
        continue;
      }
      const identity = isValidDestinationHex(announce.identityHex)
        ? announce.identityHex
        : announce.destinationHex;
      const candidate = {
        destination: identity,
        label: announce.displayName || identity,
        receivedAtMs: announce.receivedAtMs,
      };
      const existing = byIdentity.get(identity);
      if (!existing || existing.receivedAtMs < announce.receivedAtMs) {
        byIdentity.set(identity, candidate);
      }
    }
    return [...byIdentity.values()]
      .map(({ destination, label }) => ({ destination, label }))
      .sort((left, right) => {
        const byLabel = left.label.localeCompare(right.label);
        if (byLabel !== 0) {
          return byLabel;
        }
        return left.destination.localeCompare(right.destination);
      });
  });

  const savedDestinations = computed(() => new Set(savedPeers.value.map((peer) => peer.destination)));
  const ready = computed(() => status.value.running);
  const hubBootstrapProfile = computed(() => currentHubBootstrapProfile());
  const hubRegistrationReady = computed(
    () => hubRegistration.status === "ready" && Boolean(hubRegistration.linkage),
  );
  const hubRegistrationPending = computed(() => hubRegistration.status === "pending");
  const hubRegistrationSummary = computed(() => {
    const lastHubError = asTrimmedString(hubRegistration.lastError);
    switch (hubRegistration.status) {
      case "disabled":
        return "Hub sync disabled";
      case "ready":
        if (!hubRegistration.linkage) {
          return "Hub registration ready";
        }
        return `Ready | team=${hubRegistration.linkage.teamUid.slice(0, 10)}... member=${hubRegistration.linkage.teamMemberUid.slice(0, 10)}...`;
      case "error":
        return lastHubError
          ? `Error | ${lastHubError}`
          : "Hub registration error";
      case "pending":
      default:
        return lastHubError
          ? `Pending | ${lastHubError}`
          : "Pending hub registration";
    }
  });

  async function syncAutoPropagationNode(reason: string): Promise<void> {
    void reason;
  }

  function notReadyMessage(action: string): string {
    return `Cannot ${action} until the node is ready. Wait for the top-right status to show Ready.`;
  }

  function assertReadyForOutbound(action: string): void {
    if (ready.value) {
      return;
    }

    const message = notReadyMessage(action);
    logUi(
      "Debug",
      `[ready] blocked outbound action=${action} running=${status.value.running} initialized=${initialized.value}.`,
    );
    lastError.value = message;
    logUi("Warn", message);
    throw new Error(message);
  }

  function assertHubRoutingReadyForOutbound(action: string): void {
    if (settings.hub.mode !== "Connected") {
      return;
    }
    if (hasSelectedHubIdentity(settings.hub.identityHash)) {
      return;
    }

    const message = `Cannot ${action} until a connected-mode RCH hub is selected.`;
    lastError.value = message;
    logUi("Warn", message);
    throw new Error(message);
  }

  function destinationHasCapability(destinationRaw: string, capability: string): boolean {
    const destination = normalizeDestinationHex(destinationRaw);
    const peer = discoveredByDestination[destination];
    if (!peer || !peer.sources.includes("announce")) {
      return false;
    }
    return hasCapability(peer.appData ?? "", capability);
  }

  async function broadcastBytes(bytes: Uint8Array, options?: PacketSendOptions): Promise<void> {
    if (!client.value) {
      throw captureActionError("Broadcast failed", new Error("Node client is not initialized."));
    }
    try {
      logUi(
        "Debug",
        `Broadcast requested bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"}.`,
      );
      await client.value.broadcastBytes(bytes, options);
    } catch (error: unknown) {
      throw captureActionError("Broadcast failed", error);
    }
  }

  async function sendBytes(
    destinationHex: string,
    bytes: Uint8Array,
    options?: PacketSendOptions,
  ): Promise<void> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `Send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertHubRoutingReadyForOutbound("send traffic");
      const matchedPeer = peerByAnyKnownDestination(discoveredByDestination, destinationHex);
      const sendMode = options?.sendMode ?? "Auto";
      logUi(
        "Debug",
        `Send requested destination=${destinationHex} bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"} mode=${sendMode}${matchedPeer ? ` peer=${matchedPeer.label ?? matchedPeer.destination}` : ""}.`,
      );
      await nodeClient.sendBytes(destinationHex, bytes, {
        ...options,
        sendMode,
      });
      logUi(
        "Debug",
        `Send handed to native transport destination=${destinationHex} bytes=${bytes.byteLength} mode=${sendMode}.`,
      );
    } catch (error: unknown) {
      throw captureActionError(`Send failed (${destinationHex})`, error);
    }
  }

  async function sendBytesDirect(
    destinationHex: string,
    bytes: Uint8Array,
    options?: PacketSendOptions,
  ): Promise<void> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `Direct send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertHubRoutingReadyForOutbound("send traffic");
      logUi(
        "Debug",
        `Direct send requested destination=${destinationHex} bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"}.`,
      );
      await nodeClient.sendBytes(destinationHex, bytes, {
        ...options,
        sendMode: "DirectOnly",
      });
      logUi(
        "Debug",
        `Direct send handed to native transport destination=${destinationHex} bytes=${bytes.byteLength}.`,
      );
    } catch (error: unknown) {
      throw captureActionError(`Direct send failed (${destinationHex})`, error);
    }
  }

  async function sendBytesViaPropagation(
    destinationHex: string,
    bytes: Uint8Array,
    options?: PacketSendOptions,
  ): Promise<void> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `Propagation send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertHubRoutingReadyForOutbound("send traffic");
      logUi(
        "Debug",
        `Propagation send requested destination=${destinationHex} bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"}.`,
      );
      await nodeClient.sendBytes(destinationHex, bytes, {
        ...options,
        sendMode: "PropagationOnly",
      });
      logUi(
        "Debug",
        `Propagation send handed to native transport destination=${destinationHex} bytes=${bytes.byteLength}.`,
      );
    } catch (error: unknown) {
      throw captureActionError(`Propagation send failed (${destinationHex})`, error);
    }
  }

  async function sendLxmf(
    destinationHex: string,
    bodyUtf8: string,
    title?: string,
    options?: {
      sendMode?: SendMode;
    },
  ): Promise<string> {
    const nodeClient = client.value;
    if (!nodeClient) {
      throw captureActionError(
        `LXMF send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      assertHubRoutingReadyForOutbound("send LXMF");
      const matchedPeer = peerByAnyKnownDestination(discoveredByDestination, destinationHex);
      const sendMode = options?.sendMode ?? "Auto";
      logUi(
        "Debug",
        `LXMF send requested destination=${destinationHex} bytes=${new TextEncoder().encode(bodyUtf8).byteLength} mode=${sendMode}${matchedPeer ? ` peer=${matchedPeer.label ?? matchedPeer.destination}` : ""}.`,
      );
      return await nodeClient.sendLxmf({
        destinationHex,
        bodyUtf8,
        title,
        sendMode,
      });
    } catch (error: unknown) {
      throw captureActionError(`LXMF send failed (${destinationHex})`, error);
    }
  }

  function requireClient(action: string): ReticulumNodeClient {
    if (!client.value) {
      throw captureActionError(action, new Error("Node client is not initialized."));
    }
    return client.value;
  }

  function onClientEvent<K extends keyof NodeClientEvents>(
    event: K,
    handler: (payload: NodeClientEvents[K]) => void,
  ): () => void {
    return client.value?.on(event, handler) ?? (() => undefined);
  }

  async function getSosSettings() {
    return requireClient("Get SOS settings failed").getSosSettings();
  }

  async function setSosSettings(settingsRecord: Parameters<ReticulumNodeClient["setSosSettings"]>[0]): Promise<void> {
    await requireClient("Set SOS settings failed").setSosSettings(settingsRecord);
  }

  async function setSosPin(pin?: string): Promise<void> {
    await requireClient("Set SOS PIN failed").setSosPin(pin);
  }

  async function getSosStatus() {
    return requireClient("Get SOS status failed").getSosStatus();
  }

  async function triggerSos(source?: Parameters<ReticulumNodeClient["triggerSos"]>[0]) {
    return requireClient("Trigger SOS failed").triggerSos(source);
  }

  async function deactivateSos(pin?: string) {
    return requireClient("Deactivate SOS failed").deactivateSos(pin);
  }

  async function submitSosTelemetry(telemetry: Parameters<ReticulumNodeClient["submitSosTelemetry"]>[0]): Promise<void> {
    await requireClient("Submit SOS telemetry failed").submitSosTelemetry(telemetry);
  }

  async function listSosAlerts() {
    return requireClient("List SOS alerts failed").listSosAlerts();
  }

  async function listSosLocations() {
    return requireClient("List SOS locations failed").listSosLocations();
  }

  async function listSosAudio() {
    return requireClient("List SOS audio failed").listSosAudio();
  }

  async function announceNow(): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.announceNow();
    } catch (error: unknown) {
      throw captureActionError("Announce now failed", error);
    }
  }

  async function requestPeerIdentity(destinationHex: string): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.requestPeerIdentity(destinationHex);
    } catch (error: unknown) {
      throw captureActionError(`Peer identity request failed (${destinationHex})`, error);
    }
  }

  async function setActivePropagationNode(destinationHex?: string): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.setActivePropagationNode(destinationHex);
    } catch (error: unknown) {
      throw captureActionError("Set active propagation node failed", error);
    }
  }

  async function requestLxmfSync(limit?: number): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      await client.value.requestLxmfSync(limit);
    } catch (error: unknown) {
      throw captureActionError("LXMF sync request failed", error);
    }
  }

  async function broadcastJson(payload: unknown, dedicatedFields?: DedicatedFields): Promise<void> {
    const body = new TextEncoder().encode(JSON.stringify(payload));
    await broadcastBytes(body, { dedicatedFields });
  }

  async function sendJson(
    destinationHex: string,
    payload: unknown,
    dedicatedFields?: DedicatedFields,
  ): Promise<void> {
    const body = new TextEncoder().encode(JSON.stringify(payload));
    await sendBytes(destinationHex, body, { dedicatedFields });
  }

  async function reinitializeClient(): Promise<void> {
    try {
      clearLastError();
      if (client.value) {
        await client.value.dispose().catch(() => undefined);
      }
      client.value = buildClient();
      bindClientEvents(client.value);
      await configureClientLogging();
      status.value = { ...EMPTY_STATUS };
      clearAnnounceState();
      await Promise.all([
        refreshSettingsProjection(),
        refreshSavedPeersProjection(),
        refreshOperationalSummaryProjection(),
      ]);
      await refreshHubRegistrationState(false);
      appendLog("Info", "Node client recreated.");
    } catch (error: unknown) {
      throw captureActionError("Recreate client failed", error);
    }
  }

  return {
    settings,
    status,
    syncStatus,
    operationalSummary,
    announceByDestination,
    hubDirectorySnapshot,
    hubDirectoryPeers,
    hubAnnounceCandidates,
    effectiveConnectedMode,
    hubRegistration,
    hubBootstrapProfile,
    hubRegistrationReady,
    hubRegistrationPending,
    hubRegistrationSummary,
    logs,
    nodeControlEntries,
    lastError,
    lastHubRefreshAt,
    discoveredByDestination,
    savedByDestination,
    allPeers,
    discoveredPeers,
    savedPeers,
    savedVisiblePeers,
    connectedPeers,
    propagationEligibleEventPeerRoutes,
    connectedDestinations,
    intentionalPeerDestinations,
    connectedLinkDestinations,
    connectedEventPeerRoutes,
    visiblePeerCount,
    savedPeerCount,
    connectedPeerCount,
    startupSettling,
    bestPropagationNodeHex,
    telemetryDestinations,
    savedDestinations,
    ready,
    peerDisplayState,
    peerPresenceTimestamp,
    peerCachedPresenceTimestamp,
    peerPresenceState,
    init,
    startNode,
    stopNode,
    restartNode,
    connectPeer,
    disconnectPeer,
    connectAllSaved,
    disconnectAllSaved,
    refreshHubDirectory,
    refreshHubRegistrationState,
    bootstrapHubRegistration,
    forgetHubRegistryLinkage,
    setAnnounceCapabilities,
    savePeer,
    unsavePeer,
    setPeerLabel,
    updateSettings,
    getSavedPeerList,
    importPeerList,
    parsePeerListText,
    logUi,
    announceNow,
    requestPeerIdentity,
    sendBytes,
    sendBytesDirect,
    sendBytesViaPropagation,
    sendLxmf,
    onClientEvent,
    getSosSettings,
    setSosSettings,
    setSosPin,
    getSosStatus,
    triggerSos,
    deactivateSos,
    submitSosTelemetry,
    listSosAlerts,
    listSosLocations,
    listSosAudio,
    setActivePropagationNode,
    requestLxmfSync,
    broadcastBytes,
    broadcastJson,
    sendJson,
    resolvePeerLxmfDestinationByIdentity,
    destinationHasCapability,
    reinitializeClient,
    setLastError,
    assertReadyForOutbound,
    assertHubRoutingReadyForOutbound,
  };
});
