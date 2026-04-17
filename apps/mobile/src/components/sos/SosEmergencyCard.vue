<script setup lang="ts">
import { DEFAULT_SOS_SETTINGS, type SosSettingsRecord } from "@reticulum/node-client";
import { computed, onMounted, reactive, ref, watch } from "vue";

import { useSosStore } from "../../stores/sosStore";

const sosStore = useSosStore();
const pin = ref("");
const feedback = ref("");
const form = reactive<SosSettingsRecord>({ ...DEFAULT_SOS_SETTINGS });

const summary = computed(() => {
  if (!form.enabled) {
    return "Disabled";
  }
  const triggers = [
    form.triggerShake ? "shake" : "",
    form.triggerTapPattern ? "tap" : "",
    form.triggerPowerButton ? "power" : "",
  ].filter(Boolean);
  return triggers.length > 0
    ? `Enabled | ${triggers.join(", ")} | ${form.countdownSeconds}s`
    : `Enabled | manual only | ${form.countdownSeconds}s`;
});

function syncFromStore(): void {
  Object.assign(form, sosStore.settings);
}

async function save(): Promise<void> {
  await sosStore.saveSettings({
    ...form,
    countdownSeconds: Math.max(0, Number(form.countdownSeconds || 0)),
    shakeSensitivity: Math.max(1, Number(form.shakeSensitivity || 2.5)),
    audioDurationSeconds: Math.min(60, Math.max(15, Number(form.audioDurationSeconds || 30))),
    updateIntervalSeconds: Math.max(30, Number(form.updateIntervalSeconds || 120)),
  });
  feedback.value = "SOS settings saved.";
}

async function savePin(): Promise<void> {
  await sosStore.setPin(pin.value.trim() || undefined);
  pin.value = "";
  feedback.value = "SOS PIN updated.";
}

onMounted(async () => {
  await sosStore.init();
  syncFromStore();
});

watch(() => ({ ...sosStore.settings }), syncFromStore);
</script>

<template>
  <details class="panel fold-panel" open>
    <summary class="panel-summary">
      <div class="summary-copy">
        <span class="summary-icon" aria-hidden="true">
          <svg class="summary-icon-svg" viewBox="0 0 24 24" fill="none">
            <path d="M12 3.8 3.8 18.2h16.4L12 3.8Z" />
            <path d="M12 9v4.3" />
            <path d="M12 16.5h.01" />
          </svg>
        </span>
        <h2>SOS Emergency</h2>
        <p>{{ summary }}</p>
      </div>
      <span class="chevron" aria-hidden="true">&#9662;</span>
    </summary>

    <div class="panel-body">
      <div class="grid">
        <label class="checkbox">
          <input v-model="form.enabled" type="checkbox" />
          Enable SOS
        </label>
        <label>
          Message template
          <textarea v-model="form.messageTemplate" rows="3" maxlength="240" />
        </label>
        <label>
          Emergency end template
          <textarea v-model="form.cancelMessageTemplate" rows="2" maxlength="180" />
        </label>
        <label>
          Countdown seconds
          <input v-model.number="form.countdownSeconds" min="0" max="30" type="number" />
        </label>
        <label class="checkbox">
          <input v-model="form.includeLocation" type="checkbox" />
          Include GPS and battery
        </label>
      </div>

      <div class="grid">
        <label class="checkbox">
          <input v-model="form.triggerShake" type="checkbox" />
          Shake trigger
        </label>
        <label class="checkbox">
          <input v-model="form.triggerTapPattern" type="checkbox" />
          Tap pattern trigger
        </label>
        <label class="checkbox">
          <input v-model="form.triggerPowerButton" type="checkbox" />
          Power button trigger
        </label>
        <label>
          Shake sensitivity
          <input v-model.number="form.shakeSensitivity" min="1" max="6" step="0.1" type="number" />
        </label>
      </div>

      <div class="grid">
        <label class="checkbox">
          <input v-model="form.periodicUpdates" type="checkbox" />
          Periodic updates
        </label>
        <label>
          Update interval seconds
          <input v-model.number="form.updateIntervalSeconds" min="30" type="number" />
        </label>
        <label class="checkbox">
          <input v-model="form.audioRecording" type="checkbox" />
          Record ambient audio
        </label>
        <label>
          Audio seconds
          <input v-model.number="form.audioDurationSeconds" min="15" max="60" type="number" />
        </label>
      </div>

      <div class="grid">
        <label class="checkbox">
          <input v-model="form.floatingButton" type="checkbox" />
          Floating SOS button
        </label>
        <label class="checkbox">
          <input v-model="form.silentAutoAnswer" type="checkbox" />
          Silent auto-answer
        </label>
        <label>
          Deactivation PIN
          <input v-model="pin" autocomplete="new-password" inputmode="numeric" type="password" />
        </label>
        <div class="actions inline-actions">
          <button type="button" @click="savePin">Set PIN</button>
          <button type="button" @click="sosStore.setPin(undefined)">Clear PIN</button>
        </div>
      </div>

      <div class="actions">
        <button type="button" :disabled="sosStore.busy" @click="save">Save SOS</button>
        <button type="button" class="danger" @click="sosStore.trigger('Manual')">Trigger SOS</button>
      </div>
      <p v-if="feedback" class="feedback">{{ feedback }}</p>
      <p v-if="sosStore.lastError" class="feedback">{{ sosStore.lastError }}</p>
    </div>
  </details>
