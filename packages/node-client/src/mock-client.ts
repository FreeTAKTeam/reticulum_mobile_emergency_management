import type { AnnounceClass, AnnounceDestinationKind, AnnounceRecord, AppSettingsRecord, ChecklistDeleteOptions, ChecklistRecord, ChecklistTemplateRecord, ConversationRecord, EamProjectionRecord, EamReadinessSummaryRecord, EamTeamSummaryRecord, EventProjectionRecord, HubDirectoryPeerRecord, HubDirectoryUpdatedEvent, InstalledPluginRecord, LegacyImportPayload, LogLevel, MessageRecord, NodeClientEvents, NodeConfig, NodeStatus, OperationalSummary, PacketSendOptions, PeerRecord, PluginCapabilityRecord, PluginSensorRecord, ReticulumNodeClient, RnodeBleDeviceRecord, RnodeBlePairResult, RnodeBluetoothDeviceRecord, RnodeBluetoothMode, RnodeUsbDeviceRecord, RnodeUsbPairResult, SavedPeerRecord, SendLxmfRequest, SosAlertRecord, SosAudioRecord, SosDeviceTelemetryRecord, SosLocationRecord, SosSettingsRecord, SosStatusRecord, SosTriggerSource, SyncStatus, TelemetryPositionRecord } from "./contracts";
import { DEFAULT_NODE_CONFIG, browserRuntimeReadiness, countConnectedSavedPeers, randomHex32 } from "./client-defaults";
import { DEFAULT_SOS_SETTINGS, DEFAULT_SOS_STATUS } from "./client-config-converters";
import { cloneChecklistRecord, cloneChecklistTemplateRecord, createDefaultChecklistTemplates, createInMemoryChecklistTemplateFromCsv, type ChecklistCellInput, type ChecklistCreateInput, type ChecklistRowAddInput, type ChecklistRowDeleteInput, type ChecklistRowStyleInput, type ChecklistStatusInput, type ChecklistTemplateCsvInput, type ChecklistUpdateInput } from "./checklist-memory-templates";
import { addInMemoryTaskRow, createInMemoryChecklistFromTemplate, deleteInMemoryTaskRow, emitChecklistInvalidations, findInMemoryChecklist, normalizeInMemoryChecklist, setInMemoryTaskCell, setInMemoryTaskRowStyle, setInMemoryTaskStatus, updateInMemoryChecklist } from "./checklist-memory-runtime";
import { emptyEamReadinessSummary } from "./projection-converters";
import { encodeBytesToBase64, normalizeHex } from "./runtime-converters";
import { TypedEmitter } from "./typed-emitter";
import { InMemoryProjectionClient } from "./in-memory-projection-client";

const MOCK_ANNOUNCED_PEERS = [
  "c3d4f7a6e01944ef8e620f5c5a146f1a",
  "4ecf4d0dcaf0f9126f493725314110bc",
  "e6dd8260de7cb8f3ff1f77a6810dcf9d",
  "99dd0a1cf3e95fc6f1d3a6765af96752",
  "a2f0d9a5fb6b94317802fca20af739b0",
];
const MOCK_ANNOUNCED_IDENTITIES = MOCK_ANNOUNCED_PEERS.map(() => randomHex32());

const MOCK_HUB_PEERS: HubDirectoryPeerRecord[] = [
  {
    identity: randomHex32(),
    destinationHash: "7eb6e03ed67cd89bb3c5a7ac8713a109",
    displayName: "Pixel",
    announceCapabilities: ["r3akt", "emergencymessages", "telemetry"],
    clientType: "rem",
    registeredMode: "connected",
    lastSeen: "2026-04-02T12:43:28Z",
    status: "active",
  },
  {
    identity: randomHex32(),
    destinationHash: "c31298a1c68e30f7f3578fc03230591f",
    displayName: "Relay",
    announceCapabilities: ["r3akt", "emergencymessages", "telemetry_relay"],
    clientType: "rem",
    registeredMode: "connected",
    lastSeen: "2026-04-02T12:43:28Z",
    status: "active",
  },
  {
    identity: randomHex32(),
    destinationHash: "b07fd4a357fdb6b3500f5226346f56fd",
    displayName: "Console",
    announceCapabilities: ["r3akt", "group_chat"],
    clientType: "rem",
    registeredMode: "semi_autonomous",
    lastSeen: "2026-04-02T12:43:28Z",
    status: "active",
  },
];

