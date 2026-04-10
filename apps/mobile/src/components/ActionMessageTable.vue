<script setup lang="ts">
import ActionMessageItem from "./ActionMessageItem.vue";

import type { ActionMessage } from "../types/domain";

const props = defineProps<{
  messages: ActionMessage[];
  editableCallsigns: string[];
}>();

const emit = defineEmits<{
  edit: [callsign: string];
  delete: [callsign: string];
  cycle: [callsign: string, field: keyof ActionMessage];
}>();

function handleEdit(callsign: string): void {
  emit("edit", callsign);
}

function handleDelete(callsign: string): void {
  emit("delete", callsign);
}

function handleCycle(callsign: string, field: keyof ActionMessage): void {
  emit("cycle", callsign, field);
}

function isEditable(message: ActionMessage): boolean {
  return props.editableCallsigns.includes(message.callsign);
}
</script>

<template>
  <section class="table-list">
    <ActionMessageItem
      v-for="message in props.messages"
      :key="message.callsign"
      :message="message"
      :editable="isEditable(message)"
      @edit="handleEdit"
      @delete="handleDelete"
      @cycle="handleCycle"
    />
  </section>
</template>

<style scoped>
.table-list {
  display: grid;
  gap: 0.9rem;
}
</style>
