import { ref } from "vue";

import {
  listPairedRnodeBluetoothDevices,
  listRnodeUsbDevices,
  pairRnodeBleDevice,
  requestRnodeUsbPermission,
  scanRnodeBleDevices,
  startRnodeUsbBluetoothPairing,
  type RnodeBleDeviceRecord,
  type RnodeUsbDeviceRecord,
} from "../../services/rnodeBluetooth";
import { requestRnodeBluetoothPermission } from "../../services/setupPermissions";
import {
  RNODE_PROFILE_SPECS,
  rnodeProfileSummary,
} from "../../utils/rnodeProfiles";
import { selectUsbBondedRnodeCandidate } from "../../utils/rnodeUsbPairing";

export interface SettingsRnodeForm {
  rnodeEnabled: boolean;
  rnodePeripheralId: string;
  rnodeDisplayName: string;
  rnodeRegion: string;
  rnodeProfile: string;
}

export function useSettingsRnode(form: SettingsRnodeForm) {
  const scanFeedback = ref("");
  const pairedLoading = ref(false);
  const pairedDevices = ref<RnodeBleDeviceRecord[]>([]);
  const scanning = ref(false);
  const devices = ref<RnodeBleDeviceRecord[]>([]);
  const usbPairing = ref(false);
  const usbDevices = ref<RnodeUsbDeviceRecord[]>([]);
  const selectedUsbDeviceId = ref<number | null>(null);
  const USB_BOND_POLL_ATTEMPTS = 15;
  const USB_BOND_POLL_DELAY_MS = 2_000;

  function rnodeDeviceDetail(device: RnodeBleDeviceRecord): string {
    const parts = [device.address];
    if (typeof device.rssi === "number") {
      parts.push(`RSSI ${device.rssi}`);
    }
    parts.push(device.paired ? "Paired" : "Not paired");
    return parts.join(" | ");
  }

  function rnodeUsbDeviceDetail(device: RnodeUsbDeviceRecord): string {
    const parts = [
      device.productName || device.manufacturerName || device.deviceName,
      device.serialNumber ? `S/N ${device.serialNumber}` : "",
      `VID ${device.vendorId.toString(16).padStart(4, "0")}`,
      `PID ${device.productId.toString(16).padStart(4, "0")}`,
      device.hasPermission ? "USB allowed" : "USB permission needed",
    ].filter(Boolean);
    return parts.join(" | ");
  }

  async function loadPairedRnodeDevices(): Promise<void> {
    if (pairedLoading.value) {
      return;
    }
    if (!(await ensureBluetoothPermissionForRnode())) {
      return;
    }
    pairedLoading.value = true;
    scanFeedback.value = "";
    try {
      pairedDevices.value = await listPairedRnodeBluetoothDevices();
      if (pairedDevices.value.length === 0) {
        scanFeedback.value = "No paired Bluetooth devices found on this Android phone.";
      }
    } catch (error: unknown) {
      scanFeedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      pairedLoading.value = false;
    }
  }

  async function scanRnodeDevices(): Promise<void> {
    if (scanning.value) {
      return;
    }
    if (!(await ensureBluetoothPermissionForRnode())) {
      return;
    }
    scanning.value = true;
    scanFeedback.value = "";
    try {
      devices.value = await scanRnodeBleDevices();
      if (devices.value.length === 0) {
        scanFeedback.value = "No RNode BLE devices found.";
      }
    } catch (error: unknown) {
      scanFeedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      scanning.value = false;
    }
  }

  async function ensureBluetoothPermissionForRnode(): Promise<boolean> {
    const permission = await requestRnodeBluetoothPermission();
    if (permission === "granted") {
      return true;
    }
    scanFeedback.value = "Bluetooth permission is required for RNode device selection.";
    return false;
  }

  async function selectRnodeDevice(device: RnodeBleDeviceRecord): Promise<void> {
    const deviceId = device.id || device.address;
    if (!device.paired) {
      try {
        const pairResult = await pairRnodeBleDevice(deviceId);
        if (!pairResult.paired && !pairResult.bondingStarted) {
          scanFeedback.value = "Android did not start Bluetooth pairing for this RNode.";
          return;
        }
        if (!pairResult.paired) {
          form.rnodeEnabled = false;
          form.rnodePeripheralId = "";
          pairedDevices.value = await listPairedRnodeBluetoothDevices().catch(() => []);
          scanFeedback.value = "Bluetooth pairing started. Confirm the Android pairing prompt, then select the RNode from paired devices before saving.";
          return;
        }
        scanFeedback.value = "RNode is already paired.";
        form.rnodePeripheralId = pairResult.id || pairResult.address || deviceId;
      } catch (error: unknown) {
        scanFeedback.value = error instanceof Error ? error.message : String(error);
        return;
      }
    } else {
      scanFeedback.value = "";
      form.rnodePeripheralId = deviceId;
    }
    form.rnodeEnabled = true;
    form.rnodeDisplayName = device.name || device.address;
  }

  function selectRnodeUsbDevice(device: RnodeUsbDeviceRecord): void {
    selectedUsbDeviceId.value = device.deviceId;
    scanFeedback.value = `Selected USB RNode ${device.productName || device.deviceName || device.deviceId}.`;
  }

  function selectPairedRnodeForSettings(device: RnodeBleDeviceRecord): void {
    const deviceId = device.id || device.address;
    form.rnodeEnabled = true;
    form.rnodePeripheralId = deviceId;
    form.rnodeDisplayName = device.name || device.address || deviceId;
  }

  function delay(ms: number): Promise<void> {
    return new Promise((resolve) => window.setTimeout(resolve, ms));
  }

  async function waitForUsbBondedRnodeCandidate(beforePairing: RnodeBleDeviceRecord[]): Promise<RnodeBleDeviceRecord | undefined> {
    for (let attempt = 0; attempt < USB_BOND_POLL_ATTEMPTS; attempt += 1) {
      const currentPairedDevices = await listPairedRnodeBluetoothDevices().catch(() => []);
      pairedDevices.value = currentPairedDevices;
      const candidate = selectUsbBondedRnodeCandidate(beforePairing, currentPairedDevices);
      if (candidate) {
        return candidate;
      }
      if (attempt + 1 < USB_BOND_POLL_ATTEMPTS) {
        await delay(USB_BOND_POLL_DELAY_MS);
      }
    }
    return undefined;
  }

  async function pairRnodeViaUsb(): Promise<void> {
    if (usbPairing.value) {
      return;
    }
    if (!(await ensureBluetoothPermissionForRnode())) {
      return;
    }
    const pairedBeforeUsb = await listPairedRnodeBluetoothDevices().catch(() => []);
    usbPairing.value = true;
    pairedDevices.value = [];
    devices.value = [];
    usbDevices.value = [];
    scanFeedback.value = "Looking for USB-connected RNodes...";
    try {
      const devices = await listRnodeUsbDevices();
      usbDevices.value = devices;
      const selectedDevice = devices.find((candidate) => candidate.deviceId === selectedUsbDeviceId.value);
      if (devices.length > 1 && !selectedDevice) {
        scanFeedback.value = "Select the USB RNode to use, then pair via USB.";
        return;
      }
      const device = selectedDevice ?? devices[0];
      if (!device) {
        scanFeedback.value = "No USB RNode found. Connect the RNode by USB and grant Android USB access.";
        return;
      }
      selectedUsbDeviceId.value = device.deviceId;
      if (!device.hasPermission) {
        const permission = await requestRnodeUsbPermission(device.deviceId);
        if (!permission.granted) {
          scanFeedback.value = "USB permission denied for the RNode.";
          return;
        }
      }
      scanFeedback.value = "Starting RNode Bluetooth pairing mode over USB...";
      const bluetoothDeviceId = form.rnodePeripheralId.trim() || undefined;
      const result = await startRnodeUsbBluetoothPairing(device.deviceId, bluetoothDeviceId);
      if (result.paired) {
        selectPairedRnodeForSettings({
          id: result.id || result.address,
          address: result.address || result.id,
          name: result.id || result.address || "RNode",
          paired: true,
        });
        pairedDevices.value = await listPairedRnodeBluetoothDevices().catch(() => []);
        scanFeedback.value = "RNode paired over USB-assisted Bluetooth. Save settings to connect.";
        return;
      }
      if (result.pin) {
        scanFeedback.value = result.message || `RNode pairing mode started. Enter PIN ${result.pin} if Android prompts for it.`;
      } else if (result.manualPinRequired) {
        scanFeedback.value = result.message || "RNode pairing mode started. Enter the PIN shown on the RNode if Android prompts for it.";
      } else {
        scanFeedback.value = result.message || "USB-assisted RNode pairing did not complete.";
      }
      const bondedDevice = await waitForUsbBondedRnodeCandidate(pairedBeforeUsb);
      if (bondedDevice) {
        selectPairedRnodeForSettings(bondedDevice);
        scanFeedback.value = `RNode paired over USB and selected ${bondedDevice.name || bondedDevice.address || bondedDevice.id}. Save settings to connect.`;
      }
    } catch (error: unknown) {
      scanFeedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      usbPairing.value = false;
    }
  }

  return {
    RNODE_PROFILE_SPECS,
    devices,
    loadPairedRnodeDevices,
    pairedDevices,
    pairedLoading,
    pairRnodeViaUsb,
    rnodeDeviceDetail,
    rnodeProfileSummary,
    rnodeUsbDeviceDetail,
    scanFeedback,
    scanning,
    scanRnodeDevices,
    selectPairedRnodeForSettings,
    selectRnodeDevice,
    selectRnodeUsbDevice,
    selectedUsbDeviceId,
    usbDevices,
    usbPairing,
  };
}
