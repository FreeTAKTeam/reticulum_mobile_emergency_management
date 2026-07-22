import { type ApplicationAckState, type ConversationRecord, type LogLevel, type LxmfDeliveryEvent, type LxmfDeliveryMethod, type LxmfDeliveryRepresentation, type LxmfDeliveryStatus, type LxmfFallbackStage, type MessageDirection, type MessageMethod, type MessageRecord, type MessageState, type NodeErrorEvent, type NodeLogEvent, type NodeOperationalNoticeEvent, type PacketReceivedEvent, type PacketSentEvent, type PeerRecord, type ProjectionInvalidationEvent, type ProjectionScope, type SyncPhase, type SyncStatus, type TransportDeliveryState } from "./contracts";
import { toOptionalNumber } from "./converters";
import { decodeBase64ToBytes, enumVariantName, hasValue, normalizeHex, pluginRecord, toOptionalBoolean, toOptionalHex, toPeerState, toSavedFlag, toSendOutcome } from "./runtime-converters";

export function toPeerRecord(raw: Record<string, unknown>): PeerRecord {
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


export function toPacketReceivedEvent(
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
    fieldsBase64:
      typeof raw.fieldsBase64 === "string"
        ? raw.fieldsBase64
        : typeof raw.fields_base64 === "string"
          ? raw.fields_base64
          : undefined,
  };
}

export function toPacketSentEvent(raw: Record<string, unknown>): PacketSentEvent {
  const encoded = String(raw.bytesBase64 ?? raw.bytes_base64 ?? "");
  return {
    destinationHex: normalizeHex(
      String(raw.destinationHex ?? raw.destination_hex ?? ""),
    ),
    bytes: encoded ? decodeBase64ToBytes(encoded) : new Uint8Array(0),
    outcome: toSendOutcome(raw.outcome),
  };
}

export function toLxmfDeliveryStatus(raw: unknown): LxmfDeliveryStatus {
  const value = String(raw ?? "");
  const valid: LxmfDeliveryStatus[] = [
    "Sent",
    "SentToPropagation",
    "Delivered",
    "Acknowledged",
    "Failed",
    "TimedOut",
  ];
  return valid.includes(value as LxmfDeliveryStatus)
    ? (value as LxmfDeliveryStatus)
    : "Failed";
}

export function toTransportDeliveryState(raw: unknown): TransportDeliveryState {
  const value = String(raw ?? "");
  const valid: TransportDeliveryState[] = [
    "Queued",
    "Sending",
    "SentDirect",
    "SentToPropagation",
    "TransportDelivered",
    "Failed",
    "TimedOut",
    "Cancelled",
  ];
  return valid.includes(value as TransportDeliveryState)
    ? (value as TransportDeliveryState)
    : "Queued";
}

export function toApplicationAckState(raw: unknown): ApplicationAckState {
  const value = String(raw ?? "");
  const valid: ApplicationAckState[] = [
    "NotRequired",
    "Waiting",
    "Accepted",
    "Completed",
    "Rejected",
    "Failed",
  ];
  return valid.includes(value as ApplicationAckState)
    ? (value as ApplicationAckState)
    : "NotRequired";
}

export function toLxmfDeliveryMethod(raw: unknown): LxmfDeliveryMethod {
  const value = String(raw ?? "");
  const valid: LxmfDeliveryMethod[] = ["Direct", "Opportunistic", "Propagated"];
  return valid.includes(value as LxmfDeliveryMethod)
    ? (value as LxmfDeliveryMethod)
    : "Direct";
}

export function toLxmfDeliveryRepresentation(raw: unknown): LxmfDeliveryRepresentation {
  return String(raw ?? "") === "Resource" ? "Resource" : "Packet";
}

export function toLxmfFallbackStage(raw: unknown): LxmfFallbackStage | undefined {
  return String(raw ?? "") === "AfterDirectRetryBudget"
    ? "AfterDirectRetryBudget"
    : undefined;
}

