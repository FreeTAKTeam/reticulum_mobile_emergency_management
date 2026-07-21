<script setup lang="ts">
import { ref, watch } from "vue";

import { encodeLocalTeamQrExchange } from "../utils/localTeamExchange";

interface QrTeamOption {
  teamUid: string;
  label: string;
  memberDestinations: string[];
}

const props = withDefaults(defineProps<{
  teams: QrTeamOption[];
  showLauncher?: boolean;
}>(), {
  showLauncher: true,
});
const emit = defineEmits<{
  error: [message: string];
  import: [payload: string];
}>();
const selectedTeamUid = ref("");
const qrDataUrl = ref("");
const qrLabel = ref("");
const busy = ref(false);
const errorMessage = ref("");

watch(
  () => props.teams,
  (teams) => {
    if (!teams.some(({ teamUid }) => teamUid === selectedTeamUid.value)) {
      selectedTeamUid.value = teams[0]?.teamUid ?? "";
    }
  },
  { immediate: true, deep: true },
);

function selectedTeam(): QrTeamOption {
  const team = props.teams.find(({ teamUid }) => teamUid === selectedTeamUid.value);
  if (!team) throw new Error("Select a local team first.");
  return team;
}

async function showQrCode(teamUid?: string): Promise<void> {
  busy.value = true;
  errorMessage.value = "";
  try {
    if (teamUid) selectedTeamUid.value = teamUid;
    const team = selectedTeam();
    const payload = encodeLocalTeamQrExchange(team.teamUid, team.memberDestinations);
    const { toDataURL } = await import("qrcode");
    qrDataUrl.value = await toDataURL(payload, {
      errorCorrectionLevel: "M",
      margin: 4,
      width: 720,
      color: { dark: "#031329ff", light: "#ffffffff" },
    });
    qrLabel.value = team.label;
  } catch (error: unknown) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
    emit("error", errorMessage.value);
  } finally {
    busy.value = false;
  }
}

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
      scanInstructions: "Scan a QR code exported by another REM client",
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

function closeQrCode(): void {
  qrDataUrl.value = "";
  qrLabel.value = "";
}

defineExpose({ scanQrCode, showQrCode });
</script>

<template>
  <section v-if="showLauncher" class="team-qr-panel" aria-labelledby="team-qr-heading">
    <div>
      <h3 id="team-qr-heading">QR team exchange</h3>
      <p>Show a local team QR code or scan one from another REM client.</p>
    </div>
    <label>
      <span>Local team</span>
      <select v-model="selectedTeamUid" :disabled="busy || teams.length === 0">
        <option v-for="team in teams" :key="team.teamUid" :value="team.teamUid">
          {{ team.label }} · {{ team.memberDestinations.length }} members
        </option>
      </select>
    </label>
    <div class="team-qr-actions">
      <button type="button" :disabled="busy || teams.length === 0" @click="showQrCode()">
        {{ busy ? "Working…" : "Show QR" }}
      </button>
      <button type="button" :disabled="busy" @click="scanQrCode">Scan team QR</button>
    </div>
    <p v-if="errorMessage" class="team-qr-error" role="alert">{{ errorMessage }}</p>
  </section>

  <div v-if="qrDataUrl" class="team-qr-overlay" role="dialog" aria-modal="true" :aria-label="`${qrLabel} team QR code`">
    <section class="team-qr-dialog">
      <h3>{{ qrLabel }}</h3>
      <p>Scan this code from <strong>Peers → Scan team QR</strong> on another REM client.</p>
      <img :src="qrDataUrl" :alt="`${qrLabel} local team QR code`" />
      <p class="team-qr-private">Local aliases and peer labels are not included.</p>
      <button type="button" @click="closeQrCode">Close QR</button>
    </section>
  </div>
</template>

<style scoped>
.team-qr-panel { align-items: end; background: rgb(5 17 39 / 50%); border: 1px solid rgb(78 123 196 / 26%); border-radius: 13px; display: grid; gap: 0.65rem; grid-template-columns: minmax(12rem, 1fr) minmax(12rem, 0.55fr) auto; margin-bottom: 0.68rem; padding: 0.65rem; }
.team-qr-panel h3, .team-qr-dialog h3 { color: #d8ecff; font-family: var(--font-headline); margin: 0; }
.team-qr-panel p, .team-qr-dialog p { color: #90a9d2; font-family: var(--font-body); margin: 0.2rem 0 0; }
.team-qr-panel label { display: grid; gap: 0.28rem; }
.team-qr-panel label span { color: #90a9d2; font-family: var(--font-ui); font-size: 0.72rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; }
.team-qr-panel select { background: rgb(5 17 39 / 92%); border: 1px solid rgb(73 173 255 / 40%); border-radius: 9px; color: #d8ecff; font-family: var(--font-body); min-height: 2.35rem; padding: 0 0.65rem; }
.team-qr-actions { display: flex; flex-wrap: wrap; gap: 0.5rem; }
.team-qr-error { color: #fecaca !important; grid-column: 1 / -1; }
.team-qr-overlay { align-items: center; background: rgb(0 7 20 / 88%); display: flex; inset: 0; justify-content: center; padding: 1rem; position: fixed; z-index: 1200; }
.team-qr-dialog { background: #071a38; border: 1px solid rgb(92 201 255 / 65%); border-radius: 16px; box-shadow: 0 20px 70px rgb(0 0 0 / 55%); display: grid; gap: 0.8rem; justify-items: center; max-height: calc(100vh - 2rem); max-width: 42rem; overflow: auto; padding: 1rem; text-align: center; width: 100%; }
.team-qr-dialog img { background: #fff; border-radius: 10px; display: block; height: auto; max-width: min(100%, 720px); width: 100%; }
.team-qr-private { font-size: 0.88rem; }
button { --btn-bg: linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%)); --btn-bg-pressed: linear-gradient(180deg, rgb(196 240 255 / 96%), rgb(118 212 255 / 94%)); --btn-border: rgb(74 133 207 / 45%); --btn-border-pressed: rgb(224 248 255 / 86%); --btn-shadow: inset 0 1px 0 rgb(209 244 255 / 10%), 0 8px 18px rgb(2 14 32 / 18%); --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 21 47 / 24%); --btn-color: #8fdbff; --btn-color-pressed: #042541; background: var(--btn-bg); border: 1px solid var(--btn-border); border-radius: 999px; box-shadow: var(--btn-shadow); color: var(--btn-color); cursor: pointer; font-family: var(--font-ui); font-size: 0.78rem; font-weight: 700; letter-spacing: 0.08em; min-height: 32px; padding: 0 0.82rem; text-transform: uppercase; }
button:disabled { cursor: not-allowed; opacity: 0.55; }
@media (max-width: 760px) { .team-qr-panel { align-items: stretch; grid-template-columns: 1fr; } }
</style>
