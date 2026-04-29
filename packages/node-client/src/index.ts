import { Capacitor, registerPlugin } from "@capacitor/core";

export type LogLevel = "Trace" | "Debug" | "Info" | "Warn" | "Error";
export type HubMode = "Autonomous" | "SemiAutonomous" | "Connected";
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
  | "Plugins"
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

export interface NodeConfig {
  name: string;
  storageDir?: string;
  tcpClients: string[];
  broadcast: boolean;
  announceIntervalSeconds: number;
  staleAfterMinutes: number;
  announceCapabilities: string;
  hubMode: HubMode;
  hubIdentityHash?: string;
  hubApiBaseUrl?: string;
  hubApiKey?: string;
  hubRefreshIntervalSeconds: number;
}

export interface NodeStatus {
  running: boolean;
  name: string;
  identityHex: string;
  appDestinationHex: string;
  lxmfDestinationHex: string;
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
  dedicatedFields?: Record<string, string>;
  fieldsBase64?: string;
}

export interface PacketSendOptions {
  dedicatedFields?: Record<string, string>;
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
  title?: string;
  bodyUtf8: string;
  method: MessageMethod;
  state: MessageState;
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
  announceIntervalSeconds: number;
  telemetry: TelemetrySettingsRecord;
  hub: HubSettingsRecord;
  checklists: ChecklistSettingsRecord;
}

export type PluginState =
  | "Discovered"
  | "Disabled"
  | "Enabled"
  | "Loaded"
  | "Initialized"
  | "Running"
  | "Stopped"
  | "Failed";

export type PluginMessageDirection = "send" | "receive";

export interface PluginPermissionsRecord {
  storagePlugin: boolean;
  storageShared: boolean;
  messagesRead: boolean;
  messagesWrite: boolean;
  lxmfSend: boolean;
  lxmfReceive: boolean;
  notificationsRaise: boolean;
}

export interface PluginMessageDescriptorRecord {
  name: string;
  version: string;
  direction: PluginMessageDirection[];
  schema: string;
}

export interface InstalledPluginSettingsDescriptor {
  schemaPath: string;
  schema: unknown;
}

export interface InstalledPluginRecord {
  id: string;
  name: string;
  version: string;
  remApiVersion: string;
  pluginType: string;
  state: PluginState;
  libraryPath: string;
  settings?: InstalledPluginSettingsDescriptor;
  permissions: PluginPermissionsRecord;
  messages: PluginMessageDescriptorRecord[];
}

export interface PluginCatalogDiagnostic {
  pluginId?: string;
  path: string;
  message: string;
}

export interface PluginCatalogReport {
  items: InstalledPluginRecord[];
  errors: PluginCatalogDiagnostic[];
}

export interface SavedPeerRecord {
  destination: string;
  label?: string;
  savedAt: number;
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

export interface ReticulumNodeClient {
  start(config: NodeConfig): Promise<void>;
  stop(): Promise<void>;
  restart(config: NodeConfig): Promise<void>;
  getStatus(): Promise<NodeStatus>;
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
  listPlugins(): Promise<PluginCatalogReport>;
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
  deleteChecklist(checklistUid: string): Promise<void>;
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
  getEamTeamSummary(teamUid: string): Promise<EamTeamSummaryRecord | null>;
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
  mode?: "auto" | "capacitor" | "web";
}

export const DEFAULT_NODE_CONFIG: NodeConfig = {
  name: "emergency-ops-mobile",
  tcpClients: [],
  broadcast: true,
  announceIntervalSeconds: 1800,
  staleAfterMinutes: 30,
  announceCapabilities: "R3AKT,EMergencyMessages",
  hubMode: "Autonomous",
  hubRefreshIntervalSeconds: 3600,
};

export const DEFAULT_SOS_SETTINGS: SosSettingsRecord = {
  enabled: false,
  messageTemplate: "SOS! I need help...",
  cancelMessageTemplate: "SOS Cancelled - I am safe.",
  countdownSeconds: 5,
  includeLocation: true,
  triggerShake: false,
  triggerTapPattern: false,
  triggerPowerButton: false,
  shakeSensitivity: 2.5,
  audioRecording: false,
  audioDurationSeconds: 30,
  periodicUpdates: false,
  updateIntervalSeconds: 120,
  floatingButton: false,
  silentAutoAnswer: false,
  floatingButtonX: 24,
  floatingButtonY: 440,
  activePillX: 24,
  activePillY: 24,
};

export const DEFAULT_SOS_STATUS: SosStatusRecord = {
  state: "Idle",
  updatedAtMs: 0,
};

type ListenerFn<T> = (payload: T) => void;

class TypedEmitter<TEvents extends object> {
  private readonly listeners = new Map<string, Set<ListenerFn<unknown>>>();

  on<K extends keyof TEvents>(
    event: K,
    handler: ListenerFn<TEvents[K]>,
  ): () => void {
    const key = String(event);
    const bucket = this.listeners.get(key) ?? new Set<ListenerFn<unknown>>();
    bucket.add(handler as ListenerFn<unknown>);
    this.listeners.set(key, bucket);
    return () => {
      bucket.delete(handler as ListenerFn<unknown>);
      if (bucket.size === 0) {
        this.listeners.delete(key);
      }
    };
  }

  emit<K extends keyof TEvents>(event: K, payload: TEvents[K]): void {
    const bucket = this.listeners.get(String(event));
    if (!bucket) {
      return;
    }
    for (const listener of bucket) {
      (listener as ListenerFn<TEvents[K]>)(payload);
    }
  }

