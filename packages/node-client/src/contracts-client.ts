import type {
  LogLevel, HubMode, RnodeRegion, RnodeProfileId, RnodeConnectionMode, RuntimeReadinessState, PeerState, AnnounceDestinationKind,
  AnnounceClass, SendOutcome, LxmfDeliveryStatus, TransportDeliveryState, ApplicationAckState, SendMode, LxmfDeliveryMethod, LxmfDeliveryRepresentation,
  LxmfFallbackStage, MessageMethod, MessageState, MessageDirection, ClientMode, ProjectionScope, PluginState, PluginCapabilityRecord,
  PluginMessageDescriptorRecord, InstalledPluginRecord, PluginSensorRecord, SosState, SosTriggerSource, SosMessageKind, RnodeSettingsRecord, RnodeBleDeviceRecord,
  RnodeBlePairResult, RnodeUsbDeviceRecord, RnodeUsbPairResult, NodeConfig, NodeStatus, RuntimeInterfaceReadinessRecord, RuntimeReadinessSnapshot, InterfaceStatusRecord,
  PeerChange, StatusChangedEvent, InterfaceStatusChangedEvent, AnnounceReceivedEvent, AnnounceRecord, PeerChangedEvent, PacketReceivedEvent, PacketSendOptions,
  PacketSentEvent, LxmfDeliveryEvent, MessageRecord, PeerRecord, ConversationRecord, SyncPhase, SyncStatus, SendLxmfRequest,
  HubSettingsRecord,
} from "./contracts-core";

import type {
  ChecklistMode, ChecklistSyncState, ChecklistOriginType, ChecklistTaskStatus, ChecklistUserTaskStatus, ChecklistColumnType, ChecklistStatusCounts, ChecklistColumnRecord,
  ChecklistCellRecord, ChecklistTaskRecord, ChecklistFeedPublicationRecord, ChecklistRecord, ChecklistTemplateRecord, HubDirectoryPeerRecord, TelemetrySettingsRecord, ChecklistSettingsRecord,
  AppSettingsRecord, SavedPeerRecord, EamSourceRecord, EamProjectionRecord, EamTeamSummaryRecord, EamReadinessStatusMetricRecord, EamReadinessMessageRecord, EamReadinessSummaryRecord,
  EventProjectionRecord, TelemetryPositionRecord, SosSettingsRecord, SosDeviceTelemetryRecord, SosStatusRecord, SosAlertRecord, SosLocationRecord, SosAudioRecord,
  LegacyImportPayload, ProjectionInvalidationEvent, OperationalSummary, HubDirectoryUpdatedEvent, NodeLogEvent, NodeOperationalNoticeEvent,
  NodeErrorEvent, NodeClientEvents,
} from "./contracts-domain";

export type ChecklistDeleteOptions = {
  deleteRemote?: boolean;
};

