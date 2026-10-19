<script setup lang="ts">
import { computed } from "vue";

const MAX_BADGES = 5;

const props = defineProps<{ invalid?: boolean }>();
const badges = defineModel<string[]>("badges", { required: true });
const text = defineModel<string>("text", { required: true });
const full = computed(() => badges.value.length >= MAX_BADGES);

function commitText(): void {
  const typed = text.value.split(",").map((role) => role.trim()).filter(Boolean);
  if (typed.length === 0) return;
  badges.value = [...new Set([...badges.value, ...typed])].slice(0, MAX_BADGES);
  text.value = "";
}

function onInput(event: Event): void {
  text.value = (event.target as HTMLInputElement).value;
  if (text.value.includes(",")) commitText();
}

function remove(badge: string): void {
  badges.value = badges.value.filter((entry) => entry !== badge);
}

function removeLastOnBackspace(): void {
  if (!text.value && badges.value.length > 0) {
    badges.value = badges.value.slice(0, -1);
  }
}
</script>

<template>
  <div class="role-badges" :class="{ invalid: props.invalid }">
    <span v-for="badge in badges" :key="badge" class="role-chip">
      {{ badge }}
      <button type="button" class="role-chip-remove" :aria-label="`Remove ${badge}`" @click="remove(badge)">×</button>
    </span>
    <input
      id="settings-role-badges"
      :value="text"
      type="text"
      maxlength="128"
      :disabled="full"
      :placeholder="full ? 'Five badges maximum' : 'Medic, radio operator'"
      aria-describedby="settings-role-badges-hint"
      :aria-invalid="props.invalid"
      @input="onInput"
      @keydown.enter.prevent="commitText"
      @keydown.backspace="removeLastOnBackspace"
      @blur="commitText"
    />
  </div>
</template>
