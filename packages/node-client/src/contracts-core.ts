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
  | "Sos"
  | "Plugins"
  | "PluginSensors";

export type PluginState =
  | "Discovered"
  | "Untrusted"
  | "Disabled"
  | "Binding"
  | "Running"
  | "Stopped"
  | "Failed"
  | "Incompatible"
  | "Missing";

export interface PluginCapabilityRecord {
  eventsPublish: boolean;
  sensorsPublish: boolean;
  lxmfSend: boolean;
  lxmfReceive: boolean;
  notificationsRaise: boolean;
}

export interface PluginMessageDescriptorRecord {
  name: string;
  version: string;
  send: boolean;
  receive: boolean;
  schema: Record<string, unknown>;
}

export interface InstalledPluginRecord {
  pluginId: string;
  displayName: string;
  version: string;
  apiMajor: number;
  apiMinor: number;
  packageName: string;
  serviceClassName: string;
  publisherFingerprint: string;
  publisherHistory: string[];
  androidPermissions: string[];
  declaredCapabilities: PluginCapabilityRecord;
  messages: PluginMessageDescriptorRecord[];
  configurationEntrypoint?: string;
  state: PluginState;
  trusted: boolean;
  enabled: boolean;
  grantedCapabilities: PluginCapabilityRecord;
  diagnostic?: string;
  updatedAtMs: number;
}

export interface PluginSensorRecord {
  pluginId: string;
  deviceId: string;
  sensorType: string;
  displayName: string;
  value: unknown;
  unit?: string;
  operatorRnsIdentity?: string;
  confidence?: number;
  connectionState?: string;
  sampleAtMs: number;
  staleAfterMs: number;
  status: "Active" | "Stale" | "Offline";
  origin: "local" | "remote";
}

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
