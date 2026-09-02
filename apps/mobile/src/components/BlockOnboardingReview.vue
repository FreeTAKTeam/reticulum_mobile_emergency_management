<script setup lang="ts">
import type { BlockOnboardingInspection, CircleTier } from "@reticulum/node-client";
import { computed, reactive, ref } from "vue";
import { useNodeStore } from "../stores/nodeStore";
import { completePeerTierMap, onboardingDestinations } from "../utils/blockOnboardingView";

const props = defineProps<{ encodedText: string; inspection: BlockOnboardingInspection }>();
const emit = defineEmits<{ cancel: []; imported: [count: number] }>();
const nodeStore = useNodeStore();
const confirmedFingerprint = ref("");
const issuerTier = ref<CircleTier>("outer");
const overrides = reactive<Record<string, CircleTier>>({});
const busy = ref(false);
const errorMessage = ref("");
const community = reactive({ ...nodeStore.settings.community });
const destinations = computed(() => onboardingDestinations(props.inspection));
const issuerDestinations = computed(() => new Set([
  props.inspection.issuerAppDestinationHex,
].map((value) => value.toLowerCase())));
const fingerprintMatches = computed(() =>
  confirmedFingerprint.value.trim().toLowerCase() === props.inspection.signerFingerprint.toLowerCase(),
);

async function commit(): Promise<void> {
  if (!fingerprintMatches.value) return;
  busy.value = true;
  errorMessage.value = "";
  try {
    const result = await nodeStore.importBlockOnboardingCode({
      encodedText: props.encodedText,
      confirmedSignerFingerprint: confirmedFingerprint.value.trim(),
      community: { ...community, roleBadges: [...community.roleBadges] },
      peerTiers: completePeerTierMap(props.inspection, issuerTier.value, overrides),
    });
    emit("imported", result.importedPeerCount);
  } catch (error: unknown) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally { busy.value = false; }
}
</script>

<template>
  <section class="review" aria-labelledby="block-review-title">
    <header><div><p class="eyebrow">SIGNED BLOCK CODE</p><h3 id="block-review-title">Review before import</h3></div><span>Expires {{ new Date(inspection.expiresAtMs).toLocaleString() }}</span></header>
    <dl>
      <div><dt>Signer fingerprint</dt><dd>{{ inspection.signerFingerprint }}</dd></div>
      <div><dt>Issued</dt><dd>{{ new Date(inspection.issuedAtMs).toLocaleString() }}</dd></div>
      <div><dt>Hub mode</dt><dd>{{ inspection.network.hubMode }}</dd></div>
      <div><dt>Hub identity</dt><dd>{{ inspection.network.hubIdentityHash || "None" }}</dd></div>
      <div><dt>Hub API</dt><dd>{{ inspection.network.hubApiBaseUrl || "None" }}</dd></div>
      <div><dt>Hub refresh</dt><dd>{{ inspection.network.hubRefreshIntervalSeconds }} seconds</dd></div>
      <div><dt>TCP endpoints</dt><dd>{{ inspection.network.tcpClients.join(", ") || "None" }}</dd></div>
      <div><dt>Broadcast</dt><dd>{{ inspection.network.broadcast ? "Enabled" : "Disabled" }}</dd></div>
      <div><dt>Radio</dt><dd>{{ inspection.network.radio ? `${inspection.network.radio.region} · ${inspection.network.radio.profile} · ${inspection.network.radio.frequencyHz} Hz` : "No imported radio profile" }}</dd></div>
      <div><dt>Map layer</dt><dd>{{ inspection.preferredMapLayer }}</dd></div>
    </dl>
    <div class="household-grid"><label><span>Household name</span><input v-model="community.householdName" maxlength="64" /></label><label><span>Household ID</span><input v-model="community.householdId" maxlength="16" /></label><label><span>Adults</span><input v-model.number="community.adults" type="number" min="0" max="20" /></label><label><span>Children</span><input v-model.number="community.children" type="number" min="0" max="20" /></label><label><span>Pets</span><input v-model.number="community.pets" type="number" min="0" max="20" /></label><label class="role-field"><span>Role badges</span><input :value="community.roleBadges.join(', ')" maxlength="128" placeholder="Medic, radio operator" @input="community.roleBadges = [...new Set(($event.target as HTMLInputElement).value.split(',').map((role) => role.trim()).filter(Boolean))].slice(0, 5)" /></label></div>
    <div class="tier-list"><h4>Circle access</h4><label v-for="destination in destinations" :key="destination"><code>{{ destination }}</code><select v-if="issuerDestinations.has(destination)" v-model="issuerTier"><option value="outer">Outer Circle</option><option value="inner">Inner Circle</option></select><select v-else v-model="overrides[destination]"><option :value="undefined">Outer Circle (default)</option><option value="outer">Outer Circle</option><option value="inner">Inner Circle</option></select></label></div>
    <label class="fingerprint"><span>Type the signer fingerprint to confirm</span><input v-model="confirmedFingerprint" autocomplete="off" spellcheck="false" /></label>
    <p v-if="errorMessage" class="error" role="alert">{{ errorMessage }}</p>
    <div class="actions"><button type="button" @click="emit('cancel')">Cancel</button><button type="button" class="primary" :disabled="busy || !fingerprintMatches" @click="commit">{{ busy ? "Importing…" : "Verify again & import" }}</button></div>
  </section>
</template>

<style scoped>
.review { background: #071a38; border: 1px solid rgb(56 189 248 / 48%); border-radius: 14px; display: grid; gap: .85rem; padding: 1rem; }.review header { align-items: start; display: flex; justify-content: space-between; }.review h3, h4 { color: #e0f2fe; margin: 0; }.review header span, dt { color: #8ca7c4; font-size: .72rem; }.eyebrow { color: #38bdf8; font-size: .67rem; font-weight: 800; letter-spacing: .12em; margin: 0 0 .18rem; }dl { display: grid; gap: .45rem; margin: 0; }dl div { display: grid; gap: .4rem; grid-template-columns: 9rem 1fr; }dd { color: #c9e4f8; margin: 0; overflow-wrap: anywhere; }.household-grid { display: grid; gap: .55rem; grid-template-columns: 2fr 1.3fr repeat(3, .55fr); }.role-field { grid-column: 1 / -1; }label { display: grid; gap: .25rem; }label span { color: #96b0cc; font-size: .7rem; font-weight: 700; text-transform: uppercase; }input, select { background: #06152d; border: 1px solid rgb(80 145 205 / 40%); border-radius: 8px; color: #e0f2fe; min-height: 38px; padding: 0 .55rem; }.tier-list { display: grid; gap: .45rem; }.tier-list label { align-items: center; grid-template-columns: 1fr 11rem; }.tier-list code { color: #a8cce8; font-size: .7rem; overflow: hidden; text-overflow: ellipsis; }.fingerprint input { font-family: monospace; }.actions { display: flex; gap: .55rem; justify-content: flex-end; }.primary { --btn-color: #06152d; --btn-bg: #7dd3fc; }.error { color: #fda4af; margin: 0; }
@media (max-width: 720px) { .household-grid { grid-template-columns: repeat(3, 1fr); }.household-grid label:nth-child(-n+2) { grid-column: 1 / -1; }.tier-list label { grid-template-columns: 1fr; } }
</style>
