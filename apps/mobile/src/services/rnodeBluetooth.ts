import { Capacitor } from "@capacitor/core";
import {
  createReticulumNodeClient,
  type RnodeBleDeviceRecord,
  type RnodeBlePairResult,
  type RnodeBluetoothDeviceRecord,
  type RnodeBluetoothMode,
  type RnodeConnectionMode,
  type RnodeUsbDeviceRecord,
  type RnodeUsbPairResult,
} from "@reticulum/node-client";

export type { RnodeBleDeviceRecord, RnodeBlePairResult, RnodeUsbDeviceRecord, RnodeUsbPairResult };
export type { RnodeBluetoothDeviceRecord, RnodeBluetoothMode, RnodeConnectionMode };

export function normalizeRnodeBluetoothMode(mode: RnodeConnectionMode): RnodeBluetoothMode {
  return mode === "bluetooth_classic" ? "bluetooth_classic" : "ble";
}

export function rnodeBluetoothModeLabel(mode: RnodeBluetoothMode): string {
  return mode === "bluetooth_classic" ? "Bluetooth Classic" : "BLE";
}

export function rnodeBluetoothDeviceDetail(device: RnodeBluetoothDeviceRecord): string {
  const parts = [device.address];
  if (typeof device.rssi === "number") parts.push(`RSSI ${device.rssi}`);
  parts.push(device.paired ? "Paired" : "Not paired");
  parts.push((device.supportedModes ?? ["ble"]).map(rnodeBluetoothModeLabel).join(" + "));
  return parts.join(" | ");
}

export function rnodeUsbDeviceDetail(device: RnodeUsbDeviceRecord): string {
  return [
    device.productName || device.manufacturerName || device.deviceName,
    device.serialNumber ? `S/N ${device.serialNumber}` : "",
    `VID ${device.vendorId.toString(16).padStart(4, "0")}`,
    `PID ${device.productId.toString(16).padStart(4, "0")}`,
    device.hasPermission ? "USB allowed" : "USB permission needed",
  ].filter(Boolean).join(" | ");
}

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

export async function scanRnodeBluetoothDevices(
  mode: RnodeBluetoothMode,
  timeoutMs = 8000,
): Promise<RnodeBluetoothDeviceRecord[]> {
  if (!Capacitor.isNativePlatform()) {
    return [];
  }
  return createReticulumNodeClient().scanRnodeBluetoothDevices(mode, timeoutMs);
}

export async function pairRnodeBluetoothDevice(
  id: string,
  mode: RnodeBluetoothMode,
): Promise<RnodeBlePairResult> {
  if (!Capacitor.isNativePlatform()) {
    return { id, address: id, paired: false, bondingStarted: false, bondState: "unavailable" };
  }
  return createReticulumNodeClient().pairRnodeBluetoothDevice(id, mode);
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

export async function startRnodeUsbBluetoothPairing(deviceId: number, bluetoothDeviceId?: string): Promise<RnodeUsbPairResult> {
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
  return createReticulumNodeClient().startRnodeUsbBluetoothPairing(deviceId, bluetoothDeviceId);
}

export async function cancelRnodeUsbBluetoothPairing(deviceId?: number): Promise<void> {
  if (!Capacitor.isNativePlatform()) {
    return;
  }
  await createReticulumNodeClient().cancelRnodeUsbBluetoothPairing(deviceId);
}
