import { ref } from "vue";

import {
  listPairedRnodeBluetoothDevices,
  listRnodeUsbDevices,
  normalizeRnodeBluetoothMode,
  pairRnodeBluetoothDevice,
  rnodeBluetoothDeviceDetail,
  rnodeBluetoothModeLabel,
  rnodeUsbDeviceDetail,
  requestRnodeUsbPermission,
  scanRnodeBluetoothDevices,
  startRnodeUsbBluetoothPairing,
  type RnodeBluetoothDeviceRecord,
  type RnodeConnectionMode,
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
  rnodeConnectionMode: RnodeConnectionMode;
  rnodePeripheralId: string;
  rnodeDisplayName: string;
  rnodeRegion: string;
  rnodeProfile: string;
}

export function useSettingsRnode(form: SettingsRnodeForm) {
  const scanFeedback = ref("");
  const pairedLoading = ref(false);
  const pairedDevices = ref<RnodeBluetoothDeviceRecord[]>([]);
  const scanning = ref(false);
  const devices = ref<RnodeBluetoothDeviceRecord[]>([]);
  const usbPairing = ref(false);
  const usbDevices = ref<RnodeUsbDeviceRecord[]>([]);
  const selectedUsbDeviceId = ref<number | null>(null);
  const USB_BOND_POLL_ATTEMPTS = 15;
  const USB_BOND_POLL_DELAY_MS = 2_000;

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
      const mode = normalizeRnodeBluetoothMode(form.rnodeConnectionMode);
      devices.value = await scanRnodeBluetoothDevices(mode);
      if (devices.value.length === 0) {
        scanFeedback.value = `No RNode ${rnodeBluetoothModeLabel(mode)} devices found.`;
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

  async function selectRnodeDevice(device: RnodeBluetoothDeviceRecord): Promise<void> {
    const deviceId = device.id || device.address;
    const supportedModes = device.supportedModes ?? ["ble"];
    const selectedMode = normalizeRnodeBluetoothMode(form.rnodeConnectionMode);
    const mode = supportedModes.includes(selectedMode)
      ? selectedMode
      : supportedModes[0] ?? "ble";
    if (!device.paired) {
      try {
        const pairResult = await pairRnodeBluetoothDevice(deviceId, mode);
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
    form.rnodeConnectionMode = mode;
    form.rnodeDisplayName = device.name || device.address;
  }

  function selectRnodeUsbDevice(device: RnodeUsbDeviceRecord): void {
    selectedUsbDeviceId.value = device.deviceId;
    scanFeedback.value = `Selected USB RNode ${device.productName || device.deviceName || device.deviceId}.`;
  }

  function selectPairedRnodeForSettings(device: RnodeBluetoothDeviceRecord): void {
    const deviceId = device.id || device.address;
    form.rnodeEnabled = true;
    form.rnodePeripheralId = deviceId;
    form.rnodeDisplayName = device.name || device.address || deviceId;
    const supportedModes = device.supportedModes ?? ["ble"];
    if (!supportedModes.includes(normalizeRnodeBluetoothMode(form.rnodeConnectionMode))) {
      form.rnodeConnectionMode = supportedModes[0] ?? "ble";
    }
  }

  function delay(ms: number): Promise<void> {
    return new Promise((resolve) => window.setTimeout(resolve, ms));
  }

  async function waitForUsbBondedRnodeCandidate(beforePairing: RnodeBluetoothDeviceRecord[]): Promise<RnodeBluetoothDeviceRecord | undefined> {
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
          supportedModes: ["ble"],
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
    rnodeDeviceDetail: rnodeBluetoothDeviceDetail,
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
