<script setup lang="ts">
defineProps<{
  dirty: boolean;
  saving: boolean;
  feedback: string;
}>();

const emit = defineEmits<{ save: []; discard: [] }>();
</script>

<template>
  <div class="settings-save-bar">
    <p v-if="feedback" class="feedback settings-save-feedback" role="status">{{ feedback }}</p>
    <p v-else class="settings-save-state">{{ dirty ? "Unsaved changes" : "All changes saved" }}</p>
    <div class="settings-save-actions">
      <button type="button" :disabled="!dirty || saving" @click="emit('discard')">Discard</button>
      <button type="button" class="settings-save" :disabled="!dirty || saving" @click="emit('save')">
        {{ saving ? "Saving" : "Save" }}
      </button>
    </div>
  </div>
</template>
