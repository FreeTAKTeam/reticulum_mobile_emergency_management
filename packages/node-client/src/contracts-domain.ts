import type {
  LogLevel, RnodeRegion, RnodeProfileId, RnodeConnectionMode, RuntimeReadinessState, PeerState, AnnounceDestinationKind,
  AnnounceClass, SendOutcome, LxmfDeliveryStatus, TransportDeliveryState, ApplicationAckState, SendMode, LxmfDeliveryMethod, LxmfDeliveryRepresentation,
  LxmfFallbackStage, MessageMethod, MessageState, MessageDirection, ClientMode, ProjectionScope, PluginState, PluginCapabilityRecord,
  PluginMessageDescriptorRecord, InstalledPluginRecord, PluginSensorRecord, SosState, SosTriggerSource, SosMessageKind, RnodeSettingsRecord, RnodeBleDeviceRecord,
  RnodeBlePairResult, RnodeUsbDeviceRecord, RnodeUsbPairResult, NodeConfig, NodeStatus, RuntimeInterfaceReadinessRecord, RuntimeReadinessSnapshot, InterfaceStatusRecord,
  PeerChange, StatusChangedEvent, InterfaceStatusChangedEvent, AnnounceReceivedEvent, AnnounceRecord, PeerChangedEvent, PacketReceivedEvent, PacketSendOptions,
  PacketSentEvent, LxmfDeliveryEvent, MessageRecord, PeerRecord, ConversationRecord, SyncPhase, SyncStatus, SendLxmfRequest,
  HubSettingsRecord,
} from "./contracts-core";
import type { CircleTier, CommunitySettingsRecord, PowerPolicyRecord, PowerStateRecord } from "./contracts-community";
export * from "./contracts-community";

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

export const YELLOW_TEAM_UID = "d6b6e188b910d6bdd24d04b7a7ec5444";
export const CANONICAL_TEAMS = [
  { uid: YELLOW_TEAM_UID, color: "YELLOW" },
  { uid: "65ce79a3a3e4b51ec0ec52d1d3d2b0b9", color: "RED" },
  { uid: "43341e5c822d99857fa6e8641f2ca9c0", color: "BLUE" },
  { uid: "a83eb640e4c4884be14831e3d7ef5ae0", color: "ORANGE" },
  { uid: "7ac50a910f42b06cd9cb68dad3def681", color: "MAGENTA" },
  { uid: "372824ef4f15881291455562f7570233", color: "MAROON" },
  { uid: "4bf2a1d2217c8668942658137f2a6824", color: "PURPLE" },
  { uid: "cbb35fc9a8f5a91d7bd2b5e5b644edcd", color: "DARK_BLUE" },
  { uid: "d4cd5030b68df059ec6beabe416dd6a6", color: "CYAN" },
  { uid: "4d7a7a974beec395bf83491604768499", color: "TEAL" },
  { uid: "612a32262163b73a80eca944c2158546", color: "GREEN" },
  { uid: "341653613d4c76d56bee99c1f38177b1", color: "DARK_GREEN" },
  { uid: "4efe72ac30f5b85142fdcab6d96c7631", color: "BROWN" },
] as const;
export const CANONICAL_TEAM_UIDS = new Set<string>(CANONICAL_TEAMS.map(({ uid }) => uid));

export interface HubTeamRecord {
  uid: string;
  color: string;
  teamName: string;
}

export interface HubCallerMembershipRecord {
  teamUid: string;
  teamMemberUid: string;
}

export interface HubTeamMemberRecord extends HubDirectoryPeerRecord {
  teamUid: string;
  teamMemberUid: string;
}

export interface TeamAliasRecord {
  teamUid: string;
  alias: string;
}

export interface LocalTeamRecord {
  teamUid: string;
  memberDestinations: string[];
}

export interface TeamSettingsRecord {
  activeTeamUid: string;
  aliases: TeamAliasRecord[];
  localTeams: LocalTeamRecord[];
  localTeamsInitialized: boolean;
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
  teams: TeamSettingsRecord;
  checklists: ChecklistSettingsRecord;
  rnode: RnodeSettingsRecord;
  community: CommunitySettingsRecord;
  power: PowerPolicyRecord;
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
  circleTier: CircleTier;
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

export interface HubDirectoryUpdatedEvent {
  schemaVersion: number;
  hubIdentityHash?: string;
  activeTeamUid: string;
  effectiveConnectedMode: boolean;
  teams: HubTeamRecord[];
  callerMemberships: HubCallerMembershipRecord[];
  members: HubTeamMemberRecord[];
  localTeams: LocalTeamRecord[];
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
  powerStateChanged: PowerStateRecord;
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
  pluginEventPublished: { pluginId: string; event: Record<string, unknown> };
  sosStatusChanged: { status: SosStatusRecord };
  sosAlertChanged: { alert: SosAlertRecord };
  sosTelemetryRequested: Record<string, never>;
  sosAudioRecordingRequested: { incidentId: string; durationSeconds: number };
  log: NodeLogEvent;
  error: NodeErrorEvent;
}