  clear(): void {
    this.listeners.clear();
  }
}

type PluginListenerHandle = {
  remove: () => Promise<void>;
};

interface ReticulumNodePlugin {
  startNode(options: { config: Record<string, unknown> }): Promise<void>;
  stopNode(): Promise<void>;
  restartNode(options: { config: Record<string, unknown> }): Promise<void>;
  getStatus(): Promise<Record<string, unknown>>;
  connectPeer(options: { destinationHex: string }): Promise<void>;
  disconnectPeer(options: { destinationHex: string }): Promise<void>;
  announceNow(): Promise<void>;
  requestPeerIdentity(options: { destinationHex: string }): Promise<void>;
  send(options: {
    destinationHex: string;
    bytesBase64: string;
    dedicatedFields?: Record<string, string>;
    fieldsBase64?: string;
    sendMode?: SendMode;
  }): Promise<void>;
  sendLxmf(options: {
    destinationHex: string;
    bodyUtf8: string;
    title?: string;
    sendMode?: SendMode;
  }): Promise<{ messageIdHex: string }>;
  retryLxmf(options: { messageIdHex: string }): Promise<void>;
  cancelLxmf(options: { messageIdHex: string }): Promise<void>;
  broadcast(options: {
    bytesBase64: string;
    dedicatedFields?: Record<string, string>;
    fieldsBase64?: string;
  }): Promise<void>;
  setActivePropagationNode(options: { destinationHex?: string }): Promise<void>;
  requestLxmfSync(options: { limit?: number }): Promise<void>;
  listAnnounces(): Promise<{ items: Record<string, unknown>[] }>;
  getPlugins(): Promise<{ items: Record<string, unknown>[]; errors?: Record<string, unknown>[] }>;
  listPeers(): Promise<{ items: Record<string, unknown>[] }>;
  listConversations(): Promise<{ items: Record<string, unknown>[] }>;
  listMessages(options: { conversationId?: string }): Promise<{ items: Record<string, unknown>[] }>;
  deleteConversation(options: { conversationId: string }): Promise<void>;
  getLxmfSyncStatus(): Promise<Record<string, unknown>>;
  listTelemetryDestinations(): Promise<{ items: string[] }>;
  legacyImportCompleted(): Promise<{ completed: boolean }>;
  importLegacyState(options: { payload: Record<string, unknown> }): Promise<void>;
  getAppSettings(): Promise<Record<string, unknown>>;
  setAppSettings(options: { settings: Record<string, unknown> }): Promise<void>;
  getSavedPeers(): Promise<{ items: Record<string, unknown>[] }>;
  setSavedPeers(options: { savedPeers: Record<string, unknown>[] }): Promise<void>;
  getOperationalSummary(): Promise<Record<string, unknown>>;
  getChecklists(options: { search?: string; sortBy?: string }): Promise<{ items: Record<string, unknown>[] }>;
  getChecklist(options: { checklistUid: string }): Promise<Record<string, unknown>>;
  getChecklistTemplates(options: { search?: string; sortBy?: string }): Promise<{ items: Record<string, unknown>[] }>;
  importChecklistTemplateCsv(options: {
    templateUid?: string;
    name: string;
    description?: string;
    csvText: string;
    sourceFilename?: string;
  }): Promise<Record<string, unknown>>;
  createChecklistFromTemplate(options: {
    checklistUid?: string;
    missionUid?: string;
    templateUid: string;
    name: string;
    description: string;
    startTime: string;
    createdByTeamMemberRnsIdentity?: string;
    createdByTeamMemberDisplayName?: string;
  }): Promise<void>;
  createOnlineChecklist(options: {
    checklistUid?: string;
    missionUid?: string;
    templateUid: string;
    name: string;
    description: string;
    startTime: string;
    createdByTeamMemberRnsIdentity?: string;
    createdByTeamMemberDisplayName?: string;
  }): Promise<void>;
  updateChecklist(options: {
    checklistUid: string;
    patch: Record<string, unknown>;
  }): Promise<void>;
  deleteChecklist(options: { checklistUid: string }): Promise<void>;
  joinChecklist(options: { checklistUid: string }): Promise<void>;
  uploadChecklist(options: { checklistUid: string }): Promise<void>;
  setChecklistTaskStatus(options: {
    checklistUid: string;
    taskUid: string;
    userStatus: ChecklistUserTaskStatus;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  addChecklistTaskRow(options: {
    checklistUid: string;
    taskUid?: string;
    number: number;
    dueRelativeMinutes?: number;
    legacyValue?: string;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  deleteChecklistTaskRow(options: {
    checklistUid: string;
    taskUid: string;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  setChecklistTaskRowStyle(options: {
    checklistUid: string;
    taskUid: string;
    rowBackgroundColor?: string;
    lineBreakEnabled?: boolean;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  setChecklistTaskCell(options: {
    checklistUid: string;
    taskUid: string;
    columnUid: string;
    value?: string;
    updatedByTeamMemberRnsIdentity?: string;
  }): Promise<void>;
  getEams(): Promise<{ items: Record<string, unknown>[] }>;
  upsertEam(options: { eam: Record<string, unknown> }): Promise<void>;
  deleteEam(options: { callsign: string; deletedAtMs?: number }): Promise<void>;
  getEamTeamSummary(options: { teamUid: string }): Promise<Record<string, unknown>>;
  getEvents(): Promise<{ items: Record<string, unknown>[] }>;
  upsertEvent(options: { event: Record<string, unknown> }): Promise<void>;
  deleteEvent(options: { uid: string; deletedAtMs?: number }): Promise<void>;
  getTelemetryPositions(): Promise<{ items: Record<string, unknown>[] }>;
  recordLocalTelemetryFix(options: { position: Record<string, unknown> }): Promise<void>;
  deleteLocalTelemetry(options: { callsign: string }): Promise<void>;
  getSosSettings(): Promise<Record<string, unknown>>;
  setSosSettings(options: { settings: Record<string, unknown> }): Promise<void>;
  setSosPin(options: { pin?: string }): Promise<void>;
  getSosStatus(): Promise<Record<string, unknown>>;
  triggerSos(options: { source?: SosTriggerSource }): Promise<Record<string, unknown>>;
  deactivateSos(options: { pin?: string }): Promise<Record<string, unknown>>;
  submitSosTelemetry(options: { telemetry: Record<string, unknown> }): Promise<void>;
  listSosAlerts(): Promise<{ items: Record<string, unknown>[] }>;
  listSosLocations(): Promise<{ items: Record<string, unknown>[] }>;
  listSosAudio(): Promise<{ items: Record<string, unknown>[] }>;
  setAnnounceCapabilities(options: { capabilityString: string }): Promise<void>;
  setLogLevel(options: { level: LogLevel }): Promise<void>;
  logMessage(options: { level: LogLevel; message: string }): Promise<void>;
  refreshHubDirectory(): Promise<void>;
  addListener(
    eventName: string,
    listener: (event: unknown) => void,
  ): PluginListenerHandle | Promise<PluginListenerHandle>;
  removeAllListeners?(): Promise<void>;
}

const ReticulumNodePluginInstance = registerPlugin<ReticulumNodePlugin>(
  "ReticulumNode",
);

function normalizeHex(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function hasValue(value: unknown): boolean {
  return value !== undefined && value !== null;
}

function toOptionalHex(value: unknown): string | undefined {
  if (!hasValue(value)) {
    return undefined;
  }
  const normalized = normalizeHex(value);
  return normalized ? normalized : undefined;
}

function toOptionalNumber(value: unknown): number | undefined {
  if (!hasValue(value)) {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function toOptionalBoolean(value: unknown): boolean | undefined {
  if (!hasValue(value)) {
    return undefined;
  }
  return Boolean(value);
}

function decodeBase64ToBytes(value: string): Uint8Array {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: string, encoding: string): Uint8Array };
    }
  ).Buffer;
  if (bufferCtor) {
    return Uint8Array.from(bufferCtor.from(value, "base64"));
  }
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

function encodeBytesToBase64(value: Uint8Array): string {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: Uint8Array): { toString(encoding: string): string } };
    }
  ).Buffer;
  if (bufferCtor) {
    return bufferCtor.from(value).toString("base64");
  }
  let binary = "";
  for (const v of value) {
    binary += String.fromCharCode(v);
  }
  return btoa(binary);
}

function toNodeStatus(raw: Record<string, unknown>): NodeStatus {
  return {
    running: Boolean(raw.running),
    name: String(raw.name ?? ""),
    identityHex: String(raw.identityHex ?? raw.identity_hex ?? ""),
    appDestinationHex: String(
      raw.appDestinationHex ?? raw.app_destination_hex ?? "",
    ),
    lxmfDestinationHex: String(
      raw.lxmfDestinationHex ?? raw.lxmf_destination_hex ?? "",
    ),
  };
}

function enumVariantName(raw: unknown): string {
  if (typeof raw === "string") {
    return raw.trim();
  }
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return "";
  }
  const variants = Object.keys(raw as Record<string, unknown>).filter((key) => key.trim().length > 0);
  return variants.length === 1 ? variants[0]!.trim() : "";
}

function toPeerState(raw: unknown): PeerState {
  const value = enumVariantName(raw);
  switch (value.toLowerCase()) {
    case "connecting":
      return "Connecting";
    case "connected":
      return "Connected";
    case "disconnected":
      return "Disconnected";
    default:
      return "Disconnected";
  }
}

function toSavedFlag(raw: unknown, legacyManagementRaw?: unknown): boolean {
  if (typeof raw === "boolean") {
    return raw;
  }
  if (hasValue(raw)) {
    return Boolean(raw);
  }
  return enumVariantName(legacyManagementRaw).toLowerCase() === "managed";
}

function toSendOutcome(raw: unknown): SendOutcome {
  const value = String(raw ?? "");
  const valid: SendOutcome[] = [
    "SentDirect",
    "SentBroadcast",
    "DroppedMissingDestinationIdentity",
    "DroppedCiphertextTooLarge",
    "DroppedEncryptFailed",
    "DroppedNoRoute",
  ];
  return valid.includes(value as SendOutcome)
    ? (value as SendOutcome)
    : "DroppedNoRoute";
}

function toStatusChangedEvent(raw: Record<string, unknown>): StatusChangedEvent {
  const statusRaw =
    (raw.status as Record<string, unknown> | undefined) ?? raw;
  return { status: toNodeStatus(statusRaw) };
}

function toAnnounceReceivedEvent(
  raw: Record<string, unknown>,
): AnnounceReceivedEvent {
  const destinationKindRaw = String(
    raw.destinationKind ?? raw.destination_kind ?? "other",
  );
  const destinationKind: AnnounceDestinationKind =
    destinationKindRaw === "app"
      || destinationKindRaw === "lxmf_delivery"
      || destinationKindRaw === "lxmf_propagation"
      ? destinationKindRaw
      : "other";
  const announceClassRaw = String(
    raw.announceClass ?? raw.announce_class ?? "Other",
  );
  const announceClass: AnnounceClass =
    announceClassRaw === "PeerApp"
      || announceClassRaw === "RchHubServer"
      || announceClassRaw === "PropagationNode"
      || announceClassRaw === "LxmfDelivery"
      ? announceClassRaw
      : "Other";
  return {
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    identityHex: normalizeHex(
      String(raw.identityHex ?? raw.identity_hex ?? ""),
    ),
    destinationKind,
    announceClass,
    appData: String(raw.appData ?? raw.app_data ?? ""),
    displayName:
      typeof raw.displayName === "string"
        ? raw.displayName
        : typeof raw.display_name === "string"
          ? raw.display_name
          : undefined,
    hops: Number(raw.hops ?? 0),
    interfaceHex: String(raw.interfaceHex ?? raw.interface_hex ?? ""),
    receivedAtMs: Number(raw.receivedAtMs ?? raw.received_at_ms ?? Date.now()),
  };
}

function toAnnounceRecord(raw: Record<string, unknown>): AnnounceRecord {
  const event = toAnnounceReceivedEvent(raw);
  return {
    ...event,
    displayName:
      typeof raw.displayName === "string"
        ? raw.displayName
        : typeof raw.display_name === "string"
          ? raw.display_name
          : undefined,
  };
}

function toPeerChangedEvent(raw: Record<string, unknown>): PeerChangedEvent {
  const changeRaw = (raw.change as Record<string, unknown> | undefined) ?? raw;
  const activeLinkRaw = hasValue(changeRaw.activeLink)
    ? changeRaw.activeLink
    : changeRaw.active_link;
  const lastSeenAtMsRaw = hasValue(changeRaw.lastSeenAtMs)
    ? changeRaw.lastSeenAtMs
    : changeRaw.last_seen_at_ms;
  return {
    change: {
      destinationHex: normalizeHex(
        String(changeRaw.destinationHex ?? changeRaw.destination_hex ?? ""),
      ),
      identityHex: toOptionalHex(
        hasValue(changeRaw.identityHex) ? changeRaw.identityHex : changeRaw.identity_hex,
      ),
      lxmfDestinationHex: toOptionalHex(
        hasValue(changeRaw.lxmfDestinationHex)
          ? changeRaw.lxmfDestinationHex
          : changeRaw.lxmf_destination_hex,
      ),
      displayName:
        typeof changeRaw.displayName === "string"
          ? changeRaw.displayName
          : typeof changeRaw.display_name === "string"
            ? changeRaw.display_name
            : undefined,
      appData:
        typeof changeRaw.appData === "string"
          ? changeRaw.appData
          : typeof changeRaw.app_data === "string"
            ? changeRaw.app_data
            : undefined,
      state: hasValue(changeRaw.state) ? toPeerState(changeRaw.state) : undefined,
      saved: toSavedFlag(changeRaw.saved, changeRaw.managementState ?? changeRaw.management_state),
      stale: Boolean(changeRaw.stale),
      activeLink: Boolean(activeLinkRaw),
      hubDerived: Boolean(
        hasValue(changeRaw.hubDerived) ? changeRaw.hubDerived : changeRaw.hub_derived,
      ),
      lastError: (changeRaw.lastError ?? changeRaw.last_error) as
        | string
        | undefined,
      lastResolutionError:
        typeof changeRaw.lastResolutionError === "string"
          ? changeRaw.lastResolutionError
          : typeof changeRaw.last_resolution_error === "string"
            ? changeRaw.last_resolution_error
            : undefined,
      lastResolutionAttemptAtMs: toOptionalNumber(
        hasValue(changeRaw.lastResolutionAttemptAtMs)
          ? changeRaw.lastResolutionAttemptAtMs
          : changeRaw.last_resolution_attempt_at_ms,
      ),
      lastSeenAtMs: toOptionalNumber(lastSeenAtMsRaw),
      announceLastSeenAtMs: toOptionalNumber(
        hasValue(changeRaw.announceLastSeenAtMs)
          ? changeRaw.announceLastSeenAtMs
          : changeRaw.announce_last_seen_at_ms,
      ),
      lxmfLastSeenAtMs: toOptionalNumber(
        hasValue(changeRaw.lxmfLastSeenAtMs)
          ? changeRaw.lxmfLastSeenAtMs
          : changeRaw.lxmf_last_seen_at_ms,
      ),
    },
  };
}

function toPeerRecord(raw: Record<string, unknown>): PeerRecord {
  return {
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    identityHex: toOptionalHex(
      hasValue(raw.identityHex) ? raw.identityHex : raw.identity_hex,
    ),
    lxmfDestinationHex: toOptionalHex(
      hasValue(raw.lxmfDestinationHex) ? raw.lxmfDestinationHex : raw.lxmf_destination_hex,
    ),
    displayName:
      typeof raw.displayName === "string"
        ? raw.displayName
        : typeof raw.display_name === "string"
          ? raw.display_name
          : undefined,
    appData:
      typeof raw.appData === "string"
        ? raw.appData
        : typeof raw.app_data === "string"
          ? raw.app_data
          : undefined,
    state: toPeerState(raw.state),
    saved: toSavedFlag(raw.saved, raw.managementState ?? raw.management_state),
    stale: Boolean(raw.stale),
    activeLink: Boolean(raw.activeLink ?? raw.active_link),
    hubDerived: Boolean(hasValue(raw.hubDerived) ? raw.hubDerived : raw.hub_derived),
    lastResolutionError:
      typeof raw.lastResolutionError === "string"
        ? raw.lastResolutionError
        : typeof raw.last_resolution_error === "string"
          ? raw.last_resolution_error
          : undefined,
    lastResolutionAttemptAtMs: toOptionalNumber(
      hasValue(raw.lastResolutionAttemptAtMs)
        ? raw.lastResolutionAttemptAtMs
        : raw.last_resolution_attempt_at_ms,
    ),
    lastSeenAtMs: toOptionalNumber(
      hasValue(raw.lastSeenAtMs) ? raw.lastSeenAtMs : raw.last_seen_at_ms,
    ) ?? 0,
    announceLastSeenAtMs: toOptionalNumber(
      hasValue(raw.announceLastSeenAtMs)
        ? raw.announceLastSeenAtMs
        : raw.announce_last_seen_at_ms,
    ),
    lxmfLastSeenAtMs: toOptionalNumber(
      hasValue(raw.lxmfLastSeenAtMs)
        ? raw.lxmfLastSeenAtMs
        : raw.lxmf_last_seen_at_ms,
    ),
  };
}


function toDedicatedFields(raw: unknown): Record<string, string> | undefined {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return undefined;
  }
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof value === "string") {
      out[String(key)] = value;
      continue;
    }
    if (typeof value === "number" || typeof value === "boolean") {
      out[String(key)] = String(value);
    }
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

function toPacketReceivedEvent(
  raw: Record<string, unknown>,
): PacketReceivedEvent {
  const encoded = String(raw.bytesBase64 ?? raw.bytes_base64 ?? "");
  return {
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    sourceHex:
      raw.sourceHex !== undefined || raw.source_hex !== undefined
        ? normalizeHex(String(raw.sourceHex ?? raw.source_hex ?? ""))
        : undefined,
    bytes: encoded ? decodeBase64ToBytes(encoded) : new Uint8Array(0),
    dedicatedFields: toDedicatedFields(raw.dedicatedFields ?? raw.dedicated_fields),
    fieldsBase64:
      typeof raw.fieldsBase64 === "string"
        ? raw.fieldsBase64
        : typeof raw.fields_base64 === "string"
          ? raw.fields_base64
          : undefined,
  };
}

function toPacketSentEvent(raw: Record<string, unknown>): PacketSentEvent {
  const encoded = String(raw.bytesBase64 ?? raw.bytes_base64 ?? "");
  return {
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    bytes: encoded ? decodeBase64ToBytes(encoded) : new Uint8Array(0),
    outcome: toSendOutcome(raw.outcome),
  };
}

function toLxmfDeliveryStatus(raw: unknown): LxmfDeliveryStatus {
  const value = String(raw ?? "");
  const valid: LxmfDeliveryStatus[] = [
    "Sent",
    "SentToPropagation",
    "Acknowledged",
    "Failed",
    "TimedOut",
  ];
  return valid.includes(value as LxmfDeliveryStatus)
    ? (value as LxmfDeliveryStatus)
    : "Failed";
}

function toLxmfDeliveryMethod(raw: unknown): LxmfDeliveryMethod {
  const value = String(raw ?? "");
  const valid: LxmfDeliveryMethod[] = ["Direct", "Opportunistic", "Propagated"];
  return valid.includes(value as LxmfDeliveryMethod)
    ? (value as LxmfDeliveryMethod)
    : "Direct";
}

function toLxmfDeliveryRepresentation(raw: unknown): LxmfDeliveryRepresentation {
  return String(raw ?? "") === "Resource" ? "Resource" : "Packet";
}

function toLxmfFallbackStage(raw: unknown): LxmfFallbackStage | undefined {
  return String(raw ?? "") === "AfterDirectRetryBudget"
    ? "AfterDirectRetryBudget"
    : undefined;
}

function toLxmfDeliveryEvent(raw: Record<string, unknown>): LxmfDeliveryEvent {
  return {
    messageIdHex: normalizeHex(
      String(raw.messageIdHex ?? raw.message_id_hex ?? ""),
    ),
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    sourceHex:
      raw.sourceHex !== undefined || raw.source_hex !== undefined
        ? normalizeHex(String(raw.sourceHex ?? raw.source_hex ?? ""))
        : undefined,
    correlationId:
      typeof raw.correlationId === "string"
        ? raw.correlationId
        : typeof raw.correlation_id === "string"
          ? raw.correlation_id
          : undefined,
    commandId:
      typeof raw.commandId === "string"
        ? raw.commandId
        : typeof raw.command_id === "string"
          ? raw.command_id
          : undefined,
    commandType:
      typeof raw.commandType === "string"
        ? raw.commandType
        : typeof raw.command_type === "string"
          ? raw.command_type
          : undefined,
    eventUid:
      typeof raw.eventUid === "string"
        ? raw.eventUid
        : typeof raw.event_uid === "string"
          ? raw.event_uid
          : undefined,
    missionUid:
      typeof raw.missionUid === "string"
        ? raw.missionUid
        : typeof raw.mission_uid === "string"
          ? raw.mission_uid
          : undefined,
    status: toLxmfDeliveryStatus(raw.status),
    method: toLxmfDeliveryMethod(raw.method),
    representation: toLxmfDeliveryRepresentation(raw.representation),
    relayDestinationHex: toOptionalHex(
      hasValue(raw.relayDestinationHex) ? raw.relayDestinationHex : raw.relay_destination_hex,
    ),
    fallbackStage: toLxmfFallbackStage(
      hasValue(raw.fallbackStage) ? raw.fallbackStage : raw.fallback_stage,
    ),
    detail:
      typeof raw.detail === "string"
        ? raw.detail
        : undefined,
    sentAtMs: Number(raw.sentAtMs ?? raw.sent_at_ms ?? Date.now()),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
  };
}

function toMessageMethod(raw: unknown): MessageMethod {
  switch (String(raw ?? "").trim().toLowerCase()) {
    case "direct":
      return "Direct";
    case "opportunistic":
      return "Opportunistic";
    case "propagated":
      return "Propagated";
    case "resource":
      return "Resource";
    default:
      return "Direct";
  }
}

function toMessageState(raw: unknown): MessageState {
  const value = String(raw ?? "").trim().toLowerCase();
  switch (value) {
    case "queued":
      return "Queued";
    case "pathrequested":
    case "path-requested":
      return "PathRequested";
    case "linkestablishing":
    case "link-establishing":
      return "LinkEstablishing";
    case "sending":
      return "Sending";
    case "sentdirect":
    case "sent-direct":
      return "SentDirect";
    case "senttopropagation":
    case "sent-to-propagation":
      return "SentToPropagation";
    case "delivered":
      return "Delivered";
    case "failed":
      return "Failed";
    case "timedout":
    case "timed-out":
      return "TimedOut";
    case "cancelled":
    case "canceled":
      return "Cancelled";
    case "received":
      return "Received";
    default:
      return "Queued";
  }
}

function toMessageDirection(raw: unknown, record?: Record<string, unknown>): MessageDirection {
  const value = String(raw ?? "").trim().toLowerCase();
  if (value === "inbound") {
    return "Inbound";
  }
  if (value === "outbound") {
    return "Outbound";
  }
  const state = String(record?.state ?? "").trim().toLowerCase();
  const hasReceivedAt = record?.receivedAtMs !== undefined || record?.received_at_ms !== undefined;
  const hasSentAt = record?.sentAtMs !== undefined || record?.sent_at_ms !== undefined;
  return state === "received" || (hasReceivedAt && !hasSentAt) ? "Inbound" : "Outbound";
}

function toMessageRecord(raw: Record<string, unknown>): MessageRecord {
  return {
    messageIdHex: normalizeHex(String(raw.messageIdHex ?? raw.message_id_hex ?? "")),
    conversationId: String(raw.conversationId ?? raw.conversation_id ?? ""),
    direction: toMessageDirection(raw.direction, raw),
    destinationHex: normalizeHex(String(raw.destinationHex ?? raw.destination_hex ?? "")),
    sourceHex:
      raw.sourceHex !== undefined || raw.source_hex !== undefined
        ? normalizeHex(String(raw.sourceHex ?? raw.source_hex ?? ""))
        : undefined,
    title:
      typeof raw.title === "string"
        ? raw.title
        : undefined,
    bodyUtf8: String(raw.bodyUtf8 ?? raw.body_utf8 ?? ""),
    method: toMessageMethod(raw.method),
    state: toMessageState(raw.state),
    detail:
      typeof raw.detail === "string"
        ? raw.detail
        : undefined,
    sentAtMs:
      typeof raw.sentAtMs === "number"
        ? raw.sentAtMs
        : typeof raw.sent_at_ms === "number"
          ? raw.sent_at_ms
          : undefined,
    receivedAtMs:
      typeof raw.receivedAtMs === "number"
        ? raw.receivedAtMs
        : typeof raw.received_at_ms === "number"
          ? raw.received_at_ms
          : undefined,
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
  };
}

function toConversationRecord(raw: Record<string, unknown>): ConversationRecord {
  return {
    conversationId: String(raw.conversationId ?? raw.conversation_id ?? ""),
    peerDestinationHex: normalizeHex(
      String(raw.peerDestinationHex ?? raw.peer_destination_hex ?? ""),
    ),
    peerDisplayName:
      typeof raw.peerDisplayName === "string"
        ? raw.peerDisplayName
        : typeof raw.peer_display_name === "string"
          ? raw.peer_display_name
          : undefined,
    lastMessagePreview:
      typeof raw.lastMessagePreview === "string"
        ? raw.lastMessagePreview
        : typeof raw.last_message_preview === "string"
          ? raw.last_message_preview
          : undefined,
    lastMessageAtMs: Number(raw.lastMessageAtMs ?? raw.last_message_at_ms ?? Date.now()),
    unreadCount: Number(raw.unreadCount ?? raw.unread_count ?? 0),
    lastMessageState:
      raw.lastMessageState !== undefined || raw.last_message_state !== undefined
        ? toMessageState(raw.lastMessageState ?? raw.last_message_state)
        : undefined,
  };
}

function toSyncPhase(raw: unknown): SyncPhase {
  const value = String(raw ?? "");
  const valid: SyncPhase[] = [
    "Idle",
    "PathRequested",
    "LinkEstablishing",
    "RequestSent",
    "Receiving",
    "Complete",
    "Failed",
  ];
  return valid.includes(value as SyncPhase) ? (value as SyncPhase) : "Idle";
}

function toSyncStatus(raw: Record<string, unknown>): SyncStatus {
  return {
    phase: toSyncPhase(raw.phase),
    activePropagationNodeHex:
      raw.activePropagationNodeHex !== undefined || raw.active_propagation_node_hex !== undefined
        ? normalizeHex(
            String(raw.activePropagationNodeHex ?? raw.active_propagation_node_hex ?? ""),
          )
        : undefined,
    requestedAtMs:
      typeof raw.requestedAtMs === "number"
        ? raw.requestedAtMs
        : typeof raw.requested_at_ms === "number"
          ? raw.requested_at_ms
          : undefined,
    completedAtMs:
      typeof raw.completedAtMs === "number"
        ? raw.completedAtMs
        : typeof raw.completed_at_ms === "number"
          ? raw.completed_at_ms
          : undefined,
    messagesReceived: Number(raw.messagesReceived ?? raw.messages_received ?? 0),
    detail: typeof raw.detail === "string" ? raw.detail : undefined,
  };
}

function toHubDirectoryUpdatedEvent(
  raw: Record<string, unknown>,
): HubDirectoryUpdatedEvent {
  const snapshot = raw.snapshot && typeof raw.snapshot === "object" && !Array.isArray(raw.snapshot)
    ? raw.snapshot as Record<string, unknown>
    : raw;
  const items = Array.isArray(snapshot.items)
    ? snapshot.items
      .filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === "object" && !Array.isArray(item))
      .map((item) => ({
        identity: normalizeHex(item.identity ?? ""),
        destinationHash: normalizeHex(item.destinationHash ?? item.destination_hash ?? ""),
        displayName: typeof item.displayName === "string"
          ? item.displayName
          : typeof item.display_name === "string"
            ? item.display_name
            : undefined,
        announceCapabilities: Array.isArray(item.announceCapabilities)
          ? item.announceCapabilities.map((value) => String(value))
          : Array.isArray(item.announce_capabilities)
            ? item.announce_capabilities.map((value) => String(value))
            : [],
        clientType: typeof item.clientType === "string"
          ? item.clientType
          : typeof item.client_type === "string"
            ? item.client_type
            : undefined,
        registeredMode: typeof item.registeredMode === "string"
          ? item.registeredMode
          : typeof item.registered_mode === "string"
            ? item.registered_mode
            : undefined,
        lastSeen: typeof item.lastSeen === "string"
          ? item.lastSeen
          : typeof item.last_seen === "string"
            ? item.last_seen
            : undefined,
        status: typeof item.status === "string" ? item.status : undefined,
      }))
      .filter((item) => item.destinationHash.length > 0)
    : [];
  return {
    effectiveConnectedMode: Boolean(
      snapshot.effectiveConnectedMode ?? snapshot.effective_connected_mode,
    ),
    items,
    receivedAtMs: Number(snapshot.receivedAtMs ?? snapshot.received_at_ms ?? Date.now()),
  };
}

function toLogEvent(raw: Record<string, unknown>): NodeLogEvent {
  return {
    level: (String(raw.level ?? "Info") as LogLevel) ?? "Info",
    message: String(raw.message ?? ""),
  };
}

function toOperationalNoticeEvent(
  raw: Record<string, unknown>,
): NodeOperationalNoticeEvent {
  return {
    level: (String(raw.level ?? "Info") as LogLevel) ?? "Info",
    message: String(raw.message ?? ""),
    atMs: Number(raw.atMs ?? raw.at_ms ?? Date.now()),
  };
}

function toErrorEvent(raw: Record<string, unknown>): NodeErrorEvent {
  return {
    code: String(raw.code ?? "UNKNOWN"),
    message: String(raw.message ?? "Unknown plugin error"),
  };
}

function toPluginPermissionsRecord(raw: Record<string, unknown>): PluginPermissionsRecord {
  return {
    storagePlugin: Boolean(raw.storagePlugin ?? raw.storage_plugin),
    storageShared: Boolean(raw.storageShared ?? raw.storage_shared),
    messagesRead: Boolean(raw.messagesRead ?? raw.messages_read),
    messagesWrite: Boolean(raw.messagesWrite ?? raw.messages_write),
    lxmfSend: Boolean(raw.lxmfSend ?? raw.lxmf_send),
    lxmfReceive: Boolean(raw.lxmfReceive ?? raw.lxmf_receive),
    notificationsRaise: Boolean(raw.notificationsRaise ?? raw.notifications_raise),
  };
}

function toPluginMessageDirection(value: unknown): PluginMessageDirection | undefined {
  const normalized = String(value ?? "").trim().toLowerCase();
  return normalized === "send" || normalized === "receive" ? normalized : undefined;
}

function toPluginMessageDescriptorRecord(
  raw: Record<string, unknown>,
): PluginMessageDescriptorRecord {
  return {
    name: String(raw.name ?? ""),
    version: String(raw.version ?? ""),
    direction: Array.isArray(raw.direction)
      ? raw.direction.flatMap((entry) => {
          const direction = toPluginMessageDirection(entry);
          return direction ? [direction] : [];
        })
      : [],
    schema: String(raw.schema ?? ""),
  };
}

function toPluginState(value: unknown): PluginState {
  const normalized = String(value ?? "");
  switch (normalized) {
    case "Discovered":
    case "Enabled":
    case "Loaded":
    case "Initialized":
    case "Running":
    case "Stopped":
    case "Failed":
      return normalized;
    case "Disabled":
    default:
      return "Disabled";
  }
}

function toInstalledPluginRecord(raw: Record<string, unknown>): InstalledPluginRecord {
  const settings =
    raw.settings && typeof raw.settings === "object"
      ? raw.settings as Record<string, unknown>
      : undefined;
  return {
    id: String(raw.id ?? ""),
    name: String(raw.name ?? ""),
    version: String(raw.version ?? ""),
    remApiVersion: String(raw.remApiVersion ?? raw.rem_api_version ?? ""),
    pluginType: String(raw.pluginType ?? raw.plugin_type ?? "native"),
    state: toPluginState(raw.state),
    libraryPath: String(raw.libraryPath ?? raw.library_path ?? ""),
    settings: settings
      ? {
          schemaPath: String(settings.schemaPath ?? settings.schema_path ?? ""),
          schema: settings.schema,
        }
      : undefined,
    permissions: toPluginPermissionsRecord(
      raw.permissions && typeof raw.permissions === "object"
        ? raw.permissions as Record<string, unknown>
        : {},
    ),
    messages: Array.isArray(raw.messages)
      ? raw.messages
          .filter((entry): entry is Record<string, unknown> =>
            Boolean(entry && typeof entry === "object"),
          )
          .map(toPluginMessageDescriptorRecord)
      : [],
  };
}

function toPluginCatalogDiagnostic(raw: Record<string, unknown>): PluginCatalogDiagnostic {
  return {
    pluginId: typeof raw.pluginId === "string"
      ? raw.pluginId
      : typeof raw.plugin_id === "string"
        ? raw.plugin_id
        : undefined,
    path: String(raw.path ?? ""),
    message: String(raw.message ?? ""),
  };
}

function toPluginCatalogReport(raw: Record<string, unknown>): PluginCatalogReport {
  return {
    items: Array.isArray(raw.items)
      ? raw.items
          .filter((entry): entry is Record<string, unknown> =>
            Boolean(entry && typeof entry === "object"),
          )
          .map(toInstalledPluginRecord)
      : [],
    errors: Array.isArray(raw.errors)
      ? raw.errors
          .filter((entry): entry is Record<string, unknown> =>
            Boolean(entry && typeof entry === "object"),
          )
          .map(toPluginCatalogDiagnostic)
      : [],
  };
}

function toProjectionInvalidationEvent(raw: Record<string, unknown>): ProjectionInvalidationEvent {
  return {
    scope: String(raw.scope ?? "Peers") as ProjectionScope,
    key: typeof raw.key === "string" ? raw.key : undefined,
    revision: Number(raw.revision ?? 0),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
    reason: typeof raw.reason === "string" ? raw.reason : undefined,
  };
}

function normalizeHubMode(value: unknown): HubMode {
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

function toAppSettingsRecord(raw: Record<string, unknown>): AppSettingsRecord | null {
  if (!raw || Object.keys(raw).length === 0) {
    return null;
  }
  if ("settings" in raw) {
    const nested = raw.settings;
    if (!nested || typeof nested !== "object" || Array.isArray(nested)) {
      return null;
    }
    return toAppSettingsRecord(nested as Record<string, unknown>);
  }
  const telemetry = (raw.telemetry ?? {}) as Record<string, unknown>;
  const hub = (raw.hub ?? {}) as Record<string, unknown>;
  const checklists = (raw.checklists ?? {}) as Record<string, unknown>;
  const defaultTaskDueStepMinutes = Math.trunc(Number(checklists.defaultTaskDueStepMinutes ?? 30));
  return {
    displayName: String(raw.displayName ?? ""),
    autoConnectSaved: Boolean(raw.autoConnectSaved),
    announceCapabilities: String(raw.announceCapabilities ?? ""),
    tcpClients: Array.isArray(raw.tcpClients) ? raw.tcpClients.map((entry) => String(entry)) : [],
    broadcast: Boolean(raw.broadcast),
    announceIntervalSeconds: Number(raw.announceIntervalSeconds ?? 1800),
    telemetry: {
      enabled: Boolean(telemetry.enabled),
      publishIntervalSeconds: Number(telemetry.publishIntervalSeconds ?? 10),
      accuracyThresholdMeters: toOptionalNumber(telemetry.accuracyThresholdMeters),
      staleAfterMinutes: Number(telemetry.staleAfterMinutes ?? 30),
      expireAfterMinutes: Number(telemetry.expireAfterMinutes ?? 180),
    },
    hub: {
      mode: normalizeHubMode(hub.mode),
      identityHash: String(hub.identityHash ?? ""),
      apiBaseUrl: String(hub.apiBaseUrl ?? ""),
      apiKey: String(hub.apiKey ?? ""),
      refreshIntervalSeconds: Number(hub.refreshIntervalSeconds ?? 3600),
    },
    checklists: {
      defaultTaskDueStepMinutes: Number.isFinite(defaultTaskDueStepMinutes)
        ? Math.max(1, defaultTaskDueStepMinutes)
        : 30,
    },
  };
}

function toSavedPeerRecord(raw: Record<string, unknown>): SavedPeerRecord {
  return {
    destination: normalizeHex(raw.destination ?? raw.destinationHex ?? ""),
    label: typeof raw.label === "string" ? raw.label : undefined,
    savedAt: Number(raw.savedAt ?? raw.saved_at_ms ?? raw.savedAtMs ?? Date.now()),
  };
}

function toEamProjectionRecord(raw: Record<string, unknown>): EamProjectionRecord {
  const source = raw.source && typeof raw.source === "object" && !Array.isArray(raw.source)
    ? raw.source as Record<string, unknown>
    : null;
  return {
    callsign: String(raw.callsign ?? ""),
    groupName: String(raw.groupName ?? raw.group_name ?? ""),
    securityStatus: String(raw.securityStatus ?? raw.security_status ?? "Unknown"),
    capabilityStatus: String(raw.capabilityStatus ?? raw.capability_status ?? "Unknown"),
    preparednessStatus: String(raw.preparednessStatus ?? raw.preparedness_status ?? "Unknown"),
    medicalStatus: String(raw.medicalStatus ?? raw.medical_status ?? "Unknown"),
    mobilityStatus: String(raw.mobilityStatus ?? raw.mobility_status ?? "Unknown"),
    commsStatus: String(raw.commsStatus ?? raw.comms_status ?? "Unknown"),
    notes: typeof raw.notes === "string" ? raw.notes : undefined,
    updatedAt: Number(raw.updatedAt ?? raw.updated_at_ms ?? Date.now()),
    deletedAt: toOptionalNumber(raw.deletedAt ?? raw.deleted_at_ms),
    eamUid: typeof raw.eamUid === "string" ? raw.eamUid : typeof raw.eam_uid === "string" ? raw.eam_uid : undefined,
    teamMemberUid:
      typeof raw.teamMemberUid === "string"
        ? raw.teamMemberUid
        : typeof raw.team_member_uid === "string"
          ? raw.team_member_uid
          : undefined,
    teamUid:
      typeof raw.teamUid === "string"
        ? raw.teamUid
        : typeof raw.team_uid === "string"
          ? raw.team_uid
          : undefined,
    reportedAt:
      typeof raw.reportedAt === "string"
        ? raw.reportedAt
        : typeof raw.reported_at === "string"
          ? raw.reported_at
          : undefined,
    reportedBy:
      typeof raw.reportedBy === "string"
        ? raw.reportedBy
        : typeof raw.reported_by === "string"
          ? raw.reported_by
          : undefined,
    overallStatus:
      typeof raw.overallStatus === "string"
        ? raw.overallStatus
        : typeof raw.overall_status === "string"
          ? raw.overall_status
          : undefined,
    confidence: toOptionalNumber(raw.confidence),
    ttlSeconds: toOptionalNumber(raw.ttlSeconds ?? raw.ttl_seconds),
    source: source
      ? {
          rns_identity: String(source.rns_identity ?? source.rnsIdentity ?? ""),
          display_name:
            typeof source.display_name === "string"
              ? source.display_name
              : typeof source.displayName === "string"
                ? source.displayName
                : undefined,
        }
      : undefined,
    syncState:
      typeof raw.syncState === "string"
        ? raw.syncState
        : typeof raw.sync_state === "string"
          ? raw.sync_state
          : undefined,
    syncError:
      typeof raw.syncError === "string"
        ? raw.syncError
        : typeof raw.sync_error === "string"
          ? raw.sync_error
          : undefined,
    draftCreatedAt: toOptionalNumber(raw.draftCreatedAt ?? raw.draft_created_at_ms),
    lastSyncedAt: toOptionalNumber(raw.lastSyncedAt ?? raw.last_synced_at_ms),
  };
}

function eamProjectionRecordToPlugin(record: EamProjectionRecord): Record<string, unknown> {
  const normalized = toEamProjectionRecord(record as unknown as Record<string, unknown>);
  return {
    callsign: normalized.callsign,
    groupName: normalized.groupName,
    securityStatus: normalized.securityStatus,
    capabilityStatus: normalized.capabilityStatus,
    preparednessStatus: normalized.preparednessStatus,
    medicalStatus: normalized.medicalStatus,
    mobilityStatus: normalized.mobilityStatus,
    commsStatus: normalized.commsStatus,
    notes: normalized.notes,
    updatedAt: normalized.updatedAt,
    deletedAt: normalized.deletedAt,
    eamUid: normalized.eamUid,
    teamMemberUid: normalized.teamMemberUid,
    teamUid: normalized.teamUid,
    reportedAt: normalized.reportedAt,
    reportedBy: normalized.reportedBy,
    overallStatus: normalized.overallStatus,
    confidence: normalized.confidence,
    ttlSeconds: normalized.ttlSeconds,
    source: normalized.source
      ? {
          rnsIdentity: normalized.source.rns_identity,
          displayName: normalized.source.display_name,
        }
      : undefined,
    syncState: normalized.syncState,
    syncError: normalized.syncError,
    draftCreatedAt: normalized.draftCreatedAt,
    lastSyncedAt: normalized.lastSyncedAt,
  };
}

function toEamTeamSummaryRecord(raw: Record<string, unknown>): EamTeamSummaryRecord | null {
  if (!raw || Object.keys(raw).length === 0 || raw.summary === null) {
    return null;
  }
  const source = raw.summary && typeof raw.summary === "object"
    ? raw.summary as Record<string, unknown>
    : raw;
  return {
    teamUid: String(source.teamUid ?? ""),
    total: Number(source.total ?? 0),
    activeTotal: Number(source.activeTotal ?? 0),
    deletedTotal: Number(source.deletedTotal ?? 0),
    overallStatus: typeof source.overallStatus === "string" ? source.overallStatus : undefined,
    greenTotal: Number(source.greenTotal ?? 0),
    yellowTotal: Number(source.yellowTotal ?? 0),
    redTotal: Number(source.redTotal ?? 0),
    updatedAt: Number(source.updatedAt ?? Date.now()),
  };
}

function toEventProjectionRecord(raw: Record<string, unknown>): EventProjectionRecord {
  const source = (raw.source ?? {}) as Record<string, unknown>;
  const args = (raw.args ?? {}) as Record<string, unknown>;
  const sourceIdentity = String(
    source.rns_identity
      ?? raw.source_identity
      ?? raw.sourceIdentity
      ?? args.source_identity
      ?? args.sourceIdentity
      ?? "",
  );
  const sourceDisplayName =
    typeof source.display_name === "string"
      ? source.display_name
      : typeof raw.source_display_name === "string"
        ? raw.source_display_name
        : typeof raw.sourceDisplayName === "string"
          ? raw.sourceDisplayName
          : typeof args.source_display_name === "string"
            ? args.source_display_name
            : typeof args.sourceDisplayName === "string"
              ? args.sourceDisplayName
              : undefined;
  const entryUid = String(args.entry_uid ?? args.entryUid ?? raw.uid ?? raw.entry_uid ?? raw.entryUid ?? "");
  const missionUid = String(args.mission_uid ?? args.missionUid ?? raw.mission_uid ?? raw.missionUid ?? "");
  const content = String(args.content ?? raw.content ?? "");
  const callsign = String(args.callsign ?? raw.callsign ?? "");
  const serverTime =
    typeof args.server_time === "string"
      ? args.server_time
      : typeof args.serverTime === "string"
        ? args.serverTime
        : typeof raw.server_time === "string"
          ? raw.server_time
          : typeof raw.serverTime === "string"
            ? raw.serverTime
            : undefined;
  const clientTime =
    typeof args.client_time === "string"
      ? args.client_time
      : typeof args.clientTime === "string"
        ? args.clientTime
        : typeof raw.client_time === "string"
          ? raw.client_time
          : typeof raw.clientTime === "string"
            ? raw.clientTime
            : undefined;
  const keywords = Array.isArray(args.keywords)
    ? args.keywords.map((entry) => String(entry))
    : Array.isArray(raw.keywords)
      ? raw.keywords.map((entry) => String(entry))
      : [];
  const contentHashes = Array.isArray(args.content_hashes)
    ? args.content_hashes.map((entry) => String(entry))
    : Array.isArray(args.contentHashes)
      ? args.contentHashes.map((entry) => String(entry))
      : Array.isArray(raw.content_hashes)
        ? raw.content_hashes.map((entry) => String(entry))
        : Array.isArray(raw.contentHashes)
          ? raw.contentHashes.map((entry) => String(entry))
          : [];
  return {
    command_id: String(raw.command_id ?? raw.commandId ?? ""),
    source: {
      rns_identity: sourceIdentity,
      display_name: sourceDisplayName,
    },
    timestamp: String(raw.timestamp ?? serverTime ?? clientTime ?? ""),
    command_type: String(raw.command_type ?? raw.commandType ?? ""),
    args: {
      entry_uid: entryUid,
      mission_uid: missionUid,
      content,
      callsign,
      server_time: serverTime,
      client_time: clientTime,
      keywords,
      content_hashes: contentHashes,
      source_identity: sourceIdentity || undefined,
      source_display_name: sourceDisplayName,
    },
    correlation_id:
      typeof raw.correlation_id === "string"
        ? raw.correlation_id
        : typeof raw.correlationId === "string"
          ? raw.correlationId
          : undefined,
    topics: Array.isArray(raw.topics) ? raw.topics.map((entry) => String(entry)) : [],
    deleted_at: toOptionalNumber(raw.deleted_at ?? raw.deletedAt),
    updatedAt: Number(raw.updatedAt ?? raw.updated_at ?? Date.now()),
  };
}

function eventProjectionRecordToPlugin(record: EventProjectionRecord): Record<string, unknown> {
  const normalized = toEventProjectionRecord(record as unknown as Record<string, unknown>);
  return {
    uid: normalized.args.entry_uid,
    commandId: normalized.command_id,
    sourceIdentity: normalized.args.source_identity ?? normalized.source.rns_identity,
    sourceDisplayName: normalized.args.source_display_name ?? normalized.source.display_name,
    timestamp: normalized.timestamp,
    commandType: normalized.command_type,
    missionUid: normalized.args.mission_uid,
    content: normalized.args.content,
    callsign: normalized.args.callsign,
    serverTime: normalized.args.server_time,
    clientTime: normalized.args.client_time,
    keywords: normalized.args.keywords,
    contentHashes: normalized.args.content_hashes,
    updatedAt: normalized.updatedAt,
    deletedAt: normalized.deleted_at,
    correlationId: normalized.correlation_id,
    topics: normalized.topics,
  };
}

function legacyImportPayloadToPlugin(payload: LegacyImportPayload): Record<string, unknown> {
  return {
    settings: payload.settings as unknown as Record<string, unknown> | undefined,
    savedPeers: payload.savedPeers as unknown as Record<string, unknown>[],
    eams: payload.eams.map(eamProjectionRecordToPlugin),
    events: payload.events.map(eventProjectionRecordToPlugin),
    messages: payload.messages as unknown as Record<string, unknown>[],
    telemetryPositions: payload.telemetryPositions as unknown as Record<string, unknown>[],
  };
}

function toTelemetryPositionRecord(raw: Record<string, unknown>): TelemetryPositionRecord {
  return {
    callsign: String(raw.callsign ?? ""),
    lat: Number(raw.lat ?? 0),
    lon: Number(raw.lon ?? 0),
    alt: toOptionalNumber(raw.alt),
    course: toOptionalNumber(raw.course),
    speed: toOptionalNumber(raw.speed),
    accuracy: toOptionalNumber(raw.accuracy),
    updatedAt: Number(raw.updatedAt ?? Date.now()),
  };
}

function toChecklistColumnRecord(raw: Record<string, unknown>): ChecklistColumnRecord {
  return {
    columnUid: String(raw.columnUid ?? raw.column_uid ?? ""),
    columnName: String(raw.columnName ?? raw.column_name ?? ""),
    displayOrder: Number(raw.displayOrder ?? raw.display_order ?? 0),
    columnType: String(raw.columnType ?? raw.column_type ?? "SHORT_STRING") as ChecklistColumnType,
    columnEditable: Boolean(raw.columnEditable ?? raw.column_editable ?? true),
    backgroundColor: typeof raw.backgroundColor === "string"
      ? raw.backgroundColor
      : typeof raw.background_color === "string"
        ? raw.background_color
        : undefined,
    textColor: typeof raw.textColor === "string"
      ? raw.textColor
      : typeof raw.text_color === "string"
        ? raw.text_color
        : undefined,
    isRemovable: Boolean(raw.isRemovable ?? raw.is_removable ?? true),
    systemKey: typeof raw.systemKey === "string"
      ? raw.systemKey
      : typeof raw.system_key === "string"
        ? raw.system_key
        : undefined,
  };
}

function toChecklistCellRecord(raw: Record<string, unknown>): ChecklistCellRecord {
  return {
    cellUid: String(raw.cellUid ?? raw.cell_uid ?? ""),
    taskUid: String(raw.taskUid ?? raw.task_uid ?? ""),
    columnUid: String(raw.columnUid ?? raw.column_uid ?? ""),
    value: typeof raw.value === "string" ? raw.value : undefined,
    updatedAt: typeof raw.updatedAt === "string"
      ? raw.updatedAt
      : typeof raw.updated_at === "string"
        ? raw.updated_at
        : undefined,
    updatedByTeamMemberRnsIdentity:
      typeof raw.updatedByTeamMemberRnsIdentity === "string"
        ? raw.updatedByTeamMemberRnsIdentity
        : typeof raw.updated_by_team_member_rns_identity === "string"
          ? raw.updated_by_team_member_rns_identity
          : undefined,
  };
}

function toChecklistTaskRecord(raw: Record<string, unknown>): ChecklistTaskRecord {
  const cells = Array.isArray(raw.cells) ? raw.cells : [];
  return {
    taskUid: String(raw.taskUid ?? raw.task_uid ?? ""),
    number: Number(raw.number ?? 0),
    userStatus: String(raw.userStatus ?? raw.user_status ?? "PENDING") as ChecklistUserTaskStatus,
    taskStatus: String(raw.taskStatus ?? raw.task_status ?? "PENDING") as ChecklistTaskStatus,
    isLate: Boolean(raw.isLate ?? raw.is_late ?? false),
    updatedAt: typeof raw.updatedAt === "string"
      ? raw.updatedAt
      : typeof raw.updated_at === "string"
        ? raw.updated_at
        : undefined,
    deletedAt: typeof raw.deletedAt === "string"
      ? raw.deletedAt
      : typeof raw.deleted_at === "string"
        ? raw.deleted_at
        : undefined,
    customStatus: typeof raw.customStatus === "string"
      ? raw.customStatus
      : typeof raw.custom_status === "string"
        ? raw.custom_status
        : undefined,
    dueRelativeMinutes: toOptionalNumber(raw.dueRelativeMinutes ?? raw.due_relative_minutes),
    dueDtg: typeof raw.dueDtg === "string"
      ? raw.dueDtg
      : typeof raw.due_dtg === "string"
        ? raw.due_dtg
        : undefined,
    notes: typeof raw.notes === "string" ? raw.notes : undefined,
    rowBackgroundColor: typeof raw.rowBackgroundColor === "string"
      ? raw.rowBackgroundColor
      : typeof raw.row_background_color === "string"
        ? raw.row_background_color
        : undefined,
    lineBreakEnabled: toOptionalBoolean(raw.lineBreakEnabled ?? raw.line_break_enabled),
    legacyValue: typeof raw.legacyValue === "string"
      ? raw.legacyValue
      : typeof raw.legacy_value === "string"
        ? raw.legacy_value
        : undefined,
    completedAt: typeof raw.completedAt === "string"
      ? raw.completedAt
      : typeof raw.completed_at === "string"
        ? raw.completed_at
        : undefined,
    completedByTeamMemberRnsIdentity:
      typeof raw.completedByTeamMemberRnsIdentity === "string"
        ? raw.completedByTeamMemberRnsIdentity
        : typeof raw.completed_by_team_member_rns_identity === "string"
          ? raw.completed_by_team_member_rns_identity
          : undefined,
    cells: cells
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistCellRecord),
  };
}

function toChecklistFeedPublicationRecord(raw: Record<string, unknown>): ChecklistFeedPublicationRecord {
  return {
    publicationUid: String(raw.publicationUid ?? raw.publication_uid ?? ""),
    checklistUid: String(raw.checklistUid ?? raw.checklist_uid ?? ""),
    missionFeedUid: String(raw.missionFeedUid ?? raw.mission_feed_uid ?? ""),
    publishedAt: typeof raw.publishedAt === "string"
      ? raw.publishedAt
      : typeof raw.published_at === "string"
        ? raw.published_at
        : undefined,
    publishedByTeamMemberRnsIdentity:
      typeof raw.publishedByTeamMemberRnsIdentity === "string"
        ? raw.publishedByTeamMemberRnsIdentity
        : typeof raw.published_by_team_member_rns_identity === "string"
          ? raw.published_by_team_member_rns_identity
          : undefined,
  };
}

function toChecklistRecord(raw: Record<string, unknown>): ChecklistRecord {
  const columns = Array.isArray(raw.columns) ? raw.columns : [];
  const tasks = Array.isArray(raw.tasks) ? raw.tasks : [];
  const feedPublications = Array.isArray(raw.feedPublications)
    ? raw.feedPublications
    : Array.isArray(raw.feed_publications)
      ? raw.feed_publications
      : [];
  const counts = raw.counts && typeof raw.counts === "object" ? raw.counts as Record<string, unknown> : {};
  return {
    uid: String(raw.uid ?? ""),
    missionUid: typeof raw.missionUid === "string" ? raw.missionUid : typeof raw.mission_uid === "string" ? raw.mission_uid : undefined,
    templateUid: typeof raw.templateUid === "string" ? raw.templateUid : typeof raw.template_uid === "string" ? raw.template_uid : undefined,
    templateVersion: toOptionalNumber(raw.templateVersion ?? raw.template_version),
    templateName: typeof raw.templateName === "string" ? raw.templateName : typeof raw.template_name === "string" ? raw.template_name : undefined,
    name: String(raw.name ?? ""),
    description: String(raw.description ?? ""),
    startTime: typeof raw.startTime === "string" ? raw.startTime : typeof raw.start_time === "string" ? raw.start_time : undefined,
    mode: String(raw.mode ?? "ONLINE") as ChecklistMode,
    syncState: String(raw.syncState ?? raw.sync_state ?? "SYNCED") as ChecklistSyncState,
    originType: String(raw.originType ?? raw.origin_type ?? "RCH_TEMPLATE") as ChecklistOriginType,
    checklistStatus: String(raw.checklistStatus ?? raw.checklist_status ?? "PENDING") as ChecklistTaskStatus,
    createdAt: typeof raw.createdAt === "string" ? raw.createdAt : typeof raw.created_at === "string" ? raw.created_at : undefined,
    createdByTeamMemberRnsIdentity: String(
      raw.createdByTeamMemberRnsIdentity ?? raw.created_by_team_member_rns_identity ?? "",
    ),
    createdByTeamMemberDisplayName: typeof raw.createdByTeamMemberDisplayName === "string"
      ? raw.createdByTeamMemberDisplayName
      : typeof raw.created_by_team_member_display_name === "string"
        ? raw.created_by_team_member_display_name
        : undefined,
    updatedAt: typeof raw.updatedAt === "string" ? raw.updatedAt : typeof raw.updated_at === "string" ? raw.updated_at : undefined,
    lastChangedByTeamMemberRnsIdentity: typeof raw.lastChangedByTeamMemberRnsIdentity === "string"
      ? raw.lastChangedByTeamMemberRnsIdentity
      : typeof raw.last_changed_by_team_member_rns_identity === "string"
        ? raw.last_changed_by_team_member_rns_identity
        : undefined,
    deletedAt: typeof raw.deletedAt === "string" ? raw.deletedAt : typeof raw.deleted_at === "string" ? raw.deleted_at : undefined,
    uploadedAt: typeof raw.uploadedAt === "string" ? raw.uploadedAt : typeof raw.uploaded_at === "string" ? raw.uploaded_at : undefined,
    participantRnsIdentities: Array.isArray(raw.participantRnsIdentities)
      ? raw.participantRnsIdentities.filter((value): value is string => typeof value === "string")
      : Array.isArray(raw.participant_rns_identities)
        ? raw.participant_rns_identities.filter((value): value is string => typeof value === "string")
        : [],
    expectedTaskCount: toOptionalNumber(raw.expectedTaskCount ?? raw.expected_task_count),
    progressPercent: Number(raw.progressPercent ?? raw.progress_percent ?? 0),
    counts: {
      pendingCount: Number(counts.pendingCount ?? counts.pending_count ?? 0),
      lateCount: Number(counts.lateCount ?? counts.late_count ?? 0),
      completeCount: Number(counts.completeCount ?? counts.complete_count ?? 0),
    },
    columns: columns
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistColumnRecord),
    tasks: tasks
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistTaskRecord),
    feedPublications: feedPublications
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistFeedPublicationRecord),
  };
}

