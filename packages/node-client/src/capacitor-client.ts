import type { AnnounceRecord, AppSettingsRecord, ChecklistDeleteOptions, ChecklistRecord, ChecklistTemplateRecord, ChecklistUserTaskStatus, ConversationRecord, EamProjectionRecord, EamReadinessSummaryRecord, EamTeamSummaryRecord, EventProjectionRecord, InstalledPluginRecord, LegacyImportPayload, LogLevel, MessageRecord, NodeClientEvents, NodeConfig, NodeStatus, OperationalSummary, PacketSendOptions, PeerRecord, PluginCapabilityRecord, PluginSensorRecord, ReticulumNodeClient, RnodeBleDeviceRecord, RnodeBlePairResult, RnodeBluetoothDeviceRecord, RnodeBluetoothMode, RnodeUsbDeviceRecord, RnodeUsbPairResult, SavedPeerRecord, SendLxmfRequest, SosAlertRecord, SosAudioRecord, SosDeviceTelemetryRecord, SosLocationRecord, SosSettingsRecord, SosStatusRecord, SosTriggerSource, SyncStatus, TelemetryPositionRecord } from "./contracts";
import { ReticulumNodePluginInstance, type PluginListenerHandle } from "./capacitor-plugin";
import { configToPlugin, sosAudioToPlugin, sosSettingsToPlugin, toOperationalSummary, toSosAlertRecord, toSosAudioRecord, toSosLocationRecord, toSosSettingsRecord, toSosStatusRecord } from "./client-config-converters";
import { toChecklistRecord, toChecklistTemplateRecord } from "./checklist-converters";
import { toAppSettingsRecord } from "./converters";
import { toConversationRecord, toErrorEvent, toHubDirectoryUpdatedEvent, toLogEvent, toLxmfDeliveryEvent, toMessageRecord, toOperationalNoticeEvent, toPacketReceivedEvent, toPacketSentEvent, toPeerRecord, toProjectionInvalidationEvent, toSyncStatus } from "./message-converters";
import { eamProjectionRecordToPlugin, eventProjectionRecordToPlugin, legacyImportPayloadToPlugin, toEamProjectionRecord, toEamReadinessSummaryRecord, toEamTeamSummaryRecord, toEventProjectionRecord, toSavedPeerRecord, toTelemetryPositionRecord } from "./projection-converters";
import { decodeBase64ToBytes, encodeBytesToBase64, normalizeHex, pluginRecord, toAnnounceReceivedEvent, toAnnounceRecord, toInstalledPlugin, toInterfaceStatusChangedEvent, toNodeStatus, toPeerChangedEvent, toPluginSensor, toStatusChangedEvent } from "./runtime-converters";
import { TypedEmitter } from "./typed-emitter";
import { CapacitorProjectionClient } from "./capacitor-projection-client";
import { classifyPluginErrors } from "./errors";

export class CapacitorReticulumNodeClient extends CapacitorProjectionClient implements ReticulumNodeClient {
  private readonly emitter = new TypedEmitter<NodeClientEvents>();
  protected readonly plugin = classifyPluginErrors(ReticulumNodePluginInstance);
  private listenerHandles: PluginListenerHandle[] = [];
  private attachPromise: Promise<void> | null = null;
  private generation = 0;

  private async removeListenerHandle(
    handle: PluginListenerHandle,
    operation: string,
  ): Promise<void> {
    try {
      await handle.remove();
    } catch (error: unknown) {
      console.warn(`[node-client] ${operation} failed`, error);
    }
  }

  private async attachListeners(): Promise<void> {
    if (this.attachPromise) {
      return this.attachPromise;
    }

    const generation = this.generation;
    const initialHandleCount = this.listenerHandles.length;
    this.attachPromise = (async () => {
      const register = async (
        eventName: keyof NodeClientEvents,
        map: (raw: Record<string, unknown>) => NodeClientEvents[typeof eventName],
      ) => {
        if (generation !== this.generation) {
          return;
        }
        const handle = await Promise.resolve(
          this.plugin.addListener(eventName, (payload: unknown) => {
            if (generation !== this.generation) {
              return;
            }
            const objectPayload =
              payload && typeof payload === "object"
                ? (payload as Record<string, unknown>)
                : {};
            this.emitter.emit(eventName, map(objectPayload));
          }),
        );
        if (generation !== this.generation) {
          await this.removeListenerHandle(handle, "stale listener cleanup");
          return;
        }
        this.listenerHandles.push(handle);
      };

      await register("statusChanged", toStatusChangedEvent);
      await register("interfaceStatusChanged", toInterfaceStatusChangedEvent);
      await register("announceReceived", toAnnounceReceivedEvent);
      await register("peerChanged", toPeerChangedEvent);
      await register("peerResolved", toPeerRecord);
      await register("packetReceived", toPacketReceivedEvent);
      await register("packetSent", toPacketSentEvent);
      await register("lxmfDelivery", toLxmfDeliveryEvent);
      await register("messageReceived", toMessageRecord);
      await register("messageUpdated", toMessageRecord);
      await register("syncUpdated", toSyncStatus);
      await register("hubDirectoryUpdated", toHubDirectoryUpdatedEvent);
      await register("operationalNotice", toOperationalNoticeEvent);
      await register("projectionInvalidated", toProjectionInvalidationEvent);
      await register("pluginEventPublished", (raw) => ({
        pluginId: String(raw.pluginId ?? raw.plugin_id ?? ""),
        event: pluginRecord(raw.event),
      }));
      await register("sosStatusChanged", (raw) => ({ status: toSosStatusRecord(raw) }));
      await register("sosAlertChanged", (raw) => ({ alert: toSosAlertRecord(raw) }));
      await register("sosTelemetryRequested", () => ({}));
      await register("sosAudioRecordingRequested", (raw) => ({
        incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
        durationSeconds: Number(raw.durationSeconds ?? raw.duration_seconds ?? 0),
      }));
      await register("log", toLogEvent);
      await register("error", toErrorEvent);
    })().catch(async (error: unknown) => {
      const partialHandles = this.listenerHandles.splice(initialHandleCount);
      await Promise.all(partialHandles.map((handle) =>
        this.removeListenerHandle(handle, "partial listener cleanup")));
      this.attachPromise = null;
      throw error;
    });

    return this.attachPromise;
  }

