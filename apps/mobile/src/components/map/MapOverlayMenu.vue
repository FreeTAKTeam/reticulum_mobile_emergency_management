<script setup lang="ts">
import "./MapOverlayMenu.css";

import { computed, shallowRef } from "vue";

import { useMapMarkersStore } from "../../stores/mapMarkersStore";
import { mapLayerOptions, type MapLayerId } from "../../utils/telemetryMapModel";

const props = defineProps<{
  layer: MapLayerId;
  live: number;
  stale: number;
  sos: number;
}>();
const emit = defineEmits<{ selectLayer: [layer: MapLayerId] }>();

const markers = useMapMarkersStore();
const open = shallowRef(false);

const activeLayerLabel = computed(
  () => mapLayerOptions.find((option) => option.id === props.layer)?.label ?? "Base",
);
const markersLabel = computed(
  () => `Markers: ${markers.markers.length}, ${markers.visible ? "shown" : "hidden"}`,
);
const buttonLabel = computed(() =>
  props.sos > 0 ? `Map menu, SOS alerts: ${props.sos}` : "Map menu",
);

function toggle(): void {
  open.value = !open.value;
}

function selectLayer(layer: MapLayerId): void {
  open.value = false;
  emit("selectLayer", layer);
}
</script>

<template>
  <div class="map-overlay-menu">
    <button
      class="map-overlay-menu-button"
      type="button"
      :aria-label="buttonLabel"
      aria-haspopup="menu"
      :aria-expanded="open"
      :data-map-layer="layer"
      :title="`Layer: ${activeLayerLabel}`"
      @click="toggle"
    >
      <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path d="M4 7h16" />
        <path d="M4 12h16" />
        <path d="M4 17h16" />
      </svg>
      <!-- SOS stays visible outside the menu: it is the one indicator that must not hide. -->
      <span v-if="sos > 0" class="map-overlay-menu-badge" :aria-label="`SOS alerts: ${sos}`">{{ sos }}</span>
    </button>
    <div v-if="open" class="map-overlay-menu-panel" role="menu" aria-label="Map menu">
      <p class="map-overlay-menu-heading" aria-hidden="true">Status</p>
      <div class="map-overlay-menu-status is-live" :aria-label="`Live telemetry: ${live}`">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M12 20v-5" />
          <path d="M8 20h8" />
          <path d="M8.5 11.5a5 5 0 1 1 7 0" />
          <path d="M6 8a8 8 0 0 1 12 0" />
        </svg>
        <span>Live</span>
        <span class="map-overlay-menu-count">{{ live }}</span>
      </div>
      <div class="map-overlay-menu-status is-stale" :aria-label="`Stale telemetry: ${stale}`">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <circle cx="12" cy="12" r="8" />
          <path d="M12 8v4l3 2" />
        </svg>
        <span>Stale</span>
        <span class="map-overlay-menu-count">{{ stale }}</span>
      </div>
      <div class="map-overlay-menu-status is-sos" :aria-label="`SOS alerts: ${sos}`">
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M12 4 21 20H3L12 4Z" />
          <path d="M12 9v4" />
          <path d="M12 16h.01" />
        </svg>
        <span>SOS</span>
        <span class="map-overlay-menu-count">{{ sos }}</span>
      </div>
      <p class="map-overlay-menu-heading" aria-hidden="true">Layer</p>
      <button
        v-for="option in mapLayerOptions"
        :key="option.id"
        class="map-overlay-menu-item"
        type="button"
        role="menuitemradio"
        :aria-checked="layer === option.id"
        @click="selectLayer(option.id)"
      >
        {{ option.label }}
      </button>
      <p class="map-overlay-menu-heading" aria-hidden="true">Objects</p>
      <button
        class="map-overlay-menu-item"
        type="button"
        role="menuitemcheckbox"
        :aria-checked="markers.visible"
        :aria-label="markersLabel"
        @click="markers.setVisible(!markers.visible)"
      >
        <span>Markers</span>
        <span class="map-overlay-menu-count">{{ markers.markers.length }}</span>
      </button>
    </div>
  </div>
</template>
