import { Capacitor } from "@capacitor/core";
import { createReticulumNodeClient, type RnodeBleDeviceRecord, type RnodeBlePairResult } from "@reticulum/node-client";

export type { RnodeBleDeviceRecord, RnodeBlePairResult };

export async function scanRnodeBleDevices(timeoutMs = 8000): Promise<RnodeBleDeviceRecord[]> {
  if (!Capacitor.isNativePlatform()) {
    return [];
  }
  return createReticulumNodeClient().scanRnodeBleDevices(timeoutMs);
}

export async function pairRnodeBleDevice(id: string): Promise<RnodeBlePairResult> {
  if (!Capacitor.isNativePlatform()) {
    return {
      id,
      address: id,
      paired: false,
      bondingStarted: false,
      bondState: "unavailable",
    };
  }
  return createReticulumNodeClient().pairRnodeBleDevice(id);
}
