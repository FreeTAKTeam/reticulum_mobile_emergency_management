export type LogLevel = "Trace" | "Debug" | "Info" | "Warn" | "Error";
export type HubMode = "Autonomous" | "SemiAutonomous" | "Connected";
export type RnodeRegion = "US915" | "EU868";
export type RnodeProfileId = "REM-MF-URBAN-v1" | "REM-LF-RURAL-v1" | "REM-LM-EXTREME-v1";
export type RnodeConnectionMode = "ble" | "bluetooth_classic" | "usb" | "tcp";
export type RuntimeReadinessState = "Pending" | "Ready" | "Failed" | "Unsupported" | "Disabled";
export type PeerState = "Connecting" | "Connected" | "Disconnected";
export type AnnounceDestinationKind = "app" | "lxmf_delivery" | "lxmf_propagation" | "other";
export type AnnounceClass = "PeerApp" | "RchHubServer" | "PropagationNode" | "LxmfDelivery" | "Other";
export type SendOutcome =
  | "SentDirect"
  | "SentBroadcast"
  | "DroppedMissingDestinationIdentity"
  | "DroppedCiphertextTooLarge"
  | "DroppedEncryptFailed"
  | "DroppedNoRoute";
export type LxmfDeliveryStatus = "Sent" | "SentToPropagation" | "Acknowledged" | "Failed" | "TimedOut";
export type TransportDeliveryState =
  | "Queued"
  | "Sending"
  | "SentDirect"
  | "SentToPropagation"
  | "TransportDelivered"
  | "Failed"
  | "TimedOut"
  | "Cancelled";
export type ApplicationAckState =
  | "NotRequired"
  | "Waiting"
  | "Accepted"
  | "Completed"
  | "Rejected"
  | "Failed";
export type SendMode = "Auto" | "DirectOnly" | "PropagationOnly";
export type LxmfDeliveryMethod = "Direct" | "Opportunistic" | "Propagated";
export type LxmfDeliveryRepresentation = "Packet" | "Resource";
export type LxmfFallbackStage = "AfterDirectRetryBudget";
export type MessageMethod = "Direct" | "Opportunistic" | "Propagated" | "Resource";
export type MessageState =
  | "Queued"
  | "PathRequested"
  | "LinkEstablishing"
  | "Sending"
  | "SentDirect"
  | "SentToPropagation"
  | "Delivered"
  | "Failed"
  | "TimedOut"
  | "Cancelled"
  | "Received";
export type MessageDirection = "Inbound" | "Outbound";
export type ClientMode = "auto" | "capacitor";
export type ProjectionScope =
  | "AppSettings"
  | "SavedPeers"
  | "OperationalSummary"
  | "Peers"
  | "SyncStatus"
  | "HubRegistration"
  | "Eams"
  | "Events"
  | "Conversations"
  | "Messages"
  | "Telemetry"
  | "Checklists"
  | "ChecklistDetail"
  | "Sos";

export type SosState = "Idle" | "Countdown" | "Sending" | "Active";
export type SosTriggerSource =
  | "Manual"
  | "FloatingButton"
  | "Shake"
  | "TapPattern"
  | "PowerButton"
  | "Restore"
  | "Remote";
export type SosMessageKind = "Active" | "Update" | "Cancelled";

export interface RnodeSettingsRecord {
  enabled: boolean;
  connectionMode: RnodeConnectionMode;
  peripheralId: string;
  displayName: string;
  region: RnodeRegion;
  profile: RnodeProfileId;
}

export interface RnodeBleDeviceRecord {
  id: string;
  address: string;
  name: string;
  rssi?: number;
  paired: boolean;
  bondState?: string;
}

export interface RnodeBlePairResult {
  id: string;
  address: string;
  paired: boolean;
  bondingStarted: boolean;
  bondState: string;
}

export interface RnodeUsbDeviceRecord {
  deviceId: number;
  vendorId: number;
  productId: number;
  deviceName: string;
  manufacturerName: string;
  productName: string;
  serialNumber: string;
  hasPermission: boolean;
}

export interface RnodeUsbPairResult {
  id: string;
  address: string;
  paired: boolean;
  pairingModeStarted: boolean;
  manualPinRequired: boolean;
  pin?: string;
  bondState: string;
  message?: string;
}

