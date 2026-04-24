import type {
  ChecklistColumnRecord as RuntimeChecklistColumnRecord,
  ChecklistRecord as RuntimeChecklistRecord,
  ChecklistTaskRecord as RuntimeChecklistTaskRecord,
  ChecklistTemplateRecord as RuntimeChecklistTemplateRecord,
} from "@reticulum/node-client";

export type ChecklistSegment = "live" | "templates";
export type ChecklistStatus = "active" | "late" | "completed";
export type ChecklistFilter = "all" | ChecklistStatus;
export type ChecklistTaskStatus = "pending" | "late" | "completed";
export type ChecklistTaskMetaTone = "clock" | "alert" | "done";

export interface ChecklistRecord {
  id: string;
  title: string;
  subtitle: string;
  status: ChecklistStatus;
  progress: number;
  statusCountLabel: string;
  taskSync?: ChecklistTaskSync;
  metadataLines: string[];
}

export interface ChecklistTaskSync {
  received: number;
  total: number;
  pending: number;
  progress: number;
  label: string;
  detail: string;
}

export interface ChecklistTask {
  id: string;
  title: string;
  description: string;
  status: ChecklistTaskStatus;
  metaLabel: string;
  metaTone: ChecklistTaskMetaTone;
  primaryColumnUid: string;
  rowBackgroundColor?: string;
  lineBreakEnabled: boolean;
  cells: ChecklistTaskCell[];
}

export interface ChecklistTaskCell {
  columnUid: string;
  label: string;
  value: string;
  editable: boolean;
}

export interface ChecklistDetail {
  id: string;
  heroTitle: string;
  heroSubtitle: string;
  heroMetaLines: string[];
  progress: number;
  progressLabel: string;
  pendingLabel: string;
  tasksHeading: string;
  tasks: ChecklistTask[];
}

export function formatChecklistTimestamp(value?: string): string {
  if (!value) {
    return "No schedule";
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }).format(date);
}

export function checklistStatusFromRuntime(value: string): ChecklistStatus {
  if (value === "LATE") {
    return "late";
  }
  if (value === "COMPLETE" || value === "COMPLETE_LATE") {
    return "completed";
  }
  return "active";
}

export function checklistTaskStatusFromRuntime(value: string): ChecklistTaskStatus {
  if (value === "LATE") {
    return "late";
  }
  if (value === "COMPLETE" || value === "COMPLETE_LATE") {
    return "completed";
  }
  return "pending";
}

function findTaskValue(task: RuntimeChecklistTaskRecord, columnUid: string): string | undefined {
  return task.cells.find((cell) => cell.columnUid === columnUid)?.value;
}

function findPrimaryTaskColumnUid(task: RuntimeChecklistTaskRecord): string {
  if (task.cells.some((cell) => cell.columnUid === "col-task")) {
    return "col-task";
  }
  if (task.cells.some((cell) => cell.columnUid === "col-item")) {
    return "col-item";
  }
  return task.cells.find((cell) => typeof cell.value === "string" && cell.value.trim())?.columnUid
    ?? task.cells[0]?.columnUid
    ?? "col-task";
}

function runtimeChecklistCellsToUi(
  task: RuntimeChecklistTaskRecord,
  columns: RuntimeChecklistColumnRecord[],
): ChecklistTaskCell[] {
  const sortedColumns = [...columns].sort((left, right) => left.displayOrder - right.displayOrder);
  return sortedColumns.map((column) => ({
    columnUid: column.columnUid,
    label: column.columnName,
    value: findTaskValue(task, column.columnUid) ?? "",
    editable: column.columnEditable,
  }));
}

function isHydratedChecklistTask(task: RuntimeChecklistTaskRecord): boolean {
  return task.number > 0;
}

function checklistAuthorLabel(record: RuntimeChecklistRecord): string {
  const authorDisplayName = (
    record as RuntimeChecklistRecord & {
      createdByTeamMemberDisplayName?: string;
      createdByTeamMemberName?: string;
      createdByDisplayName?: string;
    }
  ).createdByTeamMemberDisplayName ?? (
    record as RuntimeChecklistRecord & {
      createdByDisplayName?: string;
    }
  ).createdByDisplayName ?? (
    record as RuntimeChecklistRecord & {
      createdByTeamMemberName?: string;
    }
  ).createdByTeamMemberName;
  return authorDisplayName?.trim() || "Original author pending";
}

function checklistMetadataLines(record: RuntimeChecklistRecord): string[] {
  return [
    `Created ${formatChecklistTimestamp(record.createdAt)}`,
    `Updated ${formatChecklistTimestamp(record.updatedAt)}`,
    checklistAuthorLabel(record),
  ];
}

function templateMetadataLines(record: RuntimeChecklistTemplateRecord): string[] {
  return [
    `Created ${formatChecklistTimestamp(record.createdAt)}`,
    `Updated ${formatChecklistTimestamp(record.updatedAt)}`,
    record.originType === "CSV_IMPORT" ? "Imported template" : "REM template library",
  ];
}