function mockHubDirectorySnapshot(): HubDirectoryUpdatedEvent {
  const yellowTeamUid = "d6b6e188b910d6bdd24d04b7a7ec5444";
  return {
    schemaVersion: 0,
    activeTeamUid: yellowTeamUid,
    effectiveConnectedMode: false,
    teams: [{ uid: yellowTeamUid, color: "YELLOW", teamName: "YELLOW" }],
    callerMemberships: [],
    members: MOCK_HUB_PEERS.map((peer) => ({
      ...peer,
      teamUid: yellowTeamUid,
      teamMemberUid: peer.identity,
    })),
    localTeams: [],
    items: MOCK_HUB_PEERS,
    receivedAtMs: Date.now(),
  };
}

export class MockReticulumNodeClient extends InMemoryProjectionClient implements ReticulumNodeClient {
  protected readonly inMemoryPrefix = "mock";
  protected status: NodeStatus = (() => {
    const lxmfDestinationHex = randomHex32();
    return {
      running: false,
      name: "mock-node",
      identityHex: randomHex32(),
      appDestinationHex: lxmfDestinationHex,
      lxmfDestinationHex,
      readiness: browserRuntimeReadiness(false),
      interfaces: [],
    };
  })();
  private capabilities = DEFAULT_NODE_CONFIG.announceCapabilities;
  private announceTimer: number | null = null;
  private readonly connected = new Set<string>();
  private readonly savedPeers = new Map<string, SavedPeerRecord>();

  private currentPeerRecords(): PeerRecord[] {
    const destinations = new Set<string>([
      ...this.savedPeers.keys(),
      ...this.connected.values(),
    ]);
    const now = Date.now();
    return [...destinations].map((destinationHex) => {
      const activeLink = this.connected.has(destinationHex);
      return {
        destinationHex,
        lxmfDestinationHex: destinationHex,
        state: activeLink ? "Connected" : "Disconnected",
        saved: this.savedPeers.has(destinationHex),
        stale: false,
        activeLink,
        hubDerived: false,
        lastSeenAtMs: activeLink ? now : 0,
      };
    });
  }

  private emitAnnounce(
    destinationHex: string,
    appData: string,
    identityHex = randomHex32(),
    destinationKind: AnnounceDestinationKind = "lxmf_delivery",
    announceClass: AnnounceClass = "LxmfDelivery",
  ): void {
    this.emitter.emit("announceReceived", {
      destinationHex,
      identityHex,
      destinationKind,
      announceClass,
      appData,
      hops: Math.max(1, Math.floor(Math.random() * 3)),
      interfaceHex: randomHex32(),
      receivedAtMs: Date.now(),
    });
  }

  private startMockAnnounces(): void {
    if (this.announceTimer !== null) {
      return;
    }
    for (const [index, peer] of MOCK_ANNOUNCED_PEERS.entries()) {
      const identityHex = MOCK_ANNOUNCED_IDENTITIES[index] ?? randomHex32();
      this.emitAnnounce(peer, this.capabilities, identityHex);
    }
    this.emitAnnounce(randomHex32(), "LXMF,Chat", randomHex32(), "other", "Other");

    this.announceTimer = window.setInterval(() => {
      const shuffled = [...MOCK_ANNOUNCED_PEERS.entries()].sort(() => Math.random() - 0.5);
      const [index, destinationHex] = shuffled[0] ?? [0, randomHex32()];
      this.emitAnnounce(
        destinationHex,
        Math.random() > 0.25 ? this.capabilities : "R3AKT,Other",
        MOCK_ANNOUNCED_IDENTITIES[index] ?? randomHex32(),
      );
    }, 5000);
  }

  private stopMockAnnounces(): void {
    if (this.announceTimer !== null) {
      clearInterval(this.announceTimer);
      this.announceTimer = null;
    }
  }

  async start(config: NodeConfig): Promise<void> {
    this.status = {
      ...this.status,
      running: true,
      name: config.name,
      readiness: browserRuntimeReadiness(true),
    };
    this.capabilities = config.announceCapabilities;
    this.emitter.emit("statusChanged", { status: { ...this.status } });
    this.emitter.emit("log", {
      level: "Info",
      message: "Mock node started",
    });
    this.startMockAnnounces();
  }

  async stop(): Promise<void> {
    for (const destinationHex of this.connected) {
      this.emitter.emit("peerChanged", {
        change: {
          destinationHex,
          state: "Disconnected",
          saved: this.savedPeers.has(destinationHex),
          stale: false,
          activeLink: false,
          hubDerived: false,
          lastSeenAtMs: Date.now(),
        },
      });
    }
    this.status = {
      ...this.status,
      running: false,
      readiness: browserRuntimeReadiness(false),
    };
    this.connected.clear();
    this.stopMockAnnounces();
    this.emitter.emit("statusChanged", { status: { ...this.status } });
  }

  async restart(config: NodeConfig): Promise<void> {
    await this.stop();
    await this.start(config);
  }

  async getStatus(): Promise<NodeStatus> {
    return { ...this.status };
  }