export interface NodeConfig {
  name: string;
  storageDir?: string;
  tcpClients: string[];
  broadcast: boolean;
  transportNodeEnabled: boolean;
  announceIntervalSeconds: number;
  staleAfterMinutes: number;
  announceCapabilities: string;
  hubMode: HubMode;
  hubIdentityHash?: string;
  hubApiBaseUrl?: string;
  hubApiKey?: string;
  hubRefreshIntervalSeconds: number;
  rnode: RnodeSettingsRecord;
}

export interface NodeStatus {
  running: boolean;
  name: string;
  identityHex: string;
  appDestinationHex: string;
  lxmfDestinationHex: string;
  lastError?: string;
  readiness: RuntimeReadinessSnapshot;
  interfaces: InterfaceStatusRecord[];
}

export interface RuntimeInterfaceReadinessRecord {
  id: string;
  label: string;
  state: RuntimeReadinessState;
  detail: string;
  lastError?: string;
}

export interface RuntimeReadinessSnapshot {
  state: RuntimeReadinessState;
  interfaces: RuntimeInterfaceReadinessRecord[];
}

export interface InterfaceStatusRecord {
  interfaceHex: string;
  label: string;
  kind: string;
  state: string;
  lastError?: string;
  rxPackets: number;
  rxBytes: number;
  lastActivityMs: number;
}

export interface PeerChange {
  destinationHex: string;
  identityHex?: string;
  lxmfDestinationHex?: string;
  displayName?: string;
  appData?: string;
  state?: PeerState;
  saved: boolean;
  stale: boolean;
  activeLink: boolean;
  hubDerived: boolean;
  lastError?: string;
  lastResolutionError?: string;
  lastResolutionAttemptAtMs?: number;
  lastSeenAtMs?: number;
  announceLastSeenAtMs?: number;
  lxmfLastSeenAtMs?: number;
}

export interface StatusChangedEvent {
  status: NodeStatus;
}

export interface InterfaceStatusChangedEvent {
  status: InterfaceStatusRecord;
}

export interface AnnounceReceivedEvent {
  destinationHex: string;
  identityHex: string;
  destinationKind: AnnounceDestinationKind;
  announceClass: AnnounceClass;
  appData: string;
  displayName?: string;
  hops: number;
  interfaceHex: string;
  receivedAtMs: number;
}

export interface AnnounceRecord {
  destinationHex: string;
  identityHex: string;
  destinationKind: AnnounceDestinationKind;
  announceClass: AnnounceClass;
  appData: string;
  displayName?: string;
  hops: number;
  interfaceHex: string;
  receivedAtMs: number;
}

export interface PeerChangedEvent {
  change: PeerChange;
}

export interface PacketReceivedEvent {
  destinationHex: string;
  sourceHex?: string;
  bytes: Uint8Array;
  fieldsBase64?: string;
}

export interface PacketSendOptions {
  fieldsBase64?: string;
  sendMode?: SendMode;
}

export interface PacketSentEvent {
  destinationHex: string;
  bytes: Uint8Array;
  outcome: SendOutcome;
}

export interface LxmfDeliveryEvent {
  messageIdHex: string;
  destinationHex: string;
  sourceHex?: string;
  correlationId?: string;
  commandId?: string;
  commandType?: string;
  eventUid?: string;
  missionUid?: string;
  status: LxmfDeliveryStatus;
  transportState: TransportDeliveryState;
  applicationAckState: ApplicationAckState;
  method: LxmfDeliveryMethod;
  representation: LxmfDeliveryRepresentation;
  relayDestinationHex?: string;
  fallbackStage?: LxmfFallbackStage;
  detail?: string;
  sentAtMs: number;
  updatedAtMs: number;
}

export interface MessageRecord {
  messageIdHex: string;
  conversationId: string;
  direction: MessageDirection;
  destinationHex: string;
  sourceHex?: string;
  requestedDestinationHex?: string;
  deliveryDestinationHex?: string;
  recipientIdentityHex?: string;
  lastWireMessageIdHex?: string;
  title?: string;
  bodyUtf8: string;
  method: MessageMethod;
  state: MessageState;
  transportState: TransportDeliveryState;
  applicationAckState: ApplicationAckState;
  detail?: string;
  sentAtMs?: number;
  receivedAtMs?: number;
  updatedAtMs: number;
}

