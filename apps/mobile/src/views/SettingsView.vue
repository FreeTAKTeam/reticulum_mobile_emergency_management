<script setup lang="ts">
import { computed, onMounted, reactive, ref, useTemplateRef } from "vue";
import { useRouter } from "vue-router";

import SettingsAboutPanel from "../components/settings/SettingsAboutPanel.vue";
import SettingsHubPanel from "../components/settings/SettingsHubPanel.vue";
import SettingsNodeConfigPanel from "../components/settings/SettingsNodeConfigPanel.vue";
import SettingsNodeControlPanel from "../components/settings/SettingsNodeControlPanel.vue";
import SettingsPluginsPanel from "../components/settings/SettingsPluginsPanel.vue";
import SettingsTelemetryPanel from "../components/settings/SettingsTelemetryPanel.vue";
import SettingsTeamsPanel from "../components/settings/SettingsTeamsPanel.vue";
import SosEmergencyCard from "../components/sos/SosEmergencyCard.vue";
import { useNodeStore } from "../stores/nodeStore";
import { ensureRequiredAnnounceCapabilities } from "../utils/peers";
import { normalizeRnodeSettings } from "../utils/rnodeProfiles";

const nodeStore = useNodeStore();
const router = useRouter();
const sosCardRef = useTemplateRef<{
  saveSettings: () => Promise<void>;
  hasUnsavedChanges: () => boolean;
}>("sosCard");
const savingSettings = ref(false);

