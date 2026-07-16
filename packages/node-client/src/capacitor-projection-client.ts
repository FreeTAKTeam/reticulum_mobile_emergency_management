import type { AppSettingsRecord, ChecklistDeleteOptions, ChecklistRecord, ChecklistTemplateRecord, ChecklistUserTaskStatus, ConversationRecord, EamProjectionRecord, EamReadinessSummaryRecord, EamTeamSummaryRecord, EventProjectionRecord, InstalledPluginRecord, LegacyImportPayload, MessageRecord, OperationalSummary, PeerRecord, PluginCapabilityRecord, PluginSensorRecord, SavedPeerRecord, SosAlertRecord, SosAudioRecord, SosDeviceTelemetryRecord, SosLocationRecord, SosSettingsRecord, SosStatusRecord, SosTriggerSource, SyncStatus, TelemetryPositionRecord } from "./contracts";
import { type ReticulumNodePlugin } from "./capacitor-plugin";
import { sosAudioToPlugin, sosSettingsToPlugin, toOperationalSummary, toSosAlertRecord, toSosAudioRecord, toSosLocationRecord, toSosSettingsRecord, toSosStatusRecord } from "./client-config-converters";
import { toChecklistRecord, toChecklistTemplateRecord } from "./checklist-converters";
import { toAppSettingsRecord } from "./converters";
import { toConversationRecord, toMessageRecord, toPeerRecord, toSyncStatus } from "./message-converters";
import { eamProjectionRecordToPlugin, eventProjectionRecordToPlugin, legacyImportPayloadToPlugin, toEamProjectionRecord, toEamReadinessSummaryRecord, toEamTeamSummaryRecord, toEventProjectionRecord, toSavedPeerRecord, toTelemetryPositionRecord } from "./projection-converters";
import { normalizeHex, pluginRecord, toInstalledPlugin, toPluginSensor } from "./runtime-converters";

export abstract class CapacitorProjectionClient {
  protected abstract get plugin(): ReticulumNodePlugin;
  protected abstract ready(): Promise<void>;

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
}
