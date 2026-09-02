<script setup lang="ts">
import { ref } from "vue";

withDefaults(defineProps<{
  showLauncher?: boolean;
}>(), {
  showLauncher: true,
});
const emit = defineEmits<{
  error: [message: string];
  import: [payload: string];
}>();
const busy = ref(false);
const errorMessage = ref("");

async function scanQrCode(): Promise<void> {
  busy.value = true;
  errorMessage.value = "";
  try {
    const {
      CapacitorBarcodeScanner,
      CapacitorBarcodeScannerAndroidScanningLibrary,
      CapacitorBarcodeScannerCameraDirection,
      CapacitorBarcodeScannerScanOrientation,
      CapacitorBarcodeScannerTypeHint,
    } = await import("@capacitor/barcode-scanner");
    const result = await CapacitorBarcodeScanner.scanBarcode({
      hint: CapacitorBarcodeScannerTypeHint.QR_CODE,
      scanInstructions: "Scan a legacy REM local-team QR code",
      scanButton: false,
      cameraDirection: CapacitorBarcodeScannerCameraDirection.BACK,
      scanOrientation: CapacitorBarcodeScannerScanOrientation.ADAPTIVE,
      android: { scanningLibrary: CapacitorBarcodeScannerAndroidScanningLibrary.ZXING },
      web: { showCameraSelection: true, scannerFPS: 10 },
    });
    const payload = result.ScanResult.trim();
    if (!payload) throw new Error("No QR code was read.");
    emit("import", payload);
  } catch (error: unknown) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
    emit("error", errorMessage.value);
  } finally {
    busy.value = false;
  }
}

defineExpose({ scanQrCode });
</script>

<template>
  <section v-if="showLauncher" class="team-qr-panel" aria-labelledby="team-qr-heading">
    <div>
      <h3 id="team-qr-heading">Legacy team QR import</h3>
      <p>Import only. New exports use a signed Block Code.</p>
    </div>
    <div class="team-qr-actions">
      <button type="button" :disabled="busy" @click="scanQrCode">{{ busy ? "Scanning…" : "Scan legacy QR" }}</button>
    </div>
    <p v-if="errorMessage" class="team-qr-error" role="alert">{{ errorMessage }}</p>
  </section>

</template>

<style scoped>
.team-qr-panel { align-items: end; background: rgb(5 17 39 / 50%); border: 1px solid rgb(78 123 196 / 26%); border-radius: 13px; display: grid; gap: 0.65rem; grid-template-columns: minmax(12rem, 1fr) minmax(12rem, 0.55fr) auto; margin-bottom: 0.68rem; padding: 0.65rem; }
.team-qr-panel h3 { color: #d8ecff; font-family: var(--font-headline); margin: 0; }
.team-qr-panel p { color: #90a9d2; font-family: var(--font-body); margin: 0.2rem 0 0; }
.team-qr-actions { display: flex; flex-wrap: wrap; gap: 0.5rem; }
.team-qr-error { color: #fecaca !important; grid-column: 1 / -1; }
button { --btn-bg: linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%)); --btn-bg-pressed: linear-gradient(180deg, rgb(196 240 255 / 96%), rgb(118 212 255 / 94%)); --btn-border: rgb(74 133 207 / 45%); --btn-border-pressed: rgb(224 248 255 / 86%); --btn-shadow: inset 0 1px 0 rgb(209 244 255 / 10%), 0 8px 18px rgb(2 14 32 / 18%); --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 21 47 / 24%); --btn-color: #8fdbff; --btn-color-pressed: #042541; background: var(--btn-bg); border: 1px solid var(--btn-border); border-radius: 999px; box-shadow: var(--btn-shadow); color: var(--btn-color); cursor: pointer; font-family: var(--font-ui); font-size: 0.78rem; font-weight: 700; letter-spacing: 0.08em; min-height: 32px; padding: 0 0.82rem; text-transform: uppercase; }
button:disabled { cursor: not-allowed; opacity: 0.55; }
@media (max-width: 760px) { .team-qr-panel { align-items: stretch; grid-template-columns: 1fr; } }
</style>
