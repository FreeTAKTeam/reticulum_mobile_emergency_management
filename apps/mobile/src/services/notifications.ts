import { Capacitor } from "@capacitor/core";
import { LocalNotifications, type ActionPerformed } from "@capacitor/local-notifications";

const UPDATES_CHANNEL_ID = "operational-updates";
const UPDATES_GROUP_ID = "operational-updates";
const NOTIFICATION_ACTIVITY_STORAGE_KEY = "reticulum.mobile.notificationActivity.v1";
const NOTIFICATION_ACTIVITY_CHANGED_EVENT = "reticulum-mobile-notification-activity";
const MAX_NOTIFICATION_ACTIVITY_RECORDS = 20;
let initState: Promise<boolean> | null = null;
let nextNotificationId = Number(Date.now() % 2_000_000_000);
let actionListenerRegistered = false;
let actionListenerRegistration: Promise<void> | null = null;
let pendingNotificationTarget: NotificationNavigationTarget | null = null;
let notificationNavigationHandler: ((target: NotificationNavigationTarget) => void | Promise<void>) | null = null;

function reportNotificationFailure(operation: string, error: unknown): void {
  console.warn(
    `[notifications] ${operation} failed: ${error instanceof Error ? error.message : String(error)}`,
    error,
  );
}

export interface NotificationNavigationTarget {
  route?: string;
  conversationId?: string;
  messageIdHex?: string;
}

export type NotificationExtra = NotificationNavigationTarget & Record<string, unknown>;

export interface NotificationActivityRecord extends NotificationNavigationTarget {
  id: number;
  title: string;
  body: string;
  at: number;
}

function isNotificationRuntimeSupported(): boolean {
  return Capacitor.getPlatform() !== "web";
}

function getNextNotificationId(): number {
  nextNotificationId = (nextNotificationId % 2_000_000_000) + 1;
  return nextNotificationId;
}

function normalizeActivityRecord(value: unknown): NotificationActivityRecord | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const record = value as Record<string, unknown>;
  const id = Number(record.id);
  const at = Number(record.at);
  const title = typeof record.title === "string" ? record.title.trim() : "";
  const body = typeof record.body === "string" ? record.body.trim() : "";
  if (!Number.isFinite(id) || !Number.isFinite(at) || !title) {
    return null;
  }
  const target = notificationTargetFromExtra(record);
  return {
    id,
    title,
    body,
    at,
    ...(target ?? {}),
  };
}

