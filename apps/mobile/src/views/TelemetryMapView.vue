<script setup lang="ts">
import "maplibre-gl/dist/maplibre-gl.css";

import maplibregl, {
  Marker,
  type LngLatLike,
  type Map as MapLibreMap,
} from "maplibre-gl";
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useRoute } from "vue-router";

import { useTelemetryMapActions } from "../composables/useTelemetryMapActions";
import { useTelemetryRouteTargets } from "../composables/useTelemetryRouteTargets";
import type { TelemetryPosition } from "../types/domain";
import { useSosStore } from "../stores/sosStore";
import { useTelemetryStore } from "../stores/telemetryStore";
import {
  buildSosPopupHtml,
  labelPlacementsFor,
  mapLayerOptions,
  mapStyleFor,
  matchesSosRouteTarget,
  markerStatusClass as telemetryMarkerStatusClass,
  safeLower,
  safeTrim,
  sosIdentityKey,
  telemetryMarkerKey,
  telemetryRenderGroups as buildTelemetryRenderGroups,
  type MapLayerId,
  type TelemetryCluster,
} from "../utils/telemetryMapModel";
import {
  ensureMarkerLabelElement,
  syncTelemetryClusterMarkers,
} from "../utils/telemetryMapMarkers";

const route = useRoute();
const sosStore = useSosStore();
const telemetryStore = useTelemetryStore();
const { popupElement: telemetryPopupElement, positionLabel } = useTelemetryMapActions();
const {
  selectedSosTarget,
  selectedSosTargetKey,
  selectedTelemetryTarget,
  selectedTelemetryTargetKey,
} = useTelemetryRouteTargets(route, () => telemetryStore.byCallsign);

const mapHost = ref<HTMLElement | null>(null);
let map: MapLibreMap | null = null;
let stopWatch: (() => void) | null = null;
let stopSosWatch: (() => void) | null = null;
let stopTelemetryFocusWatch: (() => void) | null = null;
let didFitBounds = false;
const markersByCallsign = new Map<string, Marker>();
const markerElementsByCallsign = new Map<string, HTMLDivElement>();
const telemetryPositionsByCallsign = new Map<string, TelemetryPosition>();
const telemetryClustersByKey = new Map<string, Marker>();
const telemetryClusterElementsByKey = new Map<string, HTMLDivElement>();
const sosMarkersByKey = new Map<string, Marker>();
const sosMarkerElementsByKey = new Map<string, HTMLDivElement>();
let lastFocusedSosTargetKey = "";
let lastFocusedTelemetryTargetKey = "";

const selectedMapLayer = shallowRef<MapLayerId>("base");
const layerMenuOpen = shallowRef(false);

const activeMapLayerLabel = computed(
  () => mapLayerOptions.find((option) => option.id === selectedMapLayer.value)?.label ?? "Base",
);

function markerStatusClass(position: TelemetryPosition): string {
  return telemetryMarkerStatusClass(position, telemetryStore.staleThresholdMs);
}

function telemetryRenderGroups(positions: TelemetryPosition[]): {
  clusters: TelemetryCluster[];
  individuals: TelemetryPosition[];
} {
  return buildTelemetryRenderGroups(positions, map, telemetryStore.staleThresholdMs);
}

function syncTelemetryClusters(clusters: TelemetryCluster[]): void {
  syncTelemetryClusterMarkers(
    map,
    clusters,
    telemetryClustersByKey,
    telemetryClusterElementsByKey,
  );
}

function telemetryPopup(position: TelemetryPosition): maplibregl.Popup {
  const popup = new maplibregl.Popup({
    closeButton: true,
    closeOnClick: true,
    offset: 20,
  });
  popup.on("open", () => {
    popup.setDOMContent(telemetryPopupElement(position));
  });
  return popup.setDOMContent(telemetryPopupElement(position));
}

function refreshMarkerPopup(key: string): void {
  const marker = markersByCallsign.get(key);
  const position = telemetryPositionsByCallsign.get(key);
  const popup = marker?.getPopup();
  if (!position || !popup) {
    return;
  }
  popup.setDOMContent(telemetryPopupElement(position));
}

