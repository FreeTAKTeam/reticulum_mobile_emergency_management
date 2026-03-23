import { Capacitor, registerPlugin } from "@capacitor/core";

export type LogLevel = "Trace" | "Debug" | "Info" | "Warn" | "Error";
export type HubMode = "Disabled" | "RchLxmf" | "RchHttp";
export type PeerState = "Connecting" | "Connected" | "Disconnected";
export type PeerManagementState = "Unmanaged" | "Managed";
export type PeerAvailabilityState = "Unseen" | "Discovered" | "Resolved" | "Ready";
export type AnnounceDestinationKind = "app" | "lxmf_delivery" | "lxmf_propagation" | "other";
export type SendOutcome =
  | "SentDirect"
  | "SentBroadcast"
  | "DroppedMissingDestinationIdentity"
  | "DroppedCiphertextTooLarge"
  | "DroppedEncryptFailed"
  | "DroppedNoRoute";
export type LxmfDeliveryStatus = "Sent" | "Acknowledged" | "Failed" | "TimedOut";
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

export interface NodeConfig {
  name: string;
  storageDir?: string;
  tcpClients: string[];
  broadcast: boolean;
  announceIntervalSeconds: number;
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
  managementState?: PeerManagementState;
  availabilityState?: PeerAvailabilityState;
  activeLink?: boolean;
  lastError?: string;
  lastResolutionError?: string;
  lastResolutionAttemptAtMs?: number;
  lastReadyAtMs?: number;
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
  appData: string;
  hops: number;
  interfaceHex: string;
  receivedAtMs: number;
}

export interface AnnounceRecord {
  destinationHex: string;
  identityHex: string;
  destinationKind: AnnounceDestinationKind;
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
  usePropagationNode?: boolean;
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
  managementState: PeerManagementState;
  availabilityState: PeerAvailabilityState;
  activeLink: boolean;
  lastResolutionError?: string;
  lastResolutionAttemptAtMs?: number;
  lastReadyAtMs?: number;
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
  usePropagationNode?: boolean;
}

export interface HubDirectoryUpdatedEvent {
  destinations: string[];
  receivedAtMs: number;
}

