import { computed, reactive, shallowRef } from "vue";
import { useRouter } from "vue-router";

import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { normalizeDisplayName } from "../utils/peers";
import { DEFAULT_TCP_COMMUNITY_ENDPOINTS, normalizeTcpCommunityClients } from "../utils/tcpCommunityServers";
import { markSetupWizardCompleted, markSetupWizardOpened } from "../utils/setupWizardState";
import { runDetachedStoreTask } from "../utils/detachedStoreTask";
import {
  DEFAULT_RNODE_SETTINGS,
  RNODE_FREQUENCY_MAX_HZ,
  RNODE_FREQUENCY_MIN_HZ,
  RNODE_PROFILE_SPECS,
  RNODE_REGION_SPECS,
  inferRnodeRegionFromCoordinates,
  inferRnodeRegionFromTimezone,
  isRnodeFrequencyHz,
  normalizeRnodeRegion,
  normalizeRnodeSettings,
  resolveRnodeFrequencyForRegionChange,
  rnodeProfileSummary,
} from "../utils/rnodeProfiles";
import { selectUsbBondedRnodeCandidate } from "../utils/rnodeUsbPairing";
import {
  listPairedRnodeBluetoothDevices,
  normalizeRnodeBluetoothMode,
  rnodeBluetoothDeviceDetail,
  rnodeBluetoothModeLabel,
  rnodeUsbDeviceDetail,
  scanRnodeBluetoothDevices,
  pairRnodeBluetoothDevice,
  listRnodeUsbDevices,
  requestRnodeUsbPermission,
  startRnodeUsbBluetoothPairing,
  type RnodeBluetoothDeviceRecord,
  type RnodeUsbDeviceRecord,
} from "../services/rnodeBluetooth";
import { telemetryService } from "../services/telemetry";
import {
  checkSetupPermissions,
  requestLocationPermission,
  requestNotificationPermission,
  requestRnodeBluetoothPermission,
  type SetupPermissionSnapshot,
  type SetupPermissionState,
} from "../services/setupPermissions";
import {
  SETUP_STEPS,
  USB_BOND_POLL_ATTEMPTS,
  USB_BOND_POLL_DELAY_MS,
  normalizeWizardTcpEndpoint,
  normalizeWizardTelemetryPublishIntervalSeconds,
} from "./setupWizardConfig";

export {
  normalizeWizardTcpEndpoint,
  normalizeWizardTelemetryPublishIntervalSeconds,
} from "./setupWizardConfig";
export type { SetupWizardStep, SetupWizardStepId } from "./setupWizardConfig";