export function runtimeChecklistTaskToUi(
  task: RuntimeChecklistTaskRecord,
  columns: RuntimeChecklistColumnRecord[] = [],
): ChecklistTask {
  const status = checklistTaskStatusFromRuntime(task.taskStatus);
  const title = findTaskValue(task, "col-task")
    ?? findTaskValue(task, "col-item")
    ?? task.legacyValue
    ?? `Task ${task.number}`;
  const description = findTaskValue(task, "col-description")
    ?? findTaskValue(task, "col-category")
    ?? task.notes
    ?? "No additional task details.";
  let metaLabel = "Pending action";
  if (status === "completed") {
    metaLabel = task.completedAt
      ? `Completed ${formatChecklistTimestamp(task.completedAt)}`
      : "Completed";
  } else if (status === "late") {
    metaLabel = "Action overdue";
  } else if (task.dueDtg) {
    metaLabel = `Due ${formatChecklistTimestamp(task.dueDtg)}`;
  } else if (typeof task.dueRelativeMinutes === "number") {
    metaLabel = `${task.dueRelativeMinutes} min window`;
  }

  return {
    id: task.taskUid,
    title,
    description,
    status,
    metaLabel,
    metaTone: status === "completed" ? "done" : status === "late" ? "alert" : "clock",
    primaryColumnUid: findPrimaryTaskColumnUid(task),
    rowBackgroundColor: task.rowBackgroundColor ?? undefined,
    lineBreakEnabled: task.lineBreakEnabled ?? false,
    cells: runtimeChecklistCellsToUi(task, columns),
  };
}

export function runtimeChecklistToUi(record: RuntimeChecklistRecord): ChecklistRecord {
  const status = checklistStatusFromRuntime(record.checklistStatus);
  const activeTasks = record.tasks.filter((task) => !task.deletedAt);
  const receivedTasks = activeTasks.filter(isHydratedChecklistTask).length;
  const countedTasks = record.counts.pendingCount + record.counts.lateCount + record.counts.completeCount;
  const expectedTasks = typeof record.expectedTaskCount === "number" ? record.expectedTaskCount : 0;
  const highestTaskNumber = activeTasks.reduce((highest, task) => Math.max(highest, task.number), 0);
  const totalTasks = Math.max(receivedTasks, countedTasks, expectedTasks, highestTaskNumber);
  const pendingTasks = Math.max(totalTasks - receivedTasks, 0);
  const taskSyncProgress = totalTasks === 0 ? 100 : Math.round((receivedTasks * 100) / totalTasks);
  return {
    id: record.uid,
    title: record.name,
    subtitle: record.description || "Checklist package",
    status,
    progress: Math.round(record.progressPercent),
    statusCountLabel:
      status === "completed"
        ? `${record.counts.completeCount} completed`
        : record.counts.lateCount > 0
          ? `${record.counts.lateCount} late`
          : `${record.counts.pendingCount} pending`,
    taskSync: totalTasks > 0 && pendingTasks > 0
      ? {
          received: receivedTasks,
          total: totalTasks,
          pending: pendingTasks,
          progress: taskSyncProgress,
          label: "Receiving tasks",
          detail: `${pendingTasks} ${pendingTasks === 1 ? "task" : "tasks"} pending over LXMF`,
        }
      : undefined,
    metadataLines: checklistMetadataLines(record),
  };
}

export function runtimeTemplateToUi(record: RuntimeChecklistTemplateRecord): ChecklistRecord {
  return {
    id: record.uid,
    title: record.name,
    subtitle: record.description || "Template",
    status: "active",
    progress: 0,
    statusCountLabel: `${record.tasks.length} tasks`,
    metadataLines: templateMetadataLines(record),
  };
}

export function runtimeChecklistDetailToUi(record: RuntimeChecklistRecord): ChecklistDetail {
  return {
    id: record.uid,
    heroTitle: record.name,
    heroSubtitle: record.description || "Emergency preparedness checklist",
    heroMetaLines: checklistMetadataLines(record),
    progress: Math.round(record.progressPercent),
    progressLabel: `${Math.round(record.progressPercent)}% complete`,
    pendingLabel: `${record.counts.pendingCount} pending | ${record.counts.lateCount} late | ${record.counts.completeCount} done`,
    tasksHeading: "Tasks",
    tasks: record.tasks
      .filter((task) => !task.deletedAt)
      .map((task) => runtimeChecklistTaskToUi(task, record.columns)),
  };
}

export function runtimeTemplateDetailToUi(record: RuntimeChecklistTemplateRecord): ChecklistDetail {
  return {
    id: record.uid,
    heroTitle: record.name,
    heroSubtitle: record.description || "Checklist template",
    heroMetaLines: templateMetadataLines(record),
    progress: 0,
    progressLabel: `${record.tasks.length} template tasks`,
    pendingLabel: "Template preview",
    tasksHeading: "Template Tasks",
    tasks: record.tasks
      .filter((task) => !task.deletedAt)
      .map((task) => runtimeChecklistTaskToUi(task, record.columns)),
  };
}
