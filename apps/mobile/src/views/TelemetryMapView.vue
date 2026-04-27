<script setup lang="ts">
import "maplibre-gl/dist/maplibre-gl.css";

import maplibregl, { Marker, type LngLatLike, type Map as MapLibreMap } from "maplibre-gl";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
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

interface SosRouteTarget {
  incidentId: string;
  sourceHex: string;
  messageIdHex?: string;
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

  for (const position of positions) {
    const key = position.callsign.toLowerCase();
    active.add(key);

    let marker = markersByCallsign.get(key);
    let markerElement = markerElementsByCallsign.get(key);

    if (!marker || !markerElement) {
      markerElement = document.createElement("div");
      markerElement.className = "telemetry-marker";
      markerElement.title = positionLabel(position);

      marker = new maplibregl.Marker({ element: markerElement })
        .setLngLat([position.lon, position.lat] as LngLatLike)
        .setPopup(new maplibregl.Popup({ offset: 20 }).setHTML(popupHtml(position)))
        .addTo(map);

      markersByCallsign.set(key, marker);
      markerElementsByCallsign.set(key, markerElement);
    } else {
      marker.setLngLat([position.lon, position.lat] as LngLatLike);
      marker.getPopup()?.setHTML(popupHtml(position));
      markerElement.title = positionLabel(position);
    }

    markerElement.classList.remove("is-live", "is-stale");
    markerElement.classList.add(markerStatusClass(position));
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

const lastUpdatedLabel = computed(() => {
  const latest = telemetryStore.positions[0];
  if (!latest) {
    return "No telemetry received yet.";
  }
  const ageMs = Date.now() - latest.updatedAt;
  if (ageMs < 60_000) {
    return "Last update: < 1 min ago";
  }
  const minutes = Math.round(ageMs / 60_000);
  return `Last update: ${minutes} min ago`;
});

const staleThresholdMinutesLabel = computed(() =>
  Math.max(1, nodeStore.settings.telemetry.staleAfterMinutes),
);

onMounted(() => {
  if (!mapHost.value) {
    return;
  }

  map = new maplibregl.Map({
    container: mapHost.value,
    style: "https://tiles.openfreemap.org/styles/liberty",
    center: [-98.5795, 39.8283],
    zoom: 3,
  });

  map.addControl(new maplibregl.NavigationControl({ visualizePitch: true }), "top-right");
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
    <div class="telemetry-legend">
      <span>{{ lastUpdatedLabel }}</span>
      <span><i class="dot live"></i> Live (&lt; {{ staleThresholdMinutesLabel }} min)</span>
      <span><i class="dot stale"></i> Stale (&ge; {{ staleThresholdMinutesLabel }} min)</span>
      <span><i class="dot sos"></i> SOS trail</span>
    </div>

    <div ref="mapHost" class="map-container"></div>
  </section>
</template>

<style scoped>
.telemetry-view {
  display: grid;
  gap: 0.75rem;
  grid-template-rows: auto minmax(0, 1fr);
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

.telemetry-legend {
  color: #adc0dd;
  display: flex;
  flex-wrap: wrap;
  font-size: 0.75rem;
  gap: 0.9rem;
}

.dot {
  border-radius: 999px;
  display: inline-block;
  height: 0.55rem;
  margin-right: 0.3rem;
  width: 0.55rem;
}

.dot.live {
  background: #25d8b2;
}

.dot.stale {
  background: #f9ae66;
}

.dot.sos {
  background: #ef4444;
}

.map-container {
  border: 1px solid rgb(90 142 220 / 24%);
  border-radius: 12px;
  min-height: 420px;
  overflow: hidden;
}

:deep(.telemetry-marker) {
  border: 2px solid #05203f;
  border-radius: 999px;
  box-shadow: 0 0 12px rgb(0 0 0 / 35%);
  height: 14px;
  width: 14px;
}

:deep(.telemetry-marker.is-live) {
  background: #2bd9b2;
}

:deep(.telemetry-marker.is-stale) {
  background: #ffb467;
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
  .map-container {
    min-height: min(60dvh, 520px);
  }
}
</style>