  protected async ready(): Promise<void> {
    await this.attachListeners();
  }

  async start(config: NodeConfig): Promise<void> {
    await this.ready();
    await this.plugin.startNode({ config: configToPlugin(config) });
  }

  async stop(): Promise<void> {
    await this.ready();
    await this.plugin.stopNode();
  }

  async restart(config: NodeConfig): Promise<void> {
    await this.ready();
    await this.plugin.restartNode({ config: configToPlugin(config) });
  }

  async getStatus(): Promise<NodeStatus> {
    await this.ready();
    const status = await this.plugin.getStatus();
    return toNodeStatus(status);
  }

  async checkRnodeBluetoothPermissions(): Promise<{ bluetooth: string }> {
    await this.ready();
    const result = await this.plugin.checkRnodeBluetoothPermissions();
    return { bluetooth: String(result.bluetooth ?? "unavailable") };
  }

  async requestRnodeBluetoothPermissions(): Promise<{ bluetooth: string }> {
    await this.ready();
    const result = await this.plugin.requestRnodeBluetoothPermissions();
    return { bluetooth: String(result.bluetooth ?? "unavailable") };
  }

  async listPairedRnodeBluetoothDevices(): Promise<RnodeBleDeviceRecord[]> {
    await this.ready();
    const result = await this.plugin.listPairedRnodeBluetoothDevices();
    return Array.isArray(result.items) ? result.items : [];
  }

  async scanRnodeBleDevices(timeoutMs?: number): Promise<RnodeBleDeviceRecord[]> {
    await this.ready();
    const result = await this.plugin.scanRnodeBleDevices({ timeoutMs });
    return Array.isArray(result.items) ? result.items : [];
  }

  async pairRnodeBleDevice(id: string): Promise<RnodeBlePairResult> {
    await this.ready();
    const result = await this.plugin.pairRnodeBleDevice({ id });
    return {
      id: String(result.id ?? id),
      address: String(result.address ?? result.id ?? id),
      paired: Boolean(result.paired),
      bondingStarted: Boolean(result.bondingStarted ?? result.bonding_started),
      bondState: String(result.bondState ?? result.bond_state ?? "none"),
    };
  }

  async scanRnodeBluetoothDevices(mode: RnodeBluetoothMode, timeoutMs?: number): Promise<RnodeBluetoothDeviceRecord[]> {
    await this.ready();
    const result = await this.plugin.scanRnodeBluetoothDevices({ mode, timeoutMs });
    return Array.isArray(result.items) ? result.items : [];
  }

  async pairRnodeBluetoothDevice(id: string, mode: RnodeBluetoothMode): Promise<RnodeBlePairResult> {
    await this.ready();
    const result = await this.plugin.pairRnodeBluetoothDevice({ id, mode });
    return {
      id: String(result.id ?? id),
      address: String(result.address ?? result.id ?? id),
      paired: Boolean(result.paired),
      bondingStarted: Boolean(result.bondingStarted ?? result.bonding_started),
      bondState: String(result.bondState ?? result.bond_state ?? "none"),
    };
  }

  async listRnodeUsbDevices(): Promise<RnodeUsbDeviceRecord[]> {
    await this.ready();
    const result = await this.plugin.listRnodeUsbDevices();
    return Array.isArray(result.items) ? result.items : [];
  }

  async requestRnodeUsbPermission(deviceId: number): Promise<{ deviceId: number; granted: boolean }> {
    await this.ready();
    const result = await this.plugin.requestRnodeUsbPermission({ deviceId });
    return {
      deviceId: Number(result.deviceId ?? deviceId),
      granted: Boolean(result.granted),
    };
  }

