<script setup lang="ts">
import { computed } from "vue";

import { useNodeStore } from "../../stores/nodeStore";

interface HubSettingsForm {
  hubMode: "Autonomous" | "SemiAutonomous" | "Connected";
  hubIdentityHash: string;
  hubRefreshIntervalSeconds: number;
}

const props = defineProps<{
  form: HubSettingsForm;
  runNodeAction: (action: () => Promise<void>, successMessage: string) => Promise<void>;
}>();

const nodeStore = useNodeStore();
const directoryDisabled = false;
const announceCandidates = computed(() => nodeStore.hubAnnounceCandidates);
const registrationSummary = computed(() => nodeStore.hubRegistrationSummary);
const summary = computed(() => {
  const cachedPeerCount = nodeStore.hubDirectoryPeers.length;
  const connectedOverride =
    props.form.hubMode === "SemiAutonomous" && nodeStore.effectiveConnectedMode
      ? " | server forcing connected routing"
      : "";
  if (!props.form.hubIdentityHash) {
    if (props.form.hubMode === "Connected") {
      return `${props.form.hubMode} | No hub selected | outbound blocked`;
    }
    if (props.form.hubMode === "SemiAutonomous") {
      return `${props.form.hubMode} | No hub selected | using local discovery until a hub is chosen${connectedOverride}`;
    }
    return `${props.form.hubMode} | No hub selected${connectedOverride}`;
  }
  const peerSummary = cachedPeerCount > 0 ? ` | ${cachedPeerCount} cached peers` : "";
  return `${props.form.hubMode} | ${props.form.hubIdentityHash.slice(0, 10)}...${peerSummary}${connectedOverride}`;
});

function onCandidateSelected(event: Event): void {
  props.form.hubIdentityHash = (event.target as HTMLSelectElement).value.trim();
}
</script>

<template>
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
        <p>{{ summary }}</p>
      </div>
      <span class="chevron" aria-hidden="true">&#9662;</span>
    </summary>
    <div class="panel-body">
      <p class="section-note">
        Autonomous keeps REM peer discovery local. Semi-autonomous uses the shared-TEAM directory
        from <code>rem.registry.team_peers.list</code> for direct or propagated sends and pauses team
        fanout if the directory is unavailable. Connected sends only to the selected RCH so the hub
        redistributes traffic.
      </p>
      <div class="grid">
        <label>
          Mode
          <select v-model="form.hubMode" :disabled="directoryDisabled">
            <option value="Autonomous">Autonomous</option>
            <option value="SemiAutonomous">Semi-autonomous</option>
            <option value="Connected">Connected</option>
          </select>
        </label>
        <label>
          Hub from announces (RCH servers)
          <select
            :value="form.hubIdentityHash"
            :disabled="directoryDisabled"
            @change="onCandidateSelected"
          >
            <option value="">Manual / none</option>
            <option
              v-for="candidate in announceCandidates"
              :key="candidate.destination"
              :value="candidate.destination"
            >
              {{ candidate.label }} ({{ candidate.destination.slice(0, 10) }}...)
            </option>
          </select>
        </label>
        <label>
          Hub identity hash
          <input v-model="form.hubIdentityHash" type="text" :disabled="directoryDisabled" />
        </label>
        <label>
          Refresh interval seconds
          <input
            v-model.number="form.hubRefreshIntervalSeconds"
            type="number"
            min="30"
            :disabled="directoryDisabled"
          />
        </label>
      </div>
      <p v-if="announceCandidates.length === 0" class="section-note">
        No announce entries advertising the RCH server capability set have been seen yet.
      </p>
      <p class="section-note">Hub registration: {{ registrationSummary }}</p>
      <div class="actions">
        <button
          type="button"
          :disabled="directoryDisabled"
          @click="runNodeAction(() => nodeStore.refreshHubDirectory(), 'Hub refresh requested.')"
        >
          Refresh Now
        </button>
        <button
          type="button"
          :disabled="directoryDisabled"
          @click="runNodeAction(() => nodeStore.bootstrapHubRegistration(true), 'Hub registration requested.')"
        >
          Register Team Member
        </button>
        <button
          type="button"
          :disabled="directoryDisabled"
          @click="runNodeAction(() => nodeStore.forgetHubRegistryLinkage(), 'Hub registration cleared.')"
        >
          Clear Registration
        </button>
      </div>
    </div>
  </details>
</template>