export interface ReticulumNodeClient {
  start(config: NodeConfig): Promise<void>;
  stop(): Promise<void>;
  restart(config: NodeConfig): Promise<void>;
  getStatus(): Promise<NodeStatus>;
  checkRnodeBluetoothPermissions(): Promise<{ bluetooth: string }>;
  requestRnodeBluetoothPermissions(): Promise<{ bluetooth: string }>;
  listPairedRnodeBluetoothDevices(): Promise<RnodeBleDeviceRecord[]>;
  scanRnodeBleDevices(timeoutMs?: number): Promise<RnodeBleDeviceRecord[]>;
  pairRnodeBleDevice(id: string): Promise<RnodeBlePairResult>;
  listRnodeUsbDevices(): Promise<RnodeUsbDeviceRecord[]>;
  requestRnodeUsbPermission(deviceId: number): Promise<{ deviceId: number; granted: boolean }>;
  startRnodeUsbBluetoothPairing(deviceId: number, bluetoothDeviceId?: string): Promise<RnodeUsbPairResult>;
  cancelRnodeUsbBluetoothPairing(deviceId?: number): Promise<void>;
  connectPeer(destinationHex: string): Promise<void>;
  disconnectPeer(destinationHex: string): Promise<void>;
  announceNow(): Promise<void>;
  requestPeerIdentity(destinationHex: string): Promise<void>;
  sendBytes(destinationHex: string, bytes: Uint8Array, options?: PacketSendOptions): Promise<void>;
  sendLxmf(request: SendLxmfRequest): Promise<string>;
  retryLxmf(messageIdHex: string): Promise<void>;
  cancelLxmf(messageIdHex: string): Promise<void>;
  broadcastBytes(bytes: Uint8Array, options?: PacketSendOptions): Promise<void>;
  setActivePropagationNode(destinationHex?: string): Promise<void>;
  requestLxmfSync(limit?: number): Promise<void>;
  listAnnounces(): Promise<AnnounceRecord[]>;
  refreshPlugins(): Promise<InstalledPluginRecord[]>;
  listPlugins(): Promise<InstalledPluginRecord[]>;
  approvePluginPublisher(pluginId: string, displayName?: string): Promise<void>;
  revokePluginPublisher(fingerprint: string): Promise<void>;
  setPluginEnabled(pluginId: string, enabled: boolean): Promise<void>;
  grantPluginCapabilities(pluginId: string, capabilities: PluginCapabilityRecord): Promise<void>;
  openPluginConfiguration(pluginId: string): Promise<void>;
  listPluginSensors(): Promise<PluginSensorRecord[]>;
  listPeers(): Promise<PeerRecord[]>;
  listConversations(): Promise<ConversationRecord[]>;
  listMessages(conversationId?: string): Promise<MessageRecord[]>;
  deleteConversation(conversationId: string): Promise<void>;
  getLxmfSyncStatus(): Promise<SyncStatus>;
  listTelemetryDestinations(): Promise<string[]>;
  legacyImportCompleted(): Promise<boolean>;
  importLegacyState(payload: LegacyImportPayload): Promise<void>;
  getAppSettings(): Promise<AppSettingsRecord | null>;
  setAppSettings(settings: AppSettingsRecord): Promise<void>;
  getSavedPeers(): Promise<SavedPeerRecord[]>;
  setSavedPeers(peers: SavedPeerRecord[]): Promise<void>;
  getOperationalSummary(): Promise<OperationalSummary>;
  listActiveChecklists(search?: string): Promise<ChecklistRecord[]>;
  getChecklist(checklistUid: string): Promise<ChecklistRecord | null>;
  listChecklistTemplates(search?: string): Promise<ChecklistTemplateRecord[]>;
  importChecklistTemplateCsv(input: {
    templateUid?: string;
    name: string;
    description?: string;
    csvText: string;
    sourceFilename?: string;
  }): Promise<ChecklistTemplateRecord>;
  createChecklistFromTemplate(input: {
    checklistUid?: string;
    missionUid?: string;
    templateUid: string;
    name: string;
    description: string;
    startTime: string;
    createdByTeamMemberRnsIdentity?: string;
    createdByTeamMemberDisplayName?: string;
  }): Promise<void>;
  createOnlineChecklist(input: {
    checklistUid?: string;
    missionUid?: string;
    templateUid: string;
    name: string;
    description: string;
    startTime: string;
    createdByTeamMemberRnsIdentity?: string;
    createdByTeamMemberDisplayName?: string;
  }): Promise<void>;
  updateChecklist(input: {
    checklistUid: string;
    patch: {
      missionUid?: string;
      templateUid?: string;
      name?: string;
      description?: string;
      startTime?: string;
    };
  }): Promise<void>;
  deleteChecklist(checklistUid: string, options?: ChecklistDeleteOptions): Promise<void>;
  joinChecklist(checklistUid: string): Promise<void>;
  uploadChecklist(checklistUid: string): Promise<void>;
  setChecklistTaskStatus(input: {
    checklistUid: string;
    taskUid: string;
    userStatus: ChecklistUserTaskStatus;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  addChecklistTaskRow(input: {
    checklistUid: string;
    taskUid?: string;
    number: number;
    dueRelativeMinutes?: number;
    legacyValue?: string;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  deleteChecklistTaskRow(input: {
    checklistUid: string;
    taskUid: string;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  setChecklistTaskRowStyle(input: {
    checklistUid: string;
    taskUid: string;
    rowBackgroundColor?: string;
    lineBreakEnabled?: boolean;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  setChecklistTaskCell(input: {
    checklistUid: string;
    taskUid: string;
    columnUid: string;
    value?: string;
    updatedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  getEams(): Promise<EamProjectionRecord[]>;
  upsertEam(eam: EamProjectionRecord): Promise<void>;
  deleteEam(callsign: string, deletedAtMs?: number): Promise<void>;
  deleteLocalEam(callsign: string, deletedAtMs?: number): Promise<void>;
  getEamTeamSummary(teamUid: string): Promise<EamTeamSummaryRecord | null>;
  getEamReadinessSummary(): Promise<EamReadinessSummaryRecord>;
  getEvents(): Promise<EventProjectionRecord[]>;
  upsertEvent(event: EventProjectionRecord): Promise<void>;
  deleteEvent(uid: string, deletedAtMs?: number): Promise<void>;
  getTelemetryPositions(): Promise<TelemetryPositionRecord[]>;
  recordLocalTelemetryFix(position: TelemetryPositionRecord): Promise<void>;
  deleteLocalTelemetry(callsign: string): Promise<void>;
  getSosSettings(): Promise<SosSettingsRecord>;
  setSosSettings(settings: SosSettingsRecord): Promise<void>;
  setSosPin(pin?: string): Promise<void>;
  getSosStatus(): Promise<SosStatusRecord>;
  triggerSos(source?: SosTriggerSource): Promise<SosStatusRecord>;
  deactivateSos(pin?: string): Promise<SosStatusRecord>;
  submitSosTelemetry(telemetry: SosDeviceTelemetryRecord): Promise<void>;
  listSosAlerts(): Promise<SosAlertRecord[]>;
  listSosLocations(): Promise<SosLocationRecord[]>;
  listSosAudio(): Promise<SosAudioRecord[]>;
  recordSosAudio(audio: SosAudioRecord): Promise<void>;
  setAnnounceCapabilities(capabilityString: string): Promise<void>;
  setLogLevel(level: LogLevel): Promise<void>;
  logMessage(level: LogLevel, message: string): Promise<void>;
  refreshHubDirectory(): Promise<void>;
  on<K extends keyof NodeClientEvents>(
    event: K,
    handler: (payload: NodeClientEvents[K]) => void,
  ): () => void;
  dispose(): Promise<void>;
}

export interface ReticulumNodeClientFactoryOptions {
  mode?: "auto" | "capacitor" | "mock" | "web";
}
