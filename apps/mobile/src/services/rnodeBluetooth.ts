import { Capacitor } from "@capacitor/core";
import { createReticulumNodeClient, type RnodeBleDeviceRecord } from "@reticulum/node-client";

export type { RnodeBleDeviceRecord };

export async function scanRnodeBleDevices(timeoutMs = 8000): Promise<RnodeBleDeviceRecord[]> {
  if (!Capacitor.isNativePlatform()) {
    return [];
  }
  return createReticulumNodeClient().scanRnodeBleDevices(timeoutMs);
}

export async function pairRnodeBleDevice(id: string): Promise<void> {
  if (!Capacitor.isNativePlatform()) {
    return;
  }
  await createReticulumNodeClient().pairRnodeBleDevice(id);
}
