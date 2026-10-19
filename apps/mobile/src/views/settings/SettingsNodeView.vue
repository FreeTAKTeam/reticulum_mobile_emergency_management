<script setup lang="ts">
import { computed, ref } from "vue";

import SettingsPage from "../../components/settings/SettingsPage.vue";
import { useNodeStore } from "../../stores/nodeStore";
import type { NodeUiSettings } from "../../types/domain";
import { nodeSummary } from "../../utils/settingsIndexSummaries";

const nodeStore = useNodeStore();
const feedback = ref("");
const pending = ref(false);
const summary = computed(() =>
  nodeSummary(nodeStore.status.running, nodeStore.status.interfaces.length),
);

function formatTime(at: number, empty: string): string {
  if (!Number.isFinite(at) || at <= 0) {
    return empty;
  }
  return new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

async function run(action: () => Promise<void>, success: string): Promise<void> {
  if (pending.value) return;
  pending.value = true;
  feedback.value = "";
  try {
    await action();
    feedback.value = success;
  } catch (error: unknown) {
    feedback.value = error instanceof Error ? error.message : String(error);
  } finally {
    pending.value = false;
  }
}

/** Developer switch on an action page: applies as soon as it changes. */
function setClientMode(event: Event): void {
  const clientMode = (event.target as HTMLSelectElement).value as NodeUiSettings["clientMode"];
  void run(() => nodeStore.updateSettings({ clientMode }), "Client mode updated.");
}
</script>

<template>
  <SettingsPage title="Node & diagnostics" description="Run, stop and inspect the Reticulum node on this phone.">
    <section class="panel settings-card" aria-labelledby="node-control-heading">
      <div class="config-section-header">
        <h3 id="node-control-heading">Node Control</h3>
        <p>{{ nodeStore.status.running ? "Node is running" : "Node is stopped" }} · {{ summary }}</p>
      </div>
      <div class="actions">
        <button type="button" :disabled="pending" @click="run(() => nodeStore.startNode(), 'Node started.')">Start</button>
        <button type="button" :disabled="pending" @click="run(() => nodeStore.stopNode(), 'Node stopped.')">Stop</button>
        <button type="button" :disabled="pending" @click="run(() => nodeStore.restartNode(), 'Node restarted.')">Restart</button>
        <button type="button" :disabled="pending" @click="run(() => nodeStore.reinitializeClient(), 'Node client recreated.')">Recreate client</button>
      </div>
      <p v-if="feedback" class="feedback" role="status">{{ feedback }}</p>
    </section>

    <section class="panel settings-card" aria-labelledby="node-interfaces-heading">
      <div class="config-section-header"><h3 id="node-interfaces-heading">Interfaces</h3></div>
      <div v-if="nodeStore.status.interfaces.length > 0" class="active-endpoints">
        <article v-for="iface in nodeStore.status.interfaces" :key="iface.interfaceHex" class="active-endpoint">
          <span>
            {{ iface.label || iface.interfaceHex }}
            <small>{{ iface.kind }} | {{ iface.state }} | RX {{ iface.rxPackets }} / {{ iface.rxBytes }} bytes | {{ formatTime(iface.lastActivityMs, "No RX yet") }}</small>
            <small v-if="iface.lastError">{{ iface.lastError }}</small>
          </span>
        </article>
      </div>
      <p v-else class="section-note">No interfaces are up.</p>
    </section>

    <section class="panel settings-card" aria-labelledby="node-log-heading">
      <div class="config-section-header"><h3 id="node-log-heading">Node log</h3></div>
      <div class="log-list">
        <p v-for="entry in nodeStore.nodeControlEntries" :key="entry.at" class="log">
          <time :datetime="new Date(entry.at).toISOString()">{{ formatTime(entry.at, "--:--:--") }}</time>
          <span>{{ entry.level }}</span>
          <span>{{ entry.message }}</span>
        </p>
      </div>
      <dl v-if="nodeStore.lastError" class="status-rows">
        <div class="status-row"><dt>Last error</dt><dd>{{ nodeStore.lastError }}</dd></div>
      </dl>
    </section>

    <details class="panel settings-advanced">
      <summary>Developer</summary>
      <div class="settings-fields">
        <label>
          Client mode
          <select :value="nodeStore.settings.clientMode" :disabled="pending" @change="setClientMode">
            <option value="auto">Auto</option>
            <option value="capacitor">Capacitor only</option>
          </select>
        </label>
      </div>
    </details>
  </SettingsPage>
</template>
