<script setup lang="ts">
import "./MapOverlayMenu.css";

import { computed, shallowRef } from "vue";

import { useMapMarkersStore } from "../../stores/mapMarkersStore";
import { mapLayerOptions, type MapLayerId } from "../../utils/telemetryMapModel";

const props = defineProps<{ layer: MapLayerId }>();
const emit = defineEmits<{ selectLayer: [layer: MapLayerId] }>();

const markers = useMapMarkersStore();
const open = shallowRef(false);

const activeLayerLabel = computed(
  () => mapLayerOptions.find((option) => option.id === props.layer)?.label ?? "Base",
);
const markersLabel = computed(
  () => `Markers: ${markers.markers.length}, ${markers.visible ? "shown" : "hidden"}`,
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
      aria-label="Map menu"
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
    </button>
    <div v-if="open" class="map-overlay-menu-panel" role="menu" aria-label="Map menu">
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
