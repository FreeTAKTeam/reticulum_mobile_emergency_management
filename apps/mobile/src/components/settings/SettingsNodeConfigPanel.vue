<script setup lang="ts">
import { computed, ref, useTemplateRef } from "vue";

import { useNodeStore } from "../../stores/nodeStore";
import { useTelemetryStore } from "../../stores/telemetryStore";
import { TCP_COMMUNITY_SERVERS, toTcpEndpoint } from "../../utils/tcpCommunityServers";
import { useSettingsRnode } from "./useSettingsRnode";
import { RNODE_REGION_SPECS, normalizeRnodeRegion, rnodeRegionDefaultFrequencyHz } from "../../utils/rnodeProfiles";

interface NodeSettingsForm {
  displayName: string;
  clientMode: string;
  announceCapabilities: string;
  announceIntervalSeconds: number;
  tcpClients: string[];
  broadcast: boolean;
  transportNodeEnabled: boolean;
  rnodeEnabled: boolean;
  rnodePeripheralId: string;
  rnodeDisplayName: string;
  rnodeRegion: string;
  rnodeProfile: string;
  rnodeFrequencyHz: number;
}

interface KnownTcpServerOption {
  name: string;
  endpoint: string;
  isBootstrap: boolean;
}

const props = defineProps<{ form: NodeSettingsForm }>();
const emit = defineEmits<{ feedback: [message: string] }>();
const nodeStore = useNodeStore();
const telemetryStore = useTelemetryStore();
const panel = useTemplateRef<HTMLDetailsElement>("panel");
const customTcpEndpoint = ref("");

const ownAppHash = computed(() => nodeStore.status.appDestinationHex || "Start node to populate");
const knownTcpServers = computed<KnownTcpServerOption[]>(() =>
  TCP_COMMUNITY_SERVERS.map((server) => ({
    name: server.name,
    endpoint: toTcpEndpoint(server),
    isBootstrap: Boolean(server.isBootstrap),
  })),
);
const normalizedTcpClients = computed(() => [
  ...new Set(
    props.form.tcpClients
      .map((entry: string) => entry.trim())
      .filter((entry) => entry.length > 0),
  ),
]);
const selectedTcpEndpointSet = computed(() => new Set(normalizedTcpClients.value));
const activePropagationNodeHex = computed(
  () => nodeStore.syncStatus.activePropagationNodeHex?.trim() ?? "",
);
const runtimeSummary = computed(() => {
  const endpointCount = normalizedTcpClients.value.length;
  const endpointLabel = endpointCount === 1 ? "endpoint" : "endpoints";
  const rnodeLabel = props.form.rnodeEnabled ? ` | RNode ${props.form.rnodeProfile}` : "";
  return `${props.form.clientMode} mode | ${endpointCount} TCP ${endpointLabel}${rnodeLabel}`;
});
const activePropagationNodeLabel = computed(() => {
  if (!activePropagationNodeHex.value) return "None";
  const discoveredPeer = nodeStore.discoveredByDestination[activePropagationNodeHex.value];
  if (discoveredPeer) {
    return discoveredPeer.announcedName || discoveredPeer.label || activePropagationNodeHex.value;
  }
  return nodeStore.savedByDestination[activePropagationNodeHex.value]?.label
    || activePropagationNodeHex.value;
});
const propagationSelectionSummary = computed(() => {
  if (!activePropagationNodeHex.value) {
    return "No propagation relay has been announced yet.";
  }
  if (
    nodeStore.bestPropagationNodeHex
    && nodeStore.bestPropagationNodeHex === activePropagationNodeHex.value
  ) {
    return "Auto-selected from announced Hub-capable relays.";
  }
  return "Active propagation relay is synced from runtime state.";
});
const {
  RNODE_PROFILE_SPECS,
  devices: rnodeDevices,
  loadPairedRnodeDevices,
  pairedDevices: rnodePairedDevices,
  pairedLoading: rnodePairedLoading,
  pairRnodeViaUsb,
  rnodeDeviceDetail,
  rnodeProfileSummary,
  rnodeUsbDeviceDetail,
  scanFeedback: rnodeScanFeedback,
  scanning: rnodeScanning,
  scanRnodeDevices,
  selectRnodeDevice,
  selectRnodeUsbDevice,
  selectedUsbDeviceId: selectedRnodeUsbDeviceId,
  usbDevices: rnodeUsbDevices,
  usbPairing: rnodeUsbPairing,
} = useSettingsRnode(props.form);

