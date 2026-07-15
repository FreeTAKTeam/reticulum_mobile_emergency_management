<script setup lang="ts">
import { computed, ref, useTemplateRef } from "vue";

import { copyToClipboard, shareText } from "../../services/peerExchange";
import { useNodeStore } from "../../stores/nodeStore";

const nodeStore = useNodeStore();
const importText = ref("");
const importMode = ref<"merge" | "replace">("merge");
const feedback = ref("");
const fileInput = useTemplateRef<HTMLInputElement>("fileInput");
const summary = computed(() => `${nodeStore.savedPeers.length} saved peers`);

async function exportPeerList(): Promise<void> {
  try {
    const text = JSON.stringify(nodeStore.getSavedPeerList(), null, 2);
    await copyToClipboard(text);
    await shareText("Saved peer list", text);
    feedback.value = "Peer list exported to clipboard/share.";
  } catch (error) {
    feedback.value = error instanceof Error ? error.message : String(error);
  }
}

function importPeerList(): void {
  try {
    const parsed = nodeStore.parsePeerListText(importText.value);
    nodeStore.importPeerList(parsed.peerList, importMode.value);
    feedback.value = `Imported ${parsed.peerList.peers.length} peers (${importMode.value}).`;
    if (parsed.warnings.length > 0) {
      feedback.value += ` Warnings: ${parsed.warnings.join(" ")}`;
    }
  } catch (error) {
    feedback.value = String(error);
  }
}

function openFilePicker(): void {
  fileInput.value?.click();
}

async function onFileSelected(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  importText.value = await file.text();
}
</script>

<template>
  <details class="panel fold-panel">
    <summary class="panel-summary">
      <div class="summary-copy">
        <span class="summary-icon" aria-hidden="true">&#128101;</span>
        <h2>Manage Peers</h2>
        <p>{{ summary }}</p>
      </div>
      <span class="chevron" aria-hidden="true">&#9662;</span>
    </summary>
    <div class="panel-body">
      <input
        ref="fileInput"
        class="hidden-input"
        type="file"
        accept="application/json,.json,text/plain"
        @change="onFileSelected"
      />
      <div class="actions">
        <button type="button" @click="openFilePicker">Load JSON File</button>
        <button type="button" @click="exportPeerList">Export</button>
      </div>
      <label class="full">
        Peer list JSON
        <textarea v-model="importText" rows="7" placeholder="Paste saved peer list JSON here"></textarea>
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
      <p v-if="feedback" class="feedback">{{ feedback }}</p>
    </div>
  </details>
</template>
