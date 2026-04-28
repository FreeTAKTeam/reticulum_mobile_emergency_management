import { Capacitor } from "@capacitor/core";
import { LocalNotifications, type ActionPerformed } from "@capacitor/local-notifications";

const UPDATES_CHANNEL_ID = "operational-updates";
const UPDATES_GROUP_ID = "operational-updates";
let initState: Promise<boolean> | null = null;
let nextNotificationId = Number(Date.now() % 2_000_000_000);
let actionListenerRegistered = false;
let pendingNotificationTarget: NotificationNavigationTarget | null = null;
let notificationNavigationHandler: ((target: NotificationNavigationTarget) => void | Promise<void>) | null = null;

export interface NotificationNavigationTarget {
  route?: string;
  conversationId?: string;
  messageIdHex?: string;
}

export type NotificationExtra = NotificationNavigationTarget & Record<string, unknown>;

function isNotificationRuntimeSupported(): boolean {
  return Capacitor.getPlatform() !== "web";
}

function getNextNotificationId(): number {
  nextNotificationId = (nextNotificationId % 2_000_000_000) + 1;
  return nextNotificationId;
}

async function ensureNotificationsReady(): Promise<boolean> {
  if (!isNotificationRuntimeSupported()) {
    return false;
  }

  registerNotificationActionListener();

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
        }).catch(() => undefined);
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

  const permission = await LocalNotifications.checkPermissions().catch(() => ({ display: "denied" }));
  return permission.display === "granted";
}

export async function requestNotificationPermission(): Promise<boolean> {
  if (!isNotificationRuntimeSupported()) {
    return false;
  }

  const permission = await LocalNotifications.requestPermissions().catch(() => ({ display: "denied" }));
  if (permission.display !== "granted") {
    return false;
  }
  initState = Promise.resolve(true);
  return true;
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
  void notificationNavigationHandler(target);
}

function registerNotificationActionListener(): void {
  if (actionListenerRegistered || !isNotificationRuntimeSupported()) {
    return;
  }
  actionListenerRegistered = true;
  void LocalNotifications.addListener(
    "localNotificationActionPerformed",
    (action: ActionPerformed) => {
      const target = notificationTargetFromExtra(action.notification.extra);
      if (target) {
        dispatchNotificationTarget(target);
      }
    },
  ).catch(() => undefined);
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
  await ensureNotificationsReady().catch(() => false);
}

export async function notifyOperationalUpdate(
  title: string,
  body: string,
  extra: NotificationExtra = {},
): Promise<void> {
  if (!(await ensureNotificationsReady().catch(() => false))) {
    return;
  }

  await LocalNotifications.schedule({
    notifications: [
      {
        id: getNextNotificationId(),
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
  }).catch(() => undefined);
}