function toChecklistTemplateRecord(raw: Record<string, unknown>): ChecklistTemplateRecord {
  const columns = Array.isArray(raw.columns) ? raw.columns : [];
  const tasks = Array.isArray(raw.tasks) ? raw.tasks : [];
  return {
    uid: String(raw.uid ?? ""),
    name: String(raw.name ?? ""),
    description: String(raw.description ?? ""),
    version: Number(raw.version ?? 1),
    originType: String(raw.originType ?? raw.origin_type ?? "RCH_TEMPLATE") as ChecklistOriginType,
    createdAt: typeof raw.createdAt === "string" ? raw.createdAt : typeof raw.created_at === "string" ? raw.created_at : undefined,
    updatedAt: typeof raw.updatedAt === "string" ? raw.updatedAt : typeof raw.updated_at === "string" ? raw.updated_at : undefined,
    sourceFilename: typeof raw.sourceFilename === "string"
      ? raw.sourceFilename
      : typeof raw.source_filename === "string"
        ? raw.source_filename
        : undefined,
    columns: columns
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistColumnRecord),
    tasks: tasks
      .filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object")
      .map(toChecklistTaskRecord),
  };
}

type ChecklistCreateInput = Parameters<ReticulumNodeClient["createChecklistFromTemplate"]>[0];
type ChecklistUpdateInput = Parameters<ReticulumNodeClient["updateChecklist"]>[0];
type ChecklistStatusInput = Parameters<ReticulumNodeClient["setChecklistTaskStatus"]>[0];
type ChecklistRowAddInput = Parameters<ReticulumNodeClient["addChecklistTaskRow"]>[0];
type ChecklistRowDeleteInput = Parameters<ReticulumNodeClient["deleteChecklistTaskRow"]>[0];
type ChecklistRowStyleInput = Parameters<ReticulumNodeClient["setChecklistTaskRowStyle"]>[0];
type ChecklistCellInput = Parameters<ReticulumNodeClient["setChecklistTaskCell"]>[0];
type ChecklistTemplateCsvInput = Parameters<ReticulumNodeClient["importChecklistTemplateCsv"]>[0];

