import { Capacitor } from "@capacitor/core";
import {
  createReticulumNodeClient,
  type RnodeBleDeviceRecord,
  type RnodeBlePairResult,
  type RnodeUsbDeviceRecord,
  type RnodeUsbPairResult,
} from "@reticulum/node-client";

export type { RnodeBleDeviceRecord, RnodeBlePairResult, RnodeUsbDeviceRecord, RnodeUsbPairResult };

export async function scanRnodeBleDevices(timeoutMs = 8000): Promise<RnodeBleDeviceRecord[]> {
  if (!Capacitor.isNativePlatform()) {
    return [];
  }
  return createReticulumNodeClient().scanRnodeBleDevices(timeoutMs);
}

export async function listPairedRnodeBluetoothDevices(): Promise<RnodeBleDeviceRecord[]> {
  if (!Capacitor.isNativePlatform()) {
    return [];
  }
  return createReticulumNodeClient().listPairedRnodeBluetoothDevices();
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

export async function listRnodeUsbDevices(): Promise<RnodeUsbDeviceRecord[]> {
  if (!Capacitor.isNativePlatform()) {
    return [];
  }
  return createReticulumNodeClient().listRnodeUsbDevices();
}

export async function requestRnodeUsbPermission(deviceId: number): Promise<{ deviceId: number; granted: boolean }> {
  if (!Capacitor.isNativePlatform()) {
    return { deviceId, granted: false };
  }
  return createReticulumNodeClient().requestRnodeUsbPermission(deviceId);
}

export async function startRnodeUsbBluetoothPairing(deviceId: number): Promise<RnodeUsbPairResult> {
  if (!Capacitor.isNativePlatform()) {
    return {
      id: "",
      address: "",
      paired: false,
      pairingModeStarted: false,
      manualPinRequired: false,
      bondState: "unavailable",
      message: "USB-assisted pairing is available only on Android.",
    };
  }
  return createReticulumNodeClient().startRnodeUsbBluetoothPairing(deviceId);
}

export async function cancelRnodeUsbBluetoothPairing(deviceId?: number): Promise<void> {
  if (!Capacitor.isNativePlatform()) {
    return;
  }
  await createReticulumNodeClient().cancelRnodeUsbBluetoothPairing(deviceId);
}
