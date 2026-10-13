<script setup lang="ts">
import { useMapMarkersStore } from "../../stores/mapMarkersStore";

const store = useMapMarkersStore();
</script>

<template>
  <button
    class="map-chip marker-chip"
    type="button"
    :aria-pressed="store.visible"
    :aria-label="`Markers: ${store.markers.length}, ${store.visible ? 'shown' : 'hidden'}`"
    @click="store.setVisible(!store.visible)"
  >
    <span class="map-chip-count">{{ store.markers.length }}</span>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M12 21s-6.5-6.1-6.5-11a6.5 6.5 0 0 1 13 0c0 4.9-6.5 11-6.5 11Z" />
      <path v-if="!store.visible" d="M4 4l16 16" />
    </svg>
  </button>
</template>