function cloneChecklistRecord(record: ChecklistRecord): ChecklistRecord {
  return JSON.parse(JSON.stringify(record)) as ChecklistRecord;
}

function cloneChecklistTemplateRecord(record: ChecklistTemplateRecord): ChecklistTemplateRecord {
  return JSON.parse(JSON.stringify(record)) as ChecklistTemplateRecord;
}

function defaultChecklistColumns(): ChecklistColumnRecord[] {
  return [
    {
      columnUid: "col-due-relative-dtg",
      columnName: "CompletedDTG",
      displayOrder: 0,
      columnType: "RELATIVE_TIME",
      columnEditable: false,
      isRemovable: false,
      systemKey: "DUE_RELATIVE_DTG",
    },
    {
      columnUid: "col-task",
      columnName: "Task",
      displayOrder: 1,
      columnType: "SHORT_STRING",
      columnEditable: true,
      isRemovable: false,
      systemKey: "task",
    },
    {
      columnUid: "col-description",
      columnName: "Detail",
      displayOrder: 2,
      columnType: "LONG_STRING",
      columnEditable: true,
      isRemovable: true,
    },
    {
      columnUid: "col-owner",
      columnName: "Owner",
      displayOrder: 3,
      columnType: "SHORT_STRING",
      columnEditable: true,
      isRemovable: true,
    },
  ];
}

