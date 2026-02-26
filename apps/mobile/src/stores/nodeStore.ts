import {
  DEFAULT_NODE_CONFIG,
  createReticulumNodeClient,
  type AnnounceReceivedEvent,
  type HubDirectoryUpdatedEvent,
  type NodeConfig,
  type NodeErrorEvent,
  type NodeLogEvent,
  type NodeStatus,
  type PacketReceivedEvent,
  type PeerChangedEvent,
  type ReticulumNodeClient,
  type StatusChangedEvent,
} from "@reticulum/node-client";
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
  isValidDestinationHex,
  matchesEmergencyCapabilities,
  normalizeDestinationHex,
  parsePeerListV1,
} from "../utils/peers";

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
  clientMode: "auto",
  autoConnectSaved: true,
  announceCapabilities: "R3AKT,EMergencyMessages",
  tcpClients: [...DEFAULT_NODE_CONFIG.tcpClients],
  broadcast: DEFAULT_NODE_CONFIG.broadcast,
  announceIntervalSeconds: DEFAULT_NODE_CONFIG.announceIntervalSeconds,
  showOnlyCapabilityVerified: true,
  hub: {
    mode: "Disabled",
    identityHash: "",
    apiBaseUrl: "",
    apiKey: "",
    refreshIntervalSeconds: 300,
  },
};

interface UiLogLine {
  at: number;
  level: string;
  message: string;
}

type PacketListener = (event: PacketReceivedEvent) => void;

function nowMs(): number {
  return Date.now();
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function loadStoredSettings(): NodeUiSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) {
      return { ...DEFAULT_SETTINGS, hub: { ...DEFAULT_SETTINGS.hub } };
    }
    const parsed = JSON.parse(raw) as Partial<NodeUiSettings>;
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      hub: {
        ...DEFAULT_SETTINGS.hub,
        ...(parsed.hub ?? {}),
      },
      tcpClients: Array.isArray(parsed.tcpClients)
        ? parsed.tcpClients.filter((item): item is string => typeof item === "string")
        : [...DEFAULT_SETTINGS.tcpClients],
    };
  } catch {
    return { ...DEFAULT_SETTINGS, hub: { ...DEFAULT_SETTINGS.hub } };
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
  return {
    name: DEFAULT_NODE_CONFIG.name,
    storageDir: "reticulum-mobile",
    tcpClients: settings.tcpClients.filter((entry) => entry.trim().length > 0),
    broadcast: settings.broadcast,
    announceIntervalSeconds: settings.announceIntervalSeconds,
    announceCapabilities: settings.announceCapabilities,
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
  const logs = ref<UiLogLine[]>([]);
  const lastError = ref<string>("");
  const lastHubRefreshAt = ref<number>(0);
  const initialized = ref(false);

  const client = shallowRef<ReticulumNodeClient | null>(null);
  const unsubscribeClientEvents = ref<Array<() => void>>([]);
  const packetListeners = new Set<PacketListener>();

  function appendLog(level: string, message: string): void {
    logs.value = [{ at: nowMs(), level, message }, ...logs.value].slice(0, 120);
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

  function persistSavedPeers(): void {
    saveSavedPeers(savedByDestination);
  }

  function persistSettings(): void {
    saveSettings(settings);
  }

  function buildClient(): ReticulumNodeClient {
    return createReticulumNodeClient({
      mode: settings.clientMode,
    });
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
        if (!matchesEmergencyCapabilities(event.appData)) {
          return;
        }

        const saved = savedByDestination[event.destinationHex];
        upsertDiscovered(
          event.destinationHex,
          {
            appData: event.appData,
            hops: event.hops,
            interfaceHex: event.interfaceHex,
            label: saved?.label,
            lastSeenAt: event.receivedAtMs,
            verifiedCapability: true,
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
      }),
      nodeClient.on("hubDirectoryUpdated", (event: HubDirectoryUpdatedEvent) => {
        for (const destination of event.destinations) {
          const existing = discoveredByDestination[destination];
          const saved = savedByDestination[destination];
          upsertDiscovered(
            destination,
            {
              label: existing?.label ?? saved?.label,
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
      appendLog("Info", "Node started.");

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

    setPeerState(destination, "connecting");
    try {
      clearLastError();
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
      await client.value.disconnectPeer(destination);
      setPeerState(destination, "disconnected");
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
    settings.announceCapabilities = capabilityString;
    persistSettings();

    if (!client.value || !status.value.running) {
      return;
    }
    try {
      clearLastError();
      await client.value.setAnnounceCapabilities(capabilityString);
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
    if (next.clientMode) {
      settings.clientMode = next.clientMode;
    }
    if (typeof next.autoConnectSaved === "boolean") {
      settings.autoConnectSaved = next.autoConnectSaved;
    }
    if (next.announceCapabilities !== undefined) {
      settings.announceCapabilities = next.announceCapabilities;
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

  const discoveredPeers = computed(() =>
    Object.values(discoveredByDestination).sort((a, b) => b.lastSeenAt - a.lastSeenAt),
  );

  const savedPeers = computed(() =>
    Object.values(savedByDestination).sort((a, b) => b.savedAt - a.savedAt),
  );

  const connectedDestinations = computed(() =>
    discoveredPeers.value
      .filter((peer) => peer.state === "connected")
      .map((peer) => peer.destination),
  );

  const savedDestinations = computed(() => new Set(savedPeers.value.map((peer) => peer.destination)));

  async function broadcastJson(payload: unknown): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      const body = new TextEncoder().encode(JSON.stringify(payload));
      await client.value.broadcastBytes(body);
    } catch (error: unknown) {
      throw captureActionError("Broadcast failed", error);
    }
  }

  async function sendJson(destinationHex: string, payload: unknown): Promise<void> {
    if (!client.value) {
      return;
    }
    try {
      const body = new TextEncoder().encode(JSON.stringify(payload));
      await client.value.sendBytes(destinationHex, body);
    } catch (error: unknown) {
      throw captureActionError(`Send failed (${destinationHex})`, error);
    }
  }

  async function reinitializeClient(): Promise<void> {
    try {
      clearLastError();
      if (client.value) {
        await client.value.dispose().catch(() => undefined);
      }
      client.value = buildClient();
      bindClientEvents(client.value);
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
    savedDestinations,
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
    onPacket,
    broadcastJson,
    sendJson,
    reinitializeClient,
  };
});
