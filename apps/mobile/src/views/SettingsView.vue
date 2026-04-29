<script setup lang="ts">
import { computed, reactive, ref, useTemplateRef } from "vue";
import { useRouter } from "vue-router";

import PluginSettingsSection from "../components/plugins/PluginSettingsSection.vue";
import SosEmergencyCard from "../components/sos/SosEmergencyCard.vue";
import {
  listPluginSettingsSections,
  loadPluginSettingsValues,
  savePluginSettingsValues,
  type PluginSettingsValues,
} from "../plugins/pluginSettings";
import { copyToClipboard, shareText } from "../services/peerExchange";
import { useNodeStore } from "../stores/nodeStore";
import { useTelemetryStore } from "../stores/telemetryStore";
import { ensureRequiredAnnounceCapabilities } from "../utils/peers";
import { TCP_COMMUNITY_SERVERS, toTcpEndpoint } from "../utils/tcpCommunityServers";

interface KnownTcpServerOption {
  name: string;
  endpoint: string;
  isBootstrap: boolean;
}

const nodeStore = useNodeStore();
const router = useRouter();
const telemetryStore = useTelemetryStore();
const sosCardRef = useTemplateRef<{
  saveSettings: () => Promise<void>;
  hasUnsavedChanges: () => boolean;
}>("sosCard");
const appVersion = import.meta.env.VITE_APP_VERSION ?? "0.0.0";
const savingSettings = ref(false);

const aboutItems = [
  {
    label: "Application name",
    value: "R.E.M. (Reticulum Emergency Manager)",
  },
  {
    label: "Description",
    value: "Emergency coordination, messages, events, and telemetry over Reticulum mesh networks.",
  },
  {
    label: "Version",
    value: appVersion,
  },
  {
    label: "License",
    value: "Eclipse Public License (EPL)",
  },
] as const;
const repositoryUrl = "https://github.com/FreeTAKTeam/reticulum_mobile_emergency_management";