</template>

<style scoped>
.panel {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 90%), rgb(7 16 37 / 92%)),
    radial-gradient(circle at 10% 10%, rgb(13 152 255 / 14%), transparent 38%);
  border: 1px solid rgb(74 120 193 / 33%);
  border-radius: 16px;
}

.fold-panel {
  overflow: hidden;
}

.panel-summary {
  align-items: center;
  cursor: pointer;
  display: flex;
  justify-content: space-between;
  list-style: none;
  padding: 0.9rem;
}

.panel-summary::-webkit-details-marker {
  display: none;
}

.summary-copy {
  align-items: center;
  column-gap: 0.72rem;
  display: grid;
  grid-template-columns: auto 1fr;
}

.summary-icon {
  align-items: center;
  background:
    radial-gradient(circle at 30% 30%, rgb(120 228 255 / 16%), transparent 52%),
    linear-gradient(145deg, rgb(8 29 58 / 92%), rgb(5 20 44 / 96%));
  border: 1px solid rgb(92 184 255 / 28%);
  border-radius: 11px;
  box-shadow:
    inset 0 1px 0 rgb(210 245 255 / 8%),
    0 8px 18px rgb(2 14 32 / 18%);
  color: #7fdbff;
  display: inline-flex;
  grid-row: 1 / span 2;
  height: 2.4rem;
  justify-content: center;
  width: 2.4rem;
}

.summary-icon-svg {
  height: 1.2rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
  width: 1.2rem;
}

.summary-copy h2,
.summary-copy p {
  margin: 0;
}

.summary-copy h2 {
  font-family: var(--font-headline);
  font-size: 1.3rem;
}

.summary-copy p {
  color: #90a9d2;
  font-family: var(--font-body);
  margin: 0.2rem 0 0;
}

.summary-copy p,
.feedback {
  color: #90aad4;
}

.chevron {
  color: #8fd9ff;
  font-size: 0.85rem;
  transition: transform 0.2s ease;
}

.fold-panel[open] .chevron {
  transform: rotate(180deg);
}

.panel-body {
  border-top: 1px solid rgb(69 107 168 / 33%);
  display: grid;
  gap: 0.85rem;
  padding: 0.85rem 0.9rem 0.95rem;
}

.grid {
  display: grid;
  gap: 0.6rem;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
}

label {
  color: #a0b7db;
  display: grid;
  font-family: var(--font-body);
  font-size: 0.88rem;
  gap: 0.35rem;
}

.checkbox {
  align-items: center;
  gap: 0.45rem;
  grid-template-columns: auto 1fr;
}

input,
textarea {
  background: rgb(6 17 38 / 82%);
  border: 1px solid rgb(70 110 174 / 42%);
  border-radius: 8px;
  color: #daecff;
  font-family: var(--font-body);
  font-size: 0.95rem;
  padding: 0.48rem 0.56rem;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin-top: 0.75rem;
}

button {
  background:
    linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%));
  border: 1px solid rgb(74 133 207 / 45%);
  border-radius: 999px;
  box-shadow:
    inset 0 1px 0 rgb(209 244 255 / 10%),
    0 8px 18px rgb(2 14 32 / 18%);
  color: #8fdbff;
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  min-height: 32px;
  padding: 0 0.82rem;
  text-transform: uppercase;
  touch-action: manipulation;
  transition:
    background 120ms ease,
    border-color 120ms ease,
    box-shadow 120ms ease,
    color 120ms ease,
    transform 120ms ease;
}

textarea {
  min-height: 5rem;
  resize: vertical;
}

.inline-actions {
  align-items: end;
}

.danger {
  background:
    linear-gradient(180deg, rgb(74 17 24 / 88%), rgb(45 10 17 / 92%));
  border-color: rgb(239 68 68 / 52%);
  color: #fecaca;
}

button:active {
  background:
    linear-gradient(180deg, rgb(15 73 115 / 92%), rgb(8 35 72 / 96%));
  border-color: rgb(112 197 255 / 56%);
  box-shadow:
    inset 0 1px 0 rgb(220 248 255 / 16%),
    0 4px 10px rgb(3 21 47 / 24%);
  color: #e8fbff;
  transform: translateY(1px) scale(0.985);
}

.danger:active {
  background:
    linear-gradient(180deg, rgb(111 25 32 / 94%), rgb(69 10 19 / 96%));
  border-color: rgb(252 165 165 / 56%);
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
  transform: none;
}

.feedback {
  color: #96afd5;
  font-family: var(--font-body);
  margin: 0.58rem 0 0;
}
</style>