export function useSetupWizard() {
  const nodeStore = useNodeStore();
  const sosStore = useSosStore();
  const router = useRouter();
  const activeIndex = shallowRef(0);
  const customTcpEndpoint = shallowRef("");
  const feedback = shallowRef("");
  const saving = shallowRef(false);
  const rnodePairedLoading = shallowRef(false);
  const rnodePairedDevices = shallowRef<RnodeBluetoothDeviceRecord[]>([]);
  const rnodeScanning = shallowRef(false);
  const rnodeDevices = shallowRef<RnodeBluetoothDeviceRecord[]>([]);
  const rnodeUsbPairing = shallowRef(false);
  const rnodeUsbDevices = shallowRef<RnodeUsbDeviceRecord[]>([]);
  const selectedRnodeUsbDeviceId = shallowRef<number | null>(null);
  const permissions = reactive<SetupPermissionSnapshot>({
    location: "prompt",
    notifications: "prompt",
    bluetooth: "prompt",
  });

  const draft = reactive({
    displayName: nodeStore.settings.displayName,
    tcpClients: [...nodeStore.settings.tcpClients],
    rnode: normalizeRnodeSettings(nodeStore.settings.rnode ?? DEFAULT_RNODE_SETTINGS),
    telemetryEnabled: nodeStore.settings.telemetry.enabled,
    telemetryPublishIntervalSeconds: nodeStore.settings.telemetry.publishIntervalSeconds,
    sosEnabled: sosStore.settings.enabled,
  });

  const steps = SETUP_STEPS;
  const activeStep = computed(() => steps[activeIndex.value]);
  const normalizedDisplayName = computed(() => normalizeDisplayName(draft.displayName) ?? "");
  const normalizedTcpClients = computed(() =>
    normalizeTcpCommunityClients(draft.tcpClients, DEFAULT_TCP_COMMUNITY_ENDPOINTS, true),
  );
  const normalizedTelemetryPublishIntervalSeconds = computed(() =>
    normalizeWizardTelemetryPublishIntervalSeconds(draft.telemetryPublishIntervalSeconds),
  );
  const selectedTcpEndpointSet = computed(() => new Set(normalizedTcpClients.value));
  const sosFloatingButtonEnabled = computed(() => draft.sosEnabled || sosStore.settings.floatingButton);

  const canGoNext = computed(() => {
    if (activeStep.value.id === "callsign") {
      return normalizedDisplayName.value.length > 0;
    }
    return true;
  });

  function open(): void {
    markSetupWizardOpened();
    runDetachedStoreTask(nodeStore, "setup", "permission refresh", refreshPermissions);
  }
  async function refreshPermissions(): Promise<void> {
    const snapshot = await checkSetupPermissions();
    permissions.location = snapshot.location;
    permissions.notifications = snapshot.notifications;
    permissions.bluetooth = snapshot.bluetooth;
  }

  function setTcpEndpoint(endpoint: string, selected: boolean): void {
    const next = new Set(normalizedTcpClients.value);
    if (selected) {
      next.add(endpoint);
    } else {
      next.delete(endpoint);
    }
    draft.tcpClients = [...next];
  }
  function addCustomTcpEndpoint(): void {
    const normalized = normalizeWizardTcpEndpoint(customTcpEndpoint.value);
    if (!normalized) {
      feedback.value = "Invalid endpoint. Use host:port or [ipv6]:port.";
      return;
    }
    const next = new Set(normalizedTcpClients.value);
    next.add(normalized);
    draft.tcpClients = [...next];
    customTcpEndpoint.value = "";
    feedback.value = "";
  }
  function removeTcpEndpoint(endpoint: string): void {
    draft.tcpClients = normalizedTcpClients.value.filter((entry) => entry !== endpoint);
  }

  function next(): void {
    if (!canGoNext.value) {
      feedback.value = "Set a call sign before continuing.";
      return;
    }
    feedback.value = "";
    activeIndex.value = Math.min(activeIndex.value + 1, steps.length - 1);
  }

  function back(): void {
    feedback.value = "";
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
  }

  async function requestLocation(): Promise<void> {
    permissions.location = await requestLocationPermission();
  }
  async function requestNotifications(): Promise<void> {
    permissions.notifications = await requestNotificationPermission();
  }

  async function requestBluetooth(): Promise<void> {
    permissions.bluetooth = await requestRnodeBluetoothPermission();
  }
  async function inferRnodeRegion(): Promise<void> {
    const previousRegion = draft.rnode.region;
    const locationRegion = await telemetryService.getCurrentPosition()
      .then((fix) => inferRnodeRegionFromCoordinates(fix.lat, fix.lon))
      .catch(() => undefined);
    const nextRegion = locationRegion ?? inferRnodeRegionFromTimezone();
    const source = locationRegion ? "device location" : "device time zone";
    if (!nextRegion) {
      feedback.value = "REM could not safely infer a LoRa region. Select the legal region and frequency manually.";
      return;
    }
    draft.rnode.region = nextRegion;
    draft.rnode.frequencyHz = resolveRnodeFrequencyForRegionChange(
      previousRegion,
      nextRegion,
      draft.rnode.frequencyHz,
    );
    feedback.value = `Inferred ${nextRegion} from ${source}. Confirm the legal frequency before saving.`;
  }
  function selectRnodeRegion(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLSelectElement)) return;
    const previousRegion = draft.rnode.region;
    const nextRegion = normalizeRnodeRegion(target.value);
    draft.rnode.region = nextRegion;
    draft.rnode.frequencyHz = resolveRnodeFrequencyForRegionChange(
      previousRegion,
      nextRegion,
      draft.rnode.frequencyHz,
    );
  }
  async function ensureBluetoothPermissionForRnode(): Promise<boolean> {
    if (permissions.bluetooth !== "granted") {
      permissions.bluetooth = await requestRnodeBluetoothPermission();
    }
    if (permissions.bluetooth === "granted") {
      return true;
    }
    feedback.value = "Bluetooth permission is required for RNode device selection.";
    return false;
  }

  async function loadPairedRnodeDevices(): Promise<void> {
    if (rnodePairedLoading.value) {
      return;
    }
    if (!(await ensureBluetoothPermissionForRnode())) {
      return;
    }
    rnodePairedLoading.value = true;
    feedback.value = "";
    try {
      rnodePairedDevices.value = await listPairedRnodeBluetoothDevices();
      if (rnodePairedDevices.value.length === 0) {
        feedback.value = "No paired Bluetooth devices found on this Android phone.";
      }
    } catch (error: unknown) {
      feedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      rnodePairedLoading.value = false;
    }
  }

  async function scanRnodeDevices(): Promise<void> {
    if (rnodeScanning.value) {
      return;
    }
    if (!(await ensureBluetoothPermissionForRnode())) {
      return;
    }
    rnodeScanning.value = true;
    feedback.value = "";
    try {
      const mode = normalizeRnodeBluetoothMode(draft.rnode.connectionMode);
      rnodeDevices.value = await scanRnodeBluetoothDevices(mode);
      if (rnodeDevices.value.length === 0) {
        feedback.value = `No RNode ${rnodeBluetoothModeLabel(mode)} devices found. Pair the RNode in Android Bluetooth settings or enter its device ID manually.`;
      }
    } catch (error: unknown) {
      feedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      rnodeScanning.value = false;
    }
  }

  async function selectRnodeDevice(device: RnodeBluetoothDeviceRecord): Promise<void> {
    const deviceId = device.id || device.address;
    const supportedModes = device.supportedModes ?? ["ble"];
    const selectedMode = normalizeRnodeBluetoothMode(draft.rnode.connectionMode);
    const mode = supportedModes.includes(selectedMode)
      ? selectedMode
      : supportedModes[0] ?? "ble";
    if (!device.paired) {
      try {
        const pairResult = await pairRnodeBluetoothDevice(deviceId, mode);
        if (!pairResult.paired && !pairResult.bondingStarted) {
          feedback.value = "Android did not start Bluetooth pairing for this RNode.";
          return;
        }
        if (!pairResult.paired) {
          draft.rnode.enabled = false;
          draft.rnode.peripheralId = "";
          rnodePairedDevices.value = await listPairedRnodeBluetoothDevices().catch(() => []);
          feedback.value = "Bluetooth pairing started. Confirm the Android pairing prompt, then select the RNode from paired devices before finishing setup.";
          return;
        }
        feedback.value = "RNode is already paired.";
        draft.rnode.peripheralId = pairResult.id || pairResult.address || deviceId;
      } catch (error: unknown) {
        feedback.value = error instanceof Error ? error.message : String(error);
        return;
      }
    } else {
      feedback.value = "";
      draft.rnode.peripheralId = deviceId;
    }
    draft.rnode.enabled = true;
    draft.rnode.connectionMode = mode;
    draft.rnode.displayName = device.name || device.address;
  }

  function selectRnodeUsbDevice(device: RnodeUsbDeviceRecord): void {
    selectedRnodeUsbDeviceId.value = device.deviceId;
    feedback.value = `Selected USB RNode ${device.productName || device.deviceName || device.deviceId}.`;
  }

  function selectPairedRnodeForDraft(device: RnodeBluetoothDeviceRecord): void {
    const deviceId = device.id || device.address;
    draft.rnode.enabled = true;
    draft.rnode.peripheralId = deviceId;
    draft.rnode.displayName = device.name || device.address || deviceId;
    const supportedModes = device.supportedModes ?? ["ble"];
    if (!supportedModes.includes(normalizeRnodeBluetoothMode(draft.rnode.connectionMode))) {
      draft.rnode.connectionMode = supportedModes[0] ?? "ble";
    }
  }

  function delay(ms: number): Promise<void> {
    return new Promise((resolve) => window.setTimeout(resolve, ms));
  }

  async function waitForUsbBondedRnodeCandidate(beforePairing: RnodeBluetoothDeviceRecord[]): Promise<RnodeBluetoothDeviceRecord | undefined> {
    for (let attempt = 0; attempt < USB_BOND_POLL_ATTEMPTS; attempt += 1) {
      const pairedDevices = await listPairedRnodeBluetoothDevices().catch(() => []);
      rnodePairedDevices.value = pairedDevices;
      const candidate = selectUsbBondedRnodeCandidate(beforePairing, pairedDevices);
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
    if (rnodeUsbPairing.value) {
      return;
    }
    if (!(await ensureBluetoothPermissionForRnode())) {
      return;
    }
    const pairedBeforeUsb = await listPairedRnodeBluetoothDevices().catch(() => []);
    rnodeUsbPairing.value = true;
    rnodePairedDevices.value = [];
    rnodeDevices.value = [];
    rnodeUsbDevices.value = [];
    feedback.value = "Looking for USB-connected RNodes...";
    try {
      const devices = await listRnodeUsbDevices();
      rnodeUsbDevices.value = devices;
      const selectedDevice = devices.find((candidate) => candidate.deviceId === selectedRnodeUsbDeviceId.value);
      if (devices.length > 1 && !selectedDevice) {
        feedback.value = "Select the USB RNode to use, then pair via USB.";
        return;
      }
      const device = selectedDevice ?? devices[0];
      if (!device) {
        feedback.value = "No USB RNode found. Connect the RNode by USB and grant Android USB access.";
        return;
      }
      selectedRnodeUsbDeviceId.value = device.deviceId;
      if (!device.hasPermission) {
        const permission = await requestRnodeUsbPermission(device.deviceId);
        if (!permission.granted) {
          feedback.value = "USB permission denied for the RNode.";
          return;
        }
      }
      feedback.value = "Starting RNode Bluetooth pairing mode over USB...";
      const bluetoothDeviceId = draft.rnode.peripheralId.trim() || undefined;
      const result = await startRnodeUsbBluetoothPairing(device.deviceId, bluetoothDeviceId);
      if (result.paired) {
        selectPairedRnodeForDraft({
          id: result.id || result.address,
          address: result.address || result.id,
          name: result.id || result.address || "RNode",
          paired: true,
          supportedModes: ["ble"],
        });
        feedback.value = "RNode paired over USB-assisted Bluetooth.";
        rnodePairedDevices.value = await listPairedRnodeBluetoothDevices().catch(() => []);
        return;
      }
      if (result.pin) {
        feedback.value = result.message || `RNode pairing mode started. Enter PIN ${result.pin} if Android prompts for it.`;
      } else if (result.manualPinRequired) {
        feedback.value = result.message || "RNode pairing mode started. Enter the PIN shown on the RNode if Android prompts for it.";
      } else {
        feedback.value = result.message || "USB-assisted RNode pairing did not complete.";
      }
      const bondedDevice = await waitForUsbBondedRnodeCandidate(pairedBeforeUsb);
      if (bondedDevice) {
        selectPairedRnodeForDraft(bondedDevice);
        feedback.value = `RNode paired over USB and selected ${bondedDevice.name || bondedDevice.address || bondedDevice.id}.`;
      }
    } catch (error: unknown) {
      feedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      rnodeUsbPairing.value = false;
    }
  }

  function profileSummary(profile = draft.rnode.profile): string {
    return rnodeProfileSummary(profile);
  }

  function permissionLabel(value: SetupPermissionState): string {
    switch (value) {
      case "granted":
        return "Granted";
      case "denied":
        return "Denied";
      case "unavailable":
        return "Unavailable";
      case "prompt":
      default:
        return "Not requested";
    }
  }

  async function finish(): Promise<void> {
    if (!normalizedDisplayName.value || saving.value) {
      feedback.value = "Set a call sign before finishing setup.";
      return;
    }
    if (!isRnodeFrequencyHz(draft.rnode.frequencyHz)) {
      feedback.value = `RNode LoRa frequency must be between ${RNODE_FREQUENCY_MIN_HZ} and ${RNODE_FREQUENCY_MAX_HZ} Hz.`;
      return;
    }
    saving.value = true;
    feedback.value = "";
    try {
      await nodeStore.updateSettings({
        displayName: normalizedDisplayName.value,
        tcpClients: normalizedTcpClients.value,
        telemetry: {
          ...nodeStore.settings.telemetry,
          enabled: draft.telemetryEnabled,
          publishIntervalSeconds: normalizedTelemetryPublishIntervalSeconds.value,
        },
        rnode: normalizeRnodeSettings(draft.rnode),
      });
      await sosStore.saveSettings({
        ...sosStore.settings,
        enabled: draft.sosEnabled,
        floatingButton: draft.sosEnabled ? true : sosStore.settings.floatingButton,
      });
      if (draft.telemetryEnabled && permissions.location !== "granted") {
        permissions.location = await requestLocationPermission();
      }
      markSetupWizardCompleted();
      await nodeStore.startNode();
      await router.replace("/dashboard");
    } catch (error: unknown) {
      feedback.value = error instanceof Error ? error.message : String(error);
    } finally {
      saving.value = false;
    }
  }

  return {
    activeIndex,
    activeStep,
    canGoNext,
    customTcpEndpoint,
    draft,
    feedback,
    normalizedDisplayName,
    normalizedTelemetryPublishIntervalSeconds,
    normalizedTcpClients,
    open,
    permissions,
    permissionLabel,
    profileSummary,
    refreshPermissions,
    rnodeDeviceDetail: rnodeBluetoothDeviceDetail,
    rnodePairedDevices,
    rnodePairedLoading,
    rnodeDevices,
    rnodeUsbDeviceDetail,
    rnodeUsbDevices,
    rnodeUsbPairing,
    rnodeProfiles: RNODE_PROFILE_SPECS,
    rnodeRegions: RNODE_REGION_SPECS,
    rnodeScanning,
    saving,
    selectedRnodeUsbDeviceId,
    selectedTcpEndpointSet,
    sosFloatingButtonEnabled,
    steps,
    addCustomTcpEndpoint,
    back,
    finish,
    next,
    removeTcpEndpoint,
    requestLocation,
    requestNotifications,
    requestBluetooth,
    inferRnodeRegion,
    selectRnodeRegion,
    loadPairedRnodeDevices,
    scanRnodeDevices,
    selectRnodeDevice,
    selectRnodeUsbDevice,
    pairRnodeViaUsb,
    setTcpEndpoint,
  };
}
