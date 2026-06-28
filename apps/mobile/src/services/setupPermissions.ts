import { LocalNotifications } from "@capacitor/local-notifications";
import { Capacitor } from "@capacitor/core";
import { createReticulumNodeClient } from "@reticulum/node-client";

import { telemetryService, type TelemetryPermissionState } from "./telemetry";

export type SetupPermissionState = TelemetryPermissionState;

export interface SetupPermissionSnapshot {
  location: SetupPermissionState;
  notifications: SetupPermissionState;
  bluetooth: SetupPermissionState;
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
      bluetooth: "unavailable",
    };
  }

  try {
    const notifications = await LocalNotifications.checkPermissions();
    const bluetooth = await createReticulumNodeClient()
      .checkRnodeBluetoothPermissions()
      .then((permission) => notificationPermissionToState(permission.bluetooth))
      .catch(() => "unavailable" as SetupPermissionState);
    return {
      location,
      notifications: notificationPermissionToState(notifications.display),
      bluetooth,
    };
  } catch {
    return {
      location,
      notifications: "unavailable",
      bluetooth: "unavailable",
    };
  }
}

export function requestLocationPermission(): Promise<SetupPermissionState> {
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

export async function requestRnodeBluetoothPermission(): Promise<SetupPermissionState> {
  if (!Capacitor.isNativePlatform()) {
    return "unavailable";
  }

  return createReticulumNodeClient()
    .requestRnodeBluetoothPermissions()
    .then((permission) => notificationPermissionToState(permission.bluetooth))
    .catch(() => "unavailable");
}
