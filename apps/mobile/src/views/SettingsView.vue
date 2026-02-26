<script setup lang="ts">
import { reactive, ref } from "vue";

import { copyToClipboard, shareText } from "../services/peerExchange";
import { useNodeStore } from "../stores/nodeStore";

const nodeStore = useNodeStore();

const form = reactive({
  clientMode: nodeStore.settings.clientMode,
  autoConnectSaved: nodeStore.settings.autoConnectSaved,
  showOnlyCapabilityVerified: nodeStore.settings.showOnlyCapabilityVerified,
  announceCapabilities: nodeStore.settings.announceCapabilities,
  announceIntervalSeconds: nodeStore.settings.announceIntervalSeconds,
  tcpClientsText: nodeStore.settings.tcpClients.join("\n"),
  broadcast: nodeStore.settings.broadcast,
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

function applySettings(): void {
  nodeStore.updateSettings({
    clientMode: form.clientMode,
    autoConnectSaved: form.autoConnectSaved,
    showOnlyCapabilityVerified: form.showOnlyCapabilityVerified,
    announceCapabilities: form.announceCapabilities.trim(),
    announceIntervalSeconds: Math.max(5, Number(form.announceIntervalSeconds || 30)),
    tcpClients: form.tcpClientsText
      .split(/\n/g)
      .map((line: string) => line.trim())
      .filter((line: string) => line.length > 0),
    broadcast: form.broadcast,
    hub: {
      mode: form.hubMode,
      identityHash: form.hubIdentityHash.trim(),
      apiBaseUrl: form.hubApiBaseUrl.trim(),
      apiKey: form.hubApiKey.trim(),
      refreshIntervalSeconds: Math.max(30, Number(form.hubRefreshIntervalSeconds || 300)),
    },
  });
  runtimeFeedback.value = "Settings saved.";
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
    <header>
      <h1>Settings</h1>
      <p>Node runtime, discovery filters, and directory source controls.</p>
    </header>

    <section class="panel">
      <h2>Runtime</h2>
      <div class="grid">
        <label>
          Client mode
          <select v-model="form.clientMode">
            <option value="auto">Auto</option>
            <option value="mock">Mock</option>
            <option value="capacitor">Capacitor only</option>
          </select>
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
          <input v-model="form.showOnlyCapabilityVerified" type="checkbox" />
          Show capability-verified peers by default
        </label>
      </div>
      <label class="full">
        TCP interfaces (one per line)
        <textarea v-model="form.tcpClientsText" rows="3"></textarea>
      </label>
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
    </section>

    <section class="panel">
      <h2>Hub Directory</h2>
      <div class="grid">
        <label>
          Mode
          <select v-model="form.hubMode">
            <option value="Disabled">Disabled</option>
            <option value="RchLxmf">RCH via LXMF</option>
            <option value="RchHttp">RCH via HTTP</option>
          </select>
        </label>
        <label>
          Hub identity hash
          <input v-model="form.hubIdentityHash" type="text" />
        </label>
        <label>
          Hub API base URL
          <input v-model="form.hubApiBaseUrl" type="url" />
        </label>
        <label>
          Hub API key
          <input v-model="form.hubApiKey" type="text" />
        </label>
        <label>
          Refresh interval seconds
          <input v-model.number="form.hubRefreshIntervalSeconds" type="number" min="30" />
        </label>
      </div>
      <div class="actions">
        <button type="button" @click="applySettings">Save Hub Settings</button>
        <button
          type="button"
          @click="runNodeAction(() => nodeStore.refreshHubDirectory(), 'Hub refresh requested.')"
        >
          Refresh Now
        </button>
      </div>
    </section>

    <section class="panel">
      <h2>Peer List Exchange (PeerListV1)</h2>
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
      <p class="feedback" v-if="importFeedback">{{ importFeedback }}</p>
    </section>

    <section class="panel">
      <h2>Node Control</h2>
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
      <p class="feedback" v-if="runtimeFeedback">{{ runtimeFeedback }}</p>
      <p class="feedback" v-if="nodeStore.lastError">{{ nodeStore.lastError }}</p>
      <div class="log-list">
        <p class="log" v-for="entry in nodeStore.logs.slice(0, 8)" :key="entry.at">
          {{ entry.level }} | {{ entry.message }}
        </p>
      </div>
    </section>
  </section>
</template>

<style scoped>
.view {
  display: grid;
  gap: 1rem;
}

h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.8rem, 3.5vw, 2.9rem);
  margin: 0;
}

header p {
  color: #9cb3d6;
  font-family: var(--font-body);
  margin: 0.25rem 0 0;
}

.panel {
  background: rgb(10 23 49 / 86%);
  border: 1px solid rgb(75 117 183 / 33%);
  border-radius: 14px;
  padding: 0.9rem;
}

h2 {
  font-family: var(--font-headline);
  font-size: 1.5rem;
  margin: 0 0 0.75rem;
}

.grid {
  display: grid;
  gap: 0.6rem;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
}

label {
  color: #a0b7db;
  display: grid;
  font-family: var(--font-ui);
  font-size: 0.76rem;
  gap: 0.25rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

input,
textarea,
select {
  background: rgb(6 17 38 / 82%);
  border: 1px solid rgb(70 110 174 / 42%);
  border-radius: 10px;
  color: #daecff;
  font-family: var(--font-body);
  font-size: 0.98rem;
  padding: 0.48rem 0.56rem;
}

textarea {
  resize: vertical;
}

.checkbox {
  align-items: center;
  grid-template-columns: auto 1fr;
  text-transform: none;
}

.radio {
  align-items: center;
  display: flex;
  font-size: 0.9rem;
  gap: 0.35rem;
  letter-spacing: normal;
  text-transform: none;
}

.full {
  margin-top: 0.65rem;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin-top: 0.7rem;
}

button {
  background: linear-gradient(115deg, #00a3ff, #1af1ff);
  border: 0;
  border-radius: 10px;
  color: #032749;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.82rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 35px;
  padding: 0 0.8rem;
  text-transform: uppercase;
}

.feedback {
  color: #95afd6;
  font-family: var(--font-body);
  margin: 0.62rem 0 0;
}

.log-list {
  margin-top: 0.55rem;
}

.log {
  color: #88a4d0;
  font-family: var(--font-body);
  margin: 0.28rem 0 0;
}
</style>
