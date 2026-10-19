<script setup lang="ts">
import { computed } from "vue";

import { useNodeStore } from "../../../stores/nodeStore";
import type { RoutingDraft } from "../settingsFormSlices";

/** Routing mode and hub form body, shared by Settings > Routing and the setup wizard. */
const props = defineProps<{ draft: RoutingDraft }>();
const nodeStore = useNodeStore();

const modes: Array<{ value: RoutingDraft["hubMode"]; label: string; description: string }> = [
  { value: "Autonomous", label: "Autonomous", description: "Find peers locally. No hub needed." },
  {
    value: "SemiAutonomous",
    label: "Semi-autonomous",
    description: "Use the hub's team directory, send directly. Falls back to local discovery.",
  },
  { value: "Connected", label: "Connected", description: "Send everything through the selected hub." },
];

const usesHub = computed(() => props.draft.hubMode !== "Autonomous");
const announceCandidates = computed(() => nodeStore.hubAnnounceCandidates);

function onCandidateSelected(event: Event): void {
  props.draft.hubIdentityHash = (event.target as HTMLSelectElement).value.trim();
}
</script>

<template>
  <section class="panel settings-card">
    <fieldset class="mode-options">
      <legend>Mode <span class="restart-badge">restart</span></legend>
      <label v-for="mode in modes" :key="mode.value" class="mode-option">
        <input v-model="draft.hubMode" type="radio" name="hub-mode" :value="mode.value" />
        <span>
          <strong>{{ mode.label }}</strong>
          <small>{{ mode.description }}</small>
        </span>
      </label>
    </fieldset>

    <div v-if="usesHub" class="grid">
      <label>
        Hub from announces
        <select :value="draft.hubIdentityHash" @change="onCandidateSelected">
          <option value="">Manual / none</option>
          <option v-for="candidate in announceCandidates" :key="candidate.destination" :value="candidate.destination">
            {{ candidate.label }} ({{ candidate.destination.slice(0, 10) }}…)
          </option>
        </select>
      </label>
      <label>
        <span>Hub identity hash <span class="restart-badge">restart</span></span>
        <input v-model="draft.hubIdentityHash" type="text" />
      </label>
      <label>
        Refresh interval (seconds)
        <input v-model.number="draft.hubRefreshIntervalSeconds" type="number" min="30" />
      </label>
    </div>
    <p v-if="usesHub && announceCandidates.length === 0" class="section-note">
      No RCH hub announces have been heard yet. Paste the hub identity hash instead.
    </p>
  </section>
</template>
