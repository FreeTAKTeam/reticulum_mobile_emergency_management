import maplibregl, { type Map as MapLibreMap, type Marker } from "maplibre-gl";

import type { TelemetryCluster } from "./telemetryMapModel";

export function ensureMarkerLabelElement(markerElement: HTMLDivElement): HTMLSpanElement {
  const existing = markerElement.querySelector<HTMLSpanElement>(".telemetry-marker-label");
  if (existing) return existing;
  const labelElement = document.createElement("span");
  labelElement.className = "telemetry-marker-label";
  markerElement.append(labelElement);
  return labelElement;
}

export function syncTelemetryClusterMarkers(
  map: MapLibreMap | null,
  clusters: TelemetryCluster[],
  markers: Map<string, Marker>,
  elements: Map<string, HTMLDivElement>,
): void {
  if (!map) return;
  const active = new Set<string>();
  for (const cluster of clusters) {
    active.add(cluster.key);
    let marker = markers.get(cluster.key);
    let element = elements.get(cluster.key);
    if (!marker || !element) {
      element = document.createElement("div");
      element.className = "telemetry-cluster";
      marker = new maplibregl.Marker({ element })
        .setLngLat([cluster.lon, cluster.lat])
        .addTo(map);
      markers.set(cluster.key, marker);
      elements.set(cluster.key, element);
    } else {
      marker.setLngLat([cluster.lon, cluster.lat]);
    }
    element.classList.add("telemetry-cluster");
    element.classList.remove("is-live", "is-stale", "is-mixed");
    element.classList.add(cluster.tone);
    element.dataset.count = String(cluster.count);
    element.textContent = String(cluster.count);
    element.title = `${cluster.count} telemetry positions`;
  }
  for (const [key, marker] of markers.entries()) {
    if (active.has(key)) continue;
    marker.remove();
    markers.delete(key);
    elements.delete(key);
  }
}
