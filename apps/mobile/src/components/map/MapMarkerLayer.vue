<script setup lang="ts">
import "./MapMarkerLayer.css";

import type { Map as MapLibreMap } from "maplibre-gl";
import type { MapMarkerSymbolRecord } from "@reticulum/node-client";
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, toRef, watch } from "vue";

import { useMapMarkerPins } from "../../composables/useMapMarkerPins";
import { useMapMarkersStore } from "../../stores/mapMarkersStore";
import { usePluginMapSessionsStore } from "../../stores/pluginMapSessionsStore";
import { mdiPathFor } from "../../utils/mapMarkerIcons";
import { autoMarkerName, formatCoordinate } from "../../utils/mapMarkerPresentation";
import MapMarkerDetailsSheet from "./MapMarkerDetailsSheet.vue";
import MapMarkerSymbolSheet from "./MapMarkerSymbolSheet.vue";
import PluginMapSessionList from "./PluginMapSessionList.vue";

const props = defineProps<{ map: MapLibreMap | null }>();

type MarkerUiState =
  | { kind: "idle" }
  | { kind: "picking"; at?: { lat: number; lon: number } }
  | { kind: "placing"; symbol: MapMarkerSymbolRecord }
  | { kind: "moving"; key: string }
  | { kind: "details"; key: string };

const store = useMapMarkersStore();
const sessions = usePluginMapSessionsStore();
const state = shallowRef<MarkerUiState>({ kind: "idle" });
const center = ref({ lat: 0, lon: 0 });
const mapRef = toRef(props, "map");
const markers = computed(() => store.markers);
const symbolsById = computed(() => store.symbolsById);
const visible = computed(() => store.visible);

const selectedKey = computed(() =>
  state.value.kind === "details" || state.value.kind === "moving" ? state.value.key : null,
);
const hiddenKey = computed(() => (state.value.kind === "moving" ? state.value.key : null));
const selectedMarker = computed(() =>
  store.markers.find((marker) => marker.objectDestinationHash === selectedKey.value) ?? null,
);
const aimingSymbol = computed<MapMarkerSymbolRecord | undefined>(() => {
  if (state.value.kind === "placing") {
    return state.value.symbol;
  }
  return selectedMarker.value ? store.symbolsById.get(selectedMarker.value.symbol) : undefined;
});
const aiming = computed(() => state.value.kind === "placing" || state.value.kind === "moving");
const confirmLabel = computed(() =>
  state.value.kind === "placing" ? `Place ${state.value.symbol.description.toLowerCase()}` : "Move here",
);

function readCenter(): void {
  const point = props.map?.getCenter();
  if (point) {
    center.value = { lat: point.lat, lon: point.lng };
  }
}

watch(
  () => props.map,
  (map, previous) => {
    previous?.off("move", readCenter);
    map?.on("move", readCenter);
    readCenter();
  },
  { immediate: true },
);

function openPicker(): void {
  store.lastError = "";
  state.value = { kind: "picking" };
}

async function createAt(symbol: MapMarkerSymbolRecord, lat: number, lon: number): Promise<void> {
  try {
    const marker = await store.create({
      markerType: symbol.id,
      symbol: symbol.id,
      category: symbol.category,
      name: autoMarkerName(symbol, store.markers),
      lat,
      lon,
    });
    state.value = { kind: "details", key: marker.objectDestinationHash };
  } catch {
    // The store keeps the native validation message for the open sheet.
  }
}

function pick(symbol: MapMarkerSymbolRecord): void {
  const current = state.value;
  if (current.kind === "picking" && current.at) {
    void createAt(symbol, current.at.lat, current.at.lon);
    return;
  }
  state.value = { kind: "placing", symbol };
  readCenter();
}

async function confirmAim(): Promise<void> {
  const current = state.value;
  const { lat, lon } = center.value;
  if (current.kind === "placing") {
    await createAt(current.symbol, lat, lon);
  } else if (current.kind === "moving") {
    try {
      await store.move(current.key, lat, lon);
      state.value = { kind: "details", key: current.key };
    } catch {
      // Error stays visible in the aim bar.
    }
  }
}

function cancelAim(): void {
  const current = state.value;
  state.value = current.kind === "moving" ? { kind: "details", key: current.key } : { kind: "idle" };
}

