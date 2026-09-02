<script setup lang="ts">
import type { PreferredMapLayer } from "@reticulum/node-client";

interface CommunityForm {
  householdId: string; householdName: string; adults: number; children: number; pets: number;
  roleBadgesText: string; preferredMapLayer: PreferredMapLayer;
  powerEnabled: boolean; powerThresholdPercent: 10 | 20 | 30;
}
defineProps<{ form: CommunityForm }>();
</script>

<template>
  <section class="settings-panel community-panel">
    <header><div><p class="eyebrow">COMMUNITY PROFILE</p><h2>Household & power</h2></div><p>Share only a compact, GPS-free household summary.</p></header>
    <div class="community-grid">
      <label class="wide"><span>Household name</span><input v-model="form.householdName" maxlength="64" placeholder="Harbour House" /></label>
      <label><span>Household ID</span><input v-model="form.householdId" maxlength="16" pattern="[0-9a-fA-F]{16}" placeholder="16 hex characters" /></label>
      <label><span>Adults</span><input v-model.number="form.adults" type="number" min="0" max="20" /></label>
      <label><span>Children</span><input v-model.number="form.children" type="number" min="0" max="20" /></label>
      <label><span>Pets</span><input v-model.number="form.pets" type="number" min="0" max="20" /></label>
      <label class="wide"><span>Role badges</span><input v-model="form.roleBadgesText" maxlength="128" placeholder="Medic, radio operator (up to five)" /><small>Comma-separated; public in community status.</small></label>
      <label><span>Preferred map</span><select v-model="form.preferredMapLayer"><option value="base">Base</option><option value="satellite">Satellite</option></select></label>
    </div>
    <div class="power-row">
      <label class="toggle"><input v-model="form.powerEnabled" type="checkbox" /><span>Automatic power saver</span></label>
      <label><span>Activate at</span><select v-model.number="form.powerThresholdPercent" :disabled="!form.powerEnabled"><option :value="10">10%</option><option :value="20">20%</option><option :value="30">30%</option></select></label>
      <p>Rust applies a 3% exit hysteresis and preserves emergency traffic.</p>
    </div>
  </section>
</template>

<style scoped>
.settings-panel { background: rgb(5 18 40 / 72%); border: 1px solid rgb(78 142 202 / 30%); border-radius: 14px; margin-bottom: 1rem; padding: 1rem; }.settings-panel header { align-items: end; display: flex; justify-content: space-between; }.settings-panel h2 { color: #e0f2fe; margin: 0; }.settings-panel p { color: #91aac6; margin: .2rem 0; }.eyebrow { color: #38bdf8 !important; font-family: var(--font-ui); font-size: .67rem; font-weight: 800; letter-spacing: .13em; }.community-grid { display: grid; gap: .7rem; grid-template-columns: 2fr repeat(3, minmax(5rem, .55fr)); margin-top: .85rem; }.wide { grid-column: span 2; }label { display: grid; gap: .28rem; }label > span { color: #9bb5d0; font-family: var(--font-ui); font-size: .7rem; font-weight: 750; letter-spacing: .06em; text-transform: uppercase; }input, select { background: #071a37; border: 1px solid rgb(73 145 204 / 38%); border-radius: 9px; color: #dceeff; min-height: 40px; padding: 0 .65rem; }.power-row { align-items: end; border-top: 1px solid rgb(73 145 204 / 20%); display: grid; gap: .8rem; grid-template-columns: 1fr 9rem 2fr; margin-top: .9rem; padding-top: .85rem; }.toggle { align-items: center; display: flex; }.toggle input { min-height: auto; }.power-row p, small { color: #819bb8; font-size: .78rem; }
@media (max-width: 720px) { .community-grid { grid-template-columns: repeat(2, 1fr); }.wide { grid-column: 1 / -1; }.power-row { align-items: stretch; grid-template-columns: 1fr; }.settings-panel header { align-items: start; display: grid; } }
</style>
