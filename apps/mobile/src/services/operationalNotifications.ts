import { notifyOperationalUpdate, type NotificationExtra } from "./notifications";

export type OperationalNotificationScope = "eam" | "event" | "chat" | "checklist";

const seenByScope = new Map<OperationalNotificationScope, Set<string>>();

function getScopeSet(scope: OperationalNotificationScope): Set<string> {
  const existing = seenByScope.get(scope);
  if (existing) {
    return existing;
  }
  const created = new Set<string>();
  seenByScope.set(scope, created);
  return created;
}

export function primeOperationalNotificationScope(
  scope: OperationalNotificationScope,
  keys: Iterable<string>,
): void {
  const bucket = getScopeSet(scope);
  for (const key of keys) {
    const normalized = key.trim();
    if (normalized) {
      bucket.add(normalized);
    }
  }
}

export async function notifyOperationalUpdateOnce(
  scope: OperationalNotificationScope,
  key: string,
  title: string,
  body: string,
  extra: NotificationExtra = {},
): Promise<boolean> {
  const normalizedKey = key.trim();
  if (!normalizedKey) {
    return false;
  }

  const bucket = getScopeSet(scope);
  if (bucket.has(normalizedKey)) {
    return false;
  }
  bucket.add(normalizedKey);
  await notifyOperationalUpdate(title, body, extra).catch(() => undefined);
  return true;
}

export function truncateNotificationBody(value: string, maxLength = 120): string {
  const normalized = value.trim();
  if (normalized.length <= maxLength) {
    return normalized;
  }
  return `${normalized.slice(0, Math.max(0, maxLength - 1)).trimEnd()}…`;
}