function syncMarkers(positions: TelemetryPosition[]): void {
  if (!map) {
    return;
  }

  const renderGroups = telemetryRenderGroups(positions);
  const active = new Set<string>();
  const labelPlacements = labelPlacementsFor(renderGroups.individuals);
  syncTelemetryClusters(renderGroups.clusters);

  for (const position of renderGroups.individuals) {
    const key = telemetryMarkerKey(position);
    const label = positionLabel(position);
    const placement = labelPlacements.get(key) ?? { stackIndex: 0, stackSize: 1 };
    active.add(key);
    telemetryPositionsByCallsign.set(key, position);

    let marker = markersByCallsign.get(key);
    let markerElement = markerElementsByCallsign.get(key);

    if (!marker || !markerElement) {
      markerElement = document.createElement("div");
      markerElement.className = "telemetry-marker";
      markerElement.title = label;

      marker = new maplibregl.Marker({ element: markerElement })
        .setLngLat([position.lon, position.lat] as LngLatLike)
        .setPopup(telemetryPopup(position))
        .addTo(map);
      markerElement.addEventListener("click", () => refreshMarkerPopup(key), { capture: true });

      markersByCallsign.set(key, marker);
      markerElementsByCallsign.set(key, markerElement);
    } else {
      marker.setLngLat([position.lon, position.lat] as LngLatLike);
      const popup = marker.getPopup();
      if (popup) {
        popup.setDOMContent(telemetryPopupElement(position));
      } else {
        marker.setPopup(telemetryPopup(position));
      }
      markerElement.title = label;
    }

    const labelElement = ensureMarkerLabelElement(markerElement);
    labelElement.textContent = label;
    markerElement.dataset.overlapCount = String(placement.stackSize);
    markerElement.style.setProperty("--label-offset-y", `${placement.stackIndex * 1.42}rem`);
    markerElement.classList.remove("is-live", "is-stale");
    markerElement.classList.add(markerStatusClass(position));
    markerElement.classList.toggle("is-overlapped", placement.stackSize > 1);
  }

  for (const [key, marker] of markersByCallsign.entries()) {
    if (active.has(key)) {
      continue;
    }
    marker.remove();
    markersByCallsign.delete(key);
    markerElementsByCallsign.delete(key);
    telemetryPositionsByCallsign.delete(key);
  }
}

function syncCurrentTelemetryMarkers(): void {
  syncMarkers(telemetryStore.activePositions);
}

function sosPopupHtml(point: (typeof sosStore.locations)[number]): string {
  return buildSosPopupHtml(point, sosStore.activeAlerts, sosStore.alerts);
}

function isTargetedSosPoint(point: (typeof sosStore.locations)[number], latestRecordedAtMs: number): boolean {
  return matchesSosRouteTarget(
    point,
    latestRecordedAtMs,
    selectedSosTarget.value,
    sosStore.activeAlerts,
    sosStore.alerts,
  );
}