export function toLxmfDeliveryEvent(raw: Record<string, unknown>): LxmfDeliveryEvent {
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
    transportState: toTransportDeliveryState(
      hasValue(raw.transportState) ? raw.transportState : raw.transport_state,
    ),
    applicationAckState: toApplicationAckState(
      hasValue(raw.applicationAckState) ? raw.applicationAckState : raw.application_ack_state,
    ),
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

export function toMessageMethod(raw: unknown): MessageMethod {
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

export function toMessageState(raw: unknown): MessageState {
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

export function toMessageDirection(raw: unknown, record?: Record<string, unknown>): MessageDirection {
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

export function toMessageRecord(raw: Record<string, unknown>): MessageRecord {
  return {
    messageIdHex: normalizeHex(String(raw.messageIdHex ?? raw.message_id_hex ?? "")),
    conversationId: String(raw.conversationId ?? raw.conversation_id ?? ""),
    direction: toMessageDirection(raw.direction, raw),
    destinationHex: normalizeHex(String(raw.destinationHex ?? raw.destination_hex ?? "")),
    sourceHex:
      raw.sourceHex !== undefined || raw.source_hex !== undefined
        ? normalizeHex(String(raw.sourceHex ?? raw.source_hex ?? ""))
        : undefined,
    requestedDestinationHex: toOptionalHex(
      raw.requestedDestinationHex ?? raw.requested_destination_hex,
    ),
    deliveryDestinationHex: toOptionalHex(
      raw.deliveryDestinationHex ?? raw.delivery_destination_hex,
    ),
    recipientIdentityHex: toOptionalHex(
      raw.recipientIdentityHex ?? raw.recipient_identity_hex,
    ),
    lastWireMessageIdHex: toOptionalHex(
      raw.lastWireMessageIdHex ?? raw.last_wire_message_id_hex,
    ),
    title:
      typeof raw.title === "string"
        ? raw.title
        : undefined,
    bodyUtf8: String(raw.bodyUtf8 ?? raw.body_utf8 ?? ""),
    method: toMessageMethod(raw.method),
    state: toMessageState(raw.state),
    transportState: toTransportDeliveryState(raw.transportState ?? raw.transport_state),
    applicationAckState: toApplicationAckState(raw.applicationAckState ?? raw.application_ack_state),
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

export function toConversationRecord(raw: Record<string, unknown>): ConversationRecord {
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

export function toSyncPhase(raw: unknown): SyncPhase {
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

export function toSyncStatus(raw: Record<string, unknown>): SyncStatus {
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

export { toHubDirectoryUpdatedEvent } from "./hub-directory-converter";

export function toLogEvent(raw: Record<string, unknown>): NodeLogEvent {
  return {
    level: (String(raw.level ?? "Info") as LogLevel) ?? "Info",
    message: String(raw.message ?? ""),
  };
}

export function toOperationalNoticeEvent(
  raw: Record<string, unknown>,
): NodeOperationalNoticeEvent {
  return {
    level: (String(raw.level ?? "Info") as LogLevel) ?? "Info",
    message: String(raw.message ?? ""),
    atMs: Number(raw.atMs ?? raw.at_ms ?? Date.now()),
  };
}

export function toErrorEvent(raw: Record<string, unknown>): NodeErrorEvent {
  return {
    code: String(raw.code ?? "UNKNOWN"),
    message: String(raw.message ?? "Unknown plugin error"),
  };
}

export function toProjectionInvalidationEvent(raw: Record<string, unknown>): ProjectionInvalidationEvent {
  return {
    scope: String(raw.scope ?? "Peers") as ProjectionScope,
    key: typeof raw.key === "string" ? raw.key : undefined,
    revision: Number(raw.revision ?? 0),
    updatedAtMs: Number(raw.updatedAtMs ?? raw.updated_at_ms ?? Date.now()),
    reason: typeof raw.reason === "string" ? raw.reason : undefined,
  };
}