export interface PeerRecord {
  destinationHex: string;
  identityHex?: string;
  lxmfDestinationHex?: string;
  displayName?: string;
  appData?: string;
  state: PeerState;
  saved: boolean;
  stale: boolean;
  activeLink: boolean;
  hubDerived: boolean;
  lastResolutionError?: string;
  lastResolutionAttemptAtMs?: number;
  lastSeenAtMs: number;
  announceLastSeenAtMs?: number;
  lxmfLastSeenAtMs?: number;
}

export interface ConversationRecord {
  conversationId: string;
  peerDestinationHex: string;
  peerDisplayName?: string;
  lastMessagePreview?: string;
  lastMessageAtMs: number;
  unreadCount: number;
  lastMessageState?: MessageState;
}

export type SyncPhase =
  | "Idle"
  | "PathRequested"
  | "LinkEstablishing"
  | "RequestSent"
  | "Receiving"
  | "Complete"
  | "Failed";

export interface SyncStatus {
  phase: SyncPhase;
  activePropagationNodeHex?: string;
  requestedAtMs?: number;
  completedAtMs?: number;
  messagesReceived: number;
  detail?: string;
}

export interface SendLxmfRequest {
  destinationHex: string;
  bodyUtf8: string;
  title?: string;
  sendMode?: SendMode;
}

export interface HubSettingsRecord {
  mode: HubMode;
  identityHash: string;
  apiBaseUrl: string;
  apiKey: string;
  refreshIntervalSeconds: number;
}

export type ChecklistMode = "ONLINE" | "OFFLINE";
export type ChecklistSyncState = "LOCAL_ONLY" | "UPLOAD_PENDING" | "SYNCED";
export type ChecklistOriginType = "RCH_TEMPLATE" | "BLANK_TEMPLATE" | "CSV_IMPORT" | "EXISTING_TEMPLATE_CLONE";
export type ChecklistTaskStatus = "PENDING" | "COMPLETE" | "COMPLETE_LATE" | "LATE";
export type ChecklistUserTaskStatus = "PENDING" | "COMPLETE";
export type ChecklistColumnType =
  | "SHORT_STRING"
  | "LONG_STRING"
  | "INTEGER"
  | "ACTUAL_TIME"
  | "RELATIVE_TIME";

export interface ChecklistStatusCounts {
  pendingCount: number;
  lateCount: number;
  completeCount: number;
}

export interface ChecklistColumnRecord {
  columnUid: string;
  columnName: string;
  displayOrder: number;
  columnType: ChecklistColumnType;
  columnEditable: boolean;
  backgroundColor?: string;
  textColor?: string;
  isRemovable: boolean;
  systemKey?: string;
}

export interface ChecklistCellRecord {
  cellUid: string;
  taskUid: string;
  columnUid: string;
  value?: string;
  updatedAt?: string;
  updatedByTeamMemberRnsIdentity?: string;
}

export interface ChecklistTaskRecord {
  taskUid: string;
  number: number;
  userStatus: ChecklistUserTaskStatus;
  taskStatus: ChecklistTaskStatus;
  isLate: boolean;
  updatedAt?: string;
  deletedAt?: string;
  customStatus?: string;
  dueRelativeMinutes?: number;
  dueDtg?: string;
  notes?: string;
  rowBackgroundColor?: string;
  lineBreakEnabled?: boolean;
  legacyValue?: string;
  completedAt?: string;
  completedByTeamMemberRnsIdentity?: string;
  cells: ChecklistCellRecord[];
}

export interface ChecklistFeedPublicationRecord {
  publicationUid: string;
  checklistUid: string;
  missionFeedUid: string;
  publishedAt?: string;
  publishedByTeamMemberRnsIdentity?: string;
}

export interface ChecklistRecord {
  uid: string;
  missionUid?: string;
  templateUid?: string;
  templateVersion?: number;
  templateName?: string;
  name: string;
  description: string;
  startTime?: string;
  mode: ChecklistMode;
  syncState: ChecklistSyncState;
  originType: ChecklistOriginType;
  checklistStatus: ChecklistTaskStatus;
  createdAt?: string;
  createdByTeamMemberRnsIdentity: string;
  createdByTeamMemberDisplayName?: string;
  updatedAt?: string;
  lastChangedByTeamMemberRnsIdentity?: string;
  deletedAt?: string;
  uploadedAt?: string;
  participantRnsIdentities: string[];
  expectedTaskCount?: number;
  progressPercent: number;
  counts: ChecklistStatusCounts;
  columns: ChecklistColumnRecord[];
  tasks: ChecklistTaskRecord[];
  feedPublications: ChecklistFeedPublicationRecord[];
}

