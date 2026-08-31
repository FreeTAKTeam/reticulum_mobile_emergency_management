<script setup lang="ts">
defineProps<{ active: boolean; batteryPercent?: number }>();
</script>

<template>
  <span class="power-badge" :class="{ active }" :aria-label="active ? 'Power saver active' : 'Normal power mode'">
    <span aria-hidden="true">{{ active ? "⌁" : "●" }}</span>
    {{ active ? "Saver" : "Normal" }}<template v-if="batteryPercent !== undefined"> · {{ batteryPercent }}%</template>
  </span>
</template>

<style scoped>
.power-badge { align-items: center; background: rgb(28 63 100 / 38%); border: 1px solid rgb(104 177 226 / 32%); border-radius: 999px; color: #a8cae8; display: inline-flex; font-family: var(--font-ui); font-size: .7rem; font-weight: 800; gap: .35rem; letter-spacing: .07em; padding: .24rem .55rem; text-transform: uppercase; }
.power-badge.active { background: rgb(126 75 13 / 38%); border-color: rgb(251 191 36 / 48%); color: #ffd67a; }
</style>
