import {
  DEFAULT_NODE_CONFIG,
  createReticulumNodeClient,
  type AnnounceReceivedEvent,
  type HubDirectoryUpdatedEvent,
  type LxmfDeliveryEvent,
  type LogLevel,
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
  parsePeerListV1,
  TELEMETRY_CAPABILITY,
} from "../utils/peers";
import { runtimeProfile } from "../utils/runtimeProfile";

const SETTINGS_STORAGE_KEY = "reticulum.mobile.settings.v1";
const SAVED_STORAGE_KEY = "reticulum.mobile.savedPeers.v1";

const EMPTY_STATUS: NodeStatus = {
  running: false,
  name: "",
  identityHex: "",
  appDestinationHex: "",
  lxmfDestinationHex: "",
};

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
type DedicatedFields = Record<string, string>;
type EventPeerRoute = {
  appDestinationHex: string;
  lxmfDestinationHex?: string;
  identityHex?: string;
  label?: string;
  announcedName?: string;
};
type PacketSendOptions = {
  dedicatedFields?: DedicatedFields;
  fieldsBase64?: string;
};

function shouldDisplayDiscoveredPeer(peer: DiscoveredPeer): boolean {
  if (peer.sources.includes("hub") || peer.sources.includes("import")) {
    return true;
  }
  if (!peer.sources.includes("announce")) {
    return false;
  }
  if (!peer.verifiedCapability) {
    return false;
  }
  return matchesEmergencyCapabilities(peer.appData ?? "");
}