function syncSosTrails(): void {
  if (!map) {
    return;
  }
  const active = new Set<string>();
  const features: Array<Record<string, unknown>> = [];
  let targetMarker: Marker | null = null;
  let targetCoordinates: [number, number] | null = null;
  for (const [incidentId, points] of sosStore.activeLocationsByIncident.entries()) {
    const coordinates = points.map((point) => [point.lon, point.lat]);
    const latestRecordedAtMs = points[points.length - 1]?.recordedAtMs ?? 0;
    if (coordinates.length > 1) {
      features.push({
        type: "Feature",
        properties: { incidentId },
        geometry: { type: "LineString", coordinates },
      });
    }
    for (const point of points) {
      const key = `${incidentId}:${point.sourceHex}:${point.recordedAtMs}`;
      active.add(key);
      let marker = sosMarkersByKey.get(key);
      let element = sosMarkerElementsByKey.get(key);
      if (!marker || !element) {
        element = document.createElement("div");
        marker = new maplibregl.Marker({ element })
          .setLngLat([point.lon, point.lat] as LngLatLike)
          .addTo(map);
        sosMarkersByKey.set(key, marker);
        sosMarkerElementsByKey.set(key, element);
      } else {
        marker.setLngLat([point.lon, point.lat] as LngLatLike);
      }
      const targeted = isTargetedSosPoint(point, latestRecordedAtMs);
      element.className = "sos-trail-marker";
      element.classList.toggle("is-blinking", point.recordedAtMs === latestRecordedAtMs || targeted);
      element.classList.toggle("is-targeted", targeted);
      element.title = "SOS location";
      marker.setPopup(new maplibregl.Popup({ offset: 20 }).setHTML(sosPopupHtml(point)));

      if (targeted) {
        targetMarker = marker;
        targetCoordinates = [point.lon, point.lat];
      }
    }
  }

  const payload = {
    type: "FeatureCollection",
    features,
  };
  const source = map.getSource("sos_trail") as maplibregl.GeoJSONSource | undefined;
  if (source) {
    source.setData(payload as never);
  } else if (map.isStyleLoaded()) {
    map.addSource("sos_trail", {
      type: "geojson",
      data: payload as never,
    });
    map.addLayer({
      id: "sos_trail_line",
      source: "sos_trail",
      type: "line",
      paint: {
        "line-color": "#ef4444",
        "line-width": 4,
      },
    });
  }

  for (const [key, marker] of sosMarkersByKey.entries()) {
    if (active.has(key)) {
      continue;
    }
    marker.remove();
    sosMarkersByKey.delete(key);
    sosMarkerElementsByKey.delete(key);
  }

  const focusKey = selectedSosTargetKey.value;
  if (targetMarker && targetCoordinates && focusKey && focusKey !== lastFocusedSosTargetKey) {
    map.flyTo({ center: targetCoordinates, zoom: Math.max(map.getZoom(), 14), duration: 650 });
    targetMarker.togglePopup();
    lastFocusedSosTargetKey = focusKey;
  }
}

function syncSosTrailsWhenStyleReady(): void {
  if (!map) {
    return;
  }
  if (map.isStyleLoaded()) {
    syncSosTrails();
    return;
  }
  map.once("idle", syncSosTrailsWhenStyleReady);
}

function focusSelectedTelemetryTarget(): void {
  const target = selectedTelemetryTarget.value;
  const focusKey = selectedTelemetryTargetKey.value;
  if (!map || !target || !focusKey || focusKey === lastFocusedTelemetryTargetKey) {
    return;
  }
  map.flyTo({
    center: [target.lon, target.lat],
    zoom: Math.max(map.getZoom(), 14),
    duration: 650,
  });
  const callsign = safeLower(target.callsign);
  const marker = callsign ? markersByCallsign.get(callsign) : undefined;
  marker?.togglePopup();
  lastFocusedTelemetryTargetKey = focusKey;
}

const liveTelemetryCount = computed(() =>
  Math.max(0, telemetryStore.activePositions.length - telemetryStore.stalePositions.length),
);
const staleTelemetryCount = computed(() => telemetryStore.stalePositions.length);
const sosAlertCount = computed(() => sosStore.activeAlerts.length);

function toggleLayerMenu(): void {
  layerMenuOpen.value = !layerMenuOpen.value;
}

function setMapLayer(layer: MapLayerId): void {
  layerMenuOpen.value = false;
  if (selectedMapLayer.value === layer) {
    return;
  }
  selectedMapLayer.value = layer;
  map?.setStyle(mapStyleFor(layer));
  syncSosTrailsWhenStyleReady();
}

