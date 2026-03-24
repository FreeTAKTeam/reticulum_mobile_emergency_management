import {
  DEFAULT_NODE_CONFIG,
  type AnnounceRecord,
  type PeerRecord,
  type SendMode,
  type SyncStatus,
  createReticulumNodeClient,
  type AnnounceReceivedEvent,
  type HubDirectoryUpdatedEvent,
  type LxmfDeliveryEvent,
  type LogLevel,
  type MessageRecord,
  type NodeConfig,
  type NodeErrorEvent,
  type NodeLogEvent,
  type NodeStatus,
  type PacketReceivedEvent,
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
import type {
  DiscoveredPeer,
  NodeUiSettings,
  PeerConnectionState,
  PeerListV1,
  SavedPeer,
} from "../types/domain";
import {
  createPeerListV1,
  ensureCapabilityTokens,
  extractAnnounceCapabilityText,
  extractAnnouncedName,
  formatAnnounceAppData,
  hasCapability,
  isValidDestinationHex,
  matchesEmergencyCapabilities,
  normalizeDisplayName,
  normalizeDestinationHex,
  parseCapabilityTokens,
  parsePeerListV1,
  TELEMETRY_CAPABILITY,
} from "../utils/peers";
import { runtimeProfile } from "../utils/runtimeProfile";

const SETTINGS_STORAGE_KEY = "reticulum.mobile.settings.v1";
const SAVED_STORAGE_KEY = "reticulum.mobile.savedPeers.v1";
const PEER_ONLINE_FRESHNESS_MS = 10 * 60_000;
const PEER_PRESENCE_TICK_MS = 15_000;
const EMPTY_BYTES = new Uint8Array(0);
const LXMF_SEND_ATTEMPTS = 3;
const LXMF_SEND_RETRY_DELAY_MS = 250;
const STARTUP_ANNOUNCE_SETTLE_MS = 2_500;
const STARTUP_AUTO_CONNECT_FRESHNESS_MS = 30_000;
const AUTO_CONNECT_SERIAL_DELAY_MS = 300;

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

interface HubRegistrationSnapshot {
  status: HubRegistrationStatus;
  linkage?: HubRegistryLinkage;
  lastAttemptAt?: number;
  lastReadyAt?: number;
  lastError?: string;
}

const DEFAULT_SETTINGS: NodeUiSettings = {
  displayName: DEFAULT_NODE_CONFIG.name,
  clientMode: "auto",
  autoConnectSaved: true,
  announceCapabilities: ensureCapabilityTokens("R3AKT,EMergencyMessages", [TELEMETRY_CAPABILITY]),
  tcpClients: [...DEFAULT_NODE_CONFIG.tcpClients],
  broadcast: DEFAULT_NODE_CONFIG.broadcast,
  announceIntervalSeconds: DEFAULT_NODE_CONFIG.announceIntervalSeconds,
  showOnlyCapabilityVerified: true,
  telemetry: {
    enabled: false,
    publishIntervalSeconds: 10,
    accuracyThresholdMeters: undefined,
    staleAfterMinutes: 30,
    expireAfterMinutes: 180,
  },
  hub: {
    mode: "Disabled",
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

type PacketListener = (event: PacketReceivedEvent) => void;
type LxmfDeliveryListener = (event: LxmfDeliveryEvent) => void;
type MessageListener = (message: MessageRecord) => void;
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
  return peer.managementState === "managed"
    || peer.sources.includes("hub")
    || peer.sources.includes("import")
    || peer.communicationReady
    || peer.availabilityState !== "unseen";
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

function toUiManagementState(
  state: PeerRecord["managementState"] | PeerChangedEvent["change"]["managementState"] | undefined,
): DiscoveredPeer["managementState"] {
  return state === "Managed" ? "managed" : "unmanaged";
}

function toUiAvailabilityState(
  state: PeerRecord["availabilityState"] | PeerChangedEvent["change"]["availabilityState"] | undefined,
): DiscoveredPeer["availabilityState"] {
  switch (state) {
    case "Discovered":
      return "discovered";
    case "Resolved":
      return "resolved";
    case "Ready":
      return "ready";
    default:
      return "unseen";
  }
}

function availabilityRank(peer: Pick<DiscoveredPeer, "availabilityState" | "managementState">): number {
  const availabilityRankValue = (() => {
    switch (peer.availabilityState) {
      case "ready":
        return 4;
      case "resolved":
        return 3;
      case "discovered":
        return 2;
      default:
        return 1;
    }
  })();
  return availabilityRankValue + (peer.managementState === "managed" ? 1 : 0);
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

function normalizeStoredTcpClients(value: unknown): string[] {
  const tcpClients = Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string").map((item) => item.trim())
    : [...DEFAULT_SETTINGS.tcpClients];
  const nonEmpty = tcpClients.filter((item) => item.length > 0);
  if (nonEmpty.length === 1 && nonEmpty[0].toLowerCase() === "rmap.world:4242") {
    return [];
  }
  return nonEmpty;
}

function loadStoredSettings(): NodeUiSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) {
      return { ...DEFAULT_SETTINGS, telemetry: { ...DEFAULT_SETTINGS.telemetry }, hub: { ...DEFAULT_SETTINGS.hub } };
    }
    const parsed = JSON.parse(raw) as Partial<NodeUiSettings>;
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      hub: {
        ...DEFAULT_SETTINGS.hub,
        ...(parsed.hub ?? {}),
      },
      telemetry: normalizeTelemetrySettings(parsed.telemetry),
      displayName: normalizeStoredDisplayName(parsed.displayName),
      announceCapabilities: ensureCapabilityTokens(
        typeof parsed.announceCapabilities === "string"
          ? parsed.announceCapabilities
          : DEFAULT_SETTINGS.announceCapabilities,
        [TELEMETRY_CAPABILITY],
      ),
      clientMode: normalizeClientMode(parsed.clientMode),
      tcpClients: normalizeStoredTcpClients(parsed.tcpClients),
    };
  } catch {
    return { ...DEFAULT_SETTINGS, telemetry: { ...DEFAULT_SETTINGS.telemetry }, hub: { ...DEFAULT_SETTINGS.hub } };
  }
}

