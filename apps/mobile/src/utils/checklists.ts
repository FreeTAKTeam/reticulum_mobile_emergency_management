import { ref } from "vue";

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
  scheduledAt: string;
  teamLabel: string;
  compatibilityLabel: string;
}

export interface ChecklistTask {
  id: string;
  title: string;
  description: string;
  status: ChecklistTaskStatus;
  metaLabel: string;
  metaTone: ChecklistTaskMetaTone;
}

export interface ChecklistDetail {
  id: string;
  heroTitle: string;
  heroSubtitle: string;
  progress: number;
  progressLabel: string;
  pendingLabel: string;
  tasksHeading: string;
  tasks: ChecklistTask[];
}

const liveChecklists = ref<ChecklistRecord[]>([]);
const templateChecklists = ref<ChecklistRecord[]>([]);
const checklistDetails = ref<Record<string, ChecklistDetail>>({});

export function getChecklistRecords(segment: ChecklistSegment): ChecklistRecord[] {
  return segment === "templates" ? templateChecklists.value : liveChecklists.value;
}

export function getChecklistRecordById(checklistId: string): ChecklistRecord | undefined {
  return [...liveChecklists.value, ...templateChecklists.value].find((record) => record.id === checklistId);
}

export function getChecklistDetailById(checklistId: string): ChecklistDetail | undefined {
  return checklistDetails.value[checklistId];
}

export function addLocalChecklist(record: ChecklistRecord): void {
  liveChecklists.value = [record, ...liveChecklists.value];
  checklistDetails.value = {
    ...checklistDetails.value,
    [record.id]: {
      id: record.id,
      heroTitle: record.title,
      heroSubtitle: record.subtitle,
      progress: record.progress,
      progressLabel: `${record.progress}% complete`,
      pendingLabel: record.statusCountLabel,
      tasksHeading: "Tasks",
      tasks: [],
    },
  };
}
