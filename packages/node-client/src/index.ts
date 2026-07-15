import { Capacitor, registerPlugin } from "@capacitor/core";

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
  WatchStatusServerSettings,
  WatchStatusServerState,
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
import { toApplicationAckState, toConversationRecord, toErrorEvent, toHubDirectoryUpdatedEvent, toLogEvent, toLxmfDeliveryEvent, toLxmfDeliveryMethod, toLxmfDeliveryRepresentation, toLxmfDeliveryStatus, toLxmfFallbackStage, toMessageDirection, toMessageMethod, toMessageRecord, toMessageState, toOperationalNoticeEvent, toPacketReceivedEvent, toPacketSentEvent, toPeerRecord, toProjectionInvalidationEvent, toSyncPhase, toSyncStatus, toTransportDeliveryState } from "./message-converters";
import { eamProjectionRecordToPlugin, emptyEamReadinessSummary, eventProjectionRecordToPlugin, legacyImportPayloadToPlugin, toEamProjectionRecord, toEamReadinessMessageRecord, toEamReadinessStatusMetricRecord, toEamReadinessSummaryRecord, toEamTeamSummaryRecord, toEventProjectionRecord, toSavedPeerRecord, toTelemetryPositionRecord } from "./projection-converters";
import { toChecklistCellRecord, toChecklistColumnRecord, toChecklistFeedPublicationRecord, toChecklistRecord, toChecklistTaskRecord, toChecklistTemplateRecord } from "./checklist-converters";

export * from "./contracts";
export { normalizeRnodeSettings, parseRnodeConnectionMode } from "./converters";

const GREEK_CALLSIGN_PREFIXES = [
  "Alpha",
  "Beta",
  "Gamma",
  "Delta",
  "Epsilon",
  "Zeta",
  "Eta",
  "Theta",
  "Iota",
  "Kappa",
  "Lambda",
  "Mu",
  "Nu",
  "Xi",
  "Omicron",
  "Pi",
  "Rho",
  "Sigma",
  "Tau",
  "Upsilon",
  "Phi",
  "Chi",
  "Psi",
  "Omega",
] as const;

export function generateDefaultCallSign(): string {
  const prefix = GREEK_CALLSIGN_PREFIXES[Math.floor(Math.random() * GREEK_CALLSIGN_PREFIXES.length)];
  const suffix = String(Math.floor(Math.random() * 999) + 1).padStart(3, "0");
  return `${prefix}${suffix}`;
}

