import type { ChecklistRecord } from "@reticulum/node-client";

import {
  notifyOperationalUpdateOnce,
  primeOperationalNotificationScope,
  truncateNotificationBody,
} from "../services/operationalNotifications";

type ChecklistNotificationWork = {
  key: string;
  title: string;
  body: string;
  route: string;
  timer: ReturnType<typeof setTimeout>;
};

const CHECKLIST_NOTIFICATION_DEBOUNCE_MS = 2_000;

function latestChangeStamp(record: ChecklistRecord): string {
  const stamps = [record.updatedAt, record.uploadedAt, record.deletedAt]
    .filter((value): value is string => Boolean(value?.trim()));
  return stamps.reduce((latest, value) => (value > latest ? value : latest), "");
}

function notificationKey(record: ChecklistRecord): string {
  return `${record.uid}:${latestChangeStamp(record)}`;
}

function normalizeIdentity(value: string | undefined): string {
  return (value ?? "").trim().toLowerCase();
}

function notificationBody(record: ChecklistRecord): string {
  const counts = record.counts ?? { pendingCount: 0, completeCount: 0, lateCount: 0 };
  const tasks = Array.isArray(record.tasks) ? record.tasks : [];
  const summary = `${counts.pendingCount} pending, ${counts.completeCount} complete`;
  const late = counts.lateCount > 0 ? `, ${counts.lateCount} late` : "";
  const taskCount = tasks.length === 1 ? "1 task" : `${tasks.length} tasks`;
  return truncateNotificationBody(`${summary}${late} across ${taskCount}`);
}

export function createChecklistNotificationCoordinator(
  getLocalIdentity: () => string | undefined,
): { notifyForChanges: (records: ChecklistRecord[]) => Promise<void> } {
  const pending = new Map<string, ChecklistNotificationWork>();
  let primed = false;

  function isLocalRecord(record: ChecklistRecord): boolean {
    const localIdentity = normalizeIdentity(getLocalIdentity());
    if (!localIdentity) {
      return false;
    }
    const changedBy = normalizeIdentity(record.lastChangedByTeamMemberRnsIdentity);
    if (changedBy) {
      return changedBy === localIdentity;
    }
    return normalizeIdentity(record.createdByTeamMemberRnsIdentity) === localIdentity;
  }

  function queue(record: ChecklistRecord): void {
    const key = notificationKey(record);
    if (!key.trim()) {
      return;
    }
    const existing = pending.get(record.uid);
    if (existing) {
      clearTimeout(existing.timer);
    }
    const work: ChecklistNotificationWork = {
      key,
      title: `Checklist updated: ${record.name || "Checklist"}`,
      body: notificationBody(record),
      route: `/checklists/${record.uid}`,
      timer: setTimeout(() => {
        pending.delete(record.uid);
        void notifyOperationalUpdateOnce(
          "checklist",
          work.key,
          work.title,
          work.body,
          { route: work.route },
        );
      }, CHECKLIST_NOTIFICATION_DEBOUNCE_MS),
    };
    pending.set(record.uid, work);
  }

  async function notifyForChanges(records: ChecklistRecord[]): Promise<void> {
    const activeRecords = records.filter((record) => record && !record.deletedAt);
    if (!primed) {
      primeOperationalNotificationScope(
        "checklist",
        activeRecords.map((record) => notificationKey(record)),
      );
      primed = true;
      return;
    }
    for (const record of activeRecords) {
      if (!isLocalRecord(record)) {
        queue(record);
      }
    }
  }

  return { notifyForChanges };
}
