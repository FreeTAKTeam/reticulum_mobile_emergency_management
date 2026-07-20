import { Capacitor } from "@capacitor/core";

import type {
  LogLevel,
  RuntimeReadinessState,
  PeerState,
  AnnounceDestinationKind,
  AnnounceClass,
  SendOutcome,
  LxmfDeliveryStatus,
  TransportDeliveryState,
  ApplicationAckState,
  SendMode,
  LxmfDeliveryMethod,
  LxmfDeliveryRepresentation,
  LxmfFallbackStage,
  MessageMethod,
  MessageState,
  MessageDirection,
  ClientMode,
  ProjectionScope,
  InstalledPluginRecord,
  PluginCapabilityRecord,
  PluginSensorRecord,
  SosState,
  SosTriggerSource,
  SosMessageKind,
  RnodeBleDeviceRecord,
  RnodeBlePairResult,
  RnodeUsbDeviceRecord,
  RnodeUsbPairResult,
  NodeConfig,
  NodeStatus,
  RuntimeInterfaceReadinessRecord,
  RuntimeReadinessSnapshot,
  InterfaceStatusRecord,
  PeerChange,
  StatusChangedEvent,
  InterfaceStatusChangedEvent,
  AnnounceReceivedEvent,
  AnnounceRecord,
  PeerChangedEvent,
  PacketReceivedEvent,
  PacketSendOptions,
  PacketSentEvent,
  LxmfDeliveryEvent,
  MessageRecord,
  PeerRecord,
  ConversationRecord,
  SyncPhase,
  SyncStatus,
  SendLxmfRequest,
  HubSettingsRecord,
  ChecklistMode,
  ChecklistSyncState,
  ChecklistOriginType,
  ChecklistTaskStatus,
  ChecklistUserTaskStatus,
  ChecklistColumnType,
  ChecklistStatusCounts,
  ChecklistColumnRecord,
  ChecklistCellRecord,
  ChecklistTaskRecord,
  ChecklistFeedPublicationRecord,
  ChecklistRecord,
  ChecklistTemplateRecord,
  HubDirectoryPeerRecord,
  TelemetrySettingsRecord,
  ChecklistSettingsRecord,
  AppSettingsRecord,
  SavedPeerRecord,
  EamSourceRecord,
  EamProjectionRecord,
  EamTeamSummaryRecord,
  EamReadinessStatusMetricRecord,
  EamReadinessMessageRecord,
  EamReadinessSummaryRecord,
  EventProjectionRecord,
  TelemetryPositionRecord,
  SosSettingsRecord,
  SosDeviceTelemetryRecord,
  SosStatusRecord,
  SosAlertRecord,
  SosLocationRecord,
  SosAudioRecord,
  LegacyImportPayload,
  ProjectionInvalidationEvent,
  OperationalSummary,
  HubDirectoryUpdatedEvent,
  NodeLogEvent,
  NodeOperationalNoticeEvent,
  NodeErrorEvent,
  NodeClientEvents,
  ChecklistDeleteOptions,
  ReticulumNodeClient,
  ReticulumNodeClientFactoryOptions,
} from "./contracts";
import {
  normalizeRnodeSettings,
  parseRnodeConnectionMode,
  toAppSettingsRecord,
  toOptionalNumber,
} from "./converters";
import { decodeBase64ToBytes, encodeBytesToBase64, enumVariantName, hasValue, normalizeHex, pluginRecord, toAnnounceReceivedEvent, toAnnounceRecord, toInstalledPlugin, toInterfaceStatusChangedEvent, toInterfaceStatusRecord, toNodeStatus, toOptionalBoolean, toOptionalHex, toPeerChangedEvent, toPeerState, toPluginCapabilities, toPluginSensor, toRuntimeReadinessSnapshot, toRuntimeReadinessState, toSavedFlag, toSendOutcome, toStatusChangedEvent } from "./runtime-converters";
import { TypedEmitter } from "./typed-emitter";
import { toApplicationAckState, toConversationRecord, toErrorEvent, toHubDirectoryUpdatedEvent, toLogEvent, toLxmfDeliveryEvent, toLxmfDeliveryMethod, toLxmfDeliveryRepresentation, toLxmfDeliveryStatus, toLxmfFallbackStage, toMessageDirection, toMessageMethod, toMessageRecord, toMessageState, toOperationalNoticeEvent, toPacketReceivedEvent, toPacketSentEvent, toPeerRecord, toProjectionInvalidationEvent, toSyncPhase, toSyncStatus, toTransportDeliveryState } from "./message-converters";
import { eamProjectionRecordToPlugin, emptyEamReadinessSummary, eventProjectionRecordToPlugin, legacyImportPayloadToPlugin, toEamProjectionRecord, toEamReadinessMessageRecord, toEamReadinessStatusMetricRecord, toEamReadinessSummaryRecord, toEamTeamSummaryRecord, toEventProjectionRecord, toSavedPeerRecord, toTelemetryPositionRecord } from "./projection-converters";
import { toChecklistCellRecord, toChecklistColumnRecord, toChecklistFeedPublicationRecord, toChecklistRecord, toChecklistTaskRecord, toChecklistTemplateRecord } from "./checklist-converters";
import { ReticulumNodePluginInstance, type PluginListenerHandle } from "./capacitor-plugin";
import { CapacitorReticulumNodeClient } from "./capacitor-client";
import { WebReticulumNodeClient } from "./web-client";
import { MockReticulumNodeClient } from "./mock-client";
import { DEFAULT_SOS_SETTINGS, DEFAULT_SOS_STATUS, configToPlugin, sosAudioToPlugin, sosSettingsToPlugin, toOperationalSummary, toSosAlertRecord, toSosAudioRecord, toSosLocationRecord, toSosMessageKind, toSosSettingsRecord, toSosState, toSosStatusRecord, toSosTriggerSource } from "./client-config-converters";
import { DEFAULT_NODE_CONFIG, browserRuntimeReadiness, countConnectedSavedPeers, generateDefaultCallSign, randomHex32 } from "./client-defaults";
import { cloneChecklistRecord, cloneChecklistTemplateRecord, createDefaultChecklistTemplates, createInMemoryChecklistTemplateFromCsv, defaultChecklistColumns, defaultChecklistTask, type ChecklistCellInput, type ChecklistCreateInput, type ChecklistRowAddInput, type ChecklistRowDeleteInput, type ChecklistRowStyleInput, type ChecklistStatusInput, type ChecklistTemplateCsvInput, type ChecklistUpdateInput } from "./checklist-memory-templates";
import { addInMemoryTaskRow, createInMemoryChecklistFromTemplate, deleteInMemoryTaskRow, emitChecklistInvalidations, findInMemoryChecklist, normalizeInMemoryChecklist, setInMemoryTaskCell, setInMemoryTaskRowStyle, setInMemoryTaskStatus, updateInMemoryChecklist } from "./checklist-memory-runtime";

export * from "./contracts";
export {
  NODE_ERROR_CODES,
  ReticulumNodeError,
  classifyNodeError,
  type NodeErrorCode,
  type NodeErrorDetails,
} from "./errors";
export { normalizeRnodeSettings, parseRnodeConnectionMode } from "./converters";
export { DEFAULT_SOS_SETTINGS, DEFAULT_SOS_STATUS } from "./client-config-converters";
export { DEFAULT_NODE_CONFIG, generateDefaultCallSign } from "./client-defaults";
















export function createReticulumNodeClient(
  options: ReticulumNodeClientFactoryOptions = {},
): ReticulumNodeClient {
  const mode = options.mode ?? "auto";
  if (mode === "web") {
    return new WebReticulumNodeClient();
  }
  if (mode === "mock") {
    return new MockReticulumNodeClient();
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
