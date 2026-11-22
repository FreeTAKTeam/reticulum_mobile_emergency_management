<script setup lang="ts">
import * as maplibregl from "maplibre-gl";
import type { GeoJSONSource, Map as MapLibreMap, MapLayerMouseEvent } from "maplibre-gl";
import type { FeatureCollection, Point } from "geojson";
import { onBeforeUnmount, watch } from "vue";

import { useEventsStore } from "../../stores/eventsStore";

const props = defineProps<{ map: MapLibreMap | null }>();
const events = useEventsStore();
const SOURCE = "rem-events";
const LAYER = "rem-events-circle";
const SEVERITY_COLORS = { red: "#d9534f", yellow: "#e7b55a", green: "#5cb85c", unknown: "#8a9ba8" };

function data(): FeatureCollection<Point> {
  return {
    type: "FeatureCollection",
    features: events.records.flatMap((event) => event.position ? [{
      type: "Feature" as const,
      geometry: { type: "Point" as const, coordinates: [event.position.lon, event.position.lat] },
      properties: {
        severity: event.mecp?.severityStatus ?? "unknown",
        category: event.type,
        callsign: event.callsign,
        text: event.mecp
          ? [event.mecp.codeLabels.join(", "), event.mecp.details].filter(Boolean).join(" — ")
          : event.summary,
      },
    }] : []),
  };
}

function render(): void {
  const map = props.map;
  if (!map) return;
  if (!map.isStyleLoaded()) {
    // Records rarely change, so wait for the style instead of the next update.
    map.off("idle", render);
    map.once("idle", render);
    return;
  }
  const source = map.getSource(SOURCE) as GeoJSONSource | undefined;
  if (source) {
    source.setData(data());
    return;
  }
  map.addSource(SOURCE, { type: "geojson", data: data() });
  map.addLayer({ id: LAYER, source: SOURCE, type: "circle", paint: {
    "circle-radius": 8,
    "circle-color": ["match", ["get", "severity"],
      "red", SEVERITY_COLORS.red, "yellow", SEVERITY_COLORS.yellow, "green", SEVERITY_COLORS.green,
      SEVERITY_COLORS.unknown],
    "circle-stroke-color": "#202a30",
    "circle-stroke-width": 2,
  } });
}

function showPopup(event: MapLayerMouseEvent): void {
  const feature = event.features?.[0];
  const map = props.map;
  if (!feature || !map || feature.geometry.type !== "Point") return;
  const { category, callsign, text } = feature.properties as Record<string, string>;
  const body = document.createElement("div");
  body.className = "event-map-popup";
  // textContent only: event text comes from the network.
  for (const [tag, value] of [["strong", category], ["div", callsign], ["p", text]] as const) {
    const line = document.createElement(tag);
    line.textContent = value;
    body.append(line);
  }
  new maplibregl.Popup({ closeButton: true, offset: 12 })
    .setLngLat(feature.geometry.coordinates as [number, number])
    .setDOMContent(body)
    .addTo(map);
}

function detach(map: MapLibreMap | null | undefined): void {
  if (!map) return;
  map.off("styledata", render);
  map.off("idle", render);
  map.off("click", LAYER, showPopup);
  if (map.getLayer(LAYER)) map.removeLayer(LAYER);
  if (map.getSource(SOURCE)) map.removeSource(SOURCE);
}

watch(() => props.map, (map, previous) => {
  detach(previous);
  map?.on("styledata", render);
  map?.on("click", LAYER, showPopup);
  render();
}, { immediate: true });
watch(() => events.records, render);
onBeforeUnmount(() => detach(props.map));
</script>

<template>
  <span hidden data-testid="event-map-layer" />
</template>
