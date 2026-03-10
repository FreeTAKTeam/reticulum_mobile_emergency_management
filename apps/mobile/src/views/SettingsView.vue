<script setup lang="ts">
import { computed, reactive, ref } from "vue";

import { copyToClipboard, shareText } from "../services/peerExchange";
import { useNodeStore } from "../stores/nodeStore";
import { useTelemetryStore } from "../stores/telemetryStore";
import { parseCapabilityTokens } from "../utils/peers";
import { TCP_COMMUNITY_SERVERS, toTcpEndpoint } from "../utils/tcpCommunityServers";

interface KnownTcpServerOption {
  name: string;
  endpoint: string;
  isBootstrap: boolean;
}

interface HubAnnounceCandidate {
  destination: string;
  label: string;
}

const nodeStore = useNodeStore();
const telemetryStore = useTelemetryStore();
telemetryStore.init();

const form = reactive({
  displayName: nodeStore.settings.displayName,
  clientMode: nodeStore.settings.clientMode,
  autoConnectSaved: nodeStore.settings.autoConnectSaved,
  showOnlyCapabilityVerified: nodeStore.settings.showOnlyCapabilityVerified,
  announceCapabilities: nodeStore.settings.announceCapabilities,
  announceIntervalSeconds: nodeStore.settings.announceIntervalSeconds,
  tcpClients: [...nodeStore.settings.tcpClients],
  broadcast: nodeStore.settings.broadcast,
  telemetryEnabled: nodeStore.settings.telemetry.enabled,
  telemetryPublishIntervalSeconds: nodeStore.settings.telemetry.publishIntervalSeconds,
  telemetryAccuracyThresholdMeters: nodeStore.settings.telemetry.accuracyThresholdMeters,
  hubMode: nodeStore.settings.hub.mode,
  hubIdentityHash: nodeStore.settings.hub.identityHash,
  hubApiBaseUrl: nodeStore.settings.hub.apiBaseUrl,
  hubApiKey: nodeStore.settings.hub.apiKey,
  hubRefreshIntervalSeconds: nodeStore.settings.hub.refreshIntervalSeconds,
});

const importText = ref("");
const importMode = ref<"merge" | "replace">("merge");
const importFeedback = ref("");
const runtimeFeedback = ref("");
const customTcpEndpoint = ref("");

const ownAppHash = computed(() => nodeStore.status.appDestinationHex || "Start node to populate");
const showLegacyHubHttpFields = computed(() => form.hubMode === "RchHttp");

const knownTcpServers = computed<KnownTcpServerOption[]>(() =>
  TCP_COMMUNITY_SERVERS.map((server) => ({
    name: server.name,
    endpoint: toTcpEndpoint(server),
    isBootstrap: Boolean(server.isBootstrap),
  })),
);

const normalizedTcpClients = computed(() =>
  [
    ...new Set(
      form.tcpClients
        .map((entry: string) => entry.trim())
        .filter((entry) => entry.length > 0),
    ),
  ],
);

const selectedTcpEndpointSet = computed(() => new Set(normalizedTcpClients.value));

const runtimeSummary = computed(() => {
  const endpointCount = normalizedTcpClients.value.length;
  const endpointLabel = endpointCount === 1 ? "endpoint" : "endpoints";
  const telemetrySummary = form.telemetryEnabled
    ? `telemetry every ${form.telemetryPublishIntervalSeconds}s`
    : "telemetry off";
  return `${form.clientMode} mode | ${endpointCount} TCP ${endpointLabel} | ${telemetrySummary}`;
});

function peerExposesHubCapability(appData: string): boolean {
  const tokens = parseCapabilityTokens(appData);
  return tokens.some((token) => token === "hub" || token.endsWith("hub"));
}

const hubAnnounceCandidates = computed<HubAnnounceCandidate[]>(() =>
  Object.values(nodeStore.discoveredByDestination)
    .filter((peer) => peer.sources.includes("announce"))
    .filter((peer) => peerExposesHubCapability(peer.appData ?? ""))
    .map((peer) => ({
      destination: peer.destination,
      label: peer.announcedName || peer.label || peer.destination,
    }))
    .sort((a, b) => {
      const byLabel = a.label.localeCompare(b.label);
      if (byLabel !== 0) {
        return byLabel;
      }
      return a.destination.localeCompare(b.destination);
    }),
);

