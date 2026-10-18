<script setup lang="ts">
import type { PluginCapabilityRecord } from "@reticulum/node-client";
import { ref } from "vue";

import { useNodeStore } from "../../stores/nodeStore";
import PluginMapSessionList from "../map/PluginMapSessionList.vue";
const nodeStore = useNodeStore();
const pluginActionPending = ref("");

const capabilityDefinitions: Array<{
  key: keyof PluginCapabilityRecord;
  label: string;
}> = [
  { key: "eventsPublish", label: "Publish events" },
  { key: "sensorsPublish", label: "Publish sensors" },
  { key: "lxmfSend", label: "Send LXMF" },
  { key: "lxmfReceive", label: "Receive LXMF" },
  { key: "notificationsRaise", label: "Raise notifications" },
  { key: "operationalRead", label: "Read operational status" },
  { key: "mapMarkersRead", label: "Read its own map markers" },
  { key: "mapMarkersWrite", label: "Add and edit its own map markers" },
  { key: "mapObserverRead", label: "Read this device's location" },
  { key: "mapSelectionRead", label: "Read map objects you share with it" },
  { key: "mapNearbyRead", label: "Read all map markers near this device" },
];

/** Plain words for the Android permissions a plugin app uses; anything else stays in the technical details. */
const androidPermissionLabels: Array<[RegExp, string]> = [
  [/BLUETOOTH/, "Bluetooth"],
  [/LOCATION/, "location"],
  [/POST_NOTIFICATIONS/, "notifications"],
  [/FOREGROUND_SERVICE/, "running in the background"],
  [/CAMERA/, "camera"],
  [/RECORD_AUDIO/, "microphone"],
  [/BODY_SENSORS|OTHER_SENSORS/, "body sensors"],
  [/INTERNET/, "internet"],
];

function readableAndroidPermissions(permissions: string[]): string {
  const labels = new Set<string>();
  for (const permission of permissions) {
    const match = androidPermissionLabels.find(([pattern]) => pattern.test(permission));
    if (match) labels.add(match[1]);
  }
  const list = [...labels];
  return list.length ? `${list[0][0].toUpperCase()}${list[0].slice(1)}${list.length > 1 ? `, ${list.slice(1).join(", ")}` : ""}` : "Nothing special";
}

async function runPluginAction(key: string, action: () => Promise<void>): Promise<void> {
  if (pluginActionPending.value) return;
  pluginActionPending.value = key;
  try {
    await action();
  } finally {
    pluginActionPending.value = "";
  }
}

function setPluginCapability(
  pluginId: string,
  current: PluginCapabilityRecord,
  key: keyof PluginCapabilityRecord,
  enabled: boolean,
): Promise<void> {
  return runPluginAction(`${pluginId}:${key}`, () => nodeStore.grantPluginCapabilities(
    pluginId,
    { ...current, [key]: enabled },
  ));
}
</script>

<template>
  <details class="panel fold-panel" data-testid="plugin-settings-panel">
    <summary class="panel-summary">
      <div class="summary-copy">
        <span class="summary-icon" aria-hidden="true">
          <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
            <path d="M8.5 4H4v4.5a2.5 2.5 0 1 1 0 5V20h6.5a2.5 2.5 0 1 0 5 0H20v-6.5a2.5 2.5 0 1 0 0-5V4h-6.5a2.5 2.5 0 1 0-5 0Z" />
          </svg>
        </span>
        <h2>Plugins</h2>
        <p>{{ nodeStore.plugins.length }} discovered Android plugin{{ nodeStore.plugins.length === 1 ? "" : "s" }}</p>
      </div>
      <span class="chevron" aria-hidden="true">&#9662;</span>
    </summary>
    <div class="panel-body">
      <p class="section-note">
        Plugin APKs are installed separately. Android permissions belong to the plugin app;
        the grants below only control access to REM host capabilities.
      </p>
      <div class="button-row">
        <button
          type="button"
          :disabled="Boolean(pluginActionPending)"
          @click="runPluginAction('refresh', () => nodeStore.refreshPluginProjection(true))"
        >
          Refresh installed plugins
        </button>
      </div>
      <div v-if="nodeStore.plugins.length" class="plugin-card-list">
        <article v-for="plugin in nodeStore.plugins" :key="plugin.pluginId" class="plugin-card">
          <header class="plugin-card-header">
            <div>
              <h3>{{ plugin.displayName }}</h3>
              <p>Version {{ plugin.version }}</p>
            </div>
            <span class="status-pill">{{ plugin.state }}</span>
          </header>
          <p class="section-note">
            Uses on this phone: {{ readableAndroidPermissions(plugin.androidPermissions) }}
          </p>
          <details class="plugin-technical">
            <summary>Technical details</summary>
            <p>ID: {{ plugin.pluginId }}</p>
            <p>Package: {{ plugin.packageName }}</p>
            <p>Publisher: {{ plugin.publisherFingerprint }}</p>
            <p>Android permissions: {{ plugin.androidPermissions.join(", ") || "None declared" }}</p>
          </details>
          <p v-if="plugin.diagnostic" class="feedback">{{ plugin.diagnostic }}</p>
          <div v-if="!plugin.trusted" class="button-row">
            <button
              type="button"
              :disabled="Boolean(pluginActionPending) || plugin.state === 'Incompatible' || plugin.state === 'Missing'"
              @click="runPluginAction(`${plugin.pluginId}:trust`, () => nodeStore.approvePluginPublisher(plugin.pluginId))"
            >
              Approve publisher
            </button>
          </div>
          <template v-else>
            <p class="section-note">Allow it to:</p>
            <div class="plugin-capability-grid">
              <label
                v-for="capability in capabilityDefinitions.filter((item) => plugin.declaredCapabilities[item.key])"
                :key="capability.key"
                class="checkbox"
              >
                <input
                  type="checkbox"
                  :checked="plugin.grantedCapabilities[capability.key]"
                  :disabled="Boolean(pluginActionPending)"
                  @change="setPluginCapability(
                    plugin.pluginId,
                    plugin.grantedCapabilities,
                    capability.key,
                    ($event.target as HTMLInputElement).checked,
                  )"
                />
                {{ capability.label }}
              </label>
            </div>
            <div class="button-row">
              <button
                type="button"
                :disabled="Boolean(pluginActionPending)"
                @click="runPluginAction(`${plugin.pluginId}:enabled`, () => nodeStore.setPluginEnabled(plugin.pluginId, !plugin.enabled))"
              >
                {{ plugin.enabled ? "Disable" : "Enable" }}
              </button>
              <button
                v-if="plugin.configurationEntrypoint"
                type="button"
                :disabled="Boolean(pluginActionPending) || !plugin.enabled"
                @click="runPluginAction(`${plugin.pluginId}:configure`, () => nodeStore.openPluginConfiguration(plugin.pluginId))"
              >
                Configure
              </button>
              <button
                type="button"
                class="danger-button"
                :disabled="Boolean(pluginActionPending)"
                @click="runPluginAction(`${plugin.pluginId}:revoke`, () => nodeStore.revokePluginPublisher(plugin.publisherFingerprint))"
              >
                Revoke publisher
              </button>
            </div>
            <PluginMapSessionList :plugin-id="plugin.pluginId" />
          </template>
        </article>
      </div>
      <p v-else class="section-note">No plugin APKs were discovered.</p>
    </div>
  </details>
</template>
