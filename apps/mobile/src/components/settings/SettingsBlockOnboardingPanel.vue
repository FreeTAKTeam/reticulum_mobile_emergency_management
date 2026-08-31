<script setup lang="ts">
import type { BlockOnboardingInspection, BlockRadioSettings } from "@reticulum/node-client";
import { computed, ref } from "vue";
import { useNodeStore } from "../../stores/nodeStore";
import BlockOnboardingReview from "../BlockOnboardingReview.vue";

const nodeStore = useNodeStore();
const encodedText = ref("");
const scanText = ref("");
const qrDataUrl = ref("");
const inspection = ref<BlockOnboardingInspection | null>(null);
const feedback = ref("");
const busy = ref(false);
const expiresInHours = ref(72);
const nativeReady = computed(() => nodeStore.status.running);

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function createCode(): Promise<void> {
  busy.value = true; feedback.value = ""; inspection.value = null;
  try {
    const radio: BlockRadioSettings | undefined = nodeStore.settings.rnode.enabled ? {
      region: nodeStore.settings.rnode.region,
      profile: nodeStore.settings.rnode.profile,
      frequencyHz: nodeStore.settings.rnode.frequencyHz,
    } : undefined;
    const envelope = await nodeStore.createBlockOnboardingCode({
      network: {
        tcpClients: [...nodeStore.settings.tcpClients],
        broadcast: nodeStore.settings.broadcast,
        hubMode: nodeStore.settings.hub.mode,
        hubIdentityHash: nodeStore.settings.hub.identityHash || undefined,
        hubApiBaseUrl: nodeStore.settings.hub.apiBaseUrl || undefined,
        hubRefreshIntervalSeconds: nodeStore.settings.hub.refreshIntervalSeconds,
        radio,
      },
      trustedDestinationHashes: nodeStore.savedPeers.map((peer) => peer.destination).slice(0, 32),
      preferredMapLayer: nodeStore.settings.community.preferredMapLayer,
      expiresAtMs: Date.now() + Math.min(168, Math.max(1, expiresInHours.value)) * 3_600_000,
    });
    encodedText.value = envelope.encodedText;
    const { toDataURL } = await import("qrcode");
    qrDataUrl.value = await toDataURL(encodedText.value, { errorCorrectionLevel: "M", margin: 3, width: 720 });
    feedback.value = "Signed Block Code created by the native Reticulum identity.";
  } catch (error: unknown) { feedback.value = errorText(error); }
  finally { busy.value = false; }
}

async function copyCode(): Promise<void> {
  try { await navigator.clipboard.writeText(encodedText.value); feedback.value = "Block Code copied."; }
  catch (error: unknown) { feedback.value = errorText(error); }
}

async function inspectCode(payload = scanText.value): Promise<void> {
  busy.value = true; feedback.value = "";
  try {
    const value = payload.trim();
    if (!value) throw new Error("Paste or scan a Block Code first.");
    scanText.value = value;
    inspection.value = await nodeStore.inspectBlockOnboardingCode(value);
  } catch (error: unknown) { feedback.value = errorText(error); }
  finally { busy.value = false; }
}

async function scanCode(): Promise<void> {
  busy.value = true; feedback.value = "";
  try {
    const scanner = await import("@capacitor/barcode-scanner");
    const result = await scanner.CapacitorBarcodeScanner.scanBarcode({
      hint: scanner.CapacitorBarcodeScannerTypeHint.QR_CODE,
      scanInstructions: "Scan a signed REM Block Code",
      scanButton: false,
      cameraDirection: scanner.CapacitorBarcodeScannerCameraDirection.BACK,
      scanOrientation: scanner.CapacitorBarcodeScannerScanOrientation.ADAPTIVE,
      android: { scanningLibrary: scanner.CapacitorBarcodeScannerAndroidScanningLibrary.ZXING },
      web: { showCameraSelection: true, scannerFPS: 10 },
    });
    await inspectCode(result.ScanResult);
  } catch (error: unknown) { feedback.value = errorText(error); busy.value = false; }
}

