<script setup lang="ts">
import type { PluginCapabilityRecord } from "@reticulum/node-client";
import { computed, ref } from "vue";

import PluginMapSessionList from "../../components/map/PluginMapSessionList.vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import {
  PLUGIN_CAPABILITY_DEFINITIONS,
  readableAndroidPermissions,
} from "../../components/settings/pluginPresentation";
import { useNodeStore } from "../../stores/nodeStore";

const props = defineProps<{ pluginId: string }>();
const nodeStore = useNodeStore();
const pending = ref("");
const feedback = ref("");
const plugin = computed(() => nodeStore.plugins.find((entry) => entry.pluginId === props.pluginId));
const declaredCapabilities = computed(() =>
  PLUGIN_CAPABILITY_DEFINITIONS.filter((item) => plugin.value?.declaredCapabilities[item.key]),
);

async function run(key: string, action: () => Promise<void>): Promise<void> {
  if (pending.value) return;
  pending.value = key;
  feedback.value = "";
  try {
    await action();
  } catch (error: unknown) {
    feedback.value = error instanceof Error ? error.message : String(error);
  } finally {
    pending.value = "";
  }
}

function setCapability(current: PluginCapabilityRecord, key: keyof PluginCapabilityRecord, enabled: boolean): Promise<void> {
  return run(key, () => nodeStore.grantPluginCapabilities(props.pluginId, { ...current, [key]: enabled }));
}
</script>

<template>
  <SettingsPage
    :title="plugin?.displayName ?? 'Plugin'"
    :description="plugin ? `Version ${plugin.version} · ${plugin.state}` : ''"
    :back="{ name: 'settings-plugins' }"
    back-label="Back to plugins"
  >
    <section v-if="!plugin" class="panel settings-card">
      <p class="section-note">This plugin is no longer installed.</p>
    </section>
    <template v-else>
      <section class="panel settings-card">
        <dl class="status-rows">
          <div class="status-row"><dt>Uses on this phone</dt><dd>{{ readableAndroidPermissions(plugin.androidPermissions) }}</dd></div>
        </dl>
        <p v-if="plugin.diagnostic" class="feedback">{{ plugin.diagnostic }}</p>
        <div v-if="!plugin.trusted" class="actions">
          <button
            type="button"
            :disabled="Boolean(pending) || plugin.state === 'Incompatible' || plugin.state === 'Missing'"
            @click="run('trust', () => nodeStore.approvePluginPublisher(pluginId))"
          >
            Approve publisher
          </button>
        </div>
        <div v-else class="actions">
          <button type="button" :disabled="Boolean(pending)" @click="run('enabled', () => nodeStore.setPluginEnabled(pluginId, !plugin?.enabled))">
            {{ plugin.enabled ? "Disable" : "Enable" }}
          </button>
          <button
            v-if="plugin.configurationEntrypoint"
            type="button"
            :disabled="Boolean(pending) || !plugin.enabled"
            @click="run('configure', () => nodeStore.openPluginConfiguration(pluginId))"
          >
            Configure
          </button>
        </div>
        <p v-if="feedback" class="feedback" role="status">{{ feedback }}</p>
      </section>

      <section v-if="plugin.trusted" class="panel settings-card">
        <div class="config-section-header"><h3>Allow it to</h3></div>
        <div class="plugin-capability-grid">
          <label v-for="capability in declaredCapabilities" :key="capability.key" class="checkbox">
            <input
              type="checkbox"
              :checked="plugin.grantedCapabilities[capability.key]"
              :disabled="Boolean(pending)"
              @change="setCapability(plugin.grantedCapabilities, capability.key, ($event.target as HTMLInputElement).checked)"
            />
            {{ capability.label }}
          </label>
        </div>
        <PluginMapSessionList :plugin-id="plugin.pluginId" />
      </section>

      <details class="panel settings-advanced">
        <summary>Technical details</summary>
        <dl class="status-rows">
          <div class="status-row"><dt>ID</dt><dd><code>{{ plugin.pluginId }}</code></dd></div>
          <div class="status-row"><dt>Package</dt><dd><code>{{ plugin.packageName }}</code></dd></div>
          <div class="status-row"><dt>Publisher</dt><dd><code>{{ plugin.publisherFingerprint }}</code></dd></div>
          <div class="status-row"><dt>Android permissions</dt><dd>{{ plugin.androidPermissions.join(", ") || "None declared" }}</dd></div>
        </dl>
      </details>

      <section v-if="plugin.trusted" class="panel settings-card danger-zone">
        <div class="config-section-header">
          <h3>Revoke publisher</h3>
          <p>Removes trust from every plugin signed by this publisher.</p>
        </div>
        <div class="actions">
          <button
            type="button"
            class="danger-button"
            :disabled="Boolean(pending)"
            @click="run('revoke', () => nodeStore.revokePluginPublisher(plugin?.publisherFingerprint ?? ''))"
          >
            Revoke publisher
          </button>
        </div>
      </section>
    </template>
  </SettingsPage>
</template>