function normalizeTcpEndpoint(value: string): string | undefined {
  const candidate = value.trim().replace(/^tcp:\/\//i, "");
  if (!candidate) return undefined;
  if (candidate.startsWith("[")) {
    const ipv6Match = candidate.match(/^\[[^\]]+\]:(\d{1,5})$/);
    if (!ipv6Match) return undefined;
    const port = Number(ipv6Match[1]);
    return Number.isInteger(port) && port >= 1 && port <= 65_535 ? candidate : undefined;
  }
  const separatorIndex = candidate.lastIndexOf(":");
  if (separatorIndex <= 0 || separatorIndex === candidate.length - 1) return undefined;
  const host = candidate.slice(0, separatorIndex).trim();
  const port = Number(candidate.slice(separatorIndex + 1).trim());
  return host && Number.isInteger(port) && port >= 1 && port <= 65_535
    ? `${host}:${port}`
    : undefined;
}

function toggleKnownTcpEndpoint(endpoint: string, selected: boolean): void {
  const next = new Set(normalizedTcpClients.value);
  if (selected) next.add(endpoint);
  else next.delete(endpoint);
  props.form.tcpClients = [...next];
}

function addCustomTcpEndpoint(): void {
  const normalized = normalizeTcpEndpoint(customTcpEndpoint.value);
  if (!normalized) {
    emit("feedback", "Invalid endpoint. Use host:port, tcp://host:port, or [ipv6]:port.");
    return;
  }
  props.form.tcpClients = [...new Set([...normalizedTcpClients.value, normalized])];
  customTcpEndpoint.value = "";
  emit("feedback", "");
}

function removeTcpEndpoint(endpoint: string): void {
  props.form.tcpClients = normalizedTcpClients.value.filter((entry) => entry !== endpoint);
}

function openPanel(): void {
  if (!panel.value) return;
  panel.value.open = true;
  panel.value.scrollIntoView({ behavior: "smooth", block: "start" });
}

defineExpose({ openPanel });
</script>