export const DEFAULT_NODE_CONFIG: NodeConfig = {
  name: generateDefaultCallSign(),
  tcpClients: [],
  broadcast: true,
  transportNodeEnabled: true,
  announceIntervalSeconds: 1800,
  staleAfterMinutes: 30,
  announceCapabilities: "R3AKT,EMergencyMessages",
  hubMode: "Autonomous",
  hubRefreshIntervalSeconds: 3600,
  rnode: {
    enabled: false,
    connectionMode: "ble",
    peripheralId: "",
    displayName: "",
    region: "US915",
    profile: "REM-LF-RURAL-v1",
  },
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
  checkRnodeBluetoothPermissions(): Promise<Record<string, unknown>>;
  requestRnodeBluetoothPermissions(): Promise<Record<string, unknown>>;
  listPairedRnodeBluetoothDevices(): Promise<{ items?: RnodeBleDeviceRecord[] }>;
  scanRnodeBleDevices(options?: { timeoutMs?: number }): Promise<{ items?: RnodeBleDeviceRecord[] }>;
  pairRnodeBleDevice(options: { id: string }): Promise<Record<string, unknown>>;
  listRnodeUsbDevices(): Promise<{ items?: RnodeUsbDeviceRecord[] }>;
  requestRnodeUsbPermission(options: { deviceId: number }): Promise<Record<string, unknown>>;
  startRnodeUsbBluetoothPairing(options: { deviceId: number; bluetoothDeviceId?: string }): Promise<Record<string, unknown>>;
  cancelRnodeUsbBluetoothPairing(options?: { deviceId?: number }): Promise<void>;
  connectPeer(options: { destinationHex: string }): Promise<void>;
  disconnectPeer(options: { destinationHex: string }): Promise<void>;
  announceNow(): Promise<void>;
  requestPeerIdentity(options: { destinationHex: string }): Promise<void>;
  send(options: {
    destinationHex: string;
    bytesBase64: string;
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
    fieldsBase64?: string;
  }): Promise<void>;
  setActivePropagationNode(options: { destinationHex?: string }): Promise<void>;
  requestLxmfSync(options: { limit?: number }): Promise<void>;
  listAnnounces(): Promise<{ items: Record<string, unknown>[] }>;
  refreshPlugins(): Promise<{ items: Record<string, unknown>[] }>;
  listPlugins(): Promise<{ items: Record<string, unknown>[] }>;
  approvePluginPublisher(options: { pluginId: string; displayName?: string }): Promise<void>;
  revokePluginPublisher(options: { fingerprint: string }): Promise<void>;
  setPluginEnabled(options: { pluginId: string; enabled: boolean }): Promise<void>;
  grantPluginCapabilities(options: {
    pluginId: string;
    capabilities: PluginCapabilityRecord;
  }): Promise<void>;
  openPluginConfiguration(options: { pluginId: string }): Promise<void>;
  listPluginSensors(): Promise<{ items: Record<string, unknown>[] }>;
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
  getWatchStatusServerSettings(): Promise<Record<string, unknown>>;
  setWatchStatusServerSettings(options: { enabled: boolean; port: number }): Promise<void>;
  getWatchStatusServerState(): Promise<Record<string, unknown>>;
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
  deleteChecklist(options: { checklistUid: string; deleteRemote?: boolean }): Promise<void>;
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
  deleteLocalEam(options: { callsign: string; deletedAtMs?: number }): Promise<void>;
  getEamTeamSummary(options: { teamUid: string }): Promise<Record<string, unknown>>;
  getEamReadinessSummary(): Promise<Record<string, unknown>>;
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
  recordSosAudio(options: Record<string, unknown>): Promise<void>;
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

function sosAudioToPlugin(audio: SosAudioRecord): Record<string, unknown> {
  return {
    audioId: audio.audioId,
    incidentId: audio.incidentId,
    sourceHex: audio.sourceHex,
    path: audio.path,
    mimeType: audio.mimeType,
    durationSeconds: audio.durationSeconds,
    createdAtMs: audio.createdAtMs,
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

function normalizeWatchStatusPort(value: unknown): number {
  const parsed = Number(value ?? 29_863);
  return Number.isInteger(parsed) && parsed >= 1_024 && parsed <= 65_535
    ? parsed
    : 29_863;
}

function toWatchStatusServerState(raw: Record<string, unknown> = {}): WatchStatusServerState {
  const enabled = raw.enabled === undefined ? true : Boolean(raw.enabled);
  const port = normalizeWatchStatusPort(raw.port);
  const url = String(
    raw.url
      ?? raw.currentUrl
      ?? raw.current_url
      ?? `http://localhost:${port}/info.json`,
  );

  return {
    enabled,
    port,
    url,
    currentUrl: String(raw.currentUrl ?? raw.current_url ?? url),
    running: Boolean(raw.running),
    bindError: String(raw.bindError ?? raw.bind_error ?? ""),
  };
}

function configToPlugin(config: NodeConfig): Record<string, unknown> {
  return {
    name: config.name,
    storageDir: config.storageDir,
    tcpClients: config.tcpClients,
    broadcast: config.broadcast,
    transportNodeEnabled: config.transportNodeEnabled,
    announceIntervalSeconds: config.announceIntervalSeconds,
    staleAfterMinutes: config.staleAfterMinutes,
    announceCapabilities: config.announceCapabilities,
    hubMode: config.hubMode,
    hubIdentityHash: config.hubIdentityHash,
    hubApiBaseUrl: config.hubApiBaseUrl,
    hubApiKey: config.hubApiKey,
    hubRefreshIntervalSeconds: config.hubRefreshIntervalSeconds,
    rnode: config.rnode,
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

  async refreshPlugins(): Promise<InstalledPluginRecord[]> {
    await this.ready();
    const result = await this.plugin.refreshPlugins();
    return Array.isArray(result.items) ? result.items.map(toInstalledPlugin) : [];
  }

  async listPlugins(): Promise<InstalledPluginRecord[]> {
    await this.ready();
    const result = await this.plugin.listPlugins();
    return Array.isArray(result.items) ? result.items.map(toInstalledPlugin) : [];
  }

  async approvePluginPublisher(pluginId: string, displayName?: string): Promise<void> {
    await this.ready();
    await this.plugin.approvePluginPublisher({ pluginId, displayName });
  }

  async revokePluginPublisher(fingerprint: string): Promise<void> {
    await this.ready();
    await this.plugin.revokePluginPublisher({ fingerprint });
  }

  async setPluginEnabled(pluginId: string, enabled: boolean): Promise<void> {
    await this.ready();
    await this.plugin.setPluginEnabled({ pluginId, enabled });
  }

  async grantPluginCapabilities(
    pluginId: string,
    capabilities: PluginCapabilityRecord,
  ): Promise<void> {
    await this.ready();
    await this.plugin.grantPluginCapabilities({ pluginId, capabilities });
  }

  async openPluginConfiguration(pluginId: string): Promise<void> {
    await this.ready();
    await this.plugin.openPluginConfiguration({ pluginId });
  }

  async listPluginSensors(): Promise<PluginSensorRecord[]> {
    await this.ready();
    const result = await this.plugin.listPluginSensors();
    return Array.isArray(result.items) ? result.items.map(toPluginSensor) : [];
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

  async getWatchStatusServerSettings(): Promise<WatchStatusServerState> {
    await this.ready();
    return toWatchStatusServerState(await this.plugin.getWatchStatusServerSettings());
  }

  async setWatchStatusServerSettings(settings: WatchStatusServerSettings): Promise<void> {
    await this.ready();
    await this.plugin.setWatchStatusServerSettings({
      enabled: Boolean(settings.enabled),
      port: normalizeWatchStatusPort(settings.port),
    });
  }

  async getWatchStatusServerState(): Promise<WatchStatusServerState> {
    await this.ready();
    return toWatchStatusServerState(await this.plugin.getWatchStatusServerState());
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

  async deleteChecklist(checklistUid: string, options: ChecklistDeleteOptions = {}): Promise<void> {
    await this.ready();
    await this.plugin.deleteChecklist({
      checklistUid,
      deleteRemote: options.deleteRemote ?? false,
    });
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

  async deleteLocalEam(callsign: string, deletedAtMs?: number): Promise<void> {
    await this.ready();
    await this.plugin.deleteLocalEam({ callsign, deletedAtMs });
  }

  async getEamTeamSummary(teamUid: string): Promise<EamTeamSummaryRecord | null> {
    await this.ready();
    return toEamTeamSummaryRecord(await this.plugin.getEamTeamSummary({ teamUid }));
  }

  async getEamReadinessSummary(): Promise<EamReadinessSummaryRecord> {
    await this.ready();
    return toEamReadinessSummaryRecord(await this.plugin.getEamReadinessSummary());
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

  async recordSosAudio(audio: SosAudioRecord): Promise<void> {
    await this.ready();
    await this.plugin.recordSosAudio(sosAudioToPlugin(audio));
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

function browserRuntimeReadiness(running: boolean): RuntimeReadinessSnapshot {
  return {
    state: running ? "Ready" : "Pending",
    interfaces: [
      {
        id: "local",
        label: "Reticulum Net",
        state: running ? "Ready" : "Pending",
        detail: running ? "Browser runtime is ready" : "Browser runtime is starting",
      },
    ],
  };
}

class WebReticulumNodeClient implements ReticulumNodeClient {
  private readonly emitter = new TypedEmitter<NodeClientEvents>();
  private status: NodeStatus = (() => {
    const lxmfDestinationHex = randomHex32();
    return {
      running: false,
      name: "",
      identityHex: randomHex32(),
      appDestinationHex: lxmfDestinationHex,
      lxmfDestinationHex,
      readiness: browserRuntimeReadiness(false),
      interfaces: [],
    };
  })();
  private capabilities = DEFAULT_NODE_CONFIG.announceCapabilities;
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

  private emitLocalAnnounce(): void {
    this.emitter.emit("announceReceived", {
      destinationHex: this.status.lxmfDestinationHex,
      identityHex: this.status.identityHex,
      destinationKind: "lxmf_delivery",
      announceClass: "LxmfDelivery",
      appData: this.capabilities,
      displayName: this.status.name,
      hops: 1,
      interfaceHex: randomHex32(),
      receivedAtMs: Date.now(),
    });
  }

  async start(config: NodeConfig): Promise<void> {
    this.status = {
      ...this.status,
      running: true,
      name: config.name,
      readiness: browserRuntimeReadiness(true),
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
          saved: this.savedPeers.has(destinationHex),
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
      readiness: browserRuntimeReadiness(false),
    };
    this.emitter.emit("statusChanged", { status: { ...this.status } });
  }

  async restart(config: NodeConfig): Promise<void> {
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
    this.emitLocalAnnounce();
  }

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
      requestedDestinationHex: destinationHex,
      deliveryDestinationHex: destinationHex,
      lastWireMessageIdHex: messageIdHex,
      title: request.title,
      bodyUtf8: request.bodyUtf8,
      method: "Direct",
      state: this.connected.has(destinationHex) ? "Delivered" : "Failed",
      transportState: this.connected.has(destinationHex) ? "TransportDelivered" : "Failed",
      applicationAckState: this.connected.has(destinationHex) ? "Accepted" : "Failed",
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

  async setAnnounceCapabilities(capabilityString: string): Promise<void> {
    this.capabilities = capabilityString;
    this.emitLocalAnnounce();
  }

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
  async getWatchStatusServerSettings(): Promise<WatchStatusServerState> { return toWatchStatusServerState(); }
  async setWatchStatusServerSettings(_settings: WatchStatusServerSettings): Promise<void> {}
  async getWatchStatusServerState(): Promise<WatchStatusServerState> { return toWatchStatusServerState(); }
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
  async getEams(): Promise<EamProjectionRecord[]> { return []; }
  async upsertEam(_eam: EamProjectionRecord): Promise<void> {}
  async deleteEam(_callsign: string, _deletedAtMs?: number): Promise<void> {}
  async deleteLocalEam(_callsign: string, _deletedAtMs?: number): Promise<void> {}
  async getEamTeamSummary(_teamUid: string): Promise<EamTeamSummaryRecord | null> { return null; }
  async getEamReadinessSummary(): Promise<EamReadinessSummaryRecord> { return emptyEamReadinessSummary(); }
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

  async deleteChecklist(checklistUid: string, _options: ChecklistDeleteOptions = {}): Promise<void> {
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
  async recordSosAudio(audio: SosAudioRecord): Promise<void> {
    const index = this.sosAudio.findIndex((candidate) => candidate.audioId === audio.audioId);
    if (index >= 0) {
      this.sosAudio[index] = { ...audio };
      return;
    }
    this.sosAudio.unshift({ ...audio });
  }

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

function countConnectedSavedPeers(
  connected: Set<string>,
  savedPeers: Map<string, SavedPeerRecord>,
): number {
  return [...connected].filter((destination) => savedPeers.has(destination)).length;
}

class MockReticulumNodeClient implements ReticulumNodeClient {
  private readonly emitter = new TypedEmitter<NodeClientEvents>();
  private status: NodeStatus = (() => {
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
  async getWatchStatusServerSettings(): Promise<WatchStatusServerState> { return toWatchStatusServerState(); }
  async setWatchStatusServerSettings(_settings: WatchStatusServerSettings): Promise<void> {}
  async getWatchStatusServerState(): Promise<WatchStatusServerState> { return toWatchStatusServerState(); }
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
  async getEams(): Promise<EamProjectionRecord[]> { return []; }
  async upsertEam(_eam: EamProjectionRecord): Promise<void> {}
  async deleteEam(_callsign: string, _deletedAtMs?: number): Promise<void> {}
  async deleteLocalEam(_callsign: string, _deletedAtMs?: number): Promise<void> {}
  async getEamTeamSummary(_teamUid: string): Promise<EamTeamSummaryRecord | null> { return null; }
  async getEamReadinessSummary(): Promise<EamReadinessSummaryRecord> { return emptyEamReadinessSummary(); }
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

  async deleteChecklist(checklistUid: string, _options: ChecklistDeleteOptions = {}): Promise<void> {
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
  async recordSosAudio(audio: SosAudioRecord): Promise<void> {
    const index = this.sosAudio.findIndex((candidate) => candidate.audioId === audio.audioId);
    if (index >= 0) {
      this.sosAudio[index] = { ...audio };
      return;
    }
    this.sosAudio.unshift({ ...audio });
  }

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
