<script setup lang="ts">
import { nextTick, ref, watch } from "vue";

const props = defineProps<{ open: boolean; pinSet: boolean }>();
const emit = defineEmits<{ close: []; set: [pin: string]; clear: [] }>();
const pin = ref("");
const input = ref<HTMLInputElement | null>(null);

watch(() => props.open, async (open) => {
  pin.value = "";
  if (open) {
    await nextTick();
    input.value?.focus();
  }
});

function submit(): void {
  const value = pin.value.trim();
  if (value) emit("set", value);
}
</script>

<template>
  <div v-if="open" class="settings-dialog-backdrop" @click.self="emit('close')">
    <form
      class="settings-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="sos-pin-title"
      @submit.prevent="submit"
      @keydown.esc="emit('close')"
    >
      <h3 id="sos-pin-title">Deactivation PIN</h3>
      <p>Needed to end an active SOS. {{ pinSet ? "A PIN is set." : "No PIN is set." }}</p>
      <label>
        New PIN
        <input
          ref="input"
          v-model="pin"
          autocomplete="new-password"
          inputmode="numeric"
          type="password"
        />
      </label>
      <div class="settings-dialog-actions">
        <button type="button" @click="emit('close')">Cancel</button>
        <button v-if="pinSet" type="button" class="danger-button" @click="emit('clear')">Clear PIN</button>
        <button type="submit" :disabled="!pin.trim()">Set PIN</button>
      </div>
    </form>
  </div>
</template>
