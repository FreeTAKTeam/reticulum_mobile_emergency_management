<script setup lang="ts">
import type { CommunityStatusProjectionRecord, HouseholdStatus } from "@reticulum/node-client";
import { ref } from "vue";

import {
  householdComposition,
  sharedAgoLabel,
  statusLabel,
} from "../utils/communityStatus";
import CommunityStatusPicker from "./CommunityStatusPicker.vue";

/**
 * Dashboard household card: one badge with the status your community currently
 * sees. Tapping it opens the four choices; picking one shares it.
 */
defineProps<{
  householdName: string;
  adults: number;
  children: number;
  pets: number;
  status: HouseholdStatus;
  sharedAtMs: number;
  saverActive: boolean;
  others: CommunityStatusProjectionRecord[];
  busy: boolean;
  feedback: string;
}>();
const emit = defineEmits<{ share: [status: HouseholdStatus] }>();
const choosing = ref(false);

/** Always shares the choice: the runtime skips an unchanged beacon, and a first share must go out. */
function choose(status: HouseholdStatus): void {
  choosing.value = false;
  emit("share", status);
}
</script>

<template>
  <section class="panel household-panel" aria-labelledby="household-heading">
    <header class="household-head">
      <h2 id="household-heading">Household status</h2>
      <span class="household-shared">{{ sharedAgoLabel(sharedAtMs) }}</span>
    </header>
    <p class="household-line">
      {{ householdName }} · {{ householdComposition({ adults, children, pets }) }}
      <span v-if="saverActive" class="household-saver">· Power saver on</span>
    </p>
    <button
      type="button"
      :class="['household-current', status]"
      :disabled="busy"
      :aria-expanded="choosing"
      aria-controls="household-status-choices"
      :aria-label="`Household status: ${statusLabel(status)}. Change`"
      @click="choosing = !choosing"
    >
      <span class="household-dot" aria-hidden="true" />
      <span class="household-current-label">{{ statusLabel(status) }}</span>
      <span class="household-change">{{ choosing ? "Cancel" : "Change" }}</span>
    </button>
    <CommunityStatusPicker
      v-if="choosing"
      id="household-status-choices"
      :model-value="status"
      :disabled="busy"
      @update:model-value="choose"
    />
    <p v-if="feedback" class="household-feedback" role="status">{{ feedback }}</p>
    <details v-if="others.length" class="household-others">
      <summary>{{ others.length }} other {{ others.length === 1 ? "household" : "households" }}</summary>
      <ul>
        <li v-for="record in others" :key="record.sourceIdentity">
          <strong>{{ record.householdName }}</strong>
          <span :class="['household-chip', record.status]">{{ statusLabel(record.status) }}</span>
          <small>{{ sharedAgoLabel(record.updatedAtMs).replace("Shared ", "") }}</small>
        </li>
      </ul>
    </details>
  </section>
</template>

<style scoped>
.household-panel { display: grid; gap: .6rem; }
.household-head { align-items: baseline; display: flex; gap: .75rem; justify-content: space-between; }
.household-head h2 { font-family: var(--font-headline); font-size: clamp(1.2rem, 2.4vw, 1.56rem); margin: 0; }
.household-shared { color: #8faecc; font-family: var(--font-ui); font-size: .78rem; white-space: nowrap; }
.household-line { color: #a9c3dd; font-size: .9rem; margin: 0; overflow-wrap: anywhere; }
.household-saver { color: #ffd67a; }
.household-current { --btn-bg: rgb(7 25 52 / 82%); --btn-border: rgb(92 201 255 / 55%); --btn-color: #f0f9ff; --btn-bg-pressed: rgb(16 72 120 / 92%); --btn-color-pressed: #f0f9ff; align-items: center; background: var(--btn-bg); border: 1px solid var(--btn-border); border-radius: 12px; color: var(--btn-color); cursor: pointer; display: flex; font-family: var(--font-ui); gap: .6rem; min-height: 48px; padding: 0 .9rem; text-align: left; width: 100%; }
.household-current-label { flex: 1; font-size: 1rem; font-weight: 800; letter-spacing: .03em; }
.household-change { color: #8fcaff; font-size: .75rem; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; }
.household-dot { background: #4ade80; border-radius: 50%; height: 10px; width: 10px; }
.household-current.one_missing { --btn-border: rgb(251 191 36 / 70%); } .household-current.one_missing .household-dot { background: #fbbf24; }
.household-current.evacuated { --btn-border: rgb(96 165 250 / 70%); } .household-current.evacuated .household-dot { background: #60a5fa; }
.household-current.needs_help { --btn-border: rgb(251 113 133 / 75%); } .household-current.needs_help .household-dot { background: #fb7185; }
.household-feedback { color: #9ce4ff; font-size: .82rem; margin: 0; }
.household-others { border-top: 1px solid rgb(74 120 193 / 25%); padding-top: .55rem; }
.household-others summary { color: #8fcaff; cursor: pointer; font-family: var(--font-ui); font-size: .8rem; font-weight: 700; min-height: 2rem; }
.household-others ul { display: grid; gap: .4rem; list-style: none; margin: .4rem 0 0; padding: 0; }
.household-others li { align-items: center; display: grid; gap: .5rem; grid-template-columns: minmax(0, 1fr) auto auto; }
.household-others strong { color: #e2f2ff; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.household-others small { color: #8faecc; font-size: .75rem; }
.household-chip { border-radius: 999px; font-family: var(--font-ui); font-size: .68rem; font-weight: 800; padding: .18rem .5rem; text-transform: uppercase; }
.household-chip.all_home { background: rgb(74 222 128 / 14%); color: #86efac; }
.household-chip.one_missing { background: rgb(251 191 36 / 16%); color: #fcd34d; }
.household-chip.evacuated { background: rgb(96 165 250 / 16%); color: #93c5fd; }
.household-chip.needs_help { background: rgb(251 113 133 / 18%); color: #fda4af; }
</style>
