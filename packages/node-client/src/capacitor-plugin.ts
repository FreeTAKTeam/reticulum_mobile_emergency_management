import { registerPlugin } from "@capacitor/core";
import type { ChecklistUserTaskStatus, LogLevel, PluginCapabilityRecord, RnodeBleDeviceRecord, RnodeUsbDeviceRecord, SendMode, SosTriggerSource } from "./contracts";

export type PluginListenerHandle = {
  remove: () => Promise<void>;
};

export interface ReticulumNodePlugin {
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
  getHubDirectorySnapshot(): Promise<Record<string, unknown>>;
  setActiveTeam(options: { teamUid: string }): Promise<void>;
  addListener(
    eventName: string,
    listener: (event: unknown) => void,
  ): PluginListenerHandle | Promise<PluginListenerHandle>;
  removeAllListeners?(): Promise<void>;
}

export const ReticulumNodePluginInstance = registerPlugin<ReticulumNodePlugin>(
  "ReticulumNode",
);
