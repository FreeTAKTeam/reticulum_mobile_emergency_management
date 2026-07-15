import type { ChecklistDeleteOptions, ChecklistRecord, ChecklistTemplateRecord, EamProjectionRecord, EamReadinessSummaryRecord, EamTeamSummaryRecord, EventProjectionRecord, NodeClientEvents, NodeStatus, SosAlertRecord, SosAudioRecord, SosDeviceTelemetryRecord, SosLocationRecord, SosSettingsRecord, SosStatusRecord, SosTriggerSource, TelemetryPositionRecord } from "./contracts";
import { DEFAULT_SOS_SETTINGS, DEFAULT_SOS_STATUS } from "./client-config-converters";
import { cloneChecklistRecord, cloneChecklistTemplateRecord, createDefaultChecklistTemplates, createInMemoryChecklistTemplateFromCsv, type ChecklistCellInput, type ChecklistCreateInput, type ChecklistRowAddInput, type ChecklistRowDeleteInput, type ChecklistRowStyleInput, type ChecklistStatusInput, type ChecklistTemplateCsvInput, type ChecklistUpdateInput } from "./checklist-memory-templates";
import { addInMemoryTaskRow, createInMemoryChecklistFromTemplate, deleteInMemoryTaskRow, emitChecklistInvalidations, findInMemoryChecklist, setInMemoryTaskCell, setInMemoryTaskRowStyle, setInMemoryTaskStatus, updateInMemoryChecklist } from "./checklist-memory-runtime";
import { emptyEamReadinessSummary } from "./projection-converters";
import { TypedEmitter } from "./typed-emitter";

export abstract class InMemoryProjectionClient {
  protected readonly emitter = new TypedEmitter<NodeClientEvents>();
  protected abstract status: NodeStatus;
  protected abstract readonly inMemoryPrefix: string;
  private readonly checklists: ChecklistRecord[] = [];
  private readonly checklistTemplates: ChecklistTemplateRecord[] = createDefaultChecklistTemplates();
  private sosSettings: SosSettingsRecord = { ...DEFAULT_SOS_SETTINGS };
  private sosStatus: SosStatusRecord = { ...DEFAULT_SOS_STATUS };
  private readonly sosAlerts: SosAlertRecord[] = [];
  private readonly sosLocations: SosLocationRecord[] = [];
  private readonly sosAudio: SosAudioRecord[] = [];

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
      reason: `${this.inMemoryPrefix}Settings`,
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
    emitChecklistInvalidations(this.emitter, template.uid, `${this.inMemoryPrefix}ChecklistTemplateImport`);
    return cloneChecklistTemplateRecord(template);
  }

  async createChecklistFromTemplate(input: ChecklistCreateInput): Promise<void> {
    const uid = createInMemoryChecklistFromTemplate(this.checklists, this.checklistTemplates, this.status, input);
    emitChecklistInvalidations(this.emitter, uid, `${this.inMemoryPrefix}ChecklistCreate`);
  }

  async createOnlineChecklist(input: ChecklistCreateInput): Promise<void> {
    await this.createChecklistFromTemplate(input);
  }

  async updateChecklist(input: ChecklistUpdateInput): Promise<void> {
    updateInMemoryChecklist(this.checklists, input, this.status.identityHex);
    emitChecklistInvalidations(this.emitter, input.checklistUid, `${this.inMemoryPrefix}ChecklistUpdate`);
  }

  async deleteChecklist(checklistUid: string, _options: ChecklistDeleteOptions = {}): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    checklist.deletedAt = new Date().toISOString();
    checklist.updatedAt = checklist.deletedAt;
    checklist.lastChangedByTeamMemberRnsIdentity = this.status.identityHex;
    emitChecklistInvalidations(this.emitter, checklistUid, `${this.inMemoryPrefix}ChecklistDelete`);
  }

  async joinChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    if (this.status.identityHex && !checklist.participantRnsIdentities.includes(this.status.identityHex)) {
      checklist.participantRnsIdentities.push(this.status.identityHex);
    }
    checklist.updatedAt = new Date().toISOString();
    checklist.lastChangedByTeamMemberRnsIdentity = this.status.identityHex;
    emitChecklistInvalidations(this.emitter, checklistUid, `${this.inMemoryPrefix}ChecklistJoin`);
  }

  async uploadChecklist(checklistUid: string): Promise<void> {
    const checklist = findInMemoryChecklist(this.checklists, checklistUid);
    checklist.uploadedAt = new Date().toISOString();
    checklist.syncState = "SYNCED";
    emitChecklistInvalidations(this.emitter, checklistUid, `${this.inMemoryPrefix}ChecklistUpload`);
  }

  async setChecklistTaskStatus(input: ChecklistStatusInput): Promise<void> {
    setInMemoryTaskStatus(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, `${this.inMemoryPrefix}ChecklistTaskStatus`);
  }

  async addChecklistTaskRow(input: ChecklistRowAddInput): Promise<void> {
    addInMemoryTaskRow(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, `${this.inMemoryPrefix}ChecklistTaskAdd`);
  }

  async deleteChecklistTaskRow(input: ChecklistRowDeleteInput): Promise<void> {
    deleteInMemoryTaskRow(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, `${this.inMemoryPrefix}ChecklistTaskDelete`);
  }

  async setChecklistTaskRowStyle(input: ChecklistRowStyleInput): Promise<void> {
    setInMemoryTaskRowStyle(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, `${this.inMemoryPrefix}ChecklistTaskStyle`);
  }

  async setChecklistTaskCell(input: ChecklistCellInput): Promise<void> {
    setInMemoryTaskCell(this.checklists, input);
    emitChecklistInvalidations(this.emitter, input.checklistUid, `${this.inMemoryPrefix}ChecklistTaskCell`);
  }
  async setSosPin(_pin?: string): Promise<void> {}
  async getSosStatus(): Promise<SosStatusRecord> { return { ...this.sosStatus }; }
  async triggerSos(source: SosTriggerSource = "Manual"): Promise<SosStatusRecord> {
    const now = Date.now();
    this.sosStatus = {
      state: "Active",
      incidentId: `${this.inMemoryPrefix}-${now}`,
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

  on<K extends keyof NodeClientEvents>(event: K, handler: (payload: NodeClientEvents[K]) => void): () => void {
    return this.emitter.on(event, handler);
  }
}

