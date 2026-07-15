<script setup lang="ts">
import type { PluginCapabilityRecord } from "@reticulum/node-client";
import { ref } from "vue";

import { useNodeStore } from "../../stores/nodeStore";

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
];

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
        <span class="summary-icon" aria-hidden="true">&#129513;</span>
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
              <p>{{ plugin.pluginId }} · {{ plugin.version }}</p>
            </div>
            <span class="status-pill">{{ plugin.state }}</span>
          </header>
          <p class="section-note">Package: {{ plugin.packageName }}</p>
          <p class="fingerprint">Publisher: {{ plugin.publisherFingerprint }}</p>
          <p class="section-note">
            Requests:
            {{ capabilityDefinitions
              .filter((item) => plugin.declaredCapabilities[item.key])
              .map((item) => item.label)
              .join(", ") || "No REM host capabilities" }}
          </p>
          <p class="section-note">
            Android permissions: {{ plugin.androidPermissions.join(", ") || "None declared" }}
          </p>
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
          </template>
        </article>
      </div>
      <p v-else class="section-note">No plugin APKs were discovered.</p>
    </div>
  </details>
</template>