function imported(count: number): void {
  inspection.value = null; scanText.value = "";
  feedback.value = `Block imported securely. ${count} peer${count === 1 ? "" : "s"} classified.`;
}
</script>

<template>
  <section class="settings-panel block-panel">
    <header><div><p class="eyebrow">TRUSTED ONBOARDING</p><h2>Signed Block Code</h2></div><span class="native-badge">Rust verified</span></header>
    <p>Create an expiring, signed configuration envelope. Private identity material and device-local secrets are never included.</p>
    <div class="block-columns">
      <section><h3>Create</h3><label><span>Expires in</span><select v-model.number="expiresInHours"><option :value="24">24 hours</option><option :value="72">3 days</option><option :value="168">7 days</option></select></label><button type="button" :disabled="busy || !nativeReady" @click="createCode">{{ busy ? "Working…" : "Create signed code" }}</button><small v-if="!nativeReady">Start the native node to sign with your Reticulum identity.</small></section>
      <section><h3>Scan or paste</h3><textarea v-model="scanText" rows="3" placeholder="REMBC1:…" aria-label="Signed Block Code text" /><div class="actions"><button type="button" :disabled="busy" @click="scanCode">Scan QR</button><button type="button" :disabled="busy || !scanText.trim()" @click="inspectCode()">Inspect natively</button></div></section>
    </div>
    <div v-if="qrDataUrl" class="created-code"><img :src="qrDataUrl" alt="Signed REM Block Code QR" /><div><strong>Ready to share</strong><p>{{ encodedText.length }} / 1,999 bytes · QR correction level M</p><button type="button" @click="copyCode">Copy opaque code</button></div></div>
    <p v-if="feedback" class="feedback" role="status">{{ feedback }}</p>
    <BlockOnboardingReview v-if="inspection" :encoded-text="scanText" :inspection="inspection" @cancel="inspection = null" @imported="imported" />
  </section>
</template>

<style scoped>
.settings-panel { background: rgb(5 18 40 / 72%); border: 1px solid rgb(78 142 202 / 30%); border-radius: 14px; margin-bottom: 1rem; padding: 1rem; }.settings-panel > header { align-items: start; display: flex; justify-content: space-between; }.settings-panel h2, h3 { color: #e0f2fe; margin: 0; }.settings-panel > p { color: #91aac6; }.eyebrow { color: #38bdf8 !important; font-family: var(--font-ui); font-size: .67rem; font-weight: 800; letter-spacing: .13em; margin: 0 0 .2rem; }.native-badge { background: rgb(34 197 94 / 13%); border: 1px solid rgb(74 222 128 / 30%); border-radius: 999px; color: #86efac; font-size: .69rem; font-weight: 800; padding: .28rem .55rem; text-transform: uppercase; }.block-columns { display: grid; gap: .8rem; grid-template-columns: 1fr 1fr; }.block-columns section { background: rgb(2 13 31 / 48%); border: 1px solid rgb(75 135 196 / 20%); border-radius: 11px; display: grid; gap: .6rem; padding: .75rem; }label { display: grid; gap: .25rem; }label span, small { color: #91aac6; font-size: .72rem; }textarea, select { background: #06152d; border: 1px solid rgb(80 145 205 / 40%); border-radius: 8px; color: #e0f2fe; min-height: 38px; padding: .55rem; }.actions { display: flex; flex-wrap: wrap; gap: .5rem; }.created-code { align-items: center; background: rgb(4 16 34 / 58%); border-radius: 12px; display: grid; gap: 1rem; grid-template-columns: 9rem 1fr; margin: .85rem 0; padding: .75rem; }.created-code img { background: #fff; border-radius: 8px; max-width: 9rem; width: 100%; }.created-code strong { color: #dff4ff; }.created-code p, .feedback { color: #9ab5d1; }.feedback { overflow-wrap: anywhere; }
@media (max-width: 700px) { .block-columns { grid-template-columns: 1fr; }.created-code { grid-template-columns: 6rem 1fr; } }
</style>