function nowMs(): number {
  return Date.now();
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
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
      tcpClients: Array.isArray(parsed.tcpClients)
        ? parsed.tcpClients.filter((item): item is string => typeof item === "string")
        : [...DEFAULT_SETTINGS.tcpClients],
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
  const logs = ref<UiLogLine[]>([]);
  const lastError = ref<string>("");
  const lastHubRefreshAt = ref<number>(0);
  const initialized = ref(false);

  const client = shallowRef<ReticulumNodeClient | null>(null);
  const unsubscribeClientEvents = ref<Array<() => void>>([]);
  const packetListeners = new Set<PacketListener>();
  const lxmfDeliveryListeners = new Set<LxmfDeliveryListener>();

  function appendLog(level: string, message: string): void {
    logs.value = [{ at: nowMs(), level, message }, ...logs.value].slice(0, 120);
  }

  function toPluginLogLevel(level: string): LogLevel {
    switch (level.trim().toLowerCase()) {
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
    const normalizedLevel = level.trim().toLowerCase();
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
    lastError.value = message.trim();
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
      verifiedCapability: false,
      sources,
      state: "disconnected",
    };

    discoveredByDestination[destination] = {
      ...base,
      ...patch,
      destination,
      sources,
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
      lastSeenAt: nowMs(),
    });
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

  async function configureClientLogging(): Promise<void> {
    if (!client.value) {
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
        status.value = { ...event.status };
      }),
      nodeClient.on("announceReceived", (event: AnnounceReceivedEvent) => {
        const identityHex = normalizeDestinationHex(event.identityHex ?? "");
        if (event.destinationKind === "lxmf_delivery") {
          if (isValidDestinationHex(identityHex)) {
            lxmfDestinationByIdentity[identityHex] = event.destinationHex;
            const appDestinationHex = appDestinationByIdentity[identityHex];
            if (isValidDestinationHex(appDestinationHex)) {
              upsertDiscovered(appDestinationHex, {
                identityHex,
                lxmfDestinationHex: event.destinationHex,
                lxmfLastSeenAt: event.receivedAtMs,
              });
            }
          }
          return;
        }

        const saved = savedByDestination[event.destinationHex];
        const announcedName = extractAnnouncedName(event.appData);
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
            lastSeenAt: event.receivedAtMs,
            verifiedCapability,
          },
          "announce",
        );
      }),
      nodeClient.on("peerChanged", (event: PeerChangedEvent) => {
        if (event.change.state === "Connecting") {
          setPeerState(event.change.destinationHex, "connecting");
        } else if (event.change.state === "Connected") {
          setPeerState(event.change.destinationHex, "connected");
        } else {
          setPeerState(
            event.change.destinationHex,
            "disconnected",
            event.change.lastError,
          );
        }
        logUi(
          "Debug",
          `[peers] peerChanged destination=${normalizeDestinationHex(event.change.destinationHex)} nativeState=${event.change.state} lastError=${event.change.lastError ?? "-"} ${describePeerState(event.change.destinationHex)}.`,
        );
      }),
      nodeClient.on("hubDirectoryUpdated", (event: HubDirectoryUpdatedEvent) => {
        for (const destination of event.destinations) {
          const existing = discoveredByDestination[destination];
          const saved = savedByDestination[destination];
          upsertDiscovered(
            destination,
            {
              label: existing?.label ?? saved?.label,
              announcedName: existing?.announcedName,
              verifiedCapability: existing?.verifiedCapability ?? false,
              lastSeenAt: event.receivedAtMs,
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
        latest = await client.value.getStatus();
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

  async function init(): Promise<void> {
    if (initialized.value) {
      return;
    }
    initialized.value = true;

    client.value = buildClient();
    bindClientEvents(client.value);
    await configureClientLogging();

    for (const savedPeer of Object.values(savedByDestination)) {
      upsertDiscovered(
        savedPeer.destination,
        {
          label: savedPeer.label,
          verifiedCapability: false,
          lastSeenAt: savedPeer.savedAt,
        },
        "import",
      );
    }

    await refreshStatusSnapshot();
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
      appendLog("Info", "Node started.");

      if (settings.hub.mode !== "Disabled") {
        await refreshHubDirectory().catch((error: unknown) => {
          appendLog("Warn", `Hub refresh failed after start: ${errorMessage(error)}`);
        });
      }

      if (settings.autoConnectSaved) {
        await connectAllSaved().catch((error: unknown) => {
          appendLog("Warn", `Auto connect failed: ${errorMessage(error)}`);
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
      appendLog("Info", "Node restarted with updated settings.");
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

    setPeerState(destination, "connecting");
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
      setPeerState(destination, "disconnected");
      logUi("Debug", `[peers] disconnect applied ${describePeerState(destination)}.`);
    } catch (error: unknown) {
      throw captureActionError(`Disconnect peer failed (${destination})`, error);
    }
  }

  async function connectAllSaved(): Promise<void> {
    for (const peer of Object.values(savedByDestination)) {
      await connectPeer(peer.destination);
    }
  }

  async function disconnectAllSaved(): Promise<void> {
    for (const peer of Object.values(savedByDestination)) {
      await disconnectPeer(peer.destination);
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

  const discoveredPeers = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => shouldDisplayDiscoveredPeer(peer))
      .filter((peer) => !isLocalPeer(peer))
      .sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const savedPeers = computed(() =>
    Object.values(savedByDestination).sort((a, b) => b.savedAt - a.savedAt),
  );

  const connectedDestinations = computed(() =>
    discoveredPeers.value
      .filter((peer) => peer.state === "connected")
      .map((peer) => peer.destination),
  );

  const connectedEventPeerRoutes = computed<EventPeerRoute[]>(() =>
    discoveredPeers.value
      .filter((peer) => peer.state === "connected")
      .map((peer) => ({
        appDestinationHex: peer.destination,
        lxmfDestinationHex: peer.lxmfDestinationHex,
        identityHex: peer.identityHex,
        label: peer.label,
        announcedName: peer.announcedName,
      })),
  );

  const telemetryDestinations = computed(() =>
    Object.values(discoveredByDestination)
      .filter((peer) => peer.sources.includes("announce"))
      .filter((peer) => hasCapability(peer.appData ?? "", TELEMETRY_CAPABILITY))
      .map((peer) => peer.destination),
  );

  const savedDestinations = computed(() => new Set(savedPeers.value.map((peer) => peer.destination)));
  const ready = computed(() => status.value.running);

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
    if (!client.value) {
      throw captureActionError(
        `Send failed (${destinationHex})`,
        new Error("Node client is not initialized."),
      );
    }
    try {
      logUi(
        "Debug",
        `Send requested destination=${destinationHex} bytes=${bytes.byteLength} fields=${options?.fieldsBase64 ? "lxmf" : "none"}.`,
      );
      await client.value.sendBytes(destinationHex, bytes, options);
      logUi(
        "Debug",
        `Send handed to native transport destination=${destinationHex} bytes=${bytes.byteLength}.`,
      );
    } catch (error: unknown) {
      throw captureActionError(`Send failed (${destinationHex})`, error);
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
      appendLog("Info", "Node client recreated.");
    } catch (error: unknown) {
      throw captureActionError("Recreate client failed", error);
    }
  }

  return {
    settings,
    status,
    logs,
    lastError,
    lastHubRefreshAt,
    discoveredByDestination,
    savedByDestination,
    discoveredPeers,
    savedPeers,
    connectedDestinations,
    connectedEventPeerRoutes,
    telemetryDestinations,
    savedDestinations,
    ready,
    init,
    startNode,
    stopNode,
    restartNode,
    connectPeer,
    disconnectPeer,
    connectAllSaved,
    disconnectAllSaved,
    refreshHubDirectory,
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
    sendBytes,
    broadcastBytes,
    broadcastJson,
    sendJson,
    destinationHasCapability,
    reinitializeClient,
    setLastError,
    assertReadyForOutbound,
  };
});