export function listNotificationActivity(): NotificationActivityRecord[] {
  try {
    const raw = localStorage.getItem(NOTIFICATION_ACTIVITY_STORAGE_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as unknown[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .map((record) => normalizeActivityRecord(record))
      .filter((record): record is NotificationActivityRecord => Boolean(record))
      .sort((left, right) => right.at - left.at)
      .slice(0, MAX_NOTIFICATION_ACTIVITY_RECORDS);
  } catch (error: unknown) {
    reportNotificationFailure("activity-history read", error);
    return [];
  }
}

function saveNotificationActivity(records: NotificationActivityRecord[]): void {
  try {
    localStorage.setItem(
      NOTIFICATION_ACTIVITY_STORAGE_KEY,
      JSON.stringify(records.slice(0, MAX_NOTIFICATION_ACTIVITY_RECORDS)),
    );
    window.dispatchEvent(new CustomEvent(NOTIFICATION_ACTIVITY_CHANGED_EVENT));
  } catch (error: unknown) {
    // Activity history is best-effort and must never block notification delivery.
    reportNotificationFailure("activity-history persistence", error);
  }
}

function appendNotificationActivity(
  id: number,
  title: string,
  body: string,
  extra: NotificationExtra,
): void {
  const target = notificationTargetFromExtra(extra);
  saveNotificationActivity([
    {
      id,
      title,
      body,
      at: Date.now(),
      ...(target ?? {}),
    },
    ...listNotificationActivity().filter((record) => record.id !== id),
  ]);
}

export function subscribeNotificationActivity(listener: () => void): () => void {
  window.addEventListener(NOTIFICATION_ACTIVITY_CHANGED_EVENT, listener);
  window.addEventListener("storage", listener);
  return () => {
    window.removeEventListener(NOTIFICATION_ACTIVITY_CHANGED_EVENT, listener);
    window.removeEventListener("storage", listener);
  };
}

async function ensureNotificationsReady(): Promise<boolean> {
  if (!isNotificationRuntimeSupported()) {
    return false;
  }

  await registerNotificationActionListener();

  if (!initState) {
    initState = (async () => {
      const permission = await LocalNotifications.checkPermissions();
      const granted =
        permission.display === "granted"
          ? permission
          : await LocalNotifications.requestPermissions();
      if (granted.display !== "granted") {
        return false;
      }

      if (Capacitor.getPlatform() === "android") {
        await LocalNotifications.createChannel({
          id: UPDATES_CHANNEL_ID,
          name: "Operational Updates",
          description: "Incoming mesh events and action message changes",
          importance: 4,
          visibility: 1,
          lights: true,
          lightColor: "#16edff",
          vibration: true,
        });
      }

      return true;
    })();
  }

  return initState;
}

export async function checkNotificationPermission(): Promise<boolean> {
  if (!isNotificationRuntimeSupported()) {
    return false;
  }

  try {
    const permission = await LocalNotifications.checkPermissions();
    return permission.display === "granted";
  } catch (error: unknown) {
    reportNotificationFailure("permission check", error);
    return false;
  }
}

export async function requestNotificationPermission(): Promise<boolean> {
  if (!isNotificationRuntimeSupported()) {
    return false;
  }

  const permission = await LocalNotifications.requestPermissions().catch((error: unknown) => {
    reportNotificationFailure("permission request", error);
    return { display: "denied" as const };
  });
  if (permission.display !== "granted") {
    return false;
  }
  initState = null;
  return ensureNotificationsReady().catch((error: unknown) => {
    reportNotificationFailure("permission initialization", error);
    return false;
  });
}

function notificationTargetFromExtra(extra: unknown): NotificationNavigationTarget | null {
  if (!extra || typeof extra !== "object") {
    return null;
  }
  const payload = extra as Record<string, unknown>;
  const route = typeof payload.route === "string" ? payload.route.trim() : "";
  const conversationId = typeof payload.conversationId === "string" ? payload.conversationId.trim() : "";
  const messageIdHex = typeof payload.messageIdHex === "string" ? payload.messageIdHex.trim() : "";
  if (!route && !conversationId && !messageIdHex) {
    return null;
  }
  return {
    route: route || undefined,
    conversationId: conversationId || undefined,
    messageIdHex: messageIdHex || undefined,
  };
}

function dispatchNotificationTarget(target: NotificationNavigationTarget): void {
  if (!notificationNavigationHandler) {
    pendingNotificationTarget = target;
    return;
  }
  void Promise.resolve(notificationNavigationHandler(target)).catch((error: unknown) => {
    reportNotificationFailure("notification navigation", error);
  });
}

function registerNotificationActionListener(): Promise<void> {
  if (actionListenerRegistered || !isNotificationRuntimeSupported()) {
    return Promise.resolve();
  }
  if (!actionListenerRegistration) {
    actionListenerRegistration = LocalNotifications.addListener(
      "localNotificationActionPerformed",
      (action: ActionPerformed) => {
        const target = notificationTargetFromExtra(action.notification.extra);
        if (target) {
          dispatchNotificationTarget(target);
        }
      },
    )
      .then(() => {
        actionListenerRegistered = true;
      })
      .catch((error: unknown) => {
        reportNotificationFailure("action-listener registration", error);
      })
      .finally(() => {
        actionListenerRegistration = null;
      });
  }
  return actionListenerRegistration;
}

export function registerNotificationNavigationHandler(
  handler: (target: NotificationNavigationTarget) => void | Promise<void>,
): void {
  notificationNavigationHandler = handler;
  const target = pendingNotificationTarget;
  pendingNotificationTarget = null;
  if (target) {
    dispatchNotificationTarget(target);
  }
}

export async function initAppNotifications(): Promise<void> {
  await ensureNotificationsReady().catch((error: unknown) => {
    reportNotificationFailure("initialization", error);
    return false;
  });
}

export async function notifyOperationalUpdate(
  title: string,
  body: string,
  extra: NotificationExtra = {},
): Promise<void> {
  const id = getNextNotificationId();
  appendNotificationActivity(id, title, body, extra);

  if (!(await ensureNotificationsReady().catch((error: unknown) => {
    reportNotificationFailure("delivery initialization", error);
    return false;
  }))) {
    return;
  }

  await LocalNotifications.schedule({
    notifications: [
      {
        id,
        title,
        body,
        channelId: Capacitor.getPlatform() === "android" ? UPDATES_CHANNEL_ID : undefined,
        group: Capacitor.getPlatform() === "android" ? UPDATES_GROUP_ID : undefined,
        autoCancel: true,
        summaryText: body,
        largeBody: body,
        extra: {
          at: Date.now(),
          ...extra,
        },
      },
    ],
  }).catch((error: unknown) => {
    reportNotificationFailure("delivery", error);
  });
}