export interface NodeLogEvent {
  level: LogLevel;
  message: string;
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
  listPeers(): Promise<PeerRecord[]>;
  listConversations(): Promise<ConversationRecord[]>;
  listMessages(conversationId?: string): Promise<MessageRecord[]>;
  getLxmfSyncStatus(): Promise<SyncStatus>;
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
  announceCapabilities: "R3AKT,EMergencyMessages",
  hubMode: "Disabled",
  hubRefreshIntervalSeconds: 3600,
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
    usePropagationNode?: boolean;
  }): Promise<void>;
  sendLxmf(options: {
    destinationHex: string;
    bodyUtf8: string;
    title?: string;
    usePropagationNode?: boolean;
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
  listPeers(): Promise<{ items: Record<string, unknown>[] }>;
  listConversations(): Promise<{ items: Record<string, unknown>[] }>;
  listMessages(options: { conversationId?: string }): Promise<{ items: Record<string, unknown>[] }>;
  getLxmfSyncStatus(): Promise<Record<string, unknown>>;
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

function toPeerState(raw: unknown): PeerState {
  const value = String(raw ?? "");
  if (value === "Connecting" || value === "Connected" || value === "Disconnected") {
    return value;
  }
  return "Disconnected";
}

function toPeerManagementState(raw: unknown): PeerManagementState {
  return String(raw ?? "") === "Managed" ? "Managed" : "Unmanaged";
}

function toPeerAvailabilityState(raw: unknown): PeerAvailabilityState {
  const value = String(raw ?? "");
  if (value === "Discovered" || value === "Resolved" || value === "Ready" || value === "Unseen") {
    return value;
  }
  return "Unseen";
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
  return {
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    identityHex: normalizeHex(
      String(raw.identityHex ?? raw.identity_hex ?? ""),
    ),
    destinationKind,
    appData: String(raw.appData ?? raw.app_data ?? ""),
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
  const managementStateRaw = hasValue(changeRaw.managementState)
    ? changeRaw.managementState
    : changeRaw.management_state;
  const availabilityStateRaw = hasValue(changeRaw.availabilityState)
    ? changeRaw.availabilityState
    : changeRaw.availability_state;
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
      managementState: hasValue(managementStateRaw)
        ? toPeerManagementState(managementStateRaw)
        : undefined,
      availabilityState: hasValue(availabilityStateRaw)
        ? toPeerAvailabilityState(availabilityStateRaw)
        : undefined,
      activeLink: hasValue(activeLinkRaw) ? Boolean(activeLinkRaw) : undefined,
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
      lastReadyAtMs: toOptionalNumber(
        hasValue(changeRaw.lastReadyAtMs)
          ? changeRaw.lastReadyAtMs
          : changeRaw.last_ready_at_ms,
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
    managementState: toPeerManagementState(raw.managementState ?? raw.management_state),
    availabilityState: toPeerAvailabilityState(raw.availabilityState ?? raw.availability_state),
    activeLink: Boolean(raw.activeLink ?? raw.active_link),
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
    lastReadyAtMs: toOptionalNumber(
      hasValue(raw.lastReadyAtMs) ? raw.lastReadyAtMs : raw.last_ready_at_ms,
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
  const valid: LxmfDeliveryStatus[] = ["Sent", "Acknowledged", "Failed", "TimedOut"];
  return valid.includes(value as LxmfDeliveryStatus)
    ? (value as LxmfDeliveryStatus)
    : "Failed";
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
    detail:
      typeof raw.detail === "string"
        ? raw.detail
        : undefined,
    sentAtMs: Number(raw.sentAtMs ?? raw.sent_at_ms ?? Date.now()),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
  };
}

function toMessageMethod(raw: unknown): MessageMethod {
  const value = String(raw ?? "");
  const valid: MessageMethod[] = ["Direct", "Opportunistic", "Propagated", "Resource"];
  return valid.includes(value as MessageMethod) ? (value as MessageMethod) : "Direct";
}

function toMessageState(raw: unknown): MessageState {
  const value = String(raw ?? "");
  const valid: MessageState[] = [
    "Queued",
    "PathRequested",
    "LinkEstablishing",
    "Sending",
    "SentDirect",
    "SentToPropagation",
    "Delivered",
    "Failed",
    "TimedOut",
    "Cancelled",
    "Received",
  ];
  return valid.includes(value as MessageState) ? (value as MessageState) : "Failed";
}

function toMessageDirection(raw: unknown): MessageDirection {
  return String(raw ?? "") === "Inbound" ? "Inbound" : "Outbound";
}

function toMessageRecord(raw: Record<string, unknown>): MessageRecord {
  return {
    messageIdHex: normalizeHex(String(raw.messageIdHex ?? raw.message_id_hex ?? "")),
    conversationId: String(raw.conversationId ?? raw.conversation_id ?? ""),
    direction: toMessageDirection(raw.direction),
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
  const destinations = Array.isArray(raw.destinations)
    ? raw.destinations.map((item) => normalizeHex(String(item)))
    : [];
  return {
    destinations,
    receivedAtMs: Number(raw.receivedAtMs ?? raw.received_at_ms ?? Date.now()),
  };
}

function toLogEvent(raw: Record<string, unknown>): NodeLogEvent {
  return {
    level: (String(raw.level ?? "Info") as LogLevel) ?? "Info",
    message: String(raw.message ?? ""),
  };
}

function toErrorEvent(raw: Record<string, unknown>): NodeErrorEvent {
  return {
    code: String(raw.code ?? "UNKNOWN"),
    message: String(raw.message ?? "Unknown plugin error"),
  };
}

function configToPlugin(config: NodeConfig): Record<string, unknown> {
  return {
    name: config.name,
    storageDir: config.storageDir,
    tcpClients: config.tcpClients,
    broadcast: config.broadcast,
    announceIntervalSeconds: config.announceIntervalSeconds,
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

  private async attachListeners(): Promise<void> {
    if (this.attachPromise) {
      return this.attachPromise;
    }

    this.attachPromise = (async () => {
      const register = async (
        eventName: keyof NodeClientEvents,
        map: (raw: Record<string, unknown>) => NodeClientEvents[typeof eventName],
      ) => {
        const handle = await Promise.resolve(
          this.plugin.addListener(eventName, (payload: unknown) => {
            const objectPayload =
              payload && typeof payload === "object"
                ? (payload as Record<string, unknown>)
                : {};
            this.emitter.emit(eventName, map(objectPayload));
          }),
        );
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
      await register("log", toLogEvent);
      await register("error", toErrorEvent);
    })();

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
      usePropagationNode: options?.usePropagationNode,
    });
  }

  async sendLxmf(request: SendLxmfRequest): Promise<string> {
    await this.ready();
    const result = await this.plugin.sendLxmf({
      destinationHex: normalizeHex(request.destinationHex),
      bodyUtf8: request.bodyUtf8,
      title: request.title,
      usePropagationNode: request.usePropagationNode,
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

  async getLxmfSyncStatus(): Promise<SyncStatus> {
    await this.ready();
    return toSyncStatus(await this.plugin.getLxmfSyncStatus());
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
    return this.emitter.on(event, handler);
  }

  async dispose(): Promise<void> {
    for (const handle of this.listenerHandles) {
      await handle.remove().catch(() => undefined);
    }
    this.listenerHandles = [];
    await this.plugin.removeAllListeners?.().catch(() => undefined);
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
          managementState: "Unmanaged",
          availabilityState: "Unseen",
          activeLink: false,
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
        managementState: "Managed",
        availabilityState: "Unseen",
        activeLink: false,
        lastSeenAtMs: Date.now(),
      },
    });
    this.connected.add(normalized);
    this.emitter.emit("peerChanged", {
      change: {
        destinationHex: normalized,
        state: "Connected",
        managementState: "Managed",
        availabilityState: "Ready",
        activeLink: true,
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
        managementState: "Unmanaged",
        availabilityState: "Unseen",
        activeLink: false,
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

  async listPeers(): Promise<PeerRecord[]> {
    return [];
  }

  async listConversations(): Promise<ConversationRecord[]> {
    return [];
  }

  async listMessages(_conversationId?: string): Promise<MessageRecord[]> {
    return [];
  }

  async getLxmfSyncStatus(): Promise<SyncStatus> {
    return {
      phase: "Idle",
      messagesReceived: 0,
    };
  }

  async logMessage(level: LogLevel, message: string): Promise<void> {
    this.emitter.emit("log", { level, message });
  }

  async refreshHubDirectory(): Promise<void> {
    this.emitter.emit("hubDirectoryUpdated", {
      destinations: [],
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

const MOCK_HUB_PEERS = [
  "7eb6e03ed67cd89bb3c5a7ac8713a109",
  "c31298a1c68e30f7f3578fc03230591f",
  "b07fd4a357fdb6b3500f5226346f56fd",
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

  private emitAnnounce(
    destinationHex: string,
    appData: string,
    identityHex = randomHex32(),
    destinationKind: AnnounceDestinationKind = "app",
  ): void {
    this.emitter.emit("announceReceived", {
      destinationHex,
      identityHex,
      destinationKind,
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
        managementState: "Managed",
        availabilityState: "Unseen",
        activeLink: false,
        lastSeenAtMs: Date.now(),
      },
    });
    await new Promise((resolve) => setTimeout(resolve, 200));
    this.connected.add(normalized);
    this.emitter.emit("peerChanged", {
      change: {
        destinationHex: normalized,
        state: "Connected",
        managementState: "Managed",
        availabilityState: "Ready",
        activeLink: true,
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
        managementState: "Unmanaged",
        availabilityState: "Unseen",
        activeLink: false,
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

  async listPeers(): Promise<PeerRecord[]> {
    return [];
  }

  async listConversations(): Promise<ConversationRecord[]> {
    return [];
  }

  async listMessages(_conversationId?: string): Promise<MessageRecord[]> {
    return [];
  }

  async getLxmfSyncStatus(): Promise<SyncStatus> {
    return {
      phase: "Idle",
      messagesReceived: 0,
    };
  }

  async logMessage(level: LogLevel, message: string): Promise<void> {
    this.emitter.emit("log", { level, message });
  }

  async refreshHubDirectory(): Promise<void> {
    this.emitter.emit("hubDirectoryUpdated", {
      destinations: MOCK_HUB_PEERS,
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
    await this.stop();
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