const form = reactive({
  displayName: nodeStore.settings.displayName,
  clientMode: nodeStore.settings.clientMode,
  announceCapabilities: ensureRequiredAnnounceCapabilities(nodeStore.settings.announceCapabilities),
  announceIntervalSeconds: nodeStore.settings.announceIntervalSeconds,
  tcpClients: [...nodeStore.settings.tcpClients],
  broadcast: nodeStore.settings.broadcast,
  transportNodeEnabled: nodeStore.settings.transportNodeEnabled,
  rnodeEnabled: nodeStore.settings.rnode.enabled,
  rnodePeripheralId: nodeStore.settings.rnode.peripheralId,
  rnodeDisplayName: nodeStore.settings.rnode.displayName,
  rnodeRegion: nodeStore.settings.rnode.region,
  rnodeProfile: nodeStore.settings.rnode.profile,
  rnodeFrequencyHz: nodeStore.settings.rnode.frequencyHz,
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

const runtimeFeedback = ref("");
const nodeControlPanel = useTemplateRef<{ openPanel: () => void }>("nodeControlPanel");

const normalizedTcpClients = computed(() =>
  [
    ...new Set(
      form.tcpClients
        .map((entry: string) => entry.trim())
        .filter((entry) => entry.length > 0),
    ),
  ],
);

const normalizedRnodeSettings = computed(() =>
  normalizeRnodeSettings(
    {
      enabled: form.rnodeEnabled,
      peripheralId: form.rnodePeripheralId,
      displayName: form.rnodeDisplayName,
      region: form.rnodeRegion,
      profile: form.rnodeProfile,
      frequencyHz: form.rnodeFrequencyHz,
    },
    nodeStore.settings.rnode,
  ),
);

const persistedTcpClients = computed(() =>
  [
    ...new Set(
      nodeStore.settings.tcpClients
        .map((entry: string) => entry.trim())
        .filter((entry: string) => entry.length > 0),
    ),
  ],
);

function normalizeTelemetryPublishIntervalSeconds(value: number | string | undefined | null): number {
  const parsed = Number(value ?? 60);
  return Number.isFinite(parsed) ? Math.max(1, parsed) : 60;
}

const hasMainSettingsChanges = computed(() =>
  form.displayName !== nodeStore.settings.displayName
  || form.clientMode !== nodeStore.settings.clientMode
  || ensureRequiredAnnounceCapabilities(form.announceCapabilities.trim()) !== nodeStore.settings.announceCapabilities
  || Math.max(60, Number(form.announceIntervalSeconds || 1800)) !== nodeStore.settings.announceIntervalSeconds
  || form.broadcast !== nodeStore.settings.broadcast
  || form.transportNodeEnabled !== nodeStore.settings.transportNodeEnabled
  || JSON.stringify(normalizedTcpClients.value) !== JSON.stringify(persistedTcpClients.value)
  || JSON.stringify(normalizedRnodeSettings.value) !== JSON.stringify(normalizeRnodeSettings(nodeStore.settings.rnode))
  || form.telemetryEnabled !== nodeStore.settings.telemetry.enabled
  || normalizeTelemetryPublishIntervalSeconds(form.telemetryPublishIntervalSeconds)
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



function syncSettingsForm(): void {
  form.displayName = nodeStore.settings.displayName;
  form.clientMode = nodeStore.settings.clientMode;
  form.announceCapabilities = ensureRequiredAnnounceCapabilities(nodeStore.settings.announceCapabilities);
  form.announceIntervalSeconds = nodeStore.settings.announceIntervalSeconds;
  form.tcpClients = [...nodeStore.settings.tcpClients];
  form.broadcast = nodeStore.settings.broadcast;
  form.transportNodeEnabled = nodeStore.settings.transportNodeEnabled;
  form.rnodeEnabled = nodeStore.settings.rnode.enabled;
  form.rnodePeripheralId = nodeStore.settings.rnode.peripheralId;
  form.rnodeDisplayName = nodeStore.settings.rnode.displayName;
  form.rnodeRegion = nodeStore.settings.rnode.region;
  form.rnodeProfile = nodeStore.settings.rnode.profile;
  form.rnodeFrequencyHz = nodeStore.settings.rnode.frequencyHz;
  form.telemetryEnabled = nodeStore.settings.telemetry.enabled;
  form.telemetryPublishIntervalSeconds = nodeStore.settings.telemetry.publishIntervalSeconds;
  form.telemetryAccuracyThresholdMeters = nodeStore.settings.telemetry.accuracyThresholdMeters;
  form.telemetryStaleAfterMinutes = nodeStore.settings.telemetry.staleAfterMinutes;
  form.telemetryExpireAfterMinutes = nodeStore.settings.telemetry.expireAfterMinutes;
  form.hubMode = nodeStore.settings.hub.mode;
  form.hubIdentityHash = nodeStore.settings.hub.identityHash;
  form.hubApiBaseUrl = nodeStore.settings.hub.apiBaseUrl;
  form.hubApiKey = nodeStore.settings.hub.apiKey;
  form.hubRefreshIntervalSeconds = nodeStore.settings.hub.refreshIntervalSeconds;
}

onMounted(() => {
  void nodeStore.init()
    .then(syncSettingsForm)
    .catch((error: unknown) => {
      const message = `Settings initialization failed: ${error instanceof Error ? error.message : String(error)}`;
      nodeStore.setLastError(message);
      nodeStore.logUi("Warn", message);
    });
});

async function applySettings(): Promise<void> {
  if (!hasUnsavedSettings.value || savingSettings.value) {
    return;
  }
  const previousDisplayName = nodeStore.settings.displayName;
  const previousHubMode = nodeStore.settings.hub.mode;
  const previousHubIdentityHash = nodeStore.settings.hub.identityHash;
  const previousRnode = normalizeRnodeSettings(nodeStore.settings.rnode);
  const nextRnode = normalizedRnodeSettings.value;
  const rnodeChangedBeforeSave = JSON.stringify(previousRnode) !== JSON.stringify(nextRnode);
  let rnodeApplyError = "";
  let rnodeAppliedToRuntime = false;
  savingSettings.value = true;
  try {
    await nodeStore.updateSettings({
      displayName: form.displayName,
      clientMode: form.clientMode,
      announceCapabilities: ensureRequiredAnnounceCapabilities(form.announceCapabilities.trim()),
      announceIntervalSeconds: Math.max(60, Number(form.announceIntervalSeconds || 1800)),
      tcpClients: normalizedTcpClients.value,
      broadcast: form.broadcast,
      transportNodeEnabled: form.transportNodeEnabled,
      rnode: nextRnode,
      telemetry: {
        enabled: form.telemetryEnabled,
        publishIntervalSeconds: normalizeTelemetryPublishIntervalSeconds(
          form.telemetryPublishIntervalSeconds,
        ),
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
    if (rnodeChangedBeforeSave) {
      try {
        if (nodeStore.status.running) {
          await nodeStore.restartNode();
        } else {
          await nodeStore.startNode();
        }
        rnodeAppliedToRuntime = true;
      } catch (error: unknown) {
        rnodeApplyError = error instanceof Error ? error.message : String(error);
      }
    }
  } catch (error: unknown) {
    runtimeFeedback.value = error instanceof Error ? error.message : String(error);
    return;
  } finally {
    savingSettings.value = false;
  }

  syncSettingsForm();
  const displayNameChanged = nodeStore.settings.displayName !== previousDisplayName;
  const hubRoutingChanged =
    nodeStore.settings.hub.mode !== previousHubMode
    || nodeStore.settings.hub.identityHash !== previousHubIdentityHash;
  runtimeFeedback.value = rnodeApplyError
    ? `RNode settings saved, but node start/restart failed: ${rnodeApplyError}`
    : rnodeChangedBeforeSave && rnodeAppliedToRuntime
      ? "RNode settings saved and applied to the running LoRa interface configuration."
      : nodeStore.nodeConfigRestartRequired
        ? "Settings saved. Restart the app or node to apply updated interface configuration."
      : displayNameChanged
      ? "Settings saved. Restart the node to announce the updated call sign."
      : nodeStore.status.running && hubRoutingChanged
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


async function runSetupWizard(): Promise<void> {
  await router.push({
    path: "/setup",
    query: { source: "settings" },
  });
}

function openNodeControlPanel(): void {
  nodeControlPanel.value?.openPanel();
}

</script>

<template>
  <section class="settings-view">
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
        <button
          type="button"
          class="settings-chip node-control-chip"
          aria-label="Open Node Control"
          @click="openNodeControlPanel"
        >
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

    <SettingsNodeConfigPanel
      ref="nodeControlPanel"
      :form="form"
      @feedback="runtimeFeedback = $event"
    />

    <SettingsTelemetryPanel :form="form" />

    <SettingsPluginsPanel />

    <SettingsHubPanel :form="form" :run-node-action="runNodeAction" />

    <SettingsTeamsPanel />

    <SosEmergencyCard ref="sosCard" />

    <SettingsNodeControlPanel :external-feedback="runtimeFeedback" />

    <SettingsAboutPanel />
  </section>
</template>

<style src="./styles/settings-layout.css"></style>
<style src="./styles/settings-content.css"></style>