const hubSummary = computed(() => {
  if (!form.hubIdentityHash) {
    return `${form.hubMode} | No hub selected`;
  }
  return `${form.hubMode} | ${form.hubIdentityHash.slice(0, 10)}...`;
});

const peerListSummary = computed(() => `${nodeStore.savedPeers.length} saved peers`);
const nodeControlSummary = computed(() =>
  nodeStore.status.running ? "Node is running" : "Node is stopped",
);

const telemetryStatusText = computed(() => {
  if (!form.telemetryEnabled) {
    return "Disabled";
  }
  if (telemetryStore.loopStatus === "permission_denied") {
    return "Permission denied";
  }
  if (telemetryStore.loopStatus === "gps_unavailable") {
    return "GPS unavailable";
  }
  if (telemetryStore.loopStatus === "running") {
    return "Publishing";
  }
  return "Idle";
});

function normalizeTcpEndpoint(value: string): string | undefined {
  const candidate = value.trim();
  if (!candidate) {
    return undefined;
  }

  if (candidate.startsWith("[")) {
    const ipv6Match = candidate.match(/^\[[^\]]+\]:(\d{1,5})$/);
    if (!ipv6Match) {
      return undefined;
    }
    const port = Number(ipv6Match[1]);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      return undefined;
    }
    return candidate;
  }

  const separatorIndex = candidate.lastIndexOf(":");
  if (separatorIndex <= 0 || separatorIndex === candidate.length - 1) {
    return undefined;
  }

  const host = candidate.slice(0, separatorIndex).trim();
  const portText = candidate.slice(separatorIndex + 1).trim();
  const port = Number(portText);
  if (!host || !Number.isInteger(port) || port < 1 || port > 65535) {
    return undefined;
  }

  return `${host}:${port}`;
}

function toggleKnownTcpEndpoint(endpoint: string, selected: boolean): void {
  const next = new Set(normalizedTcpClients.value);
  if (selected) {
    next.add(endpoint);
  } else {
    next.delete(endpoint);
  }
  form.tcpClients = [...next];
}

function addCustomTcpEndpoint(): void {
  const normalized = normalizeTcpEndpoint(customTcpEndpoint.value);
  if (!normalized) {
    runtimeFeedback.value = "Invalid endpoint. Use host:port or [ipv6]:port.";
    return;
  }
  const next = new Set(normalizedTcpClients.value);
  next.add(normalized);
  form.tcpClients = [...next];
  customTcpEndpoint.value = "";
  runtimeFeedback.value = "";
}

function removeTcpEndpoint(endpoint: string): void {
  form.tcpClients = normalizedTcpClients.value.filter((entry) => entry !== endpoint);
}

function onHubCandidateSelected(event: Event): void {
  const value = (event.target as HTMLSelectElement).value;
  form.hubIdentityHash = value.trim();
}

function applySettings(): void {
  const previousDisplayName = nodeStore.settings.displayName;
  nodeStore.updateSettings({
    displayName: form.displayName,
    clientMode: form.clientMode,
    autoConnectSaved: form.autoConnectSaved,
    showOnlyCapabilityVerified: form.showOnlyCapabilityVerified,
    announceCapabilities: form.announceCapabilities.trim(),
    announceIntervalSeconds: Math.max(5, Number(form.announceIntervalSeconds || 1800)),
    tcpClients: normalizedTcpClients.value,
    broadcast: form.broadcast,
    telemetry: {
      enabled: form.telemetryEnabled,
      publishIntervalSeconds: Math.min(60, Math.max(5, Number(form.telemetryPublishIntervalSeconds || 10))),
      accuracyThresholdMeters:
        form.telemetryAccuracyThresholdMeters === undefined || form.telemetryAccuracyThresholdMeters === null || form.telemetryAccuracyThresholdMeters === 0
          ? undefined
          : Math.max(1, Number(form.telemetryAccuracyThresholdMeters)),
    },
    hub: {
      mode: form.hubMode,
      identityHash: form.hubIdentityHash.trim(),
      apiBaseUrl: form.hubApiBaseUrl.trim(),
      apiKey: form.hubApiKey.trim(),
      refreshIntervalSeconds: Math.max(30, Number(form.hubRefreshIntervalSeconds || 3600)),
    },
  });
  form.displayName = nodeStore.settings.displayName;
  form.tcpClients = [...nodeStore.settings.tcpClients];
  form.telemetryPublishIntervalSeconds = nodeStore.settings.telemetry.publishIntervalSeconds;
  form.telemetryAccuracyThresholdMeters = nodeStore.settings.telemetry.accuracyThresholdMeters;
  runtimeFeedback.value =
    nodeStore.settings.displayName !== previousDisplayName
      ? "Settings saved. Restart the node to announce the updated call sign."
      : "Settings saved.";
}