export interface ChecklistTemplateRecord {
  uid: string;
  name: string;
  description: string;
  version: number;
  originType: ChecklistOriginType;
  createdAt?: string;
  updatedAt?: string;
  sourceFilename?: string;
  columns: ChecklistColumnRecord[];
  tasks: ChecklistTaskRecord[];
}

export interface HubDirectoryPeerRecord {
  identity: string;
  destinationHash: string;
  displayName?: string;
  announceCapabilities: string[];
  clientType?: string;
  registeredMode?: string;
  lastSeen?: string;
  status?: string;
}

export interface TelemetrySettingsRecord {
  enabled: boolean;
  publishIntervalSeconds: number;
  accuracyThresholdMeters?: number;
  staleAfterMinutes: number;
  expireAfterMinutes: number;
}

export interface ChecklistSettingsRecord {
  defaultTaskDueStepMinutes: number;
}

export interface AppSettingsRecord {
  displayName: string;
  autoConnectSaved: boolean;
  announceCapabilities: string;
  tcpClients: string[];
  broadcast: boolean;
  transportNodeEnabled: boolean;
  announceIntervalSeconds: number;
  telemetry: TelemetrySettingsRecord;
  hub: HubSettingsRecord;
  checklists: ChecklistSettingsRecord;
  rnode: RnodeSettingsRecord;
}

export interface SavedPeerRecord {
  destination: string;
  label?: string;
  savedAt: number;
  identityHex?: string;
  lxmfDestinationHex?: string;
  appData?: string;
  displayName?: string;
  lastRouteSeenAtMs?: number;
  lastHops?: number;
}

export interface EamSourceRecord {
  rns_identity: string;
  display_name?: string;
}

export interface EamProjectionRecord {
  callsign: string;
  groupName: string;
  securityStatus: string;
  capabilityStatus: string;
  preparednessStatus: string;
  medicalStatus: string;
  mobilityStatus: string;
  commsStatus: string;
  notes?: string;
  updatedAt: number;
  deletedAt?: number;
  eamUid?: string;
  teamMemberUid?: string;
  teamUid?: string;
  reportedAt?: string;
  reportedBy?: string;
  overallStatus?: string;
  confidence?: number;
  ttlSeconds?: number;
  source?: EamSourceRecord;
  syncState?: string;
  syncError?: string;
  draftCreatedAt?: number;
  lastSyncedAt?: number;
}

export interface EamTeamSummaryRecord {
  teamUid: string;
  total: number;
  activeTotal: number;
  deletedTotal: number;
  overallStatus?: string;
  greenTotal: number;
  yellowTotal: number;
  redTotal: number;
  updatedAt: number;
}

export interface EamReadinessStatusMetricRecord {
  field: string;
  label: string;
  score: number;
  band: string;
  ringColor: string;
}

export interface EamReadinessMessageRecord {
  callsign: string;
  overallScore: number;
  overallBand: string;
  overallRingColor: string;
}

export interface EamReadinessSummaryRecord {
  activeTotal: number;
  updatedAt: number;
  statusMetrics: EamReadinessStatusMetricRecord[];
  messages: EamReadinessMessageRecord[];
}

export interface EventProjectionRecord {
  command_id: string;
  source: {
    rns_identity: string;
    display_name?: string;
  };
  timestamp: string;
  command_type: string;
  args: {
    entry_uid: string;
    mission_uid: string;
    content: string;
    callsign: string;
    server_time?: string;
    client_time?: string;
    keywords: string[];
    content_hashes: string[];
    source_identity?: string;
    source_display_name?: string;
  };
  correlation_id?: string;
  topics: string[];
  deleted_at?: number;
  updatedAt: number;
}

export interface TelemetryPositionRecord {
  callsign: string;
  lat: number;
  lon: number;
  alt?: number;
  course?: number;
  speed?: number;
  accuracy?: number;
  updatedAt: number;
}

export interface SosSettingsRecord {
  enabled: boolean;
  messageTemplate: string;
  cancelMessageTemplate: string;
  countdownSeconds: number;
  includeLocation: boolean;
  triggerShake: boolean;
  triggerTapPattern: boolean;
  triggerPowerButton: boolean;
  shakeSensitivity: number;
  audioRecording: boolean;
  audioDurationSeconds: number;
  periodicUpdates: boolean;
  updateIntervalSeconds: number;
  floatingButton: boolean;
  silentAutoAnswer: boolean;
  deactivationPinHash?: string;
  deactivationPinSalt?: string;
  floatingButtonX: number;
  floatingButtonY: number;
  activePillX: number;
  activePillY: number;
}