  async checkRnodeBluetoothPermissions(): Promise<{ bluetooth: string }> {
    return { bluetooth: "unavailable" };
  }

  async requestRnodeBluetoothPermissions(): Promise<{ bluetooth: string }> {
    return { bluetooth: "unavailable" };
  }

  async listPairedRnodeBluetoothDevices(): Promise<RnodeBleDeviceRecord[]> {
    return [];
  }

  async scanRnodeBleDevices(_timeoutMs?: number): Promise<RnodeBleDeviceRecord[]> {
    return [];
  }

  async pairRnodeBleDevice(id: string): Promise<RnodeBlePairResult> {
    return {
      id,
      address: id,
      paired: false,
      bondingStarted: false,
      bondState: "unavailable",
    };
  }

  async scanRnodeBluetoothDevices(_mode: RnodeBluetoothMode, _timeoutMs?: number): Promise<RnodeBluetoothDeviceRecord[]> {
    return [];
  }

  async pairRnodeBluetoothDevice(id: string, _mode: RnodeBluetoothMode): Promise<RnodeBlePairResult> {
    return this.pairRnodeBleDevice(id);
  }

  async listRnodeUsbDevices(): Promise<RnodeUsbDeviceRecord[]> {
    return [];
  }

  async requestRnodeUsbPermission(deviceId: number): Promise<{ deviceId: number; granted: boolean }> {
    return { deviceId, granted: false };
  }

  async startRnodeUsbBluetoothPairing(deviceId: number, _bluetoothDeviceId?: string): Promise<RnodeUsbPairResult> {
    return {
      id: "",
      address: "",
      paired: false,
      pairingModeStarted: false,
      manualPinRequired: false,
      bondState: "unavailable",
      message: `USB-assisted pairing is unavailable for USB device ${deviceId}.`,
    };
  }

  async cancelRnodeUsbBluetoothPairing(_deviceId?: number): Promise<void> {}

  async connectPeer(destinationHex: string): Promise<void> {
    const normalized = normalizeHex(destinationHex);
    this.emitter.emit("peerChanged", {
      change: {
        destinationHex: normalized,
        state: "Connecting",
        saved: true,
        stale: false,
        activeLink: false,
        hubDerived: false,
        lastSeenAtMs: Date.now(),
      },
    });
    await new Promise((resolve) => setTimeout(resolve, 200));
    this.connected.add(normalized);
    this.emitter.emit("peerChanged", {
      change: {
        destinationHex: normalized,
        state: "Connected",
        saved: true,
        stale: false,
        activeLink: true,
        hubDerived: false,
        lastSeenAtMs: Date.now(),
      },
    });
  }

  async disconnectPeer(destinationHex: string): Promise<void> {
    const normalized = normalizeHex(destinationHex);
    this.connected.delete(normalized);
    this.emitter.emit("peerChanged", {
      change: {
        destinationHex: normalized,
        state: "Disconnected",
        saved: this.savedPeers.has(normalized),
        stale: false,
        activeLink: false,
        hubDerived: false,
        lastSeenAtMs: Date.now(),
      },
    });
  }

  async announceNow(): Promise<void> {
    this.emitAnnounce(
      this.status.lxmfDestinationHex,
      this.capabilities,
      this.status.identityHex,
    );
  }

  async requestPeerIdentity(_destinationHex: string): Promise<void> {}

  async sendBytes(destinationHex: string, bytes: Uint8Array, _options?: PacketSendOptions): Promise<void> {
    this.emitter.emit("packetSent", {
      destinationHex: normalizeHex(destinationHex),
      bytes,
      outcome: "SentDirect",
    });
  }

  async sendLxmf(request: SendLxmfRequest): Promise<string> {
    const destinationHex = normalizeHex(request.destinationHex);
    const now = Date.now();
    const messageIdHex = randomHex32();
    this.emitter.emit("messageUpdated", {
      messageIdHex,
      conversationId: destinationHex,
      direction: "Outbound",
      destinationHex,
      requestedDestinationHex: destinationHex,
      deliveryDestinationHex: destinationHex,
      lastWireMessageIdHex: messageIdHex,
      title: request.title,
      bodyUtf8: request.bodyUtf8,
      method: "Direct",
      state: "SentDirect",
      transportState: "SentDirect",
      applicationAckState: "Waiting",
      sentAtMs: now,
      updatedAtMs: now,
    });
    window.setTimeout(() => {
      this.emitter.emit("messageUpdated", {
        messageIdHex,
        conversationId: destinationHex,
        direction: "Outbound",
        destinationHex,
        requestedDestinationHex: destinationHex,
        deliveryDestinationHex: destinationHex,
        lastWireMessageIdHex: messageIdHex,
        title: request.title,
        bodyUtf8: request.bodyUtf8,
        method: "Direct",
        state: "SentDirect",
        transportState: "TransportDelivered",
        applicationAckState: "Waiting",
        detail: "mock transport receipt",
        sentAtMs: now,
        updatedAtMs: Date.now(),
      });
    }, 300);
    return messageIdHex;
  }