<template>
    <details ref="panel" class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <path d="M5 7h10" />
              <path d="M5 17h14" />
              <path d="M15 7a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" transform="translate(0 2)" />
              <path d="M9 17a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" transform="translate(0 2)" />
            </svg>
          </span>
          <h2>Node Config</h2>
          <p>{{ runtimeSummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <div class="grid">
          <label>
            Client mode
            <select v-model="form.clientMode">
              <option value="auto">Auto</option>
              <option value="capacitor">Capacitor only</option>
            </select>
          </label>
          <label>
            Call Sign
            <input v-model="form.displayName" type="text" maxlength="64" />
          </label>
          <label>
            Own app hash
            <input :value="ownAppHash" class="readonly-input" type="text" readonly />
          </label>
          <label>
            Announce capabilities
            <input v-model="form.announceCapabilities" type="text" />
          </label>
          <label>
            Announce interval seconds
            <input v-model.number="form.announceIntervalSeconds" type="number" min="60" />
          </label>
          <label class="checkbox">
            <input v-model="form.broadcast" type="checkbox" />
            Broadcast enabled
          </label>
          <label class="checkbox">
            <input v-model="form.transportNodeEnabled" type="checkbox" />
            Transport node forwarding
          </label>
        </div>

        <section class="config-section" aria-labelledby="tcp-interfaces-heading">
          <div class="config-section-header">
            <h3 id="tcp-interfaces-heading">TCP Interfaces</h3>
            <p>Known community servers and custom host:port endpoints.</p>
          </div>

          <div class="server-list">
            <label
              v-for="server in knownTcpServers"
              :key="server.endpoint"
              class="server-option"
            >
              <input
                type="checkbox"
                :checked="selectedTcpEndpointSet.has(server.endpoint)"
                @change="
                  toggleKnownTcpEndpoint(
                    server.endpoint,
                    ($event.target as HTMLInputElement).checked,
                  )
                "
              />
              <div class="server-option-body">
                <p class="server-name">{{ server.name }}</p>
                <p class="server-endpoint">{{ server.endpoint }}</p>
              </div>
              <span v-if="server.isBootstrap" class="bootstrap-badge">Bootstrap</span>
            </label>
          </div>

          <div class="tcp-custom-row">
            <input
              v-model="customTcpEndpoint"
              type="text"
              placeholder="Add custom endpoint (host:port or tcp://host:port)"
            />
            <button type="button" @click="addCustomTcpEndpoint">Add</button>
          </div>

          <div v-if="normalizedTcpClients.length > 0" class="active-endpoints">
            <article v-for="endpoint in normalizedTcpClients" :key="endpoint" class="active-endpoint">
              <span>{{ endpoint }}</span>
              <button type="button" class="inline-remove" @click="removeTcpEndpoint(endpoint)">
                Remove
              </button>
            </article>
          </div>
          <p v-else class="section-note">No TCP endpoints configured.</p>
        </section>

        <section class="config-section" aria-labelledby="lora-interface-heading">
          <div class="config-section-header">
            <h3 id="lora-interface-heading">LoRa / RNode</h3>
            <p>Paired Android Bluetooth device and REM radio profile.</p>
          </div>

          <div class="grid">
            <label class="checkbox">
              <input v-model="form.rnodeEnabled" type="checkbox" />
              Enable RNode Bluetooth LoRa
            </label>
            <label>
              RNode device id
              <input v-model="form.rnodePeripheralId" type="text" placeholder="Bluetooth address or peripheral id" />
            </label>
            <label>
              RNode display name
              <input v-model="form.rnodeDisplayName" type="text" placeholder="Optional label" />
            </label>
            <label>
              Region
              <select v-model="form.rnodeRegion">
                <option v-for="region in RNODE_REGION_SPECS" :key="region.id" :value="region.id">
                  {{ region.id }} - {{ region.label }}
                </option>
              </select>
            </label>
            <label>
              Frequency (Hz)
              <input
                v-model.number="form.rnodeFrequencyHz"
                type="number"
                min="1"
                step="1000"
                :placeholder="String(rnodeRegionDefaultFrequencyHz(normalizeRnodeRegion(form.rnodeRegion)))"
              />
            </label>
            <label>
              REM LoRa profile
              <select v-model="form.rnodeProfile">
                <option v-for="profile in RNODE_PROFILE_SPECS" :key="profile.id" :value="profile.id">
                  {{ profile.id }} - {{ profile.label }}
                </option>
              </select>
            </label>
            <label>
              Reticulum syntax
              <input :value="rnodeProfileSummary(form.rnodeProfile)" class="readonly-input" type="text" readonly />
            </label>
          </div>

          <div class="tcp-custom-row">
            <button type="button" :disabled="rnodePairedLoading" @click="loadPairedRnodeDevices">
              {{ rnodePairedLoading ? "Loading paired" : "Show paired Bluetooth" }}
            </button>
            <button type="button" :disabled="rnodeScanning" @click="scanRnodeDevices">
              {{ rnodeScanning ? "Scanning" : "Scan RNode BLE" }}
            </button>
            <button type="button" :disabled="rnodeUsbPairing" @click="pairRnodeViaUsb">
              {{ rnodeUsbPairing ? "Pairing via USB" : "Pair via USB" }}
            </button>
          </div>
          <p v-if="rnodeScanFeedback" class="feedback">{{ rnodeScanFeedback }}</p>
          <div v-if="rnodePairedDevices.length > 0" class="server-list">
            <button
              v-for="device in rnodePairedDevices"
              :key="`paired-${device.id}`"
              type="button"
              class="server-option device-option"
              @click="selectRnodeDevice(device)"
            >
              <div class="server-option-body">
                <p class="server-name">{{ device.name || device.address }}</p>
                <p class="server-endpoint">{{ rnodeDeviceDetail(device) }}</p>
              </div>
              <span class="bootstrap-badge">Paired</span>
            </button>
          </div>
          <div v-if="rnodeDevices.length > 0" class="server-list">
            <button
              v-for="device in rnodeDevices"
              :key="device.id"
              type="button"
              class="server-option device-option"
              @click="selectRnodeDevice(device)"
            >
              <div class="server-option-body">
                <p class="server-name">{{ device.name || device.address }}</p>
                <p class="server-endpoint">{{ rnodeDeviceDetail(device) }}</p>
              </div>
              <span class="bootstrap-badge">RNode</span>
            </button>
          </div>
          <div v-if="rnodeUsbDevices.length > 0" class="server-list">
            <button
              v-for="device in rnodeUsbDevices"
              :key="`usb-${device.deviceId}`"
              type="button"
              class="server-option device-option"
              @click="selectRnodeUsbDevice(device)"
            >
              <div class="server-option-body">
                <p class="server-name">{{ device.productName || device.deviceName || `USB ${device.deviceId}` }}</p>
                <p class="server-endpoint">{{ rnodeUsbDeviceDetail(device) }}</p>
              </div>
              <span class="bootstrap-badge">
                {{ selectedRnodeUsbDeviceId === device.deviceId ? "Selected" : "USB" }}
              </span>
            </button>
          </div>
        </section>

        <p class="section-note">
          Save TCP or LoRa changes, then restart REM before validating interface traffic.
          When TCP and LoRa are both active, Reticulum selects the route.
        </p>

        <div class="grid propagation-grid">
          <label>
            Active propagation node
            <input :value="activePropagationNodeLabel" class="readonly-input" type="text" readonly />
          </label>
          <label>
            Selection mode
            <input :value="propagationSelectionSummary" class="readonly-input" type="text" readonly />
          </label>
        </div>

        <p v-if="telemetryStore.telemetryError" class="feedback">{{ telemetryStore.telemetryError }}</p>
      </div>
    </details>
</template>