async function runNodeAction(
  action: () => Promise<void>,
  successMessage: string,
): Promise<void> {
  try {
    await action();
    runtimeFeedback.value = successMessage;
  } catch (error: unknown) {
    runtimeFeedback.value = error instanceof Error ? error.message : String(error);
  }
}

async function exportPeerList(): Promise<void> {
  try {
    const payload = JSON.stringify(nodeStore.getSavedPeerList(), null, 2);
    await copyToClipboard(payload);
    await shareText("PeerListV1", payload);
    importFeedback.value = "Peer list exported to clipboard/share.";
  } catch (error: unknown) {
    importFeedback.value = error instanceof Error ? error.message : String(error);
  }
}

function importPeerList(): void {
  try {
    const parsed = nodeStore.parsePeerListText(importText.value);
    nodeStore.importPeerList(parsed.peerList, importMode.value);
    importFeedback.value = `Imported ${parsed.peerList.peers.length} peers (${importMode.value}).`;
    if (parsed.warnings.length > 0) {
      importFeedback.value += ` Warnings: ${parsed.warnings.join(" ")}`;
    }
  } catch (error) {
    importFeedback.value = String(error);
  }
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div>
        <h1>Settings</h1>
        <p>Node runtime, discovery filters, and directory source controls.</p>
      </div>
      <div class="header-actions">
        <span class="badge">{{ nodeStore.status.running ? "Node Active" : "Node Offline" }}</span>
      </div>
    </header>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div>
          <h2>Runtime</h2>
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
            <input v-model.number="form.announceIntervalSeconds" type="number" min="5" />
          </label>
          <label class="checkbox">
            <input v-model="form.autoConnectSaved" type="checkbox" />
            Auto connect saved peers on startup
          </label>
          <label class="checkbox">
            <input v-model="form.broadcast" type="checkbox" />
            Broadcast enabled
          </label>
          <label class="checkbox">
            <input v-model="form.telemetryEnabled" type="checkbox" />
            Enable telemetry sharing
          </label>
          <label>
            Telemetry publish interval (seconds)
            <input v-model.number="form.telemetryPublishIntervalSeconds" type="number" min="5" max="60" />
          </label>
          <label>
            Telemetry accuracy threshold (meters, optional)
            <input v-model.number="form.telemetryAccuracyThresholdMeters" type="number" min="0" placeholder="Unset" />
          </label>
          <label class="full">
            Telemetry status
            <input :value="telemetryStatusText" class="readonly-input" type="text" readonly />
          </label>
          <label class="checkbox">
            <input v-model="form.showOnlyCapabilityVerified" type="checkbox" />
            Show capability-verified peers by default
          </label>
        </div>

        <p class="section-note">
          TCP interfaces: choose from known community servers (Columba list) or add custom
          host:port endpoints.
        </p>

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
            placeholder="Add custom endpoint (host:port)"
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

        <div class="actions">
          <button type="button" @click="applySettings">Save</button>
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.reinitializeClient(), 'Node client recreated.')"
          >
            Recreate Client
          </button>
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.restartNode(), 'Node restarted.')"
          >
            Restart Node
          </button>
        </div>
        <p v-if="telemetryStore.telemetryError" class="feedback">{{ telemetryStore.telemetryError }}</p>
      </div>
    </details>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div>
          <h2>RCH Hub Directory</h2>
          <p>{{ hubSummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <p class="section-note">
          Uses Reticulum LXMF and the RCH <code>ListClients</code> command to fetch the active
          client list.
        </p>

        <div class="grid">
          <label>
            Mode
            <select v-model="form.hubMode">
              <option value="Disabled">Disabled</option>
              <option value="RchLxmf">RCH via Reticulum (LXMF)</option>
              <option value="RchHttp">Legacy HTTP (deprecated)</option>
            </select>
          </label>
          <label>
            Hub from announces (Hub capability)
            <select :value="form.hubIdentityHash" @change="onHubCandidateSelected">
              <option value="">Manual / none</option>
              <option
                v-for="candidate in hubAnnounceCandidates"
                :key="candidate.destination"
                :value="candidate.destination"
              >
                {{ candidate.label }} ({{ candidate.destination.slice(0, 10) }}...)
              </option>
            </select>
          </label>
          <label>
            Hub identity hash
            <input v-model="form.hubIdentityHash" type="text" />
          </label>
          <label v-if="showLegacyHubHttpFields">
            Legacy hub API base URL
            <input v-model="form.hubApiBaseUrl" type="url" />
          </label>
          <label v-if="showLegacyHubHttpFields">
            Legacy hub API key
            <input v-model="form.hubApiKey" type="text" />
          </label>
          <label>
            Refresh interval seconds
            <input v-model.number="form.hubRefreshIntervalSeconds" type="number" min="30" />
          </label>
        </div>

        <p v-if="hubAnnounceCandidates.length === 0" class="section-note">
          No announce entries exposing Hub capability have been seen yet.
        </p>

        <div class="actions">
          <button type="button" @click="applySettings">Save Hub Settings</button>
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.refreshHubDirectory(), 'Hub refresh requested.')"
          >
            Refresh Now
          </button>
        </div>
      </div>
    </details>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div>
          <h2>Peer List Exchange (PeerListV1)</h2>
          <p>{{ peerListSummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <div class="actions">
          <button type="button" @click="exportPeerList">Export + Share</button>
        </div>
        <label class="full">
          Import JSON
          <textarea v-model="importText" rows="7"></textarea>
        </label>
        <div class="actions">
          <label class="radio">
            <input v-model="importMode" type="radio" value="merge" />
            Merge
          </label>
          <label class="radio">
            <input v-model="importMode" type="radio" value="replace" />
            Replace
          </label>
          <button type="button" @click="importPeerList">Import</button>
        </div>
        <p v-if="importFeedback" class="feedback">{{ importFeedback }}</p>
      </div>
    </details>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div>
          <h2>Node Control</h2>
          <p>{{ nodeControlSummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <div class="actions">
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.startNode(), 'Node started.')"
          >
            Start
          </button>
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.stopNode(), 'Node stopped.')"
          >
            Stop
          </button>
        </div>
        <p v-if="runtimeFeedback" class="feedback">{{ runtimeFeedback }}</p>
        <p v-if="nodeStore.lastError" class="feedback">{{ nodeStore.lastError }}</p>
        <div class="log-list">
          <p v-for="entry in nodeStore.logs" :key="entry.at" class="log">
            {{ entry.level }} | {{ entry.message }}
          </p>
        </div>
      </div>
    </details>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

.view-header {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.header-actions {
  align-items: center;
  display: flex;
  gap: 0.55rem;
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.4rem, 3vw, 2.4rem);
  line-height: 1;
  margin: 0;
}

.view-header p {
  color: #9cb3d6;
  font-family: var(--font-body);
  font-size: clamp(0.95rem, 1.4vw, 1.15rem);
  margin: 0.2rem 0 0;
}

.badge {
  background: rgb(9 61 108 / 68%);
  border: 1px solid rgb(73 173 255 / 62%);
  border-radius: 999px;
  color: #64beff;
  font-family: var(--font-ui);
  font-size: 0.8rem;
  letter-spacing: 0.08em;
  padding: 0.42rem 0.75rem;
  text-transform: uppercase;
}

.panel {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 92%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 14%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 33%);
  border-radius: 16px;
}

.fold-panel {
  overflow: hidden;
}

.panel-summary {
  align-items: center;
  cursor: pointer;
  display: flex;
  justify-content: space-between;
  list-style: none;
  padding: 0.9rem;
}

.panel-summary::-webkit-details-marker {
  display: none;
}

.panel-summary h2 {
  font-family: var(--font-headline);
  font-size: 1.3rem;
  margin: 0;
}

.panel-summary p {
  color: #90a9d2;
  font-family: var(--font-body);
  margin: 0.2rem 0 0;
}

.chevron {
  color: #8fd9ff;
  font-size: 0.85rem;
  transition: transform 0.2s ease;
}

.fold-panel[open] .chevron {
  transform: rotate(180deg);
}

.panel-body {
  border-top: 1px solid rgb(69 107 168 / 33%);
  padding: 0.85rem 0.9rem 0.95rem;
}

.section-note {
  color: #90aad4;
  font-family: var(--font-body);
  margin: 0.65rem 0 0.8rem;
}

.grid {
  display: grid;
  gap: 0.6rem;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
}

label {
  color: #a0b7db;
  display: grid;
  font-family: var(--font-body);
  font-size: 0.88rem;
  gap: 0.3rem;
}

input,
textarea,
select {
  background: rgb(6 17 38 / 82%);
  border: 1px solid rgb(70 110 174 / 42%);
  border-radius: 10px;
  color: #daecff;
  font-family: var(--font-body);
  font-size: 0.95rem;
  padding: 0.48rem 0.56rem;
}

.readonly-input {
  color: #89d8ff;
}

textarea {
  resize: vertical;
}

.checkbox {
  align-items: center;
  gap: 0.45rem;
  grid-template-columns: auto 1fr;
}

.radio {
  align-items: center;
  display: flex;
  gap: 0.35rem;
}

.full {
  margin-top: 0.65rem;
}

.server-list {
  display: grid;
  gap: 0.45rem;
  max-height: 15rem;
  overflow-y: auto;
  padding-right: 0.2rem;
  scrollbar-gutter: stable;
}

.server-option {
  align-items: center;
  background: rgb(9 24 50 / 70%);
  border: 1px solid rgb(71 112 176 / 29%);
  border-radius: 11px;
  display: grid;
  gap: 0.45rem;
  grid-template-columns: auto 1fr auto;
  margin: 0;
  padding: 0.55rem 0.65rem;
}

.server-option-body {
  display: grid;
  gap: 0.1rem;
}

.server-name {
  color: #d5eaff;
  font-family: var(--font-ui);
  font-size: 0.84rem;
  letter-spacing: 0.05em;
  margin: 0;
}

.server-endpoint {
  color: #89a8d4;
  font-family: var(--font-body);
  font-size: 0.82rem;
  margin: 0;
  overflow-wrap: anywhere;
}

.bootstrap-badge {
  background: rgb(13 120 195 / 38%);
  border: 1px solid rgb(95 193 255 / 45%);
  border-radius: 999px;
  color: #8fe3ff;
  font-family: var(--font-ui);
  font-size: 0.65rem;
  letter-spacing: 0.07em;
  padding: 0.2rem 0.45rem;
  text-transform: uppercase;
}

.tcp-custom-row {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.65rem;
}

.tcp-custom-row input {
  flex: 1;
}

.active-endpoints {
  display: grid;
  gap: 0.4rem;
  margin-top: 0.65rem;
}

.active-endpoint {
  align-items: center;
  background: rgb(7 20 44 / 80%);
  border: 1px solid rgb(67 106 165 / 35%);
  border-radius: 10px;
  color: #d5eaff;
  display: flex;
  font-family: var(--font-ui);
  font-size: 0.82rem;
  justify-content: space-between;
  letter-spacing: 0.03em;
  padding: 0.44rem 0.58rem;
}

.inline-remove {
  background: rgb(8 27 58 / 86%);
  border: 1px solid rgb(74 133 207 / 45%);
  border-radius: 8px;
  color: #8fdbff;
  font-size: 0.7rem;
  min-height: 26px;
  padding: 0 0.55rem;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin-top: 0.75rem;
}

button {
  background: linear-gradient(118deg, #0b9fff, #20ecff);
  border: 0;
  border-radius: 10px;
  color: #03284b;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 34px;
  padding: 0 0.76rem;
  text-transform: uppercase;
}

.feedback {
  color: #96afd5;
  font-family: var(--font-body);
  margin: 0.58rem 0 0;
}

.log-list {
  background: rgb(5 16 35 / 76%);
  border: 1px solid rgb(68 105 164 / 28%);
  border-radius: 12px;
  margin-top: 0.55rem;
  max-height: 13rem;
  overflow-y: auto;
  padding: 0.35rem 0.65rem 0.55rem;
  scrollbar-gutter: stable;
}

.log {
  color: #88a4d0;
  font-family: var(--font-body);
  margin: 0.28rem 0 0;
  overflow-wrap: anywhere;
}

@media (max-width: 760px) {
  .view-header {
    align-items: flex-start;
    flex-direction: column;
    gap: 0.65rem;
  }

  .header-actions {
    align-self: stretch;
    justify-content: flex-end;
  }

  .server-option {
    grid-template-columns: auto 1fr;
  }

  .bootstrap-badge {
    justify-self: start;
    margin-left: 1.55rem;
  }
}
</style>
