<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  label: string;
  value: "Red" | "Yellow" | "Green" | "Unknown";
}>();

const cssClass = computed(() => {
  if (props.value === "Green") {
    return "pill green";
  }
  if (props.value === "Yellow") {
    return "pill yellow";
  }
  if (props.value === "Red") {
    return "pill red";
  }
  return "pill unknown";
});
</script>

<template>
  <span :class="cssClass">{{
    props.label.trim().length > 0 ? `${props.label}: ${props.value}` : props.value
  }}</span>
</template>

<style scoped>
.pill {
  border-radius: 999px;
  display: inline-flex;
  font-family: var(--font-body);
  font-size: 0.88rem;
  font-weight: 700;
  letter-spacing: 0.01em;
  line-height: 1;
  margin-right: 0.5rem;
  margin-top: 0.45rem;
  padding: 0.38rem 0.7rem 0.42rem;
  text-shadow: 0 0 8px rgb(0 0 0 / 35%);
}

.green {
  background: linear-gradient(120deg, #0f8b5f, #16ce79);
  box-shadow: 0 0 18px rgb(22 206 121 / 24%);
}

.yellow {
  background: linear-gradient(120deg, #a07b00, #f5cc19);
  box-shadow: 0 0 18px rgb(245 204 25 / 24%);
}

.red {
  background: linear-gradient(120deg, #8f1d28, #ff3648);
  box-shadow: 0 0 18px rgb(255 54 72 / 24%);
}

.unknown {
  background: linear-gradient(120deg, #2d3f66, #4f6f9f);
  color: #afbed8;
}
</style>
