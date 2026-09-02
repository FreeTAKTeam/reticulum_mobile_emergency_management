<script setup lang="ts">
import type { CircleTier, HouseholdStatus } from "@reticulum/node-client";
import { freshnessLabel, householdComposition, statusLabel } from "../utils/communityStatus";
import PowerSaverBadge from "./PowerSaverBadge.vue";

const props = withDefaults(defineProps<{
  householdName?: string; adults?: number; children?: number; pets?: number;
  status?: HouseholdStatus; updatedAtMs?: number; saverActive?: boolean; tier?: CircleTier;
}>(), {
  householdName: "Unknown household", adults: 0, children: 0, pets: 0,
  status: "all_home", updatedAtMs: 0, saverActive: false, tier: undefined,
});
</script>

<template>
  <article class="community-summary">
    <div><strong>{{ householdName }}</strong><p>{{ householdComposition(props) }}</p></div>
    <div class="summary-state"><span class="status">{{ statusLabel(status) }}</span><small>{{ freshnessLabel(updatedAtMs) }}</small></div>
    <span v-if="tier" class="tier" :class="tier">{{ tier === "inner" ? "Inner Circle" : "Outer Circle" }}</span>
    <PowerSaverBadge :active="saverActive" />
  </article>
</template>

<style scoped>
.community-summary { align-items: center; background: linear-gradient(135deg, rgb(7 25 52 / 88%), rgb(8 32 61 / 64%)); border: 1px solid rgb(75 145 210 / 30%); border-radius: 13px; display: grid; gap: .65rem; grid-template-columns: minmax(9rem, 1.5fr) minmax(7rem, 1fr) auto auto; padding: .72rem .82rem; }
strong { color: #e2f2ff; font-family: var(--font-headline); } p, small { color: #8faecc; margin: .15rem 0 0; }.summary-state { display: grid; }.status { color: #7dd3fc; font-family: var(--font-ui); font-size: .76rem; font-weight: 800; text-transform: uppercase; }.tier { border-radius: 999px; font-family: var(--font-ui); font-size: .68rem; font-weight: 800; padding: .25rem .5rem; text-transform: uppercase; }.tier.inner { background: rgb(34 197 94 / 16%); color: #86efac; }.tier.outer { background: rgb(148 163 184 / 14%); color: #cbd5e1; }
@media (max-width: 650px) { .community-summary { grid-template-columns: 1fr auto; }.summary-state { grid-column: 1 / -1; } }
</style>
