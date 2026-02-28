import { Capacitor } from "@capacitor/core";
import { LocalNotifications } from "@capacitor/local-notifications";

const UPDATES_CHANNEL_ID = "operational-updates";
const UPDATES_GROUP_ID = "operational-updates";
let initState: Promise<boolean> | null = null;
let nextNotificationId = Number(Date.now() % 2_000_000_000);

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

export async function initAppNotifications(): Promise<void> {
  await ensureNotificationsReady();
}

export async function notifyOperationalUpdate(
  title: string,
  body: string,
): Promise<void> {
  if (!(await ensureNotificationsReady())) {
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
        },
      },
    ],
  }).catch(() => undefined);
}
