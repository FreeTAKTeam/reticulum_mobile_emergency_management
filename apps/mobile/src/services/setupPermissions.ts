import { LocalNotifications } from "@capacitor/local-notifications";
import { Capacitor } from "@capacitor/core";

import { telemetryService, type TelemetryPermissionState } from "./telemetry";

export type SetupPermissionState = TelemetryPermissionState;

export interface SetupPermissionSnapshot {
  location: SetupPermissionState;
  notifications: SetupPermissionState;
}

function notificationPermissionToState(value: string | undefined): SetupPermissionState {
  if (value === "granted") {
    return "granted";
  }
  if (value === "denied") {
    return "denied";
  }
  return "prompt";
}

export async function checkSetupPermissions(): Promise<SetupPermissionSnapshot> {
  const location = await telemetryService.getPermissionState();
  if (!Capacitor.isNativePlatform()) {
    return {
      location,
      notifications: "unavailable",
    };
  }

  try {
    const notifications = await LocalNotifications.checkPermissions();
    return {
      location,
      notifications: notificationPermissionToState(notifications.display),
    };
  } catch {
    return {
      location,
      notifications: "unavailable",
    };
  }
}

export async function requestLocationPermission(): Promise<SetupPermissionState> {
  return telemetryService.requestPermission();
}

export async function requestNotificationPermission(): Promise<SetupPermissionState> {
  if (!Capacitor.isNativePlatform()) {
    return "unavailable";
  }

  try {
    const permission = await LocalNotifications.requestPermissions();
    return notificationPermissionToState(permission.display);
  } catch {
    return "unavailable";
  }
}
