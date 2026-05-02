<script setup lang="ts">
import "maplibre-gl/dist/maplibre-gl.css";

import maplibregl, {
  Marker,
  type LngLatLike,
  type Map as MapLibreMap,
  type StyleSpecification,
} from "maplibre-gl";
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useRoute } from "vue-router";

import type { TelemetryPosition } from "../types/domain";
import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { useTelemetryStore } from "../stores/telemetryStore";

const nodeStore = useNodeStore();
const route = useRoute();
const sosStore = useSosStore();
const telemetryStore = useTelemetryStore();

const mapHost = ref<HTMLElement | null>(null);
let map: MapLibreMap | null = null;
let stopWatch: (() => void) | null = null;
let stopSosWatch: (() => void) | null = null;
let didFitBounds = false;
const markersByCallsign = new Map<string, Marker>();
const markerElementsByCallsign = new Map<string, HTMLDivElement>();
const sosMarkersByKey = new Map<string, Marker>();
const sosMarkerElementsByKey = new Map<string, HTMLDivElement>();
let lastFocusedSosTargetKey = "";

type MapLayerId = "base" | "satellite";

interface SosRouteTarget {
  incidentId: string;
  sourceHex: string;
  messageIdHex?: string;
}

interface MarkerLabelPlacement {
  stackIndex: number;
  stackSize: number;
}

const BASE_MAP_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";
const mapLayerOptions: Array<{ id: MapLayerId; label: string }> = [
  { id: "base", label: "Base" },
  { id: "satellite", label: "Satellite" },
];

const selectedMapLayer = shallowRef<MapLayerId>("base");
const layerMenuOpen = shallowRef(false);

const activeMapLayerLabel = computed(
  () => mapLayerOptions.find((option) => option.id === selectedMapLayer.value)?.label ?? "Base",
);

function satelliteMapStyle(): StyleSpecification {
  return {
    version: 8,
    sources: {
      "esri-world-imagery": {
        type: "raster",
        tiles: [
          "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
        ],
        tileSize: 256,
        attribution: "Tiles &copy; Esri",
      },
    },
    layers: [
      {
        id: "esri-world-imagery",
        type: "raster",
        source: "esri-world-imagery",
      },
    ],
  };
}

function mapStyleFor(layer: MapLayerId): string | StyleSpecification {
  return layer === "satellite" ? satelliteMapStyle() : BASE_MAP_STYLE_URL;
}