export interface SosDeviceTelemetryRecord {
  lat?: number;
  lon?: number;
  alt?: number;
  speed?: number;
  course?: number;
  accuracy?: number;
  batteryPercent?: number;
  batteryCharging?: boolean;
  updatedAtMs: number;
}

export interface SosStatusRecord {
  state: SosState;
  incidentId?: string;
  triggerSource?: SosTriggerSource;
  countdownDeadlineMs?: number;
  activatedAtMs?: number;
  lastSentAtMs?: number;
  lastUpdateAtMs?: number;
  updatedAtMs: number;
}

export interface SosAlertRecord {
  incidentId: string;
  sourceHex: string;
  conversationId: string;
  state: SosMessageKind;
  active: boolean;
  bodyUtf8: string;
  lat?: number;
  lon?: number;
  batteryPercent?: number;
  audioId?: string;
  messageIdHex?: string;
  receivedAtMs: number;
  updatedAtMs: number;
}

export interface SosLocationRecord {
  incidentId: string;
  sourceHex: string;
  lat: number;
  lon: number;
  alt?: number;
  accuracy?: number;
  batteryPercent?: number;
  recordedAtMs: number;
}

export interface SosAudioRecord {
  audioId: string;
  incidentId: string;
  sourceHex: string;
  path: string;
  mimeType: string;
  durationSeconds: number;
  createdAtMs: number;
}

export interface LegacyImportPayload {
  settings?: AppSettingsRecord;
  savedPeers: SavedPeerRecord[];
  eams: EamProjectionRecord[];
  events: EventProjectionRecord[];
  messages: MessageRecord[];
  telemetryPositions: TelemetryPositionRecord[];
}

export interface ProjectionInvalidationEvent {
  scope: ProjectionScope;
  key?: string;
  revision: number;
  updatedAtMs: number;
  reason?: string;
}

export interface OperationalSummary {
  running: boolean;
  peerCountTotal: number;
  savedPeerCount: number;
  connectedPeerCount: number;
  conversationCount: number;
  messageCount: number;
  eamCount: number;
  eventCount: number;
  telemetryCount: number;
  activePropagationNodeHex?: string;
  updatedAtMs: number;
}

export interface WatchStatusServerSettings {
  enabled: boolean;
  port: number;
}

export interface WatchStatusServerState extends WatchStatusServerSettings {
  url: string;
  currentUrl: string;
  running: boolean;
  bindError?: string;
}

export interface HubDirectoryUpdatedEvent {
  effectiveConnectedMode: boolean;
  items: HubDirectoryPeerRecord[];
  receivedAtMs: number;
}

export interface NodeLogEvent {
  level: LogLevel;
  message: string;
}

export interface NodeOperationalNoticeEvent {
  level: LogLevel;
  message: string;
  atMs: number;
}

export interface NodeErrorEvent {
  code: string;
  message: string;
}

export interface NodeClientEvents {
  statusChanged: StatusChangedEvent;
  interfaceStatusChanged: InterfaceStatusChangedEvent;
  announceReceived: AnnounceReceivedEvent;
  peerChanged: PeerChangedEvent;
  peerResolved: PeerRecord;
  packetReceived: PacketReceivedEvent;
  packetSent: PacketSentEvent;
  lxmfDelivery: LxmfDeliveryEvent;
  messageReceived: MessageRecord;
  messageUpdated: MessageRecord;
  syncUpdated: SyncStatus;
  hubDirectoryUpdated: HubDirectoryUpdatedEvent;
  operationalNotice: NodeOperationalNoticeEvent;
  projectionInvalidated: ProjectionInvalidationEvent;
  sosStatusChanged: { status: SosStatusRecord };
  sosAlertChanged: { alert: SosAlertRecord };
  sosTelemetryRequested: Record<string, never>;
  sosAudioRecordingRequested: { incidentId: string; durationSeconds: number };
  log: NodeLogEvent;
  error: NodeErrorEvent;
}

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
  getWatchStatusServerSettings(): Promise<WatchStatusServerState>;
  setWatchStatusServerSettings(settings: WatchStatusServerSettings): Promise<void>;
  getWatchStatusServerState(): Promise<WatchStatusServerState>;
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