function defaultChecklistTask(taskUid: string, number: number, title: string, detail: string): ChecklistTaskRecord {
  const now = new Date().toISOString();
  return {
    taskUid,
    number,
    userStatus: "PENDING",
    taskStatus: "PENDING",
    isLate: false,
    updatedAt: now,
    dueRelativeMinutes: number * 30,
    legacyValue: title,
    lineBreakEnabled: false,
    cells: [
      {
        cellUid: `${taskUid}:col-task`,
        taskUid,
        columnUid: "col-task",
        value: title,
        updatedAt: now,
      },
      {
        cellUid: `${taskUid}:col-description`,
        taskUid,
        columnUid: "col-description",
        value: detail,
        updatedAt: now,
      },
      {
        cellUid: `${taskUid}:col-owner`,
        taskUid,
        columnUid: "col-owner",
        value: "Unassigned",
        updatedAt: now,
      },
    ],
  };
}

function parseCsvRows(csvText: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  for (let index = 0; index < csvText.length; index += 1) {
    const char = csvText[index];
    const next = csvText[index + 1];
    if (quoted) {
      if (char === "\"" && next === "\"") {
        cell += "\"";
        index += 1;
      } else if (char === "\"") {
        quoted = false;
      } else {
        cell += char;
      }
      continue;
    }
    if (char === "\"") {
      quoted = true;
    } else if (char === ",") {
      row.push(cell.replace(/^\uFEFF/, "").trim());
      cell = "";
    } else if (char === "\n") {
      row.push(cell.replace(/^\uFEFF/, "").trim());
      rows.push(row);
      row = [];
      cell = "";
    } else if (char !== "\r") {
      cell += char;
    }
  }
  row.push(cell.replace(/^\uFEFF/, "").trim());
  rows.push(row);
  return rows.filter((entry) => entry.some((value) => value.trim().length > 0));
}

function normalizeCsvHeader(value: string): string {
  return value.replace(/^\uFEFF/, "").toLowerCase().replace(/[^a-z0-9]/g, "");
}

function isDueCsvHeader(value: string): boolean {
  return ["completeddtg", "due", "duerelativedtg", "duerelativeminutes", "dueminutes"].includes(normalizeCsvHeader(value));
}

function isTitleCsvHeader(value: string): boolean {
  return ["item", "task", "name", "title"].includes(normalizeCsvHeader(value));
}

function isDescriptionCsvHeader(value: string): boolean {
  return ["description", "detail", "details", "notes"].includes(normalizeCsvHeader(value));
}