function safeTrim(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function safeLower(value: unknown): string {
  return safeTrim(value).toLowerCase();
}

function routeQueryString(value: unknown): string {
  return Array.isArray(value) ? safeTrim(value[0]) : safeTrim(value);
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function lineBreakHtml(value: string): string {
  return escapeHtml(value).replace(/\r?\n/g, "<br>");
}

function visibleSosBodyText(body: string): string {
  return body
    .split(/\r?\n/)
    .filter((line) => !safeTrim(line).toLowerCase().startsWith("gps:"))
    .join("\n")
    .trim();
}

function sosIdentityKey(incidentId: string, sourceHex: string): string {
  return `${safeLower(incidentId)}:${safeLower(sourceHex)}`;
}

const selectedSosTarget = computed<SosRouteTarget | null>(() => {
  const incidentId = routeQueryString(route.query.incident);
  const sourceHex = routeQueryString(route.query.source);
  if (!incidentId || !sourceHex) {
    return null;
  }
  const messageIdHex = routeQueryString(route.query.message);
  return {
    incidentId,
    sourceHex,
    ...(messageIdHex ? { messageIdHex } : {}),
  };
});

const selectedSosTargetKey = computed(() => {
  const target = selectedSosTarget.value;
  if (!target) {
    return "";
  }
  return `${sosIdentityKey(target.incidentId, target.sourceHex)}:${safeLower(target.messageIdHex)}`;
});

function markerStatusClass(position: TelemetryPosition): string {
  return Date.now() - position.updatedAt > telemetryStore.staleThresholdMs ? "is-stale" : "is-live";
}

function speedLine(position: TelemetryPosition): string {
  if (position.speed === undefined && position.course === undefined) {
    return "";
  }
  const parts: string[] = [];
  if (position.speed !== undefined) {
    parts.push(`Speed ${position.speed.toFixed(1)}`);
  }
  if (position.course !== undefined) {
    parts.push(`Course ${position.course.toFixed(0)}&deg;`);
  }
  return `<div class="popup-secondary">${parts.join(" &middot; ")}</div>`;
}

function positionLabel(position: TelemetryPosition): string {
  const peer = nodeStore.discoveredByDestination[safeTrim(position.callsign).toLowerCase()];
  return safeTrim(peer?.announcedName) || safeTrim(peer?.label) || position.callsign;
}

function telemetryMarkerKey(position: TelemetryPosition): string {
  return position.callsign.toLowerCase();
}

function telemetryCoordinateKey(position: TelemetryPosition): string {
  return `${position.lat}:${position.lon}`;
}

function labelPlacementsFor(positions: TelemetryPosition[]): Map<string, MarkerLabelPlacement> {
  const groups = new Map<string, TelemetryPosition[]>();
  for (const position of positions) {
    const coordinateKey = telemetryCoordinateKey(position);
    groups.set(coordinateKey, [...(groups.get(coordinateKey) ?? []), position]);
  }

  const placements = new Map<string, MarkerLabelPlacement>();
  for (const group of groups.values()) {
    group.forEach((position, stackIndex) => {
      placements.set(telemetryMarkerKey(position), {
        stackIndex,
        stackSize: group.length,
      });
    });
  }
  return placements;
}

function ensureMarkerLabelElement(markerElement: HTMLDivElement): HTMLSpanElement {
  const existing = markerElement.querySelector<HTMLSpanElement>(".telemetry-marker-label");
  if (existing) {
    return existing;
  }

  const labelElement = document.createElement("span");
  labelElement.className = "telemetry-marker-label";
  markerElement.append(labelElement);
  return labelElement;
}

function popupHtml(position: TelemetryPosition): string {
  const label = positionLabel(position);
  const identityLine =
    label === position.callsign
      ? ""
      : `<div class="popup-secondary">${position.callsign}</div>`;
  return `
    <div class="popup-title">${label}</div>
    ${identityLine}
    <div class="popup-secondary">Updated ${new Date(position.updatedAt).toLocaleString()}</div>
    ${speedLine(position)}
  `;
}

function syncMarkers(positions: TelemetryPosition[]): void {
  if (!map) {
    return;
  }

  const active = new Set<string>();
  const labelPlacements = labelPlacementsFor(positions);

  for (const position of positions) {
    const key = telemetryMarkerKey(position);
    const label = positionLabel(position);
    const placement = labelPlacements.get(key) ?? { stackIndex: 0, stackSize: 1 };
    active.add(key);

    let marker = markersByCallsign.get(key);
    let markerElement = markerElementsByCallsign.get(key);

    if (!marker || !markerElement) {
      markerElement = document.createElement("div");
      markerElement.className = "telemetry-marker";
      markerElement.title = label;

      marker = new maplibregl.Marker({ element: markerElement })
        .setLngLat([position.lon, position.lat] as LngLatLike)
        .setPopup(new maplibregl.Popup({ offset: 20 }).setHTML(popupHtml(position)))
        .addTo(map);

      markersByCallsign.set(key, marker);
      markerElementsByCallsign.set(key, markerElement);
    } else {
      marker.setLngLat([position.lon, position.lat] as LngLatLike);
      marker.getPopup()?.setHTML(popupHtml(position));
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
  }
}

function sosPopupHtml(point: (typeof sosStore.locations)[number]): string {
  const alert = sosStore.alerts.find((candidate) =>
    sosIdentityKey(candidate.incidentId, candidate.sourceHex)
      === sosIdentityKey(point.incidentId, point.sourceHex),
  );
  const body = visibleSosBodyText(safeTrim(alert?.bodyUtf8)) || "SOS emergency";
  const battery =
    point.batteryPercent !== undefined
      ? `<div class="popup-secondary">Battery ${point.batteryPercent.toFixed(0)}%</div>`
      : "";
  return `
    <div class="popup-title popup-title-sos">SOS EMERGENCY</div>
    <div class="popup-body">${lineBreakHtml(body)}</div>
    <div class="popup-secondary">Source ${escapeHtml(point.sourceHex)}</div>
    <div class="popup-secondary">${point.lat.toFixed(6)}, ${point.lon.toFixed(6)}</div>
    ${battery}
    <div class="popup-secondary">Updated ${new Date(point.recordedAtMs).toLocaleString()}</div>
  `;
}

function isTargetedSosPoint(point: (typeof sosStore.locations)[number], latestRecordedAtMs: number): boolean {
  const target = selectedSosTarget.value;
  if (!target || point.recordedAtMs !== latestRecordedAtMs) {
    return false;
  }
  const sameSource = sosIdentityKey(point.incidentId, point.sourceHex)
    === sosIdentityKey(target.incidentId, target.sourceHex);
  if (!sameSource) {
    return false;
  }
  if (!target.messageIdHex) {
    return true;
  }
  const alert = sosStore.alerts.find((candidate) =>
    sosIdentityKey(candidate.incidentId, candidate.sourceHex)
      === sosIdentityKey(point.incidentId, point.sourceHex),
  );
  return safeLower(alert?.messageIdHex) === safeLower(target.messageIdHex);
}

function syncSosTrails(): void {
  if (!map) {
    return;
  }
  const active = new Set<string>();
  const features: Array<Record<string, unknown>> = [];
  let targetMarker: Marker | null = null;
  let targetCoordinates: [number, number] | null = null;
  for (const [incidentId, points] of sosStore.locationsByIncident.entries()) {
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

const liveTelemetryCount = computed(() =>
  Math.max(0, telemetryStore.activePositions.length - telemetryStore.stalePositions.length),
);
const staleTelemetryCount = computed(() => telemetryStore.stalePositions.length);
const sosAlertCount = computed(() => sosStore.alerts.length);

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
    },
    { immediate: true, deep: true },
  );
  stopSosWatch = watch(
    () => [
      sosStore.locations,
      sosStore.alerts,
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
  stopWatch = null;
  stopSosWatch = null;
  for (const marker of markersByCallsign.values()) {
    marker.remove();
  }
  for (const marker of sosMarkersByKey.values()) {
    marker.remove();
  }
  markersByCallsign.clear();
  markerElementsByCallsign.clear();
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

<style scoped>
.telemetry-view {
  display: flex;
  height: calc(100% + 0.2rem);
  margin-bottom: -0.2rem;
  min-height: 100%;
}

.telemetry-header h1 {
  font-family: var(--font-headline);
  font-size: clamp(1.2rem, 3vw, 1.9rem);
  margin: 0;
}

.telemetry-header p {
  color: #9cb3d6;
  font-size: 0.85rem;
  margin: 0.2rem 0 0;
}

.map-frame {
  flex: 1 1 auto;
  min-height: 0;
  position: relative;
}

.map-overlay {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.52rem;
  left: 0.82rem;
  max-width: calc(100% - 1.64rem);
  position: absolute;
  top: 0.82rem;
  z-index: 3;
}

.map-chip {
  align-items: center;
  background: rgb(7 25 54 / 84%);
  border: 1px solid rgb(73 173 255 / 46%);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(183 235 255 / 8%),
    0 0 18px rgb(33 153 255 / 7%);
  color: #8fcaff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: clamp(0.88rem, 2vw, 1.02rem);
  font-weight: 700;
  gap: 0.42rem;
  justify-content: center;
  min-height: 2.64rem;
  min-width: 3.35rem;
  padding: 0.42rem 0.58rem;
}

button.map-chip {
  cursor: pointer;
}

.map-chip svg {
  flex: 0 0 auto;
  height: 1.05rem;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 1.05rem;
}

.map-chip-count {
  font-variant-numeric: tabular-nums;
  min-width: 0;
}

.live-chip {
  border-color: rgb(67 218 157 / 48%);
  color: #58f090;
}

.stale-chip {
  border-color: rgb(225 159 79 / 48%);
  color: #f7b860;
}

.sos-chip {
  border-color: rgb(239 68 68 / 58%);
  color: #ff5e64;
}

.layer-control {
  position: relative;
}

.layer-chip {
  color: #8fcaff;
  min-width: 2.72rem;
  padding-inline: 0.56rem;
}

.layer-menu {
  background: rgb(4 17 39 / 94%);
  border: 1px solid rgb(113 175 255 / 46%);
  border-radius: 8px;
  box-shadow: 0 16px 34px rgb(0 0 0 / 38%);
  display: grid;
  gap: 0.25rem;
  min-width: 8rem;
  padding: 0.34rem;
  position: absolute;
  right: 0;
  top: calc(100% + 0.42rem);
}

.layer-option {
  background: transparent;
  border: 1px solid transparent;
  border-radius: 6px;
  color: #d9ecff;
  font-family: var(--font-ui);
  font-size: 0.82rem;
  font-weight: 800;
  padding: 0.46rem 0.58rem;
  text-align: left;
}

.layer-option[aria-checked="true"] {
  background: rgb(43 217 178 / 16%);
  border-color: rgb(72 224 186 / 42%);
  color: #7af4d3;
}

.map-container {
  border: 1px solid rgb(90 142 220 / 24%);
  border-radius: 12px;
  height: 100%;
  min-height: inherit;
  overflow: hidden;
}

:deep(.telemetry-marker) {
  align-items: center;
  border: 2px solid #05203f;
  border-radius: 999px;
  box-shadow: 0 0 12px rgb(0 0 0 / 35%);
  display: flex;
  height: 14px;
  justify-content: center;
  position: relative;
  width: 14px;
}

:deep(.telemetry-marker::after) {
  align-items: center;
  background: #071a36;
  border: 1px solid rgb(176 214 255 / 72%);
  border-radius: 999px;
  color: #d9ecff;
  content: attr(data-overlap-count);
  display: none;
  font-family: var(--font-ui);
  font-size: 0.58rem;
  font-weight: 800;
  height: 0.88rem;
  justify-content: center;
  line-height: 1;
  position: absolute;
  right: -0.58rem;
  top: -0.58rem;
  width: 0.88rem;
}

:deep(.telemetry-marker.is-overlapped::after) {
  display: flex;
}

:deep(.telemetry-marker.is-live) {
  background: #2bd9b2;
}

:deep(.telemetry-marker.is-stale) {
  background: #ffb467;
}

:deep(.telemetry-marker-label) {
  background: rgb(4 17 39 / 92%);
  border: 1px solid rgb(130 185 255 / 50%);
  border-radius: 5px;
  box-shadow: 0 5px 14px rgb(0 0 0 / 34%);
  color: #d9ecff;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 800;
  left: 50%;
  line-height: 1;
  max-width: 8.5rem;
  min-width: max-content;
  overflow: hidden;
  padding: 0.24rem 0.42rem;
  pointer-events: none;
  position: absolute;
  text-overflow: ellipsis;
  top: calc(100% + 0.32rem + var(--label-offset-y, 0rem));
  transform: translateX(-50%);
  white-space: nowrap;
}

:deep(.telemetry-marker.is-live .telemetry-marker-label) {
  border-color: rgb(72 224 186 / 58%);
}

:deep(.telemetry-marker.is-stale .telemetry-marker-label) {
  border-color: rgb(255 180 103 / 62%);
  color: #ffe1bd;
}

:deep(.sos-trail-marker) {
  background: #ef4444;
  border: 2px solid #7f1d1d;
  border-radius: 999px;
  box-shadow: 0 0 14px rgb(239 68 68 / 70%);
  height: 12px;
  width: 12px;
}

:deep(.sos-trail-marker.is-blinking) {
  animation: sos-marker-pulse 1s ease-in-out infinite;
}

:deep(.sos-trail-marker.is-targeted) {
  border-color: #fecaca;
  height: 16px;
  width: 16px;
}

:deep(.popup-title) {
  color: #0a244a;
  font-size: 0.9rem;
  font-weight: 700;
}

:deep(.popup-title-sos) {
  color: #b91c1c;
}

:deep(.popup-body) {
  color: #0a244a;
  font-size: 0.82rem;
  line-height: 1.35;
  margin: 0.25rem 0;
  max-width: 14rem;
}

:deep(.popup-secondary) {
  color: #2c476f;
  font-size: 0.75rem;
}

@keyframes sos-marker-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 0 rgb(239 68 68 / 66%), 0 0 14px rgb(239 68 68 / 76%);
    transform: scale(1);
  }

  50% {
    box-shadow: 0 0 0 9px rgb(239 68 68 / 0%), 0 0 22px rgb(239 68 68 / 92%);
    transform: scale(1.18);
  }
}

@media (max-width: 780px) {
  .map-frame {
    min-height: 0;
  }

  .map-overlay {
    gap: 0.4rem;
    left: 0.58rem;
    max-width: calc(100% - 1.16rem);
    top: 0.58rem;
  }

  .map-chip {
    font-size: 0.72rem;
    gap: 0.3rem;
    min-height: 2.4rem;
    min-width: 2.86rem;
    padding-inline: 0.42rem;
  }

  .map-chip svg {
    height: 0.9rem;
    width: 0.9rem;
  }
}
</style>