onMounted(() => {
  if (!mapHost.value) {
    return;
  }

  map = new maplibregl.Map({
    container: mapHost.value,
    style: mapStyleFor(selectedMapLayer.value),
    center: [-98.5795, 39.8283],
    zoom: 3,
  });

  map.addControl(new maplibregl.NavigationControl({ visualizePitch: true }), "bottom-right");
  map.on("load", syncSosTrails);
  map.on("zoomend", syncCurrentTelemetryMarkers);

  stopWatch = watch(
    () => telemetryStore.activePositions,
    (positions) => {
      syncMarkers(positions);
      if (positions.length === 0) {
        didFitBounds = false;
        return;
      }
      if (map && !didFitBounds) {
        const bounds = new maplibregl.LngLatBounds();
        for (const position of positions) {
          bounds.extend([position.lon, position.lat]);
        }
        map.fitBounds(bounds, { padding: 60, maxZoom: 13, duration: 600 });
        didFitBounds = true;
      }
      focusSelectedTelemetryTarget();
    },
    { immediate: true, deep: true },
  );
  stopTelemetryFocusWatch = watch(
    () => [
      route.query.callsign,
      route.query.lat,
      route.query.lon,
      telemetryStore.activePositions.length,
    ],
    () => focusSelectedTelemetryTarget(),
    { immediate: true },
  );
  stopSosWatch = watch(
    () => [
      sosStore.activeLocations,
      sosStore.activeAlerts,
      route.query.incident,
      route.query.source,
      route.query.message,
    ],
    () => syncSosTrails(),
    { immediate: true, deep: true },
  );
});

onBeforeUnmount(() => {
  stopWatch?.();
  stopSosWatch?.();
  stopTelemetryFocusWatch?.();
  stopWatch = null;
  stopSosWatch = null;
  stopTelemetryFocusWatch = null;
  for (const marker of markersByCallsign.values()) {
    marker.remove();
  }
  for (const marker of telemetryClustersByKey.values()) {
    marker.remove();
  }
  for (const marker of sosMarkersByKey.values()) {
    marker.remove();
  }
  markersByCallsign.clear();
  markerElementsByCallsign.clear();
  telemetryPositionsByCallsign.clear();
  telemetryClustersByKey.clear();
  telemetryClusterElementsByKey.clear();
  sosMarkersByKey.clear();
  sosMarkerElementsByKey.clear();
  map?.remove();
  map = null;
});
</script>

<template>
  <section class="telemetry-view">
    <div class="map-frame">
      <div ref="mapHost" class="map-container"></div>

      <div class="map-overlay" aria-label="Map indicators">
        <span class="map-chip live-chip" :aria-label="`Live telemetry: ${liveTelemetryCount}`">
          <span class="map-chip-count">{{ liveTelemetryCount }}</span>
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 20v-5" />
            <path d="M8 20h8" />
            <path d="M8.5 11.5a5 5 0 1 1 7 0" />
            <path d="M6 8a8 8 0 0 1 12 0" />
          </svg>
        </span>
        <span class="map-chip stale-chip" :aria-label="`Stale telemetry: ${staleTelemetryCount}`">
          <span class="map-chip-count">{{ staleTelemetryCount }}</span>
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <circle cx="12" cy="12" r="8" />
            <path d="M12 8v4l3 2" />
          </svg>
        </span>
        <span class="map-chip sos-chip" :aria-label="`SOS alerts: ${sosAlertCount}`">
          <span class="map-chip-count">{{ sosAlertCount }}</span>
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M12 4 21 20H3L12 4Z" />
            <path d="M12 9v4" />
            <path d="M12 16h.01" />
          </svg>
        </span>
        <div class="layer-control">
          <button
            class="map-chip layer-chip"
            type="button"
            :aria-expanded="layerMenuOpen"
            :aria-label="`Map layer: ${activeMapLayerLabel}`"
            aria-haspopup="menu"
            :data-map-layer="selectedMapLayer"
            @click="toggleLayerMenu"
          >
            <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="M12 4 4 8l8 4 8-4-8-4Z" />
              <path d="M4 12l8 4 8-4" />
              <path d="M4 16l8 4 8-4" />
            </svg>
          </button>
          <div v-if="layerMenuOpen" class="layer-menu" role="menu" aria-label="Map layer options">
            <button
              v-for="option in mapLayerOptions"
              :key="option.id"
              class="layer-option"
              type="button"
              role="menuitemradio"
              :aria-checked="selectedMapLayer === option.id"
              @click="setMapLayer(option.id)"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped src="./TelemetryMapView.css"></style>
  matchesSosRouteTarget,