function csvColumnUid(header: string, index: number, used: Map<string, number>): string {
  const slug = header
    .replace(/^\uFEFF/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || `column-${index + 1}`;
  const base = `col-${slug}`;
  const count = (used.get(base) ?? 0) + 1;
  used.set(base, count);
  return count === 1 ? base : `${base}-${count}`;
}

function parseDueRelativeMinutes(value: string): number {
  let text = value.trim().toLowerCase();
  if (!text || text.startsWith("-")) {
    throw new Error("Invalid CompletedDTG value");
  }
  if (text.startsWith("+")) {
    text = text.slice(1).trim();
  }
  const hhmm = text.match(/^(\d+):(\d{1,2})$/);
  if (hhmm) {
    const hours = Number(hhmm[1]);
    const minutes = Number(hhmm[2]);
    if (!Number.isFinite(hours) || !Number.isFinite(minutes) || minutes >= 60) {
      throw new Error("Invalid CompletedDTG value");
    }
    return hours * 60 + minutes;
  }
  const hours = text.match(/^(\d+)\s*(h|hour|hours)$/);
  if (hours) {
    return Number(hours[1]) * 60;
  }
  const minutes = Number(text);
  if (!Number.isInteger(minutes) || minutes < 0) {
    throw new Error("Invalid CompletedDTG value");
  }
  return minutes;
}

function createInMemoryChecklistTemplateFromCsv(input: ChecklistTemplateCsvInput): ChecklistTemplateRecord {
  const name = input.name.trim();
  const rows = parseCsvRows(input.csvText);
  if (!name || rows.length < 2) {
    throw new Error("CSV must include a header row and at least one task row");
  }
  const headerRow = rows[0];
  const taskRows = rows.slice(1);
  const maxColumns = taskRows.reduce((max, row) => Math.max(max, row.length), headerRow.length);
  if (maxColumns === 0) {
    throw new Error("CSV header row is empty");
  }
  const headers = Array.from({ length: maxColumns }, (_, index) => headerRow[index]?.trim() || `Column ${index + 1}`);
  const dueHeaderIndex = headers.findIndex(isDueCsvHeader);
  const now = new Date().toISOString();
  const columns: ChecklistColumnRecord[] = [{
    columnUid: "col-due-relative-dtg",
    columnName: dueHeaderIndex >= 0 ? headers[dueHeaderIndex] : "CompletedDTG",
    displayOrder: 0,
    columnType: "RELATIVE_TIME",
    columnEditable: false,
    isRemovable: false,
    systemKey: "DUE_RELATIVE_DTG",
  }];
  const used = new Map<string, number>([["col-due-relative-dtg", 1]]);
  const headerColumnUids = new Map<number, string>();
  for (const [index, header] of headers.entries()) {
    if (index === dueHeaderIndex) {
      continue;
    }
    const columnUid = csvColumnUid(header, index, used);
    headerColumnUids.set(index, columnUid);
    columns.push({
      columnUid,
      columnName: header,
      displayOrder: columns.length,
      columnType: "SHORT_STRING",
      columnEditable: true,
      isRemovable: true,
    });
  }
  if (headerColumnUids.size === 0) {
    throw new Error("CSV must include at least one task data column");
  }
  const titleHeaderIndex = headers.findIndex((header, index) => index !== dueHeaderIndex && isTitleCsvHeader(header));
  const descriptionHeaderIndex = headers.findIndex((header, index) => index !== dueHeaderIndex && isDescriptionCsvHeader(header));
  const templateUid = input.templateUid?.trim() || `tmpl-web-${Date.now().toString(36)}`;
  const tasks = taskRows.map((row, index): ChecklistTaskRecord => {
    const number = index + 1;
    const taskUid = `${templateUid}-task-${number}`;
    const dueValue = dueHeaderIndex >= 0 ? row[dueHeaderIndex]?.trim() || "" : "";
    const dueRelativeMinutes = dueValue ? parseDueRelativeMinutes(dueValue) : number * 30;
    const title = (titleHeaderIndex >= 0 ? row[titleHeaderIndex]?.trim() : "")
      || headers.map((_, headerIndex) => headerIndex === dueHeaderIndex ? "" : row[headerIndex]?.trim() || "").find(Boolean)
      || `Task ${number}`;
    const notes = descriptionHeaderIndex >= 0 ? row[descriptionHeaderIndex]?.trim() || undefined : undefined;
    return {
      taskUid,
      number,
      userStatus: "PENDING",
      taskStatus: "PENDING",
      isLate: false,
      updatedAt: now,
      dueRelativeMinutes,
      notes,
      legacyValue: title,
      lineBreakEnabled: false,
      cells: [...headerColumnUids.entries()].map(([headerIndex, columnUid]) => ({
        cellUid: `${taskUid}:${columnUid}`,
        taskUid,
        columnUid,
        value: row[headerIndex]?.trim() || "",
        updatedAt: now,
      })),
    };
  });
  return {
    uid: templateUid,
    name,
    description: input.description?.trim() || "Imported CSV checklist template",
    version: 1,
    originType: "CSV_IMPORT",
    createdAt: now,
    updatedAt: now,
    sourceFilename: input.sourceFilename,
    columns,
    tasks,
  };
}

function createDefaultChecklistTemplates(): ChecklistTemplateRecord[] {
  const now = new Date().toISOString();
  return [
    {
      uid: "tmpl-web-autonomous-emergency",
      name: "Autonomous Emergency Checklist",
      description: "Browser visual debugging template",
      version: 1,
      originType: "RCH_TEMPLATE",
      createdAt: now,
      updatedAt: now,
      columns: defaultChecklistColumns(),
      tasks: [
        defaultChecklistTask("tmpl-web-task-1", 1, "Confirm team readiness", "Verify operator, comms, and battery status."),
        defaultChecklistTask("tmpl-web-task-2", 2, "Prepare evacuation route", "Confirm the primary route and one alternate."),
        defaultChecklistTask("tmpl-web-task-3", 3, "Share situation update", "Broadcast current status to collaborating REM nodes."),
      ],
    },
  ];
}

function formatRfc3339FromEpochMs(epochMs: number): string | undefined {
  if (!Number.isFinite(epochMs)) {
    return undefined;
  }
  return new Date(epochMs).toISOString().replace(".000Z", "Z");
}

function checklistTaskStatusFor(userStatus: ChecklistUserTaskStatus, isLate: boolean): ChecklistTaskStatus {
  if (userStatus === "COMPLETE") {
    return isLate ? "COMPLETE_LATE" : "COMPLETE";
  }
  return isLate ? "LATE" : "PENDING";
}

function normalizeInMemoryChecklist(record: ChecklistRecord): void {
  const startMs = typeof record.startTime === "string" ? Date.parse(record.startTime) : Number.NaN;
  const nowMs = Date.now();
  for (const task of record.tasks) {
    const dueMs = Number.isFinite(startMs) && typeof task.dueRelativeMinutes === "number"
      ? startMs + task.dueRelativeMinutes * 60_000
      : Number.NaN;
    task.dueDtg = formatRfc3339FromEpochMs(dueMs);
    if (Number.isFinite(dueMs)) {
      task.isLate = task.userStatus === "COMPLETE"
        ? Boolean(task.completedAt && Date.parse(task.completedAt) > dueMs)
        : nowMs > dueMs;
    }
    task.taskStatus = checklistTaskStatusFor(task.userStatus, task.isLate);
  }
  const activeTasks = record.tasks.filter((task) => !task.deletedAt);
  const pendingCount = activeTasks.filter((task) => task.taskStatus === "PENDING").length;
  const lateCount = activeTasks.filter((task) => task.taskStatus === "LATE").length;
  const completeCount = activeTasks.filter((task) =>
    task.taskStatus === "COMPLETE" || task.taskStatus === "COMPLETE_LATE",
  ).length;
  record.counts = { pendingCount, lateCount, completeCount };
  const total = activeTasks.length;
  record.expectedTaskCount = Math.max(record.expectedTaskCount ?? 0, total);
  record.progressPercent = total === 0 ? 0 : (completeCount * 100) / total;
  record.checklistStatus =
    lateCount > 0
      ? "LATE"
      : pendingCount > 0 || total === 0
        ? "PENDING"
        : "COMPLETE";
}

function emitChecklistInvalidations(
  emitter: TypedEmitter<NodeClientEvents>,
  checklistUid: string | undefined,
  reason: string,
): void {
  const revision = Date.now();
  emitter.emit("projectionInvalidated", {
    scope: "Checklists",
    revision,
    updatedAtMs: revision,
    reason,
  });
  if (checklistUid) {
    emitter.emit("projectionInvalidated", {
      scope: "ChecklistDetail",
      key: checklistUid,
      revision,
      updatedAtMs: revision,
      reason,
    });
  }
}

function findInMemoryChecklist(checklists: ChecklistRecord[], checklistUid: string): ChecklistRecord {
  const checklist = checklists.find((item) => item.uid === checklistUid);
  if (!checklist) {
    throw new Error(`Checklist ${checklistUid} not found`);
  }
  return checklist;
}

function createInMemoryChecklistFromTemplate(
  checklists: ChecklistRecord[],
  templates: ChecklistTemplateRecord[],
  status: NodeStatus,
  input: ChecklistCreateInput,
): string {
  const template = templates.find((item) => item.uid === input.templateUid) ?? templates[0];
  if (!template) {
    throw new Error("Checklist template not found");
  }
  const now = new Date().toISOString();
  const checklistUid = input.checklistUid?.trim() || `chk-web-${Date.now().toString(36)}`;
  const creatorIdentity = input.createdByTeamMemberRnsIdentity?.trim() || status.identityHex;
  const checklist: ChecklistRecord = {
    uid: checklistUid,
    missionUid: input.missionUid,
    templateUid: template.uid,
    templateVersion: template.version,
    templateName: template.name,
    name: input.name,
    description: input.description,
    startTime: input.startTime,
    mode: "ONLINE",
    syncState: "SYNCED",
    originType: template.originType,
    checklistStatus: "PENDING",
    createdAt: now,
    createdByTeamMemberRnsIdentity: creatorIdentity,
    createdByTeamMemberDisplayName: input.createdByTeamMemberDisplayName,
    updatedAt: now,
    lastChangedByTeamMemberRnsIdentity: creatorIdentity,
    participantRnsIdentities: creatorIdentity ? [creatorIdentity] : [],
    expectedTaskCount: template.tasks.filter((task) => !task.deletedAt).length,
    progressPercent: 0,
    counts: { pendingCount: 0, lateCount: 0, completeCount: 0 },
    columns: cloneChecklistTemplateRecord(template).columns,
    tasks: cloneChecklistTemplateRecord(template).tasks.map((task) => ({
      ...task,
      taskUid: task.taskUid.replace(/^tmpl-web-/, `${checklistUid}-`),
      cells: task.cells.map((cell) => ({
        ...cell,
        taskUid: cell.taskUid.replace(/^tmpl-web-/, `${checklistUid}-`),
        cellUid: cell.cellUid.replace(/^tmpl-web-/, `${checklistUid}-`),
      })),
    })),
    feedPublications: [],
  };
  for (const task of checklist.tasks) {
    task.cells = task.cells.map((cell) => ({
      ...cell,
      taskUid: task.taskUid,
      cellUid: `${task.taskUid}:${cell.columnUid}`,
    }));
  }
  normalizeInMemoryChecklist(checklist);
  checklists.push(checklist);
  return checklist.uid;
}

function updateInMemoryChecklist(checklists: ChecklistRecord[], input: ChecklistUpdateInput, changedBy?: string): void {
  const checklist = findInMemoryChecklist(checklists, input.checklistUid);
  checklist.missionUid = input.patch.missionUid ?? checklist.missionUid;
  checklist.templateUid = input.patch.templateUid ?? checklist.templateUid;
  checklist.name = input.patch.name ?? checklist.name;
  checklist.description = input.patch.description ?? checklist.description;
  checklist.startTime = input.patch.startTime ?? checklist.startTime;
  checklist.updatedAt = new Date().toISOString();
  checklist.lastChangedByTeamMemberRnsIdentity = changedBy || checklist.lastChangedByTeamMemberRnsIdentity;
}

function setInMemoryTaskStatus(checklists: ChecklistRecord[], input: ChecklistStatusInput): void {
  const checklist = findInMemoryChecklist(checklists, input.checklistUid);
  const task = checklist.tasks.find((item) => item.taskUid === input.taskUid);
  if (!task) {
    throw new Error(`Checklist task ${input.taskUid} not found`);
  }
  const now = new Date().toISOString();
  task.userStatus = input.userStatus;
  task.taskStatus = input.userStatus === "COMPLETE" ? "COMPLETE" : "PENDING";
  task.completedAt = input.userStatus === "COMPLETE" ? now : undefined;
  task.completedByTeamMemberRnsIdentity =
    input.userStatus === "COMPLETE" ? input.changedByTeamMemberRnsIdentity : undefined;
  task.updatedAt = now;
  checklist.updatedAt = now;
  checklist.lastChangedByTeamMemberRnsIdentity = input.changedByTeamMemberRnsIdentity;
  normalizeInMemoryChecklist(checklist);
}

function addInMemoryTaskRow(checklists: ChecklistRecord[], input: ChecklistRowAddInput): void {
  const checklist = findInMemoryChecklist(checklists, input.checklistUid);
  const now = new Date().toISOString();
  const taskUid = input.taskUid?.trim() || `${checklist.uid}-task-${Date.now().toString(36)}`;
  const title = input.legacyValue?.trim() || `Task ${input.number}`;
  checklist.tasks.push({
    taskUid,
    number: input.number,
    userStatus: "PENDING",
    taskStatus: "PENDING",
    isLate: false,
    updatedAt: now,
    dueRelativeMinutes: input.dueRelativeMinutes,
    legacyValue: title,
    lineBreakEnabled: false,
    cells: checklist.columns.map((column) => ({
      cellUid: `${taskUid}:${column.columnUid}`,
      taskUid,
      columnUid: column.columnUid,
      value: column.columnUid === "col-task" ? title : "",
      updatedAt: now,
    })),
  });
  checklist.updatedAt = now;
  checklist.lastChangedByTeamMemberRnsIdentity =
    input.changedByTeamMemberRnsIdentity || checklist.lastChangedByTeamMemberRnsIdentity;
  checklist.expectedTaskCount = Math.max(checklist.expectedTaskCount ?? 0, checklist.tasks.length);
  normalizeInMemoryChecklist(checklist);
}

function deleteInMemoryTaskRow(checklists: ChecklistRecord[], input: ChecklistRowDeleteInput): void {
  const checklist = findInMemoryChecklist(checklists, input.checklistUid);
  const now = new Date().toISOString();
  const task = checklist.tasks.find((item) => item.taskUid === input.taskUid);
  if (task) {
    task.deletedAt = now;
    task.updatedAt = now;
  }
  checklist.updatedAt = now;
  checklist.lastChangedByTeamMemberRnsIdentity =
    input.changedByTeamMemberRnsIdentity || checklist.lastChangedByTeamMemberRnsIdentity;
  normalizeInMemoryChecklist(checklist);
}

function setInMemoryTaskRowStyle(checklists: ChecklistRecord[], input: ChecklistRowStyleInput): void {
  const checklist = findInMemoryChecklist(checklists, input.checklistUid);
  const task = checklist.tasks.find((item) => item.taskUid === input.taskUid);
  if (!task) {
    throw new Error(`Checklist task ${input.taskUid} not found`);
  }
  const now = new Date().toISOString();
  task.rowBackgroundColor = input.rowBackgroundColor;
  task.lineBreakEnabled = input.lineBreakEnabled ?? task.lineBreakEnabled;
  task.updatedAt = now;
  checklist.updatedAt = now;
  checklist.lastChangedByTeamMemberRnsIdentity = input.changedByTeamMemberRnsIdentity;
}

function setInMemoryTaskCell(checklists: ChecklistRecord[], input: ChecklistCellInput): void {
  const checklist = findInMemoryChecklist(checklists, input.checklistUid);
  const task = checklist.tasks.find((item) => item.taskUid === input.taskUid);
  if (!task) {
    throw new Error(`Checklist task ${input.taskUid} not found`);
  }
  const now = new Date().toISOString();
  let cell = task.cells.find((item) => item.columnUid === input.columnUid);
  if (!cell) {
    cell = {
      cellUid: `${task.taskUid}:${input.columnUid}`,
      taskUid: task.taskUid,
      columnUid: input.columnUid,
    };
    task.cells.push(cell);
  }
  cell.value = input.value;
  cell.updatedAt = now;
  cell.updatedByTeamMemberRnsIdentity = input.updatedByTeamMemberRnsIdentity;
  if (input.columnUid === "col-task") {
    task.legacyValue = input.value;
  }
  task.updatedAt = now;
  checklist.updatedAt = now;
  checklist.lastChangedByTeamMemberRnsIdentity = input.updatedByTeamMemberRnsIdentity;
  normalizeInMemoryChecklist(checklist);
}

function toSosState(value: unknown): SosState {
  const normalized = String(value ?? "Idle");
  return normalized === "Countdown" || normalized === "Sending" || normalized === "Active"
    ? normalized
    : "Idle";
}

function toSosTriggerSource(value: unknown): SosTriggerSource | undefined {
  const normalized = String(value ?? "");
  if (
    normalized === "Manual"
    || normalized === "FloatingButton"
    || normalized === "Shake"
    || normalized === "TapPattern"
    || normalized === "PowerButton"
    || normalized === "Restore"
    || normalized === "Remote"
  ) {
    return normalized;
  }
  return undefined;
}

function toSosMessageKind(value: unknown): SosMessageKind {
  const normalized = String(value ?? "Active");
  return normalized === "Update" || normalized === "Cancelled" ? normalized : "Active";
}

function toSosSettingsRecord(raw: Record<string, unknown>): SosSettingsRecord {
  return {
    ...DEFAULT_SOS_SETTINGS,
    enabled: Boolean(raw.enabled),
    messageTemplate: String(raw.messageTemplate ?? raw.message_template ?? DEFAULT_SOS_SETTINGS.messageTemplate),
    cancelMessageTemplate: String(raw.cancelMessageTemplate ?? raw.cancel_message_template ?? DEFAULT_SOS_SETTINGS.cancelMessageTemplate),
    countdownSeconds: Number(raw.countdownSeconds ?? raw.countdown_seconds ?? DEFAULT_SOS_SETTINGS.countdownSeconds),
    includeLocation: Boolean(raw.includeLocation ?? raw.include_location ?? DEFAULT_SOS_SETTINGS.includeLocation),
    triggerShake: Boolean(raw.triggerShake ?? raw.trigger_shake),
    triggerTapPattern: Boolean(raw.triggerTapPattern ?? raw.trigger_tap_pattern),
    triggerPowerButton: Boolean(raw.triggerPowerButton ?? raw.trigger_power_button),
    shakeSensitivity: Number(raw.shakeSensitivity ?? raw.shake_sensitivity ?? DEFAULT_SOS_SETTINGS.shakeSensitivity),
    audioRecording: Boolean(raw.audioRecording ?? raw.audio_recording),
    audioDurationSeconds: Number(raw.audioDurationSeconds ?? raw.audio_duration_seconds ?? DEFAULT_SOS_SETTINGS.audioDurationSeconds),
    periodicUpdates: Boolean(raw.periodicUpdates ?? raw.periodic_updates),
    updateIntervalSeconds: Number(raw.updateIntervalSeconds ?? raw.update_interval_seconds ?? DEFAULT_SOS_SETTINGS.updateIntervalSeconds),
    floatingButton: Boolean(raw.floatingButton ?? raw.floating_button),
    silentAutoAnswer: Boolean(raw.silentAutoAnswer ?? raw.silent_auto_answer),
    deactivationPinHash: typeof raw.deactivationPinHash === "string" ? raw.deactivationPinHash : typeof raw.deactivation_pin_hash === "string" ? raw.deactivation_pin_hash : undefined,
    deactivationPinSalt: typeof raw.deactivationPinSalt === "string" ? raw.deactivationPinSalt : typeof raw.deactivation_pin_salt === "string" ? raw.deactivation_pin_salt : undefined,
    floatingButtonX: Number(raw.floatingButtonX ?? raw.floating_button_x ?? DEFAULT_SOS_SETTINGS.floatingButtonX),
    floatingButtonY: Number(raw.floatingButtonY ?? raw.floating_button_y ?? DEFAULT_SOS_SETTINGS.floatingButtonY),
    activePillX: Number(raw.activePillX ?? raw.active_pill_x ?? DEFAULT_SOS_SETTINGS.activePillX),
    activePillY: Number(raw.activePillY ?? raw.active_pill_y ?? DEFAULT_SOS_SETTINGS.activePillY),
  };
}

function toSosStatusRecord(raw: Record<string, unknown>): SosStatusRecord {
  const nested = raw.status;
  if (nested && typeof nested === "object" && !Array.isArray(nested)) {
    return toSosStatusRecord(nested as Record<string, unknown>);
  }
  return {
    state: toSosState(raw.state),
    incidentId: typeof raw.incidentId === "string" ? raw.incidentId : typeof raw.incident_id === "string" ? raw.incident_id : undefined,
    triggerSource: toSosTriggerSource(raw.triggerSource ?? raw.trigger_source),
    countdownDeadlineMs: toOptionalNumber(raw.countdownDeadlineMs ?? raw.countdown_deadline_ms),
    activatedAtMs: toOptionalNumber(raw.activatedAtMs ?? raw.activated_at_ms),
    lastSentAtMs: toOptionalNumber(raw.lastSentAtMs ?? raw.last_sent_at_ms),
    lastUpdateAtMs: toOptionalNumber(raw.lastUpdateAtMs ?? raw.last_update_at_ms),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
  };
}

function toSosAlertRecord(raw: Record<string, unknown>): SosAlertRecord {
  const nested = raw.alert;
  if (nested && typeof nested === "object" && !Array.isArray(nested)) {
    return toSosAlertRecord(nested as Record<string, unknown>);
  }
  return {
    incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
    sourceHex: normalizeHex(raw.sourceHex ?? raw.source_hex),
    conversationId: String(raw.conversationId ?? raw.conversation_id ?? ""),
    state: toSosMessageKind(raw.state),
    active: Boolean(raw.active ?? true),
    bodyUtf8: String(raw.bodyUtf8 ?? raw.body_utf8 ?? ""),
    lat: toOptionalNumber(raw.lat),
    lon: toOptionalNumber(raw.lon),
    batteryPercent: toOptionalNumber(raw.batteryPercent ?? raw.battery_percent),
    audioId: typeof raw.audioId === "string" ? raw.audioId : typeof raw.audio_id === "string" ? raw.audio_id : undefined,
    messageIdHex: toOptionalHex(raw.messageIdHex ?? raw.message_id_hex),
    receivedAtMs: Number(raw.receivedAtMs ?? raw.received_at_ms ?? Date.now()),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
  };
}

function toSosLocationRecord(raw: Record<string, unknown>): SosLocationRecord {
  return {
    incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
    sourceHex: normalizeHex(raw.sourceHex ?? raw.source_hex),
    lat: Number(raw.lat ?? 0),
    lon: Number(raw.lon ?? 0),
    alt: toOptionalNumber(raw.alt),
    accuracy: toOptionalNumber(raw.accuracy),
    batteryPercent: toOptionalNumber(raw.batteryPercent ?? raw.battery_percent),
    recordedAtMs: Number(raw.recordedAtMs ?? raw.recorded_at_ms ?? Date.now()),
  };
}

function toSosAudioRecord(raw: Record<string, unknown>): SosAudioRecord {
  return {
    audioId: String(raw.audioId ?? raw.audio_id ?? ""),
    incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
    sourceHex: normalizeHex(raw.sourceHex ?? raw.source_hex),
    path: String(raw.path ?? ""),
    mimeType: String(raw.mimeType ?? raw.mime_type ?? "audio/mp4"),
    durationSeconds: Number(raw.durationSeconds ?? raw.duration_seconds ?? 0),
    createdAtMs: Number(raw.createdAtMs ?? raw.created_at_ms ?? Date.now()),
  };
}

function sosSettingsToPlugin(settings: SosSettingsRecord): Record<string, unknown> {
  return {
    enabled: settings.enabled,
    messageTemplate: settings.messageTemplate,
    cancelMessageTemplate: settings.cancelMessageTemplate,
    countdownSeconds: settings.countdownSeconds,
    includeLocation: settings.includeLocation,
    triggerShake: settings.triggerShake,
    triggerTapPattern: settings.triggerTapPattern,
    triggerPowerButton: settings.triggerPowerButton,
    shakeSensitivity: settings.shakeSensitivity,
    audioRecording: settings.audioRecording,
    audioDurationSeconds: settings.audioDurationSeconds,
    periodicUpdates: settings.periodicUpdates,
    updateIntervalSeconds: settings.updateIntervalSeconds,
    floatingButton: settings.floatingButton,
    silentAutoAnswer: settings.silentAutoAnswer,
    deactivationPinHash: settings.deactivationPinHash,
    deactivationPinSalt: settings.deactivationPinSalt,
    floatingButtonX: settings.floatingButtonX,
    floatingButtonY: settings.floatingButtonY,
    activePillX: settings.activePillX,
    activePillY: settings.activePillY,
  };
}

function toOperationalSummary(raw: Record<string, unknown>): OperationalSummary {
  return {
    running: Boolean(raw.running),
    peerCountTotal: Number(raw.peerCountTotal ?? 0),
    savedPeerCount: Number(raw.savedPeerCount ?? 0),
    connectedPeerCount: Number(raw.connectedPeerCount ?? raw.connected_peer_count ?? 0),
    conversationCount: Number(raw.conversationCount ?? 0),
    messageCount: Number(raw.messageCount ?? 0),
    eamCount: Number(raw.eamCount ?? 0),
    eventCount: Number(raw.eventCount ?? 0),
    telemetryCount: Number(raw.telemetryCount ?? 0),
    activePropagationNodeHex: toOptionalHex(raw.activePropagationNodeHex),
    updatedAtMs: Number(raw.updatedAtMs ?? Date.now()),
  };
}

function configToPlugin(config: NodeConfig): Record<string, unknown> {
  return {
    name: config.name,
    storageDir: config.storageDir,
    tcpClients: config.tcpClients,
    broadcast: config.broadcast,
    announceIntervalSeconds: config.announceIntervalSeconds,
    staleAfterMinutes: config.staleAfterMinutes,
    announceCapabilities: config.announceCapabilities,
    hubMode: config.hubMode,
    hubIdentityHash: config.hubIdentityHash,
    hubApiBaseUrl: config.hubApiBaseUrl,
    hubApiKey: config.hubApiKey,
    hubRefreshIntervalSeconds: config.hubRefreshIntervalSeconds,
  };
}

class CapacitorReticulumNodeClient implements ReticulumNodeClient {
  private readonly emitter = new TypedEmitter<NodeClientEvents>();
  private readonly plugin = ReticulumNodePluginInstance;
  private listenerHandles: PluginListenerHandle[] = [];
  private attachPromise: Promise<void> | null = null;
  private generation = 0;

  private async attachListeners(): Promise<void> {
    if (this.attachPromise) {
      return this.attachPromise;
    }

    const generation = this.generation;
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
            const objectPayload =
              payload && typeof payload === "object"
                ? (payload as Record<string, unknown>)
                : {};
            this.emitter.emit(eventName, map(objectPayload));
          }),
        );
        if (generation !== this.generation) {
          await handle.remove().catch(() => undefined);
          return;
        }
        this.listenerHandles.push(handle);
      };

      await register("statusChanged", toStatusChangedEvent);
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
      await register("sosStatusChanged", (raw) => ({ status: toSosStatusRecord(raw) }));
      await register("sosAlertChanged", (raw) => ({ alert: toSosAlertRecord(raw) }));
      await register("sosTelemetryRequested", () => ({}));
      await register("sosAudioRecordingRequested", (raw) => ({
        incidentId: String(raw.incidentId ?? raw.incident_id ?? ""),
        durationSeconds: Number(raw.durationSeconds ?? raw.duration_seconds ?? 0),
      }));
      await register("log", toLogEvent);
      await register("error", toErrorEvent);
    })().catch((error) => {
      this.attachPromise = null;
      throw error;
    });

    return this.attachPromise;
  }

  private async ready(): Promise<void> {
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
      dedicatedFields: options?.dedicatedFields,
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
      dedicatedFields: options?.dedicatedFields,
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

  async listPlugins(): Promise<PluginCatalogReport> {
    await this.ready();
    return toPluginCatalogReport(await this.plugin.getPlugins());
  }

  async listPeers(): Promise<PeerRecord[]> {
    await this.ready();
    const result = await this.plugin.listPeers();
    return Array.isArray(result.items) ? result.items.map(toPeerRecord) : [];
  }

  async listConversations(): Promise<ConversationRecord[]> {
    await this.ready();
    const result = await this.plugin.listConversations();
    return Array.isArray(result.items) ? result.items.map(toConversationRecord) : [];
  }

  async listMessages(conversationId?: string): Promise<MessageRecord[]> {
    await this.ready();
    const result = await this.plugin.listMessages({ conversationId });
    return Array.isArray(result.items) ? result.items.map(toMessageRecord) : [];
  }

  async deleteConversation(conversationId: string): Promise<void> {
    await this.ready();
    await this.plugin.deleteConversation({ conversationId });
  }

  async getLxmfSyncStatus(): Promise<SyncStatus> {
    await this.ready();
    return toSyncStatus(await this.plugin.getLxmfSyncStatus());
  }

  async listTelemetryDestinations(): Promise<string[]> {
    await this.ready();
    const result = await this.plugin.listTelemetryDestinations();
    return Array.isArray(result.items) ? result.items.map((item) => normalizeHex(item)) : [];
  }

  async legacyImportCompleted(): Promise<boolean> {
    await this.ready();
    const result = await this.plugin.legacyImportCompleted();
    return Boolean(result.completed);
  }

  async importLegacyState(payload: LegacyImportPayload): Promise<void> {
    await this.ready();
    await this.plugin.importLegacyState({ payload: legacyImportPayloadToPlugin(payload) });
  }

  async getAppSettings(): Promise<AppSettingsRecord | null> {
    await this.ready();
    return toAppSettingsRecord(await this.plugin.getAppSettings());
  }

  async setAppSettings(settings: AppSettingsRecord): Promise<void> {
    await this.ready();
    await this.plugin.setAppSettings({ settings: settings as unknown as Record<string, unknown> });
  }

  async getSavedPeers(): Promise<SavedPeerRecord[]> {
    await this.ready();
    const result = await this.plugin.getSavedPeers();
    return Array.isArray(result.items) ? result.items.map(toSavedPeerRecord) : [];
  }

  async setSavedPeers(peers: SavedPeerRecord[]): Promise<void> {
    await this.ready();
    await this.plugin.setSavedPeers({ savedPeers: peers as unknown as Record<string, unknown>[] });
  }

  async getOperationalSummary(): Promise<OperationalSummary> {
    await this.ready();
    return toOperationalSummary(await this.plugin.getOperationalSummary());
  }

  async listActiveChecklists(search?: string): Promise<ChecklistRecord[]> {
    await this.ready();
    const result = await this.plugin.getChecklists({ search, sortBy: "updated_at_desc" });
    return Array.isArray(result.items) ? result.items.map(toChecklistRecord) : [];
  }

  async getChecklist(checklistUid: string): Promise<ChecklistRecord | null> {
    await this.ready();
    const result = await this.plugin.getChecklist({ checklistUid });
    const checklist =
      result.checklist && typeof result.checklist === "object"
        ? result.checklist as Record<string, unknown>
        : result && typeof result === "object" && "uid" in result
          ? result as Record<string, unknown>
          : null;
    return checklist ? toChecklistRecord(checklist) : null;
  }

  async listChecklistTemplates(search?: string): Promise<ChecklistTemplateRecord[]> {
    await this.ready();
    const result = await this.plugin.getChecklistTemplates({ search, sortBy: "updated_at_desc" });
    return Array.isArray(result.items) ? result.items.map(toChecklistTemplateRecord) : [];
  }

  async importChecklistTemplateCsv(input: {
    templateUid?: string;
    name: string;
    description?: string;
    csvText: string;
    sourceFilename?: string;
  }): Promise<ChecklistTemplateRecord> {
    await this.ready();
    return toChecklistTemplateRecord(await this.plugin.importChecklistTemplateCsv(input));
  }

  async createChecklistFromTemplate(input: {
    checklistUid?: string;
    missionUid?: string;
    templateUid: string;
    name: string;
    description: string;
    startTime: string;
    createdByTeamMemberRnsIdentity?: string;
    createdByTeamMemberDisplayName?: string;
  }): Promise<void> {
    await this.ready();
    await this.plugin.createChecklistFromTemplate(input);
  }

  async createOnlineChecklist(input: {
    checklistUid?: string;
    missionUid?: string;
    templateUid: string;
    name: string;
    description: string;
    startTime: string;
    createdByTeamMemberRnsIdentity?: string;
    createdByTeamMemberDisplayName?: string;
  }): Promise<void> {
    await this.ready();
    await this.plugin.createOnlineChecklist(input);
  }

  async updateChecklist(input: {
    checklistUid: string;
    patch: {
      missionUid?: string;
      templateUid?: string;
      name?: string;
      description?: string;
      startTime?: string;
    };
  }): Promise<void> {
    await this.ready();
    await this.plugin.updateChecklist(input);
  }

  async deleteChecklist(checklistUid: string): Promise<void> {
    await this.ready();
    await this.plugin.deleteChecklist({ checklistUid });
  }

  async joinChecklist(checklistUid: string): Promise<void> {
    await this.ready();
    await this.plugin.joinChecklist({ checklistUid });
  }

  async uploadChecklist(checklistUid: string): Promise<void> {
    await this.ready();
    await this.plugin.uploadChecklist({ checklistUid });
  }

  async setChecklistTaskStatus(input: {
    checklistUid: string;
    taskUid: string;
    userStatus: ChecklistUserTaskStatus;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void> {
    await this.ready();
    await this.plugin.setChecklistTaskStatus(input);
  }

  async addChecklistTaskRow(input: {
    checklistUid: string;
    taskUid?: string;
    number: number;
    dueRelativeMinutes?: number;
    legacyValue?: string;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void> {
    await this.ready();
    await this.plugin.addChecklistTaskRow(input);
  }

  async deleteChecklistTaskRow(input: {
    checklistUid: string;
    taskUid: string;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void> {
    await this.ready();
    await this.plugin.deleteChecklistTaskRow(input);
  }

  async setChecklistTaskRowStyle(input: {
    checklistUid: string;
    taskUid: string;
    rowBackgroundColor?: string;
    lineBreakEnabled?: boolean;
    changedByTeamMemberRnsIdentity?: string;
  }): Promise<void> {
    await this.ready();
    await this.plugin.setChecklistTaskRowStyle(input);
  }

  async setChecklistTaskCell(input: {
    checklistUid: string;
    taskUid: string;
    columnUid: string;
    value?: string;
    updatedByTeamMemberRnsIdentity?: string;
  }): Promise<void> {
    await this.ready();
    await this.plugin.setChecklistTaskCell(input);
  }

  async getEams(): Promise<EamProjectionRecord[]> {
    await this.ready();
    const result = await this.plugin.getEams();
    return Array.isArray(result.items) ? result.items.map(toEamProjectionRecord) : [];
  }

  async upsertEam(eam: EamProjectionRecord): Promise<void> {
    await this.ready();
    await this.plugin.upsertEam({ eam: eamProjectionRecordToPlugin(eam) });
  }

  async deleteEam(callsign: string, deletedAtMs?: number): Promise<void> {
    await this.ready();
    await this.plugin.deleteEam({ callsign, deletedAtMs });
  }

  async getEamTeamSummary(teamUid: string): Promise<EamTeamSummaryRecord | null> {
    await this.ready();
    return toEamTeamSummaryRecord(await this.plugin.getEamTeamSummary({ teamUid }));
  }

  async getEvents(): Promise<EventProjectionRecord[]> {
    await this.ready();
    const result = await this.plugin.getEvents();
    return Array.isArray(result.items) ? result.items.map(toEventProjectionRecord) : [];
  }

  async upsertEvent(event: EventProjectionRecord): Promise<void> {
    await this.ready();
    await this.plugin.upsertEvent({ event: eventProjectionRecordToPlugin(event) });
  }

  async deleteEvent(uid: string, deletedAtMs?: number): Promise<void> {
    await this.ready();
    await this.plugin.deleteEvent({ uid, deletedAtMs });
  }

  async getTelemetryPositions(): Promise<TelemetryPositionRecord[]> {
    await this.ready();
    const result = await this.plugin.getTelemetryPositions();
    return Array.isArray(result.items) ? result.items.map(toTelemetryPositionRecord) : [];
  }

  async recordLocalTelemetryFix(position: TelemetryPositionRecord): Promise<void> {
    await this.ready();
    await this.plugin.recordLocalTelemetryFix({ position: position as unknown as Record<string, unknown> });
  }

  async deleteLocalTelemetry(callsign: string): Promise<void> {
    await this.ready();
    await this.plugin.deleteLocalTelemetry({ callsign });
  }

  async getSosSettings(): Promise<SosSettingsRecord> {
    await this.ready();
    return toSosSettingsRecord(await this.plugin.getSosSettings());
  }

  async setSosSettings(settings: SosSettingsRecord): Promise<void> {
    await this.ready();
    await this.plugin.setSosSettings({ settings: sosSettingsToPlugin(settings) });
  }

  async setSosPin(pin?: string): Promise<void> {
    await this.ready();
    await this.plugin.setSosPin({ pin });
  }

  async getSosStatus(): Promise<SosStatusRecord> {
    await this.ready();
    return toSosStatusRecord(await this.plugin.getSosStatus());
  }

  async triggerSos(source: SosTriggerSource = "Manual"): Promise<SosStatusRecord> {
    await this.ready();
    return toSosStatusRecord(await this.plugin.triggerSos({ source }));
  }

  async deactivateSos(pin?: string): Promise<SosStatusRecord> {
    await this.ready();
    return toSosStatusRecord(await this.plugin.deactivateSos({ pin }));
  }

  async submitSosTelemetry(telemetry: SosDeviceTelemetryRecord): Promise<void> {
    await this.ready();
    await this.plugin.submitSosTelemetry({ telemetry: telemetry as unknown as Record<string, unknown> });
  }

  async listSosAlerts(): Promise<SosAlertRecord[]> {
    await this.ready();
    const result = await this.plugin.listSosAlerts();
    return Array.isArray(result.items) ? result.items.map(toSosAlertRecord) : [];
  }

  async listSosLocations(): Promise<SosLocationRecord[]> {
    await this.ready();
    const result = await this.plugin.listSosLocations();
    return Array.isArray(result.items) ? result.items.map(toSosLocationRecord) : [];
  }

  async listSosAudio(): Promise<SosAudioRecord[]> {
    await this.ready();
    const result = await this.plugin.listSosAudio();
    return Array.isArray(result.items) ? result.items.map(toSosAudioRecord) : [];
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

  on<K extends keyof NodeClientEvents>(
    event: K,
    handler: (payload: NodeClientEvents[K]) => void,
  ): () => void {
    void this.attachListeners().catch(() => undefined);
    return this.emitter.on(event, handler);
  }

  async dispose(): Promise<void> {
    this.generation += 1;
    for (const handle of this.listenerHandles) {
      await handle.remove().catch(() => undefined);
    }
    this.listenerHandles = [];
    this.attachPromise = null;
    this.emitter.clear();
  }
}

class WebReticulumNodeClient implements ReticulumNodeClient {
  private readonly emitter = new TypedEmitter<NodeClientEvents>();
  private status: NodeStatus = {
    running: false,
    name: "",
    identityHex: randomHex32(),
    appDestinationHex: randomHex32(),
    lxmfDestinationHex: randomHex32(),
  };
  private readonly connected = new Set<string>();
  private readonly savedPeers = new Map<string, SavedPeerRecord>();
  private readonly checklists: ChecklistRecord[] = [];
  private readonly checklistTemplates: ChecklistTemplateRecord[] = createDefaultChecklistTemplates();
  private sosSettings: SosSettingsRecord = { ...DEFAULT_SOS_SETTINGS };
  private sosStatus: SosStatusRecord = { ...DEFAULT_SOS_STATUS };
  private readonly sosAlerts: SosAlertRecord[] = [];
  private readonly sosLocations: SosLocationRecord[] = [];
  private readonly sosAudio: SosAudioRecord[] = [];

  private currentPeerRecords(): PeerRecord[] {
    const destinations = new Set<string>([
      ...this.savedPeers.keys(),
      ...this.connected.values(),
    ]);
    const now = Date.now();
    return [...destinations].map((destinationHex) => ({
      destinationHex,
      state: this.connected.has(destinationHex) ? "Connected" : "Disconnected",
      saved: this.savedPeers.has(destinationHex),
      stale: false,
      activeLink: this.connected.has(destinationHex),
      hubDerived: false,
      lastSeenAtMs: now,
    }));
  }

  async start(config: NodeConfig): Promise<void> {
    this.status = {
      ...this.status,
      running: true,
      name: config.name,
    };
    this.emitter.emit("statusChanged", { status: { ...this.status } });
    this.emitter.emit("log", {
      level: "Info",
      message: "Web runtime node started.",
    });
  }

  async stop(): Promise<void> {
    for (const destinationHex of this.connected) {
      this.emitter.emit("peerChanged", {
        change: {
          destinationHex,
          state: "Disconnected",
          saved: false,
          stale: false,
          activeLink: false,
          hubDerived: false,
          lastSeenAtMs: Date.now(),
        },
      });
    }
    this.connected.clear();
    this.status = {
      ...this.status,
      running: false,
    };
    this.emitter.emit("statusChanged", { status: { ...this.status } });
  }

  async restart(config: NodeConfig): Promise<void> {
    await this.start(config);
  }

  async getStatus(): Promise<NodeStatus> {
    return { ...this.status };
  }

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
        saved: false,
        stale: false,
        activeLink: false,
        hubDerived: false,
        lastSeenAtMs: Date.now(),
      },
    });
  }

  async announceNow(): Promise<void> {}

  async requestPeerIdentity(_destinationHex: string): Promise<void> {}

  async sendBytes(destinationHex: string, bytes: Uint8Array, _options?: PacketSendOptions): Promise<void> {
    const normalized = normalizeHex(destinationHex);
    this.emitter.emit("packetSent", {
      destinationHex: normalized,
      bytes,
      outcome: this.connected.has(normalized) ? "SentDirect" : "DroppedNoRoute",
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
      title: request.title,
      bodyUtf8: request.bodyUtf8,
      method: "Direct",
      state: this.connected.has(destinationHex) ? "Delivered" : "Failed",
      detail: this.connected.has(destinationHex) ? "web mock delivery" : "web mock missing route",
      sentAtMs: now,
      updatedAtMs: now,
    });
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

  async setAnnounceCapabilities(_capabilityString: string): Promise<void> {}

  async setLogLevel(level: LogLevel): Promise<void> {
    this.emitter.emit("log", {
      level,
      message: `Web runtime log level set to ${level}.`,
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

  async listPlugins(): Promise<PluginCatalogReport> {
    return { items: [], errors: [] };
  }

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
    const connectedPeerCount = [...this.connected].filter((destination) => this.savedPeers.has(destination)).length;
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
  async getEams(): Promise<EamProjectionRecord[]> { return []; }
  async upsertEam(_eam: EamProjectionRecord): Promise<void> {}
  async deleteEam(_callsign: string, _deletedAtMs?: number): Promise<void> {}
  async getEamTeamSummary(_teamUid: string): Promise<EamTeamSummaryRecord | null> { return null; }
  async getEvents(): Promise<EventProjectionRecord[]> { return []; }
  async upsertEvent(_event: EventProjectionRecord): Promise<void> {}
  async deleteEvent(_uid: string, _deletedAtMs?: number): Promise<void> {}
  async getTelemetryPositions(): Promise<TelemetryPositionRecord[]> { return []; }
  async recordLocalTelemetryFix(_position: TelemetryPositionRecord): Promise<void> {}
  async deleteLocalTelemetry(_callsign: string): Promise<void> {}

  async getSosSettings(): Promise<SosSettingsRecord> { return { ...this.sosSettings }; }
  async setSosSettings(settings: SosSettingsRecord): Promise<void> {
    this.sosSettings = { ...settings };
    this.emitter.emit("projectionInvalidated", {
      scope: "Sos",
      revision: Date.now(),
      updatedAtMs: Date.now(),
      reason: "webSettings",
    });
  }

  async listActiveChecklists(search?: string): Promise<ChecklistRecord[]> {
    const needle = search?.trim().toLowerCase();
    return this.checklists
      .filter((item) => !item.deletedAt)
      .filter((item) => !needle || item.name.toLowerCase().includes(needle))
      .map(cloneChecklistRecord);
  }

  async getChecklist(checklistUid: string): Promise<ChecklistRecord | null> {
    const checklist = this.checklists.find((item) => item.uid === checklistUid && !item.deletedAt);
    return checklist ? cloneChecklistRecord(checklist) : null;
  }

  async listChecklistTemplates(search?: string): Promise<ChecklistTemplateRecord[]> {
    const needle = search?.trim().toLowerCase();
    return this.checklistTemplates
      .filter((item) => !needle || item.name.toLowerCase().includes(needle))
      .map(cloneChecklistTemplateRecord);
  }

  async importChecklistTemplateCsv(input: ChecklistTemplateCsvInput): Promise<ChecklistTemplateRecord> {
    const template = createInMemoryChecklistTemplateFromCsv(input);
    const existingIndex = this.checklistTemplates.findIndex((item) => item.uid === template.uid);
    if (existingIndex >= 0) {
      this.checklistTemplates.splice(existingIndex, 1, template);
    } else {
      this.checklistTemplates.unshift(template);
    }
    emitChecklistInvalidations(this.emitter, template.uid, "webChecklistTemplateImport");
    return cloneChecklistTemplateRecord(template);
  }

  async createChecklistFromTemplate(input: ChecklistCreateInput): Promise<void> {
    const uid = createInMemoryChecklistFromTemplate(this.checklists, this.checklistTemplates, this.status, input);
    emitChecklistInvalidations(this.emitter, uid, "webChecklistCreate");
  }

  async createOnlineChecklist(input: ChecklistCreateInput): Promise<void> {
    await this.createChecklistFromTemplate(input);
  }

  async updateChecklist(input: ChecklistUpdateInput): Promise<void> {
    updateInMemoryChecklist(this.checklists, input, this.status.identityHex);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "webChecklistUpdate");
  }

  async deleteChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    checklist.deletedAt = new Date().toISOString();
    checklist.updatedAt = checklist.deletedAt;
    checklist.lastChangedByTeamMemberRnsIdentity = this.status.identityHex;
    emitChecklistInvalidations(this.emitter, checklistUid, "webChecklistDelete");
  }

  async joinChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    if (this.status.identityHex && !checklist.participantRnsIdentities.includes(this.status.identityHex)) {
      checklist.participantRnsIdentities.push(this.status.identityHex);
    }
    checklist.updatedAt = new Date().toISOString();
    checklist.lastChangedByTeamMemberRnsIdentity = this.status.identityHex;
    emitChecklistInvalidations(this.emitter, checklistUid, "webChecklistJoin");
  }

  async uploadChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    checklist.uploadedAt = new Date().toISOString();
    checklist.syncState = "SYNCED";
    emitChecklistInvalidations(this.emitter, checklistUid, "webChecklistUpload");
  }

  async setChecklistTaskStatus(input: ChecklistStatusInput): Promise<void> {
    setInMemoryTaskStatus(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "webChecklistTaskStatus");
  }

  async addChecklistTaskRow(input: ChecklistRowAddInput): Promise<void> {
    addInMemoryTaskRow(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "webChecklistTaskAdd");
  }

  async deleteChecklistTaskRow(input: ChecklistRowDeleteInput): Promise<void> {
    deleteInMemoryTaskRow(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "webChecklistTaskDelete");
  }

  async setChecklistTaskRowStyle(input: ChecklistRowStyleInput): Promise<void> {
    setInMemoryTaskRowStyle(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "webChecklistTaskStyle");
  }

  async setChecklistTaskCell(input: ChecklistCellInput): Promise<void> {
    setInMemoryTaskCell(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "webChecklistTaskCell");
  }
  async setSosPin(_pin?: string): Promise<void> {}
  async getSosStatus(): Promise<SosStatusRecord> { return { ...this.sosStatus }; }
  async triggerSos(source: SosTriggerSource = "Manual"): Promise<SosStatusRecord> {
    const now = Date.now();
    this.sosStatus = {
      state: "Active",
      incidentId: `web-${now}`,
      triggerSource: source,
      activatedAtMs: now,
      lastSentAtMs: now,
      updatedAtMs: now,
    };
    this.emitter.emit("sosStatusChanged", { status: { ...this.sosStatus } });
    return { ...this.sosStatus };
  }
  async deactivateSos(_pin?: string): Promise<SosStatusRecord> {
    this.sosStatus = { state: "Idle", updatedAtMs: Date.now() };
    this.emitter.emit("sosStatusChanged", { status: { ...this.sosStatus } });
    return { ...this.sosStatus };
  }
  async submitSosTelemetry(_telemetry: SosDeviceTelemetryRecord): Promise<void> {}
  async listSosAlerts(): Promise<SosAlertRecord[]> { return [...this.sosAlerts]; }
  async listSosLocations(): Promise<SosLocationRecord[]> { return [...this.sosLocations]; }
  async listSosAudio(): Promise<SosAudioRecord[]> { return [...this.sosAudio]; }

  async logMessage(level: LogLevel, message: string): Promise<void> {
    this.emitter.emit("log", { level, message });
  }

  async refreshHubDirectory(): Promise<void> {
    this.emitter.emit("hubDirectoryUpdated", {
      effectiveConnectedMode: false,
      items: [],
      receivedAtMs: Date.now(),
    });
  }

  on<K extends keyof NodeClientEvents>(
    event: K,
    handler: (payload: NodeClientEvents[K]) => void,
  ): () => void {
    return this.emitter.on(event, handler);
  }

  async dispose(): Promise<void> {
    this.emitter.clear();
  }
}

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

function randomHex32(): string {
  const chars = "0123456789abcdef";
  let out = "";
  for (let i = 0; i < 32; i += 1) {
    out += chars[Math.floor(Math.random() * chars.length)];
  }
  return out;
}

class MockReticulumNodeClient implements ReticulumNodeClient {
  private readonly emitter = new TypedEmitter<NodeClientEvents>();
  private status: NodeStatus = {
    running: false,
    name: "mock-node",
    identityHex: randomHex32(),
    appDestinationHex: randomHex32(),
    lxmfDestinationHex: randomHex32(),
  };
  private capabilities = DEFAULT_NODE_CONFIG.announceCapabilities;
  private announceTimer: number | null = null;
  private readonly connected = new Set<string>();
  private readonly savedPeers = new Map<string, SavedPeerRecord>();
  private readonly checklists: ChecklistRecord[] = [];
  private readonly checklistTemplates: ChecklistTemplateRecord[] = createDefaultChecklistTemplates();
  private sosSettings: SosSettingsRecord = { ...DEFAULT_SOS_SETTINGS };
  private sosStatus: SosStatusRecord = { ...DEFAULT_SOS_STATUS };
  private readonly sosAlerts: SosAlertRecord[] = [];
  private readonly sosLocations: SosLocationRecord[] = [];
  private readonly sosAudio: SosAudioRecord[] = [];

  private currentPeerRecords(): PeerRecord[] {
    const destinations = new Set<string>([
      ...this.savedPeers.keys(),
      ...this.connected.values(),
    ]);
    const now = Date.now();
    return [...destinations].map((destinationHex) => ({
      destinationHex,
      state: this.connected.has(destinationHex) ? "Connected" : "Disconnected",
      saved: this.savedPeers.has(destinationHex),
      stale: false,
      activeLink: this.connected.has(destinationHex),
      hubDerived: false,
      lastSeenAtMs: now,
    }));
  }

  private emitAnnounce(
    destinationHex: string,
    appData: string,
    identityHex = randomHex32(),
    destinationKind: AnnounceDestinationKind = "app",
    announceClass: AnnounceClass = "PeerApp",
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
      this.emitAnnounce(peer, "R3AKT,EMergencyMessages", identityHex, "app");
      this.emitAnnounce(randomHex32(), "6ac46f686174", identityHex, "lxmf_delivery");
    }
    this.emitAnnounce(randomHex32(), "LXMF,Chat", randomHex32(), "other");

    this.announceTimer = window.setInterval(() => {
      const shuffled = [...MOCK_ANNOUNCED_PEERS.entries()].sort(() => Math.random() - 0.5);
      const [index, destinationHex] = shuffled[0] ?? [0, randomHex32()];
      this.emitAnnounce(
        destinationHex,
        Math.random() > 0.25 ? this.capabilities : "R3AKT,Other",
        MOCK_ANNOUNCED_IDENTITIES[index] ?? randomHex32(),
        "app",
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
    this.status = {
      ...this.status,
      running: false,
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
        saved: false,
        stale: false,
        activeLink: false,
        hubDerived: false,
        lastSeenAtMs: Date.now(),
      },
    });
  }

  async announceNow(): Promise<void> {
    this.emitAnnounce(this.status.appDestinationHex, this.capabilities, this.status.identityHex, "app");
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
      title: request.title,
      bodyUtf8: request.bodyUtf8,
      method: "Direct",
      state: "SentDirect",
      sentAtMs: now,
      updatedAtMs: now,
    });
    window.setTimeout(() => {
      this.emitter.emit("messageUpdated", {
        messageIdHex,
        conversationId: destinationHex,
        direction: "Outbound",
        destinationHex,
        title: request.title,
        bodyUtf8: request.bodyUtf8,
        method: "Direct",
        state: "Delivered",
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
    this.emitAnnounce(this.status.appDestinationHex, capabilityString);
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

  async listPlugins(): Promise<PluginCatalogReport> {
    return { items: [], errors: [] };
  }

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
    const connectedPeerCount = [...this.connected].filter((destination) => this.savedPeers.has(destination)).length;
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
  async getEams(): Promise<EamProjectionRecord[]> { return []; }
  async upsertEam(_eam: EamProjectionRecord): Promise<void> {}
  async deleteEam(_callsign: string, _deletedAtMs?: number): Promise<void> {}
  async getEamTeamSummary(_teamUid: string): Promise<EamTeamSummaryRecord | null> { return null; }
  async getEvents(): Promise<EventProjectionRecord[]> { return []; }
  async upsertEvent(_event: EventProjectionRecord): Promise<void> {}
  async deleteEvent(_uid: string, _deletedAtMs?: number): Promise<void> {}
  async getTelemetryPositions(): Promise<TelemetryPositionRecord[]> { return []; }
  async recordLocalTelemetryFix(_position: TelemetryPositionRecord): Promise<void> {}
  async deleteLocalTelemetry(_callsign: string): Promise<void> {}

  async getSosSettings(): Promise<SosSettingsRecord> { return { ...this.sosSettings }; }
  async setSosSettings(settings: SosSettingsRecord): Promise<void> {
    this.sosSettings = { ...settings };
    this.emitter.emit("projectionInvalidated", {
      scope: "Sos",
      revision: Date.now(),
      updatedAtMs: Date.now(),
      reason: "mockSettings",
    });
  }

  async listActiveChecklists(search?: string): Promise<ChecklistRecord[]> {
    const needle = search?.trim().toLowerCase();
    return this.checklists
      .filter((item) => !item.deletedAt)
      .filter((item) => !needle || item.name.toLowerCase().includes(needle))
      .map(cloneChecklistRecord);
  }

  async getChecklist(checklistUid: string): Promise<ChecklistRecord | null> {
    const checklist = this.checklists.find((item) => item.uid === checklistUid && !item.deletedAt);
    return checklist ? cloneChecklistRecord(checklist) : null;
  }

  async listChecklistTemplates(search?: string): Promise<ChecklistTemplateRecord[]> {
    const needle = search?.trim().toLowerCase();
    return this.checklistTemplates
      .filter((item) => !needle || item.name.toLowerCase().includes(needle))
      .map(cloneChecklistTemplateRecord);
  }

  async importChecklistTemplateCsv(input: ChecklistTemplateCsvInput): Promise<ChecklistTemplateRecord> {
    const template = createInMemoryChecklistTemplateFromCsv(input);
    const existingIndex = this.checklistTemplates.findIndex((item) => item.uid === template.uid);
    if (existingIndex >= 0) {
      this.checklistTemplates.splice(existingIndex, 1, template);
    } else {
      this.checklistTemplates.unshift(template);
    }
    emitChecklistInvalidations(this.emitter, template.uid, "mockChecklistTemplateImport");
    return cloneChecklistTemplateRecord(template);
  }

  async createChecklistFromTemplate(input: ChecklistCreateInput): Promise<void> {
    const uid = createInMemoryChecklistFromTemplate(this.checklists, this.checklistTemplates, this.status, input);
    emitChecklistInvalidations(this.emitter, uid, "mockChecklistCreate");
  }

  async createOnlineChecklist(input: ChecklistCreateInput): Promise<void> {
    await this.createChecklistFromTemplate(input);
  }

  async updateChecklist(input: ChecklistUpdateInput): Promise<void> {
    updateInMemoryChecklist(this.checklists, input, this.status.identityHex);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "mockChecklistUpdate");
  }

  async deleteChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    checklist.deletedAt = new Date().toISOString();
    checklist.updatedAt = checklist.deletedAt;
    checklist.lastChangedByTeamMemberRnsIdentity = this.status.identityHex;
    emitChecklistInvalidations(this.emitter, checklistUid, "mockChecklistDelete");
  }

  async joinChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    if (this.status.identityHex && !checklist.participantRnsIdentities.includes(this.status.identityHex)) {
      checklist.participantRnsIdentities.push(this.status.identityHex);
    }
    checklist.updatedAt = new Date().toISOString();
    checklist.lastChangedByTeamMemberRnsIdentity = this.status.identityHex;
    emitChecklistInvalidations(this.emitter, checklistUid, "mockChecklistJoin");
  }

  async uploadChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    checklist.uploadedAt = new Date().toISOString();
    checklist.syncState = "SYNCED";
    emitChecklistInvalidations(this.emitter, checklistUid, "mockChecklistUpload");
  }

  async setChecklistTaskStatus(input: ChecklistStatusInput): Promise<void> {
    setInMemoryTaskStatus(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "mockChecklistTaskStatus");
  }

  async addChecklistTaskRow(input: ChecklistRowAddInput): Promise<void> {
    addInMemoryTaskRow(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "mockChecklistTaskAdd");
  }

  async deleteChecklistTaskRow(input: ChecklistRowDeleteInput): Promise<void> {
    deleteInMemoryTaskRow(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "mockChecklistTaskDelete");
  }

  async setChecklistTaskRowStyle(input: ChecklistRowStyleInput): Promise<void> {
    setInMemoryTaskRowStyle(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "mockChecklistTaskStyle");
  }

  async setChecklistTaskCell(input: ChecklistCellInput): Promise<void> {
    setInMemoryTaskCell(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, "mockChecklistTaskCell");
  }
  async setSosPin(_pin?: string): Promise<void> {}
  async getSosStatus(): Promise<SosStatusRecord> { return { ...this.sosStatus }; }
  async triggerSos(source: SosTriggerSource = "Manual"): Promise<SosStatusRecord> {
    const now = Date.now();
    this.sosStatus = {
      state: "Active",
      incidentId: `mock-${now}`,
      triggerSource: source,
      activatedAtMs: now,
      lastSentAtMs: now,
      updatedAtMs: now,
    };
    this.emitter.emit("sosStatusChanged", { status: { ...this.sosStatus } });
    return { ...this.sosStatus };
  }
  async deactivateSos(_pin?: string): Promise<SosStatusRecord> {
    this.sosStatus = { state: "Idle", updatedAtMs: Date.now() };
    this.emitter.emit("sosStatusChanged", { status: { ...this.sosStatus } });
    return { ...this.sosStatus };
  }
  async submitSosTelemetry(_telemetry: SosDeviceTelemetryRecord): Promise<void> {}
  async listSosAlerts(): Promise<SosAlertRecord[]> { return [...this.sosAlerts]; }
  async listSosLocations(): Promise<SosLocationRecord[]> { return [...this.sosLocations]; }
  async listSosAudio(): Promise<SosAudioRecord[]> { return [...this.sosAudio]; }

  async logMessage(level: LogLevel, message: string): Promise<void> {
    this.emitter.emit("log", { level, message });
  }

  async refreshHubDirectory(): Promise<void> {
    this.emitter.emit("hubDirectoryUpdated", {
      effectiveConnectedMode: false,
      items: MOCK_HUB_PEERS,
      receivedAtMs: Date.now(),
    });
  }

  on<K extends keyof NodeClientEvents>(
    event: K,
    handler: (payload: NodeClientEvents[K]) => void,
  ): () => void {
    return this.emitter.on(event, handler);
  }

  async dispose(): Promise<void> {
    this.emitter.clear();
  }
}

export function createReticulumNodeClient(
  options: ReticulumNodeClientFactoryOptions = {},
): ReticulumNodeClient {
  const mode = options.mode ?? "auto";
  if (mode === "web") {
    return new WebReticulumNodeClient();
  }
  if (mode === "capacitor") {
    return new CapacitorReticulumNodeClient();
  }
  if (Capacitor.getPlatform() === "web") {
    return new WebReticulumNodeClient();
  }
  return new CapacitorReticulumNodeClient();
}

export function bytesToBase64(bytes: Uint8Array): string {
  return encodeBytesToBase64(bytes);
}

export function base64ToBytes(base64: string): Uint8Array {
  return decodeBase64ToBytes(base64);
}