  async retryLxmf(_messageIdHex: string): Promise<void> {}

  async cancelLxmf(_messageIdHex: string): Promise<void> {}

  async broadcastBytes(bytes: Uint8Array, _options?: PacketSendOptions): Promise<void> {
    for (const destinationHex of this.connected) {
      this.emitter.emit("packetSent", {
        destinationHex,
        bytes,
        outcome: "SentBroadcast",
      });
    }
  }

  async setAnnounceCapabilities(capabilityString: string): Promise<void> {
    this.capabilities = capabilityString;
    this.emitAnnounce(
      this.status.lxmfDestinationHex,
      capabilityString,
      this.status.identityHex,
    );
  }

  async setLogLevel(level: LogLevel): Promise<void> {
    this.emitter.emit("log", {
      level,
      message: `Mock log level set to ${level}`,
    });
  }

  async setActivePropagationNode(_destinationHex?: string): Promise<void> {}

  async requestLxmfSync(_limit?: number): Promise<void> {
    this.emitter.emit("syncUpdated", {
      phase: "Idle",
      messagesReceived: 0,
    });
  }

  async listAnnounces(): Promise<AnnounceRecord[]> {
    return [];
  }

  async refreshPlugins(): Promise<InstalledPluginRecord[]> { return []; }
  async listPlugins(): Promise<InstalledPluginRecord[]> { return []; }
  async approvePluginPublisher(_pluginId: string, _displayName?: string): Promise<void> {}
  async revokePluginPublisher(_fingerprint: string): Promise<void> {}
  async setPluginEnabled(_pluginId: string, _enabled: boolean): Promise<void> {}
  async grantPluginCapabilities(
    _pluginId: string,
    _capabilities: PluginCapabilityRecord,
  ): Promise<void> {}
  async openPluginConfiguration(_pluginId: string): Promise<void> {}
  async listPluginSensors(): Promise<PluginSensorRecord[]> { return []; }

  async listPeers(): Promise<PeerRecord[]> {
    return this.currentPeerRecords();
  }

  async listConversations(): Promise<ConversationRecord[]> {
    return [];
  }

  async listMessages(_conversationId?: string): Promise<MessageRecord[]> {
    return [];
  }

  async deleteConversation(_conversationId: string): Promise<void> {
    return undefined;
  }

  async getLxmfSyncStatus(): Promise<SyncStatus> {
    return {
      phase: "Idle",
      messagesReceived: 0,
    };
  }

  async listTelemetryDestinations(): Promise<string[]> {
    return this.currentPeerRecords()
      .filter((peer) => peer.activeLink)
      .map((peer) => peer.destinationHex);
  }

  async legacyImportCompleted(): Promise<boolean> { return false; }
  async importLegacyState(_payload: LegacyImportPayload): Promise<void> {}
  async getAppSettings(): Promise<AppSettingsRecord | null> { return null; }
  async setAppSettings(_settings: AppSettingsRecord): Promise<void> {}
  async getSavedPeers(): Promise<SavedPeerRecord[]> {
    return [...this.savedPeers.values()];
  }
  async setSavedPeers(peers: SavedPeerRecord[]): Promise<void> {
    this.savedPeers.clear();
    for (const peer of peers) {
      const destination = normalizeHex(peer.destination);
      if (!destination) {
        continue;
      }
      this.savedPeers.set(destination, {
        destination,
        label: peer.label,
        savedAt: peer.savedAt,
      });
    }
  }
  async getOperationalSummary(): Promise<OperationalSummary> {
    const connectedPeerCount = countConnectedSavedPeers(this.connected, this.savedPeers);
    return {
      running: this.status.running,
      peerCountTotal: this.currentPeerRecords().length,
      savedPeerCount: this.savedPeers.size,
      connectedPeerCount,
      conversationCount: 0,
      messageCount: 0,
      eamCount: 0,
      eventCount: 0,
      telemetryCount: 0,
      updatedAtMs: Date.now(),
    };
  }

  async logMessage(level: LogLevel, message: string): Promise<void> {
    this.emitter.emit("log", { level, message });
  }

  async refreshHubDirectory(): Promise<void> {
    this.emitter.emit("hubDirectoryUpdated", mockHubDirectorySnapshot());
  }

  async getHubDirectorySnapshot(): Promise<HubDirectoryUpdatedEvent> {
    return mockHubDirectorySnapshot();
  }

  async setActiveTeam(_teamUid: string): Promise<void> {}

  async dispose(): Promise<void> {
    this.emitter.clear();
  }
}
