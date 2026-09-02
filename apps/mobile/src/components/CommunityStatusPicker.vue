<script setup lang="ts">
import type { HouseholdStatus } from "@reticulum/node-client";
import { COMMUNITY_STATUS_OPTIONS } from "../utils/communityStatus";

defineProps<{ modelValue: HouseholdStatus; disabled?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: HouseholdStatus] }>();
</script>

<template>
  <div class="status-grid" role="group" aria-label="Publish household status">
    <button
      v-for="option in COMMUNITY_STATUS_OPTIONS"
      :key="option.value"
      type="button"
      :class="[option.value, { selected: modelValue === option.value }]"
      :disabled="disabled"
      :aria-pressed="modelValue === option.value"
      @click="emit('update:modelValue', option.value)"
    >
      <span class="status-pulse" aria-hidden="true" />
      {{ option.label }}
    </button>
  </div>
</template>

<style scoped>
.status-grid { display: grid; gap: .6rem; grid-template-columns: repeat(4, minmax(0, 1fr)); }
button { --btn-bg: rgb(7 25 52 / 82%); --btn-bg-pressed: rgb(209 241 255 / 94%); --btn-border: rgb(73 142 204 / 35%); --btn-border-pressed: #e3f6ff; --btn-color: #b8d9f3; --btn-color-pressed: #06233d; align-items: center; background: var(--btn-bg); border: 1px solid var(--btn-border); border-radius: 12px; box-shadow: inset 0 1px 0 rgb(209 244 255 / 7%); color: var(--btn-color); cursor: pointer; display: flex; font-family: var(--font-ui); font-size: .75rem; font-weight: 800; gap: .5rem; justify-content: center; letter-spacing: .04em; min-height: 46px; }
button.selected { border-color: rgb(92 201 255 / 72%); box-shadow: inset 0 0 0 1px rgb(92 201 255 / 24%); }
.status-pulse { background: #4ade80; border-radius: 50%; height: 8px; width: 8px; }
.one_missing .status-pulse { background: #fbbf24; }.evacuated .status-pulse { background: #60a5fa; }.needs_help .status-pulse { background: #fb7185; }
@media (max-width: 650px) { .status-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
