import type { ChecklistRecord, ChecklistTaskStatus, ChecklistTemplateRecord, ChecklistUserTaskStatus, NodeClientEvents, NodeStatus } from "./contracts";
import { cloneChecklistRecord, cloneChecklistTemplateRecord, defaultChecklistTask, type ChecklistCellInput, type ChecklistCreateInput, type ChecklistRowAddInput, type ChecklistRowDeleteInput, type ChecklistRowStyleInput, type ChecklistStatusInput, type ChecklistUpdateInput } from "./checklist-memory-templates";
import { TypedEmitter } from "./typed-emitter";

export function formatRfc3339FromEpochMs(epochMs: number): string | undefined {
  if (!Number.isFinite(epochMs)) {
    return undefined;
  }
  return new Date(epochMs).toISOString().replace(".000Z", "Z");
}

export function checklistTaskStatusFor(userStatus: ChecklistUserTaskStatus, isLate: boolean): ChecklistTaskStatus {
  if (userStatus === "COMPLETE") {
    return isLate ? "COMPLETE_LATE" : "COMPLETE";
  }
  return isLate ? "LATE" : "PENDING";
}

export function normalizeInMemoryChecklist(record: ChecklistRecord): void {
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

export function emitChecklistInvalidations(
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

export function findInMemoryChecklist(checklists: ChecklistRecord[], checklistUid: string): ChecklistRecord {
  const checklist = checklists.find((item) => item.uid === checklistUid);
  if (!checklist) {
    throw new Error(`Checklist ${checklistUid} not found`);
  }
  return checklist;
}

export function createInMemoryChecklistFromTemplate(
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

export function updateInMemoryChecklist(checklists: ChecklistRecord[], input: ChecklistUpdateInput, changedBy?: string): void {
  const checklist = findInMemoryChecklist(checklists, input.checklistUid);
  checklist.missionUid = input.patch.missionUid ?? checklist.missionUid;
  checklist.templateUid = input.patch.templateUid ?? checklist.templateUid;
  checklist.name = input.patch.name ?? checklist.name;
  checklist.description = input.patch.description ?? checklist.description;
  checklist.startTime = input.patch.startTime ?? checklist.startTime;
  checklist.updatedAt = new Date().toISOString();
  checklist.lastChangedByTeamMemberRnsIdentity = changedBy || checklist.lastChangedByTeamMemberRnsIdentity;
}

export function setInMemoryTaskStatus(checklists: ChecklistRecord[], input: ChecklistStatusInput): void {
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

export function addInMemoryTaskRow(checklists: ChecklistRecord[], input: ChecklistRowAddInput): void {
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

export function deleteInMemoryTaskRow(checklists: ChecklistRecord[], input: ChecklistRowDeleteInput): void {
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

export function setInMemoryTaskRowStyle(checklists: ChecklistRecord[], input: ChecklistRowStyleInput): void {
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

export function setInMemoryTaskCell(checklists: ChecklistRecord[], input: ChecklistCellInput): void {
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
