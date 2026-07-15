<script setup lang="ts">
defineProps<{
  start: number;
  end: number;
  total: number;
  hasPrevious: boolean;
  hasNext: boolean;
  previousLabel?: string;
  nextLabel?: string;
}>();

defineEmits<{
  previous: [];
  next: [];
}>();
</script>

<template>
  <nav v-if="total > 200" class="list-window-controls" aria-label="Large list navigation">
    <button type="button" :disabled="!hasPrevious" @click="$emit('previous')">
      {{ previousLabel || "Previous" }}
    </button>
    <span>{{ start + 1 }}-{{ end }} of {{ total }}</span>
    <button type="button" :disabled="!hasNext" @click="$emit('next')">
      {{ nextLabel || "Next" }}
    </button>
  </nav>
</template>

<style scoped>
.list-window-controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  margin: 0.75rem 0;
}

.list-window-controls span {
  color: var(--muted);
  font-size: 0.85rem;
  font-variant-numeric: tabular-nums;
}

.list-window-controls button {
  min-width: 6.5rem;
}
</style>
