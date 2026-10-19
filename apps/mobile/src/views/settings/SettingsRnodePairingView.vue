<script setup lang="ts">
import { useRouter } from "vue-router";

import type { NetworkDraft } from "../../components/settings/settingsFormSlices";
import { useSettingsRnode } from "../../components/settings/useSettingsRnode";
import type { RnodeBluetoothDeviceRecord } from "../../services/rnodeBluetooth";

const props = defineProps<{ draft: NetworkDraft }>();
const router = useRouter();
const {
  devices,
  loadPairedRnodeDevices,
  pairedDevices,
  pairedLoading,
  pairRnodeViaUsb,
  rnodeDeviceDetail,
  rnodeUsbDeviceDetail,
  scanFeedback,
  scanning,
  scanRnodeDevices,
  selectRnodeDevice,
  selectRnodeUsbDevice,
  selectedUsbDeviceId,
  usbDevices,
  usbPairing,
} = useSettingsRnode(props.draft);

async function returnToNetwork(): Promise<void> {
  await router.push({ name: "settings-network" });
}

/** Picking a paired device writes it into the Network draft and goes back there. */
async function useDevice(device: RnodeBluetoothDeviceRecord): Promise<void> {
  await selectRnodeDevice(device);
  if (props.draft.rnodeEnabled && props.draft.rnodePeripheralId.trim()) {
    await returnToNetwork();
  }
}
</script>

<template>
  <section class="panel settings-card">
    <div class="grid">
      <label>
        Bluetooth bearer
        <select v-model="draft.rnodeConnectionMode">
          <option value="ble">Bluetooth Low Energy (BLE)</option>
          <option value="bluetooth_classic">Bluetooth Classic (SPP)</option>
        </select>
      </label>
    </div>
    <div class="tcp-custom-row wrap">
      <button type="button" :disabled="pairedLoading" @click="loadPairedRnodeDevices">
        {{ pairedLoading ? "Loading paired" : "Show paired Bluetooth" }}
      </button>
      <button type="button" :disabled="scanning" @click="scanRnodeDevices">
        {{ scanning ? "Scanning" : draft.rnodeConnectionMode === "bluetooth_classic" ? "Scan RNode Classic" : "Scan RNode BLE" }}
      </button>
      <button type="button" :disabled="usbPairing" @click="pairRnodeViaUsb">
        {{ usbPairing ? "Pairing via USB" : "Pair via USB" }}
      </button>
    </div>
    <p v-if="scanFeedback" class="feedback" role="status">{{ scanFeedback }}</p>

    <div v-if="pairedDevices.length > 0" class="server-list">
      <button
        v-for="device in pairedDevices"
        :key="`paired-${device.id}`"
        type="button"
        class="server-option device-option"
        @click="useDevice(device)"
      >
        <div class="server-option-body">
          <p class="server-name">{{ device.name || device.address }}</p>
          <p class="server-endpoint">{{ rnodeDeviceDetail(device) }}</p>
        </div>
        <span class="bootstrap-badge">Paired</span>
      </button>
    </div>
    <div v-if="devices.length > 0" class="server-list">
      <button
        v-for="device in devices"
        :key="device.id"
        type="button"
        class="server-option device-option"
        @click="useDevice(device)"
      >
        <div class="server-option-body">
          <p class="server-name">{{ device.name || device.address }}</p>
          <p class="server-endpoint">{{ rnodeDeviceDetail(device) }}</p>
        </div>
        <span class="bootstrap-badge">RNode</span>
      </button>
    </div>
    <div v-if="usbDevices.length > 0" class="server-list">
      <button
        v-for="device in usbDevices"
        :key="`usb-${device.deviceId}`"
        type="button"
        class="server-option device-option"
        @click="selectRnodeUsbDevice(device)"
      >
        <div class="server-option-body">
          <p class="server-name">{{ device.productName || device.deviceName || `USB ${device.deviceId}` }}</p>
          <p class="server-endpoint">{{ rnodeUsbDeviceDetail(device) }}</p>
        </div>
        <span class="bootstrap-badge">{{ selectedUsbDeviceId === device.deviceId ? "Selected" : "USB" }}</span>
      </button>
    </div>
  </section>

  <section class="panel settings-card">
    <div class="config-section-header">
      <h3>Selected device</h3>
      <p>Filled in when you pick a device above, or enter it by hand.</p>
    </div>
    <div class="grid">
      <label>
        RNode device id
        <input v-model="draft.rnodePeripheralId" type="text" placeholder="Bluetooth address or peripheral id" />
      </label>
      <label>
        RNode display name
        <input v-model="draft.rnodeDisplayName" type="text" placeholder="Optional label" />
      </label>
    </div>
    <div class="actions">
      <button type="button" @click="returnToNetwork">Done</button>
    </div>
  </section>
</template>