const form = reactive({
  displayName: nodeStore.settings.displayName,
  clientMode: nodeStore.settings.clientMode,
  autoConnectSaved: nodeStore.settings.autoConnectSaved,
  announceCapabilities: ensureRequiredAnnounceCapabilities(nodeStore.settings.announceCapabilities),
  announceIntervalSeconds: nodeStore.settings.announceIntervalSeconds,
  tcpClients: [...nodeStore.settings.tcpClients],
  broadcast: nodeStore.settings.broadcast,
  telemetryEnabled: nodeStore.settings.telemetry.enabled,
  telemetryPublishIntervalSeconds: nodeStore.settings.telemetry.publishIntervalSeconds,
  telemetryAccuracyThresholdMeters: nodeStore.settings.telemetry.accuracyThresholdMeters,
  telemetryStaleAfterMinutes: nodeStore.settings.telemetry.staleAfterMinutes,
  telemetryExpireAfterMinutes: nodeStore.settings.telemetry.expireAfterMinutes,
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
const peerListFileInput = useTemplateRef<HTMLInputElement>("peerListFileInput");

const ownAppHash = computed(() => nodeStore.status.appDestinationHex || "Start node to populate");

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
const activePropagationNodeHex = computed(
  () => nodeStore.syncStatus.activePropagationNodeHex?.trim() ?? "",
);

const runtimeSummary = computed(() => {
  const endpointCount = normalizedTcpClients.value.length;
  const endpointLabel = endpointCount === 1 ? "endpoint" : "endpoints";
  return `${form.clientMode} mode | ${endpointCount} TCP ${endpointLabel}`;
});

const hubAnnounceCandidates = computed(() => nodeStore.hubAnnounceCandidates);

const hubSummary = computed(() => {
  const cachedPeerCount = nodeStore.hubDirectoryPeers.length;
  const connectedOverride =
    form.hubMode === "SemiAutonomous" && nodeStore.effectiveConnectedMode
      ? " | server forcing connected routing"
      : "";
  if (!form.hubIdentityHash) {
    if (form.hubMode === "Connected") {
      return `${form.hubMode} | No hub selected | outbound blocked`;
    }
    if (form.hubMode === "SemiAutonomous") {
      return `${form.hubMode} | No hub selected | using local discovery until a hub is chosen${connectedOverride}`;
    }
    return `${form.hubMode} | No hub selected${connectedOverride}`;
  }
  const peerSummary = cachedPeerCount > 0 ? ` | ${cachedPeerCount} cached peers` : "";
  return `${form.hubMode} | ${form.hubIdentityHash.slice(0, 10)}...${peerSummary}${connectedOverride}`;
});
const hubRegistrationSummary = computed(() => nodeStore.hubRegistrationSummary);
const peerListSummary = computed(() => `${nodeStore.savedPeers.length} saved peers`);
const nodeControlSummary = computed(() =>
  nodeStore.status.running ? "Node is running" : "Node is stopped",
);
const pluginSettingsSections = listPluginSettingsSections();
const pluginSettingsValues = reactive<Record<string, PluginSettingsValues>>(
  Object.fromEntries(
    pluginSettingsSections.map((section) => [
      section.pluginId,
      loadPluginSettingsValues(section),
    ]),
  ),
);
const pluginSettingsFeedback = ref("");
const pluginSettingsSummary = computed(() => {
  const pluginCount = pluginSettingsSections.length;
  if (pluginCount === 0) {
    return "No installed plug-ins";
  }
  return pluginCount === 1 ? "1 plug-in configurable" : `${pluginCount} plug-ins configurable`;
});
const activePropagationNodeLabel = computed(() => {
  if (!activePropagationNodeHex.value) {
    return "None";
  }

  const discoveredPeer = nodeStore.discoveredByDestination[activePropagationNodeHex.value];
  if (discoveredPeer) {
    return discoveredPeer.announcedName || discoveredPeer.label || activePropagationNodeHex.value;
  }

  const savedPeer = nodeStore.savedByDestination[activePropagationNodeHex.value];
  return savedPeer?.label || activePropagationNodeHex.value;
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

const telemetrySummary = computed(() => {
  if (!form.telemetryEnabled) {
    return "Disabled";
  }

  return `${telemetryStatusText.value} | every ${form.telemetryPublishIntervalSeconds}s`;
});

const persistedTcpClients = computed(() =>
  [
    ...new Set(
      nodeStore.settings.tcpClients
        .map((entry: string) => entry.trim())
        .filter((entry: string) => entry.length > 0),
    ),
  ],
);

const hasMainSettingsChanges = computed(() =>
  form.displayName !== nodeStore.settings.displayName
  || form.clientMode !== nodeStore.settings.clientMode
  || form.autoConnectSaved !== nodeStore.settings.autoConnectSaved
  || ensureRequiredAnnounceCapabilities(form.announceCapabilities.trim()) !== nodeStore.settings.announceCapabilities
  || Math.max(5, Number(form.announceIntervalSeconds || 1800)) !== nodeStore.settings.announceIntervalSeconds
  || form.broadcast !== nodeStore.settings.broadcast
  || JSON.stringify(normalizedTcpClients.value) !== JSON.stringify(persistedTcpClients.value)
  || form.telemetryEnabled !== nodeStore.settings.telemetry.enabled
  || Math.min(60, Math.max(5, Number(form.telemetryPublishIntervalSeconds || 60)))
    !== nodeStore.settings.telemetry.publishIntervalSeconds
  || (
    form.telemetryAccuracyThresholdMeters === undefined || form.telemetryAccuracyThresholdMeters === null || form.telemetryAccuracyThresholdMeters === 0
      ? undefined
      : Math.max(1, Number(form.telemetryAccuracyThresholdMeters))
  ) !== nodeStore.settings.telemetry.accuracyThresholdMeters
  || Math.max(1, Number(form.telemetryStaleAfterMinutes || 30))
    !== nodeStore.settings.telemetry.staleAfterMinutes
  || Math.max(
    Math.max(1, Number(form.telemetryStaleAfterMinutes || 30)),
    Number(form.telemetryExpireAfterMinutes || 180),
  ) !== nodeStore.settings.telemetry.expireAfterMinutes
  || form.hubMode !== nodeStore.settings.hub.mode
  || form.hubIdentityHash.trim() !== nodeStore.settings.hub.identityHash
  || form.hubApiBaseUrl.trim() !== nodeStore.settings.hub.apiBaseUrl
  || form.hubApiKey.trim() !== nodeStore.settings.hub.apiKey
  || Math.max(30, Number(form.hubRefreshIntervalSeconds || 3600))
    !== nodeStore.settings.hub.refreshIntervalSeconds,
);

const hasUnsavedSettings = computed(
  () => hasMainSettingsChanges.value || Boolean(sosCardRef.value?.hasUnsavedChanges()),
);
const unsavedSettingsCount = computed(() =>
  Number(hasMainSettingsChanges.value) + Number(Boolean(sosCardRef.value?.hasUnsavedChanges())),
);

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

async function applySettings(): Promise<void> {
  if (!hasUnsavedSettings.value || savingSettings.value) {
    return;
  }
  const previousDisplayName = nodeStore.settings.displayName;
  const previousHubMode = nodeStore.settings.hub.mode;
  const previousHubIdentityHash = nodeStore.settings.hub.identityHash;
  savingSettings.value = true;
  try {
    nodeStore.updateSettings({
      displayName: form.displayName,
      clientMode: form.clientMode,
      autoConnectSaved: form.autoConnectSaved,
      announceCapabilities: ensureRequiredAnnounceCapabilities(form.announceCapabilities.trim()),
      announceIntervalSeconds: Math.max(5, Number(form.announceIntervalSeconds || 1800)),
      tcpClients: normalizedTcpClients.value,
      broadcast: form.broadcast,
      telemetry: {
        enabled: form.telemetryEnabled,
        publishIntervalSeconds: Math.min(60, Math.max(5, Number(form.telemetryPublishIntervalSeconds || 60))),
        accuracyThresholdMeters:
          form.telemetryAccuracyThresholdMeters === undefined || form.telemetryAccuracyThresholdMeters === null || form.telemetryAccuracyThresholdMeters === 0
            ? undefined
            : Math.max(1, Number(form.telemetryAccuracyThresholdMeters)),
        staleAfterMinutes: Math.max(1, Number(form.telemetryStaleAfterMinutes || 30)),
        expireAfterMinutes: Math.max(
          Math.max(1, Number(form.telemetryStaleAfterMinutes || 30)),
          Number(form.telemetryExpireAfterMinutes || 180),
        ),
      },
      hub: {
        mode: form.hubMode,
        identityHash: form.hubIdentityHash.trim(),
        apiBaseUrl: form.hubApiBaseUrl.trim(),
        apiKey: form.hubApiKey.trim(),
        refreshIntervalSeconds: Math.max(30, Number(form.hubRefreshIntervalSeconds || 3600)),
      },
    });
    await sosCardRef.value?.saveSettings();
  } catch (error: unknown) {
    runtimeFeedback.value = error instanceof Error ? error.message : String(error);
    return;
  } finally {
    savingSettings.value = false;
  }

  form.displayName = nodeStore.settings.displayName;
  form.announceCapabilities = nodeStore.settings.announceCapabilities;
  form.tcpClients = [...nodeStore.settings.tcpClients];
  form.telemetryPublishIntervalSeconds = nodeStore.settings.telemetry.publishIntervalSeconds;
  form.telemetryAccuracyThresholdMeters = nodeStore.settings.telemetry.accuracyThresholdMeters;
  form.telemetryStaleAfterMinutes = nodeStore.settings.telemetry.staleAfterMinutes;
  form.telemetryExpireAfterMinutes = nodeStore.settings.telemetry.expireAfterMinutes;
  runtimeFeedback.value =
    nodeStore.settings.displayName !== previousDisplayName
      ? "Settings saved. Restart the node to announce the updated call sign."
      : nodeStore.status.running
          && (
            nodeStore.settings.hub.mode !== previousHubMode
            || nodeStore.settings.hub.identityHash !== previousHubIdentityHash
          )
        ? "Hub settings saved. Restart the node to apply updated hub routing."
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

function openPeerListFilePicker(): void {
  peerListFileInput.value?.click();
}

async function runSetupWizard(): Promise<void> {
  await router.push({
    path: "/setup",
    query: { source: "settings" },
  });
}

async function onPeerListFileSelected(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) {
    return;
  }
  importText.value = await file.text();
}

function updatePluginSettings(pluginId: string, values: PluginSettingsValues): void {
  pluginSettingsValues[pluginId] = values;
  pluginSettingsFeedback.value = "";
}

function savePluginSettings(pluginId: string): void {
  const section = pluginSettingsSections.find((entry) => entry.pluginId === pluginId);
  if (!section) {
    pluginSettingsFeedback.value = "Plug-in settings section is no longer registered.";
    return;
  }
  savePluginSettingsValues(pluginId, pluginSettingsValues[pluginId] ?? {});
  pluginSettingsFeedback.value = `${section.name} settings saved.`;
}
</script>

<template>
  <section class="view">
    <header class="view-header">
      <div class="header-actions">
        <span class="settings-chip unsaved-chip">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M9 4h6" />
            <path d="M9 4a2 2 0 0 0-2 2H5a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-2a2 2 0 0 0-2-2" />
            <path d="M8 11h8" />
            <path d="M8 15h6" />
          </svg>
          <span>Unsaved: {{ unsavedSettingsCount }}</span>
        </span>
        <button type="button" class="settings-chip node-control-chip" aria-label="Node Control">
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5Z" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 8.92 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82 1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z" />
          </svg>
          <span>Node Control</span>
          <svg class="chevron" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="m7 10 5 5 5-5" />
          </svg>
        </button>
        <button
          type="button"
          class="settings-chip setup-chip"
          aria-label="Run setup wizard"
          @click="runSetupWizard"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 3v5" />
            <path d="M12 16v5" />
            <path d="M4 12h5" />
            <path d="M15 12h5" />
            <path d="M7.8 7.8l2.2 2.2" />
            <path d="M14 14l2.2 2.2" />
            <path d="M16.2 7.8 14 10" />
            <path d="M10 14l-2.2 2.2" />
          </svg>
          <span>Setup Wizard</span>
        </button>
        <button
          type="button"
          class="settings-save"
          :disabled="!hasUnsavedSettings || savingSettings"
          @click="applySettings"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M5 4h12l2 2v14H5V4Z" />
            <path d="M8 4v6h8V4" />
            <path d="M8 20v-6h8v6" />
          </svg>
          <span>{{ savingSettings ? "Saving" : "Save" }}</span>
        </button>
      </div>
    </header>

    <details class="panel fold-panel">
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

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <path
                d="M12 20.5s5-4.7 5-9.1a5 5 0 1 0-10 0c0 4.4 5 9.1 5 9.1Z"
              />
              <path d="M12 13.2a1.9 1.9 0 1 0 0-3.8 1.9 1.9 0 0 0 0 3.8Z" />
            </svg>
          </span>
          <h2>Telemetry</h2>
          <p>{{ telemetrySummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <div class="grid">
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
            <input
              v-model.number="form.telemetryAccuracyThresholdMeters"
              type="number"
              min="0"
              placeholder="Unset"
            />
          </label>
          <label>
            Telemetry goes stale after (minutes)
            <input v-model.number="form.telemetryStaleAfterMinutes" type="number" min="1" />
          </label>
          <label>
            Telemetry disappears after (minutes)
            <input v-model.number="form.telemetryExpireAfterMinutes" type="number" min="1" />
          </label>
          <label>
            Telemetry status
            <input :value="telemetryStatusText" class="readonly-input" type="text" readonly />
          </label>
        </div>
      </div>
    </details>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <path d="M12 3.5a7 7 0 1 0 7 7" />
              <path d="M12 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z" />
              <path d="M15.7 4.2l4.1.1-.1 4.1" />
              <path d="M19.7 4.3l-5.1 5.1" />
            </svg>
          </span>
          <h2>RCH Hub Directory</h2>
          <p>{{ hubSummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <p class="section-note">
          Autonomous keeps REM peer discovery local. Semi-autonomous seeds peer routing from the
          selected RCH via <code>rem.registry.peers.list</code> and still sends directly to those
          peers. Connected sends only to the selected RCH so the hub redistributes traffic.
        </p>

        <div class="grid">
          <label>
            Mode
            <select v-model="form.hubMode">
              <option value="Autonomous">Autonomous</option>
              <option value="SemiAutonomous">Semi-autonomous</option>
              <option value="Connected">Connected</option>
            </select>
          </label>
          <label>
            Hub from announces (RCH servers)
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
          <label>
            Refresh interval seconds
            <input v-model.number="form.hubRefreshIntervalSeconds" type="number" min="30" />
          </label>
        </div>

        <p v-if="hubAnnounceCandidates.length === 0" class="section-note">
          No announce entries advertising the RCH server capability set have been seen yet.
        </p>
        <p class="section-note">
          Hub registration: {{ hubRegistrationSummary }}
        </p>

        <div class="actions">
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.refreshHubDirectory(), 'Hub refresh requested.')"
          >
            Refresh Now
          </button>
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.bootstrapHubRegistration(true), 'Hub registration requested.')"
          >
            Register Team Member
          </button>
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.forgetHubRegistryLinkage(), 'Hub registration cleared.')"
          >
            Clear Registration
          </button>
        </div>
      </div>
    </details>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <path d="M12 5v4" />
              <path d="M12 15v4" />
              <path d="M5 12h4" />
              <path d="M15 12h4" />
              <path d="M7.8 7.8l2.8 2.8" />
              <path d="M13.4 13.4l2.8 2.8" />
              <path d="M16.2 7.8l-2.8 2.8" />
              <path d="M10.6 13.4l-2.8 2.8" />
              <circle cx="12" cy="12" r="2.2" />
            </svg>
          </span>
          <h2>Manage Peers</h2>
          <p>{{ peerListSummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <p class="section-note">
          Peer List Exchange (PeerListV1) lets you export or import saved peer lists.
        </p>
        <input
          ref="peerListFileInput"
          type="file"
          accept="application/json"
          class="hidden-input"
          @change="onPeerListFileSelected"
        />
        <div class="actions">
          <button type="button" @click="openPeerListFilePicker">Load JSON File</button>
          <button type="button" @click="exportPeerList">Export</button>
        </div>
        <label class="full">
          Import JSON
          <textarea v-model="importText" rows="7" placeholder="Paste PeerListV1 JSON here"></textarea>
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

    <SosEmergencyCard ref="sosCard" />

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <path d="M8 8h8" />
              <path d="M8 12h5" />
              <path d="M5 4h14v16H5z" />
              <path d="M16.5 15.5h2" />
              <path d="M17.5 14.5v2" />
            </svg>
          </span>
          <h2>Plugin</h2>
          <p>{{ pluginSettingsSummary }}</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <p class="section-note">
          Installed plug-ins can contribute host-rendered configuration controls here.
        </p>
        <div v-if="pluginSettingsSections.length > 0" class="plugin-settings-list">
          <PluginSettingsSection
            v-for="section in pluginSettingsSections"
            :key="section.pluginId"
            :section="section"
            :values="pluginSettingsValues[section.pluginId] ?? {}"
            @update="updatePluginSettings"
            @save="savePluginSettings"
          />
        </div>
        <p v-else class="section-note">
          No plug-ins are installed or enabled for configuration yet.
        </p>
        <p v-if="pluginSettingsFeedback" class="feedback">{{ pluginSettingsFeedback }}</p>
      </div>
    </details>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <circle cx="6" cy="12" r="1.6" />
              <circle cx="12" cy="6" r="1.6" />
              <circle cx="18" cy="8" r="1.6" />
              <circle cx="18" cy="16" r="1.6" />
              <circle cx="10" cy="18" r="1.6" />
              <path d="M7.4 10.9 10.6 7.1" />
              <path d="M13.5 6.5 16.5 7.5" />
              <path d="M18 9.6v4.8" />
              <path d="M16.7 17.1 11.3 16.9" />
              <path d="M8.7 16.9 6.9 13.5" />
              <path d="M11.2 7.5 10.4 16.4" />
            </svg>
          </span>
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
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.reinitializeClient(), 'Node client recreated.')"
          >
            Restart UI
          </button>
          <button
            type="button"
            @click="runNodeAction(() => nodeStore.restartNode(), 'Node restarted.')"
          >
            Restart
          </button>
        </div>
        <p v-if="runtimeFeedback" class="feedback">{{ runtimeFeedback }}</p>
        <p v-if="nodeStore.lastError" class="feedback">{{ nodeStore.lastError }}</p>
        <div class="log-list">
          <p v-for="entry in nodeStore.nodeControlEntries" :key="entry.at" class="log">
            {{ entry.level }} | {{ entry.message }}
          </p>
        </div>
      </div>
    </details>

    <details class="panel fold-panel">
      <summary class="panel-summary">
        <div class="summary-copy">
          <span class="summary-icon" aria-hidden="true">
            <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="8" />
              <path d="M12 10.8v5.4" />
              <path d="M12 7.8h.01" />
            </svg>
          </span>
          <h2>About</h2>
          <p>Application information</p>
        </div>
        <span class="chevron" aria-hidden="true">&#9662;</span>
      </summary>
      <div class="panel-body">
        <dl class="about-list">
          <div
            v-for="item in aboutItems"
            :key="item.label"
            class="about-row"
          >
            <dt>{{ item.label }}</dt>
            <dd>{{ item.value }}</dd>
          </div>
          <div class="about-row">
            <dt>GitHub repository URL</dt>
            <dd>
              <a :href="repositoryUrl" target="_blank" rel="noreferrer">
                {{ repositoryUrl }}
              </a>
            </dd>
          </div>
        </dl>
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
  display: block;
}

.header-actions {
  align-items: center;
  display: grid;
  gap: 0.72rem;
  grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.35fr) minmax(0, 1.12fr) minmax(6.8rem, 0.9fr);
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

.settings-chip,
.settings-save {
  align-items: center;
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(183 235 255 / 8%),
    0 0 18px rgb(33 153 255 / 7%);
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: clamp(0.78rem, 1.9vw, 0.96rem);
  font-weight: 700;
  gap: 0.54rem;
  justify-content: center;
  min-height: 2.95rem;
  min-width: 0;
  padding: 0.46rem 0.72rem;
  text-transform: none;
}

.settings-chip svg,
.settings-save svg {
  flex: 0 0 auto;
  height: 1.14rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1.14rem;
}

.settings-chip span,
.settings-save span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.unsaved-chip {
  background: rgb(45 27 4 / 82%);
  border: 1px solid rgb(251 161 38 / 74%);
  color: #ffb13d;
}

.node-control-chip {
  --btn-bg: rgb(7 25 54 / 84%);
  --btn-border: rgb(73 173 255 / 52%);
  --btn-color: #a7c7ee;
  background: rgb(7 25 54 / 84%);
  border: 1px solid rgb(73 173 255 / 52%);
  color: #a7c7ee;
  cursor: pointer;
}

.setup-chip {
  --btn-bg: rgb(8 39 74 / 84%);
  --btn-border: rgb(92 205 255 / 50%);
  --btn-color: #8ee6ff;
  background: rgb(8 39 74 / 84%);
  border: 1px solid rgb(92 205 255 / 50%);
  color: #8ee6ff;
  cursor: pointer;
}

.node-control-chip .chevron {
  margin-left: auto;
}

.settings-save {
  --btn-bg: linear-gradient(180deg, rgb(31 118 225 / 88%), rgb(17 72 167 / 92%));
  --btn-border: rgb(73 173 255 / 66%);
  --btn-color: #e3f5ff;
  background: linear-gradient(180deg, rgb(31 118 225 / 88%), rgb(17 72 167 / 92%));
  border: 1px solid rgb(73 173 255 / 66%);
  color: #e3f5ff;
  cursor: pointer;
}

.settings-save:disabled {
  background: linear-gradient(180deg, rgb(33 111 214 / 55%), rgb(17 72 167 / 56%));
  border-color: rgb(73 173 255 / 42%);
  color: rgb(184 215 244 / 56%);
  cursor: not-allowed;
  opacity: 1;
  transform: none;
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

.summary-copy {
  align-items: center;
  column-gap: 0.72rem;
  display: grid;
  grid-template-columns: auto 1fr;
}

.summary-icon {
  align-items: center;
  background:
    radial-gradient(circle at 30% 30%, rgb(120 228 255 / 16%), transparent 52%),
    linear-gradient(145deg, rgb(8 29 58 / 92%), rgb(5 20 44 / 96%));
  border: 1px solid rgb(92 184 255 / 28%);
  border-radius: 11px;
  box-shadow:
    inset 0 1px 0 rgb(210 245 255 / 8%),
    0 8px 18px rgb(2 14 32 / 18%);
  color: #7fdbff;
  display: inline-flex;
  grid-row: 1 / span 2;
  height: 2.4rem;
  justify-content: center;
  width: 2.4rem;
}

.summary-icon-svg {
  height: 1.2rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
  width: 1.2rem;
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

.propagation-grid {
  margin-top: 0.75rem;
}

.active-endpoints {
  display: grid;
  gap: 0.4rem;
  margin-top: 0.65rem;
}

.plugin-settings-list {
  display: grid;
  gap: 0.65rem;
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
  font-size: 0.7rem;
  min-height: 26px;
  padding: 0 0.62rem;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin-top: 0.75rem;
}

button {
  --btn-bg: linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%));
  --btn-bg-pressed: linear-gradient(180deg, rgb(196 240 255 / 96%), rgb(118 212 255 / 94%));
  --btn-border: rgb(74 133 207 / 45%);
  --btn-border-pressed: rgb(224 248 255 / 86%);
  --btn-shadow: inset 0 1px 0 rgb(209 244 255 / 10%), 0 8px 18px rgb(2 14 32 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 21 47 / 24%);
  --btn-color: #8fdbff;
  --btn-color-pressed: #042541;
  background: var(--btn-bg);
  border: 1px solid var(--btn-border);
  border-radius: 999px;
  box-shadow: var(--btn-shadow);
  color: var(--btn-color);
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 32px;
  padding: 0 0.82rem;
  text-transform: uppercase;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
  transform: none;
}

.feedback {
  color: #96afd5;
  font-family: var(--font-body);
  margin: 0.58rem 0 0;
}

.hidden-input {
  display: none;
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

.about-list {
  display: grid;
  gap: 0.55rem;
  margin: 0;
}

.about-row {
  background: rgb(7 20 44 / 72%);
  border: 1px solid rgb(67 106 165 / 30%);
  border-radius: 12px;
  display: grid;
  gap: 0.22rem;
  padding: 0.62rem 0.72rem;
}

.about-row dt,
.about-row dd {
  margin: 0;
}

.about-row dt {
  color: #60d8ff;
  font-family: var(--font-ui);
  font-size: 0.7rem;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.about-row dd,
.about-row a {
  color: #d5eaff;
  font-family: var(--font-body);
  overflow-wrap: anywhere;
}

.about-row a {
  text-decoration-color: rgb(96 216 255 / 55%);
  text-underline-offset: 0.16rem;
}

@media (max-width: 760px) {
  .view-header {
    align-items: stretch;
  }

  .header-actions {
    gap: 0.48rem;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .settings-chip,
  .settings-save {
    font-size: 0.72rem;
    gap: 0.34rem;
    min-height: 2.62rem;
    padding-inline: 0.42rem;
  }

  .settings-chip svg,
  .settings-save svg {
    height: 0.98rem;
    width: 0.98rem;
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
