import {
  createReticulumNodeClient,
  type WearableDevice,
  type WearablePermissionState,
  type WearableSensorEvent,
  type WearableStatusRecord,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, ref } from "vue";

import { useNodeStore } from "./nodeStore";

function mergeDevice(items: WearableDevice[], device: WearableDevice): WearableDevice[] {
  const index = items.findIndex((entry) => entry.deviceId === device.deviceId);
  if (index === -1) {
    return [device, ...items];
  }
  const next = [...items];
  next[index] = { ...next[index], ...device };
  return next;
}

function mergeStatus(items: WearableStatusRecord[], status: WearableStatusRecord): WearableStatusRecord[] {
  const index = items.findIndex(
    (entry) => entry.deviceId === status.deviceId && entry.sensorType === status.sensorType,
  );
  if (index === -1) {
    return [status, ...items];
  }
  const next = [...items];
  next[index] = status;
  return next;
}

function defaultPermission(): WearablePermissionState {
  return {
    granted: false,
    missing: [],
  };
}

export const useWearablesStore = defineStore("wearables", () => {
  const nodeStore = useNodeStore();
  const client = createReticulumNodeClient({ mode: nodeStore.settings.clientMode });

  const permission = ref<WearablePermissionState>(defaultPermission());
  const discoveredDevices = ref<WearableDevice[]>([]);
  const wearableStatus = ref<WearableStatusRecord[]>([]);
  const lastEvent = ref<WearableSensorEvent | null>(null);
  const scanning = ref(false);
  const lastError = ref<string | null>(null);
  const initialized = ref(false);

  const activeHeartRates = computed(() =>
    wearableStatus.value.filter((status) => status.sensorType === "heart_rate_bpm" && status.status === "Active"),
  );

  const latestHeartRate = computed(() =>
    activeHeartRates.value
      .filter((status) => typeof status.value === "number")
      .sort((left, right) => right.lastSeenTimestampMs - left.lastSeenTimestampMs)[0],
  );

  async function refreshPermissions(): Promise<void> {
    permission.value = await client.getWearablePermissionState();
  }

  async function refreshStatus(): Promise<void> {
    wearableStatus.value = await client.getWearableStatus();
  }

  async function refreshManagerStatus(): Promise<void> {
    const status = await client.getWearableManagerStatus();
    scanning.value = status.scanning;
    discoveredDevices.value = status.items;
  }

  async function init(): Promise<void> {
    if (initialized.value) {
      return;
    }
    initialized.value = true;
    client.on("wearableDeviceDiscovered", (device) => {
      discoveredDevices.value = mergeDevice(discoveredDevices.value, device);
    });
    client.on("wearableConnectionChanged", (device) => {
      discoveredDevices.value = mergeDevice(discoveredDevices.value, device);
    });
    client.on("wearableSensorEvent", (event) => {
      lastEvent.value = event;
    });
    client.on("wearableScanStopped", (event) => {
      scanning.value = event.scanning;
    });
    client.on("wearableError", (event) => {
      lastError.value = event.message;
    });
    client.on("wearableSensorUpdated", (status) => {
      wearableStatus.value = mergeStatus(wearableStatus.value, status);
    });
    client.on("projectionInvalidated", (event) => {
      if (event.scope === "Wearables") {
        void refreshStatus().catch((error: unknown) => {
          lastError.value = error instanceof Error ? error.message : String(error);
        });
      }
    });
    await Promise.all([
      refreshPermissions().catch(() => undefined),
      refreshManagerStatus().catch(() => undefined),
      refreshStatus().catch(() => undefined),
    ]);
  }

  async function requestPermissions(): Promise<void> {
    permission.value = await client.requestWearablePermissions();
  }

  async function startScan(timeoutMs = 15_000): Promise<void> {
    lastError.value = null;
    await init();
    const result = await client.startWearableScan(timeoutMs);
    scanning.value = result.scanning;
  }

  async function stopScan(): Promise<void> {
    const result = await client.stopWearableScan();
    scanning.value = result.scanning;
  }

  async function listBondedDevices(): Promise<void> {
    discoveredDevices.value = (await client.listBondedWearableDevices()).reduce(
      (items, device) => mergeDevice(items, device),
      discoveredDevices.value,
    );
  }

  async function connect(deviceId: string): Promise<void> {
    lastError.value = null;
    const device = await client.connectWearable(deviceId);
    if (device) {
      discoveredDevices.value = mergeDevice(discoveredDevices.value, device);
    }
  }

  async function disconnect(): Promise<void> {
    await client.disconnectWearable();
    await refreshManagerStatus().catch(() => undefined);
  }

  return {
    permission,
    discoveredDevices,
    wearableStatus,
    lastEvent,
    scanning,
    lastError,
    activeHeartRates,
    latestHeartRate,
    init,
    refreshPermissions,
    refreshStatus,
    refreshManagerStatus,
    requestPermissions,
    startScan,
    stopScan,
    listBondedDevices,
    connect,
    disconnect,
  };
});
