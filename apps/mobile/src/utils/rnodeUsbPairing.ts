import type { RnodeBleDeviceRecord } from "../services/rnodeBluetooth";

function bluetoothDeviceKey(device: RnodeBleDeviceRecord): string {
  return String(device.id || device.address || "").trim().toLowerCase();
}

function isNamedRnode(device: RnodeBleDeviceRecord): boolean {
  return /rnode/i.test(device.name || "");
}

export function selectUsbBondedRnodeCandidate(
  beforePairing: RnodeBleDeviceRecord[],
  afterPairing: RnodeBleDeviceRecord[],
): RnodeBleDeviceRecord | undefined {
  const beforeKeys = new Set(beforePairing.map(bluetoothDeviceKey).filter(Boolean));
  const newlyPaired = afterPairing.filter((device) => {
    const key = bluetoothDeviceKey(device);
    return device.paired && key.length > 0 && !beforeKeys.has(key);
  });
  const namedRnodes = newlyPaired.filter(isNamedRnode);
  if (namedRnodes.length === 1) {
    return namedRnodes[0];
  }
  if (newlyPaired.length === 1) {
    return newlyPaired[0];
  }
  const pairedAfter = afterPairing.filter((device) => device.paired && bluetoothDeviceKey(device).length > 0);
  if (beforeKeys.size === 0 && pairedAfter.length === 1) {
    return pairedAfter[0];
  }
  return undefined;
}
