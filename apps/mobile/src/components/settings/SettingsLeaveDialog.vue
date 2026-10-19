<script setup lang="ts">
import { nextTick, ref, watch } from "vue";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ answer: [leave: boolean] }>();
const keepButton = ref<HTMLButtonElement | null>(null);

watch(() => props.open, async (open) => {
  if (open) {
    await nextTick();
    keepButton.value?.focus();
  }
});
</script>

<template>
  <div v-if="open" class="settings-dialog-backdrop" @click.self="emit('answer', false)">
    <div
      class="settings-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="settings-leave-title"
      @keydown.esc="emit('answer', false)"
    >
      <h3 id="settings-leave-title">Discard unsaved changes?</h3>
      <p>Your edits on this page have not been saved.</p>
      <div class="settings-dialog-actions">
        <button ref="keepButton" type="button" @click="emit('answer', false)">Keep editing</button>
        <button type="button" class="danger-button" @click="emit('answer', true)">Discard</button>
      </div>
    </div>
  </div>
</template>
