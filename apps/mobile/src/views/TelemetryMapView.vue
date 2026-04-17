<script setup lang="ts">
import "maplibre-gl/dist/maplibre-gl.css";

import maplibregl, { Marker, type LngLatLike, type Map as MapLibreMap } from "maplibre-gl";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

import type { TelemetryPosition } from "../types/domain";
import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { useTelemetryStore } from "../stores/telemetryStore";

const nodeStore = useNodeStore();
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

function safeTrim(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

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

function syncSosTrails(): void {
  if (!map) {
    return;
  }
  const active = new Set<string>();
  const features = [];
  for (const [incidentId, points] of sosStore.locationsByIncident.entries()) {
    const coordinates = points.map((point) => [point.lon, point.lat]);
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
      if (sosMarkersByKey.has(key)) {
        continue;
      }
      const element = document.createElement("div");
      element.className = "sos-trail-marker";
      element.title = "SOS location";
      const marker = new maplibregl.Marker({ element })
        .setLngLat([point.lon, point.lat] as LngLatLike)
        .addTo(map);
      sosMarkersByKey.set(key, marker);
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
    () => sosStore.locations,
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
  map?.remove();
  map = null;
});
</script>

<template>
  <section class="telemetry-view">
    <header class="telemetry-header">
      <h1>Telemetry Map</h1>
      <p>{{ lastUpdatedLabel }}</p>
    </header>

    <div class="telemetry-legend">
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
  grid-template-rows: auto auto minmax(0, 1fr);
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

:deep(.popup-title) {
  color: #0a244a;
  font-size: 0.9rem;
  font-weight: 700;
}

:deep(.popup-secondary) {
  color: #2c476f;
  font-size: 0.75rem;
}

@media (max-width: 780px) {
  .map-container {
    min-height: min(60dvh, 520px);
  }
}
</style>