function startMove(): void {
  const marker = selectedMarker.value;
  if (!marker || !props.map) {
    return;
  }
  props.map.easeTo({ center: [marker.lon, marker.lat], duration: 300 });
  state.value = { kind: "moving", key: marker.objectDestinationHash };
}

async function rename(name: string): Promise<void> {
  if (selectedKey.value) {
    await store.rename(selectedKey.value, name).catch(() => undefined);
  }
}

async function remove(): Promise<void> {
  if (!selectedKey.value) {
    return;
  }
  try {
    await store.remove(selectedKey.value);
    state.value = { kind: "idle" };
  } catch {
    // Error stays visible in the details sheet.
  }
}

/** The operator explicitly hands this one marker to a plugin for reading. */
function shareWithPlugin(pluginId: string): void {
  if (selectedKey.value) {
    void sessions.start(pluginId, [selectedKey.value]);
  }
}

function longPress(lat: number, lon: number): void {
  if (state.value.kind !== "idle" || !store.visible) {
    return;
  }
  const latest = store.recent[0];
  if (latest) {
    void createAt(latest, lat, lon);
  } else {
    state.value = { kind: "picking", at: { lat, lon } };
  }
}

watch(selectedMarker, (marker) => {
  if (!marker && (state.value.kind === "details" || state.value.kind === "moving")) {
    state.value = { kind: "idle" };
  }
});

useMapMarkerPins({
  map: mapRef,
  markers,
  symbolsById,
  visible,
  selectedKey,
  hiddenKey,
  onSelect: (key) => {
    if (!aiming.value) {
      state.value = { kind: "details", key };
    }
  },
  onLongPress: longPress,
});

onMounted(() => {
  void store.init();
});

onBeforeUnmount(() => {
  props.map?.off("move", readCenter);
});
</script>

<template>
  <div class="map-marker-layer">
    <PluginMapSessionList compact class="plugin-map-sessions-overlay" />
    <div v-if="aiming" class="marker-crosshair" aria-hidden="true">
      <span class="marker-crosshair-pin">
        <svg viewBox="0 0 24 24"><path :d="mdiPathFor(aimingSymbol?.mdi)" fill="currentColor" /></svg>
      </span>
      <span class="marker-crosshair-stem"></span>
    </div>

    <section v-if="aiming" class="marker-sheet marker-aim-bar" aria-label="Aim marker">
      <p class="marker-aim-hint">
        Drag the map to aim · <span data-testid="marker-aim-coords">{{ formatCoordinate(center.lat, center.lon) }}</span>
      </p>
      <p v-if="store.lastError" class="marker-sheet-error" role="alert">{{ store.lastError }}</p>
      <div class="marker-actions two">
        <button type="button" @click="cancelAim">Cancel</button>
        <button type="button" class="marker-primary" :disabled="store.busy" @click="confirmAim">
          {{ confirmLabel }}
        </button>
      </div>
    </section>

    <MapMarkerSymbolSheet
      v-else-if="state.kind === 'picking'"
      :symbols="store.symbols"
      :recent="store.recent"
      :loading="store.loading"
      :error="store.lastError"
      @pick="pick"
      @close="state = { kind: 'idle' }"
    />

    <MapMarkerDetailsSheet
      v-else-if="state.kind === 'details' && selectedMarker"
      :marker="selectedMarker"
      :symbol="store.symbolsById.get(selectedMarker.symbol)"
      :busy="store.busy"
      :error="store.lastError || sessions.lastError"
      :plugins="sessions.eligiblePlugins"
      :shared-with="sessions.sessionsFor(selectedMarker.objectDestinationHash).map((session) => session.pluginId)"
      @share="shareWithPlugin"
      @rename="rename"
      @move="startMove"
      @delete="remove"
      @close="state = { kind: 'idle' }"
    />

    <button
      v-else-if="store.visible"
      type="button"
      class="marker-add-button"
      aria-label="Add marker"
      @click="openPicker"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 21s-6.5-6.1-6.5-11a6.5 6.5 0 0 1 13 0c0 4.9-6.5 11-6.5 11Z" fill="none" stroke="currentColor" stroke-width="1.8" />
        <path d="M12 7v6M9 10h6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
      </svg>
    </button>
  </div>
</template>