  async startRnodeUsbBluetoothPairing(deviceId: number, bluetoothDeviceId?: string): Promise<RnodeUsbPairResult> {
    await this.ready();
    const result = await this.plugin.startRnodeUsbBluetoothPairing({ deviceId, bluetoothDeviceId });
    return {
      id: String(result.id ?? result.address ?? ""),
      address: String(result.address ?? result.id ?? ""),
      paired: Boolean(result.paired),
      pairingModeStarted: Boolean(result.pairingModeStarted ?? result.pairing_mode_started),
      manualPinRequired: Boolean(result.manualPinRequired ?? result.manual_pin_required),
      pin: typeof result.pin === "string" ? result.pin : undefined,
      bondState: String(result.bondState ?? result.bond_state ?? "none"),
      message: typeof result.message === "string" ? result.message : undefined,
    };
  }

  async cancelRnodeUsbBluetoothPairing(deviceId?: number): Promise<void> {
    await this.ready();
    await this.plugin.cancelRnodeUsbBluetoothPairing({ deviceId });
  }

  async connectPeer(destinationHex: string): Promise<void> {
    await this.ready();
    await this.plugin.connectPeer({ destinationHex: normalizeHex(destinationHex) });
  }

  async disconnectPeer(destinationHex: string): Promise<void> {
    await this.ready();
    await this.plugin.disconnectPeer({
      destinationHex: normalizeHex(destinationHex),
    });
  }

  async announceNow(): Promise<void> {
    await this.ready();
    await this.plugin.announceNow();
  }

  async requestPeerIdentity(destinationHex: string): Promise<void> {
    await this.ready();
    await this.plugin.requestPeerIdentity({
      destinationHex: normalizeHex(destinationHex),
    });
  }

  async sendBytes(destinationHex: string, bytes: Uint8Array, options?: PacketSendOptions): Promise<void> {
    await this.ready();
    await this.plugin.send({
      destinationHex: normalizeHex(destinationHex),
      bytesBase64: encodeBytesToBase64(bytes),
      fieldsBase64: options?.fieldsBase64,
      sendMode: options?.sendMode,
    });
  }

  async sendLxmf(request: SendLxmfRequest): Promise<string> {
    await this.ready();
    const result = await this.plugin.sendLxmf({
      destinationHex: normalizeHex(request.destinationHex),
      bodyUtf8: request.bodyUtf8,
      title: request.title,
      sendMode: request.sendMode,
    });
    return normalizeHex(String(result.messageIdHex ?? ""));
  }

  async retryLxmf(messageIdHex: string): Promise<void> {
    await this.ready();
    await this.plugin.retryLxmf({ messageIdHex: normalizeHex(messageIdHex) });
  }

  async cancelLxmf(messageIdHex: string): Promise<void> {
    await this.ready();
    await this.plugin.cancelLxmf({ messageIdHex: normalizeHex(messageIdHex) });
  }

  async broadcastBytes(bytes: Uint8Array, options?: PacketSendOptions): Promise<void> {
    await this.ready();
    await this.plugin.broadcast({
      bytesBase64: encodeBytesToBase64(bytes),
      fieldsBase64: options?.fieldsBase64,
    });
  }

  async setActivePropagationNode(destinationHex?: string): Promise<void> {
    await this.ready();
    await this.plugin.setActivePropagationNode({
      destinationHex: destinationHex ? normalizeHex(destinationHex) : undefined,
    });
  }

  async requestLxmfSync(limit?: number): Promise<void> {
    await this.ready();
    await this.plugin.requestLxmfSync({ limit });
  }

  async listAnnounces(): Promise<AnnounceRecord[]> {
    await this.ready();
    const result = await this.plugin.listAnnounces();
    return Array.isArray(result.items) ? result.items.map(toAnnounceRecord) : [];
  }


  async setAnnounceCapabilities(capabilityString: string): Promise<void> {
    await this.ready();
    await this.plugin.setAnnounceCapabilities({ capabilityString });
  }

  async setLogLevel(level: LogLevel): Promise<void> {
    await this.ready();
    await this.plugin.setLogLevel({ level });
  }

  async logMessage(level: LogLevel, message: string): Promise<void> {
    await this.ready();
    await this.plugin.logMessage({ level, message });
  }

  async refreshHubDirectory(): Promise<void> {
    await this.ready();
    await this.plugin.refreshHubDirectory();
  }

  async getHubDirectorySnapshot() {
    await this.ready();
    return toHubDirectoryUpdatedEvent(await this.plugin.getHubDirectorySnapshot());
  }

  async setActiveTeam(teamUid: string): Promise<void> {
    await this.ready();
    await this.plugin.setActiveTeam({ teamUid });
  }

  on<K extends keyof NodeClientEvents>(
    event: K,
    handler: (payload: NodeClientEvents[K]) => void,
  ): () => void {
    // Event subscriptions are synchronous; the next awaited client operation retries and surfaces failures.
    void this.attachListeners().catch(() => undefined);
    return this.emitter.on(event, handler);
  }

  async dispose(): Promise<void> {
    this.generation += 1;
    const handles = this.listenerHandles.splice(0);
    await Promise.all(handles.map((handle) =>
      this.removeListenerHandle(handle, "listener disposal")));
    this.attachPromise = null;
    this.emitter.clear();
  }
}