function saveSettings(settings: NodeUiSettings): void {
  localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
}

function loadSavedPeers(): Record<string, SavedPeer> {
  try {
    const raw = localStorage.getItem(SAVED_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as SavedPeer[];
    const out: Record<string, SavedPeer> = {};
    for (const peer of parsed) {
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
  } catch {
    return {};
  }
}

function saveSavedPeers(savedPeers: Record<string, SavedPeer>): void {
  localStorage.setItem(SAVED_STORAGE_KEY, JSON.stringify(Object.values(savedPeers)));
}

function toNodeConfig(settings: NodeUiSettings): NodeConfig {
  const displayName = normalizeDisplayName(settings.displayName) ?? DEFAULT_NODE_CONFIG.name;
  return {
    name: displayName,
    storageDir: "reticulum-mobile",
    tcpClients: settings.tcpClients.filter((entry) => entry.trim().length > 0),
    broadcast: settings.broadcast,
    announceIntervalSeconds: settings.announceIntervalSeconds,
    announceCapabilities: formatAnnounceAppData(
      ensureCapabilityTokens(settings.announceCapabilities, [TELEMETRY_CAPABILITY]),
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
  const settings = reactive<NodeUiSettings>(loadStoredSettings());
  const status = ref<NodeStatus>({ ...EMPTY_STATUS });
  const discoveredByDestination = reactive<Record<string, DiscoveredPeer>>({});
  const savedByDestination = reactive<Record<string, SavedPeer>>(loadSavedPeers());
  const appDestinationByIdentity = reactive<Record<string, string>>({});
  const lxmfDestinationByIdentity = reactive<Record<string, string>>({});
  const livePresenceByDestination = reactive<Record<string, number>>({});
  const liveLxmfPresenceByIdentity = reactive<Record<string, number>>({});
  const logs = ref<UiLogLine[]>([]);
  const lastError = ref<string>("");
  const lastHubRefreshAt = ref<number>(0);
  const syncStatus = ref<SyncStatus>({ ...EMPTY_SYNC_STATUS });
  const hubRegistration = reactive<HubRegistrationSnapshot>({
    status: settings.hub.mode === "Disabled" ? "disabled" : "pending",
    linkage: loadHubRegistryLinkage() ?? undefined,
    lastReadyAt: loadHubRegistryLinkage()?.updatedAt,
  });
  const initialized = ref(false);
  const presenceNow = ref(nowMs());

  const client = shallowRef<ReticulumNodeClient | null>(null);
  const unsubscribeClientEvents = ref<Array<() => void>>([]);
  const packetListeners = new Set<PacketListener>();
  const lxmfDeliveryListeners = new Set<LxmfDeliveryListener>();
  const messageListeners = new Set<MessageListener>();
  const identityResolutionInFlight = new Set<string>();
  const autoConnectInFlight = new Set<string>();
  const autoConnectQueue: string[] = [];
  let hubRegistryBootstrapInFlight: Promise<void> | null = null;
  let propagationSelectionInFlight = false;
  let presenceTickerId: number | null = null;
  let messagingRefreshTimerId: number | null = null;
  const startupSettling = ref(false);
  const autoConnectQueueActive = ref(false);
  let deferredMessagingRefreshReason: string | null = null;

  function appendLog(level: string, message: string): void {
    logs.value = [{ at: nowMs(), level, message }, ...logs.value].slice(0, 120);
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

  async function retryLxmfSend<T>(
    action: string,
    destinationHex: string,
    send: (attempt: number) => Promise<T>,
    sendViaPropagation?: () => Promise<T>,
  ): Promise<T> {
    let lastError: unknown;

    for (let attempt = 1; attempt <= LXMF_SEND_ATTEMPTS; attempt += 1) {
      try {
        appendLog(
          "Debug",
          `${action} attempt ${attempt}/${LXMF_SEND_ATTEMPTS} destination=${destinationHex}.`,
        );
        return await send(attempt);
      } catch (error: unknown) {
        lastError = error;
        const message = errorMessage(error);
        if (attempt >= LXMF_SEND_ATTEMPTS) {
          break;
        }

        appendLog(
          "Warn",
          `${action} attempt ${attempt}/${LXMF_SEND_ATTEMPTS} failed destination=${destinationHex}: ${message}. Retrying.`,
        );
        await sleep(LXMF_SEND_RETRY_DELAY_MS * attempt);
      }
    }

    const propagationNodeHex = activePropagationNodeHex(syncStatus.value);
    if (propagationNodeHex && sendViaPropagation) {
      try {
        appendLog(
          "Warn",
          `${action} direct delivery exhausted after ${LXMF_SEND_ATTEMPTS} attempts destination=${destinationHex}. Switching to propagation relay ${propagationNodeHex}.`,
        );
        return await sendViaPropagation();
      } catch (error: unknown) {
        lastError = error;
        appendLog(
          "Error",
          `${action} propagation fallback failed destination=${destinationHex} relay=${propagationNodeHex}: ${errorMessage(error)}.`,
        );
      }
    } else if (lastError) {
      appendLog(
        "Error",
        `${action} failed after ${LXMF_SEND_ATTEMPTS} direct attempts destination=${destinationHex}: ${errorMessage(lastError)}.`,
      );
    }

    if (lastError instanceof Error) {
      throw lastError;
    }

    throw new Error(`${action} failed after ${LXMF_SEND_ATTEMPTS} attempts.`);
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
      verifiedCapability: false,
      sources,
      state: "disconnected",
      managementState: "unmanaged",
      availabilityState: "unseen",
      communicationReady: false,
      missionReady: false,
      relayEligible: false,
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
      managementState: patch.managementState ?? base.managementState,
      availabilityState: patch.availabilityState ?? base.availabilityState,
      communicationReady: patch.communicationReady ?? base.communicationReady,
      missionReady: patch.missionReady ?? base.missionReady,
      relayEligible: patch.relayEligible ?? base.relayEligible,
      stale: patch.stale ?? base.stale,
      activeLink: patch.activeLink ?? base.activeLink,
      lastError: Object.prototype.hasOwnProperty.call(patch, "lastError")
        ? patch.lastError
        : base.lastError,
      lastResolutionError: Object.prototype.hasOwnProperty.call(patch, "lastResolutionError")
        ? patch.lastResolutionError
        : base.lastResolutionError,
      lastResolutionAttemptAt: patch.lastResolutionAttemptAt ?? base.lastResolutionAttemptAt,
      lastReadyAt: patch.lastReadyAt ?? base.lastReadyAt,
      lastSeenAt: patch.lastSeenAt ?? base.lastSeenAt,
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
        managementState: toUiManagementState(peer.managementState),
        availabilityState: toUiAvailabilityState(peer.availabilityState),
        communicationReady: peer.communicationReady,
        missionReady: peer.missionReady,
        relayEligible: peer.relayEligible,
        stale: peer.stale,
        activeLink: peer.activeLink,
        lastError: peer.lastResolutionError,
        lastResolutionError: peer.lastResolutionError,
        lastResolutionAttemptAt: peer.lastResolutionAttemptAtMs,
        lastReadyAt: peer.lastReadyAtMs,
        verifiedCapability: matchesEmergencyCapabilities(peer.appData ?? ""),
      },
      "announce",
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
        managementState: change.managementState
          ? toUiManagementState(change.managementState)
          : undefined,
        availabilityState: change.availabilityState
          ? toUiAvailabilityState(change.availabilityState)
          : undefined,
        communicationReady: change.communicationReady,
        missionReady: change.missionReady,
        relayEligible: change.relayEligible,
        stale: change.stale,
        activeLink: change.activeLink,
        lastError: change.lastError,
        lastResolutionError: change.lastResolutionError,
        lastResolutionAttemptAt: change.lastResolutionAttemptAtMs,
        lastReadyAt: change.lastReadyAtMs,
        lastSeenAt: change.lastSeenAtMs,
        announceLastSeenAt: change.announceLastSeenAtMs,
        lxmfLastSeenAt: change.lxmfLastSeenAtMs,
      },
      "announce",
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

      const retainedSources = peer.sources.filter((source) => source !== "announce");
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
        state: peer.managementState === "managed" ? "connecting" : "disconnected",
        availabilityState: "unseen",
        communicationReady: false,
        missionReady: false,
        relayEligible: false,
        stale: false,
        activeLink: false,
        lastError: undefined,
        lastResolutionError: undefined,
      };
    }
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
      `management=${peer.managementState}`,
      `availability=${peer.availabilityState}`,
      `communicationReady=${peer.communicationReady}`,
      `missionReady=${peer.missionReady}`,
      `relayEligible=${peer.relayEligible}`,
      `stale=${peer.stale}`,
      `activeLink=${peer.activeLink}`,
      `label=${peer.label ?? "-"}`,
      `announced=${peer.announcedName ?? "-"}`,
      `identity=${peer.identityHex ?? "-"}`,
      `lxmf=${peer.lxmfDestinationHex ?? "-"}`,
      `sources=${peer.sources.join("+") || "-"}`,
      `verified=${peer.verifiedCapability}`,
    ].join(" ");
  }

  function persistSavedPeers(): void {
    saveSavedPeers(savedByDestination);
  }

  function persistSettings(): void {
    saveSettings(settings);
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
    const normalizedDestination = normalizeDestinationHex(destination);
    if (!settings.autoConnectSaved || !status.value.running) {
      return false;
    }
    if (isLocalPeerDestination(normalizedDestination) || !savedByDestination[normalizedDestination]) {
      return false;
    }
    const peer = discoveredByDestination[normalizedDestination];
    if (peer?.managementState === "managed" || peer?.state === "connecting" || peer?.activeLink) {
      return false;
    }
    return !autoConnectInFlight.has(normalizedDestination);
  }

  function scheduleSavedPeerAutoConnect(destination: string, reason: string): void {
    const normalizedDestination = normalizeDestinationHex(destination);
    if (!shouldAutoConnectSavedPeer(normalizedDestination)) {
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
        if (!shouldAutoConnectSavedPeer(nextDestination)) {
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
    const verifiedCapability = matchesEmergencyCapabilities(event.appData);
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
        verifiedCapability,
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
      if (deferredMessagingRefreshReason) {
        const pendingReason = deferredMessagingRefreshReason;
        deferredMessagingRefreshReason = null;
        scheduleMessagingSnapshotRefresh(`${reason} settle release (${pendingReason})`, 150);
      }
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
    return buildHubRegistryBootstrapProfile({
      callsign: settings.displayName,
      localIdentityHex: status.value.identityHex,
      hubIdentityHash: settings.hub.identityHash,
    });
  }

  function setHubRegistrationPending(lastErrorValue?: string): void {
    hubRegistration.status = settings.hub.mode === "Disabled" ? "disabled" : "pending";
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
    hubRegistration.status = settings.hub.mode === "Disabled" ? "disabled" : "error";
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
    if (settings.hub.mode === "Disabled") {
      hubRegistration.status = "disabled";
      hubRegistration.lastError = "";
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
      onPacket: (listener) => onPacket(listener),
    };
  }

  async function bootstrapHubRegistration(force = false): Promise<void> {
    if (settings.hub.mode === "Disabled") {
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
    if (!attemptBootstrap || settings.hub.mode === "Disabled") {
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
      await client.value.setLogLevel("Debug");
      logUi("Debug", "Node client log level set to Debug.");
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
        void refreshHubRegistrationState(event.status.running && settings.hub.mode !== "Disabled");
      }),
      nodeClient.on("announceReceived", (event: AnnounceReceivedEvent) => {
        scheduleMessagingSnapshotRefresh(
          `announce ${event.destinationKind}:${normalizeDestinationHex(event.destinationHex)}`,
        );
      }),
      nodeClient.on("peerChanged", (event: PeerChangedEvent) => {
        presenceNow.value = nowMs();
        applyPeerChanged(event.change);
        logUi(
          "Debug",
          `[peers] peerChanged destination=${normalizeDestinationHex(event.change.destinationHex)} nativeState=${event.change.state} lastError=${event.change.lastError ?? "-"} ${describePeerState(event.change.destinationHex)}.`,
        );
      }),
      nodeClient.on("peerResolved", (peer: PeerRecord) => {
        presenceNow.value = peer.lastSeenAtMs;
        upsertResolvedPeer(peer);
        logUi(
          "Debug",
          `[peers] peerResolved destination=${normalizeDestinationHex(peer.destinationHex)} state=${peer.state} management=${peer.managementState} availability=${peer.availabilityState} activeLink=${peer.activeLink} identity=${peer.identityHex ?? "-"} lxmf=${peer.lxmfDestinationHex ?? "-"} display=${peer.displayName ?? "-"} appData=${peer.appData ?? "-"}.`,
        );
      }),
      nodeClient.on("hubDirectoryUpdated", (event: HubDirectoryUpdatedEvent) => {
        presenceNow.value = event.receivedAtMs;
        for (const destination of event.destinations) {
          const existing = discoveredByDestination[destination];
          const saved = savedByDestination[destination];
          upsertDiscovered(
            destination,
            {
              label: existing?.label ?? saved?.label,
              announcedName: existing?.announcedName,
              verifiedCapability: existing?.verifiedCapability ?? false,
            },
            "hub",
          );
        }
        lastHubRefreshAt.value = event.receivedAtMs;
      }),
      nodeClient.on("packetReceived", (event: PacketReceivedEvent) => {
        for (const listener of packetListeners) {
          listener(event);
        }
      }),
      nodeClient.on("lxmfDelivery", (event: LxmfDeliveryEvent) => {
        for (const listener of lxmfDeliveryListeners) {
          listener(event);
        }
      }),
      nodeClient.on("messageReceived", (message: MessageRecord) => {
        for (const listener of messageListeners) {
          listener(message);
        }
      }),
      nodeClient.on("messageUpdated", (message: MessageRecord) => {
        for (const listener of messageListeners) {
          listener(message);
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
        appendLog("Error", lastError.value);
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
      return;
    }

    try {
      const [peers, nextSyncStatus] = await Promise.all([
        client.value.listPeers(),
        client.value.getLxmfSyncStatus(),
      ]);
      reconcileNativePeerSnapshot(peers);
      for (const peer of peers) {
        upsertResolvedPeer(peer);
      }
      syncStatus.value = { ...nextSyncStatus };
    } catch (error: unknown) {
      appendLog("Debug", `Messaging snapshot refresh skipped: ${errorMessage(error)}`);
    }
  }

  function scheduleMessagingSnapshotRefresh(reason: string, delayMs = 400): void {
    if (startupSettling.value) {
      deferredMessagingRefreshReason = reason;
      return;
    }
    if (messagingRefreshTimerId !== null) {
      window.clearTimeout(messagingRefreshTimerId);
    }
    messagingRefreshTimerId = window.setTimeout(() => {
      messagingRefreshTimerId = null;
      void refreshMessagingState()
        .then(() => {
          appendLog("Debug", `[peers] refreshed native peer snapshot after ${reason}.`);
        })
        .catch(() => undefined);
    }, delayMs);
  }

  async function init(): Promise<void> {
    if (initialized.value) {
      return;
    }
    initialized.value = true;

    client.value = buildClient();
    bindClientEvents(client.value);
    if (presenceTickerId === null) {
      presenceTickerId = window.setInterval(() => {
        presenceNow.value = nowMs();
        if (status.value.running && !startupSettling.value) {
          void refreshMessagingState().catch(() => undefined);
        }
      }, PEER_PRESENCE_TICK_MS);
    }

    for (const savedPeer of Object.values(savedByDestination)) {
      upsertDiscovered(
        savedPeer.destination,
        {
          label: savedPeer.label,
          verifiedCapability: false,
          lastSeenAt: savedPeer.savedAt,
          communicationReady: false,
          missionReady: false,
          relayEligible: false,
          stale: false,
          activeLink: false,
        },
        "import",
      );
    }

    await refreshStatusSnapshot();
    await refreshMessagingState();
    await refreshHubRegistrationState(status.value.running && settings.hub.mode !== "Disabled");
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
      await configureClientLogging();
      await settleStartupDiscovery("startup");
      await refreshHubRegistrationState(true);
      appendLog("Info", "Node started.");

      if (settings.hub.mode !== "Disabled") {
        await refreshHubDirectory().catch((error: unknown) => {
          appendLog("Warn", `Hub refresh failed after start: ${errorMessage(error)}`);
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
      appendLog("Info", "Node stopped.");
      syncStatus.value = { ...EMPTY_SYNC_STATUS };
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
      await configureClientLogging();
      await settleStartupDiscovery("restart");
      await refreshHubRegistrationState(true);
      appendLog("Info", "Node restarted with updated settings.");

      if (settings.hub.mode !== "Disabled") {
        await refreshHubDirectory().catch((error: unknown) => {
          appendLog("Warn", `Hub refresh failed after restart: ${errorMessage(error)}`);
        });
      }
    } catch (error: unknown) {
      throw captureActionError("Restart node failed", error);
    }
  }

  async function connectPeer(destinationRaw: string): Promise<void> {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!client.value || !isValidDestinationHex(destination)) {
      return;
    }
    if (isLocalPeerDestination(destination)) {
      appendLog("Warn", `Skipped self-connect for ${destination}.`);
      return;
    }

    try {
      clearLastError();
      logUi("Debug", `[peers] connect requested ${describePeerState(destination)}.`);
      await client.value.connectPeer(destination);
    } catch (error: unknown) {
      const message = errorMessage(error);
      setPeerState(destination, "disconnected", message);
      throw captureActionError(`Connect peer failed (${destination})`, error);
    }
  }

  async function disconnectPeer(destinationRaw: string): Promise<void> {
    const destination = normalizeDestinationHex(destinationRaw);
    if (!client.value || !isValidDestinationHex(destination)) {
      return;
    }
    try {
      clearLastError();
      logUi("Debug", `[peers] disconnect requested ${describePeerState(destination)}.`);
      await client.value.disconnectPeer(destination);
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
      if (!client.value) {
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
    settings.announceCapabilities = ensureCapabilityTokens(capabilityString, [TELEMETRY_CAPABILITY]);
    persistSettings();

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
    const destination = normalizeDestinationHex(destinationRaw);
    if (!isValidDestinationHex(destination)) {
      return;
    }
    const discovered = discoveredByDestination[destination];
    savedByDestination[destination] = {
      destination,
      label: discovered?.label,
      savedAt: nowMs(),
    };
    persistSavedPeers();
  }

  async function unsavePeer(destinationRaw: string): Promise<void> {
    const destination = normalizeDestinationHex(destinationRaw);
    delete savedByDestination[destination];
    persistSavedPeers();
    if (discoveredByDestination[destination]) {
      discoveredByDestination[destination].sources = discoveredByDestination[
        destination
      ].sources.filter((source) => source !== "import");
    }
  }

  async function setPeerLabel(destinationRaw: string, label: string): Promise<void> {
    const destination = normalizeDestinationHex(destinationRaw);
    const normalizedLabel = label.trim();
    if (savedByDestination[destination]) {
      savedByDestination[destination] = {
        ...savedByDestination[destination],
        label: normalizedLabel || undefined,
      };
      persistSavedPeers();
    }
    if (discoveredByDestination[destination]) {
      discoveredByDestination[destination].label = normalizedLabel || undefined;
    }
  }

  function updateSettings(next: Partial<NodeUiSettings>): void {
    if (next.displayName !== undefined) {
      settings.displayName = normalizeStoredDisplayName(next.displayName);
    }
    if (next.clientMode) {
      settings.clientMode = next.clientMode;
    }
    if (typeof next.autoConnectSaved === "boolean") {
      settings.autoConnectSaved = next.autoConnectSaved;
    }
    if (next.announceCapabilities !== undefined) {
      settings.announceCapabilities = ensureCapabilityTokens(next.announceCapabilities, [TELEMETRY_CAPABILITY]);
    }
    if (next.tcpClients) {
      settings.tcpClients = [...next.tcpClients];
    }
    if (typeof next.broadcast === "boolean") {
      settings.broadcast = next.broadcast;
    }
    if (next.announceIntervalSeconds !== undefined) {
      settings.announceIntervalSeconds = next.announceIntervalSeconds;
    }
    if (typeof next.showOnlyCapabilityVerified === "boolean") {
      settings.showOnlyCapabilityVerified = next.showOnlyCapabilityVerified;
    }
    if (next.telemetry) {
      settings.telemetry = normalizeTelemetrySettings(next.telemetry, settings.telemetry);
    }
    if (next.hub) {
      settings.hub = {
        ...settings.hub,
        ...next.hub,
      };
    }
    persistSettings();
    void refreshHubRegistrationState(settings.hub.mode !== "Disabled");
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
          verifiedCapability: discoveredByDestination[destination]?.verifiedCapability ?? false,
          lastSeenAt: nowMs(),
        },
        "import",
      );
    }

    persistSavedPeers();
  }

  function parsePeerListText(text: string): ReturnType<typeof parsePeerListV1> {
    return parsePeerListV1(text);
  }

  function onPacket(listener: PacketListener): () => void {
    packetListeners.add(listener);
    return () => {
      packetListeners.delete(listener);
    };
  }

  function onLxmfDelivery(listener: LxmfDeliveryListener): () => void {
    lxmfDeliveryListeners.add(listener);
    return () => {
      lxmfDeliveryListeners.delete(listener);
    };
  }

  function onMessage(listener: MessageListener): () => void {
    messageListeners.add(listener);
    return () => {
      messageListeners.delete(listener);
    };
  }

  function hasFreshPresence(lastSeenAt?: number): boolean {
    return typeof lastSeenAt === "number"
      && Number.isFinite(lastSeenAt)
      && (presenceNow.value - lastSeenAt) <= PEER_ONLINE_FRESHNESS_MS;
  }

  function peerPresenceTimestamp(
    peer: Pick<DiscoveredPeer, "lastReadyAt" | "lastSeenAt">,
  ): number | undefined {
    const seenAt = Math.max(peer.lastReadyAt ?? 0, peer.lastSeenAt ?? 0);
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

function peerPresenceState(
  peer: Pick<DiscoveredPeer, "availabilityState" | "activeLink">,
): "online" | "offline" {
  return peer.activeLink || peer.availabilityState === "ready" ? "online" : "offline";
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

function peerHasKnownMissionRoute(
  peer: Pick<DiscoveredPeer, "destination" | "lxmfDestinationHex">,
): boolean {
  return peerHasKnownLxmfRoute(peer);
}

function peerHasDirectLxmfDelivery(
  peer: Pick<DiscoveredPeer, "communicationReady">,
): boolean {
  return peer.communicationReady;
}

function peerCanAcceptMissionTraffic(
  peer: Pick<
    DiscoveredPeer,
    "destination" | "lxmfDestinationHex" | "communicationReady" | "missionReady"
  >,
): boolean {
  return peer.communicationReady && peer.missionReady && peerHasKnownMissionRoute(peer);
}

function peerCanRelayMissionTraffic(
  peer: Pick<DiscoveredPeer, "destination" | "lxmfDestinationHex" | "relayEligible">,
): boolean {
  return peer.relayEligible && peerHasKnownMissionRoute(peer);
}

function peerIsManagedForAutoFanout(
  peer: Pick<DiscoveredPeer, "destination" | "managementState">,
  savedByDestination: Record<string, SavedPeer>,
): boolean {
  const destination = normalizeDestinationHex(peer.destination);
  return peer.managementState === "managed" || Boolean(savedByDestination[destination]);
}

function peerHasEventRoute(
  peer: Pick<
    DiscoveredPeer,
    "destination" | "lxmfDestinationHex" | "communicationReady" | "missionReady"
  >,
): boolean {
  return peerCanAcceptMissionTraffic(peer);
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
        const byAvailabilityRank = availabilityRank(b) - availabilityRank(a);
        if (byAvailabilityRank !== 0) {
          return byAvailabilityRank;
        }
        return b.lastSeenAt - a.lastSeenAt;
      }),
  );
  const allPeers = discoveredPeers;

  const communicationReadyPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => !isLocalPeer(peer))
      .filter((peer) => peer.communicationReady)
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const missionReadyPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => !isLocalPeer(peer))
      .filter((peer) => peer.missionReady)
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const relayEligiblePeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => !isLocalPeer(peer))
      .filter((peer) => peerCanRelayMissionTraffic(peer))
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const autoFanoutPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => !isLocalPeer(peer))
      .filter((peer) => peerIsManagedForAutoFanout(peer, savedByDestination))
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const propagationEligibleEventPeerRoutes = computed<EventPeerRoute[]>(() =>
    (!bestPropagationNodeHex.value ? [] : autoFanoutPeers.value)
      .filter((peer) => peer.missionReady)
      .filter((peer) => peerCanRelayMissionTraffic(peer))
      .filter((peer) => !peer.communicationReady)
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

  const connectedDestinations = computed(() =>
    communicationReadyPeers.value.map((peer) => peer.destination),
  );

  const connectedLinkDestinations = computed(() =>
    discoveredPeers.value
      .filter((peer) => peer.activeLink)
      .map((peer) => peer.destination),
  );

  const connectedEventPeerRoutes = computed<EventPeerRoute[]>(() =>
    autoFanoutPeers.value
      .filter((peer) => peerHasEventRoute(peer))
      .map((peer) => ({
        appDestinationHex: peer.destination,
        lxmfDestinationHex: peer.lxmfDestinationHex!,
        identityHex: peer.identityHex,
        label: peer.label,
        announcedName: peer.announcedName,
        sendMode: "Auto",
      })),
  );

  const communicationReadyPeerCount = computed(() => communicationReadyPeers.value.length);
  const missionReadyPeerCount = computed(() => missionReadyPeers.value.length);
  const relayEligiblePeerCount = computed(() => relayEligiblePeers.value.length);
  const propagationCandidateDestinations = computed(() =>
    activePropagationNodeHex(syncStatus.value)
      ? [activePropagationNodeHex(syncStatus.value)!]
      : [],
  );
  const bestPropagationNodeHex = computed(() => activePropagationNodeHex(syncStatus.value));

  const telemetryDestinations = computed(() =>
    communicationReadyPeers.value
      .filter((peer) => hasCapability(peer.appData ?? "", TELEMETRY_CAPABILITY))
      .map((peer) => peer.destination),
  );

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
    hubRegistration,
    hubBootstrapProfile,
    hubRegistrationReady,
    hubRegistrationPending,
    hubRegistrationSummary,
    logs,
    lastError,
    lastHubRefreshAt,
    discoveredByDestination,
    savedByDestination,
    allPeers,
    discoveredPeers,
    savedPeers,
    communicationReadyPeers,
    missionReadyPeers,
    relayEligiblePeers,
    propagationEligibleEventPeerRoutes,
    connectedDestinations,
    connectedLinkDestinations,
    connectedEventPeerRoutes,
    communicationReadyPeerCount,
    missionReadyPeerCount,
    relayEligiblePeerCount,
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
    onPacket,
    onLxmfDelivery,
    onMessage,
    announceNow,
    requestPeerIdentity,
    sendBytes,
    sendBytesDirect,
    sendBytesViaPropagation,
    sendLxmf,
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
  };
});
