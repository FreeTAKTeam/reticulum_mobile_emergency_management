<script setup lang="ts">
import "./EventMapLayer.css";

import * as maplibregl from "maplibre-gl";
import type { Map as MapLibreMap } from "maplibre-gl";
import { onBeforeUnmount, watch } from "vue";
import { useRouter } from "vue-router";

import { useEventsStore } from "../../stores/eventsStore";
import type { EventTimelineRecord } from "../../utils/eventProjection";
import { mecpCategoryIconBody } from "../../utils/mecpCategoryIcons";

const props = defineProps<{ map: MapLibreMap | null }>();
const events = useEventsStore();
const router = useRouter();
let markers: maplibregl.Marker[] = [];

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text = ""): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = className;
  // textContent only: event text comes from the network.
  node.textContent = text;
  return node;
}

// Not a <button>: the global button press transform would fight MapLibre's positioning transform.
function badge(event: EventTimelineRecord): HTMLElement {
  const node = el("div", `event-map-marker is-${event.mecp?.severityStatus ?? "unknown"}`);
  node.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${mecpCategoryIconBody(event.mecp?.categoryCode)}</svg>`;
  return node;
}

function popupElement(event: EventTimelineRecord): HTMLDivElement {
  const body = el("div", "event-map-popup");
  const head = el("div", "event-map-popup-head");
  const heading = el("div", "");
  heading.append(el("div", "popup-title", event.mecp?.codeLabels.join(" + ") || event.type));
  if (event.mecp) {
    heading.append(el("span", `event-map-chip is-${event.mecp.severityStatus}`, event.mecp.severity));
  }
  head.append(badge(event), heading);
  const open = el("button", "popup-action-button", "Open event");
  open.type = "button";
  open.addEventListener("click", () => void router.push({ name: "events", query: { event: event.uid } }));
  const actions = el("div", "popup-actions");
  actions.append(open);
  body.append(
    head,
    el("div", "popup-body", event.mecp ? event.mecp.details : event.summary),
    el("div", "popup-secondary", [event.callsign, event.mecp?.codes.join(" ")].filter(Boolean).join(" · ")),
    el("div", "popup-secondary", `${event.position?.lat.toFixed(6)}, ${event.position?.lon.toFixed(6)}`),
    el("div", "popup-secondary", `Updated ${new Date(event.updatedAt).toLocaleString()}`),
    actions,
  );
  return body;
}

function clear(): void {
  for (const marker of markers) marker.remove();
  markers = [];
}

// Records rarely change, so rebuild every marker rather than diffing them.
function render(): void {
  clear();
  const map = props.map;
  if (!map) return;
  for (const event of events.records) {
    if (!event.position) continue;
    const element = badge(event);
    element.setAttribute("role", "button");
    element.setAttribute("aria-label", `${event.type} event from ${event.callsign}`);
    element.dataset.eventUid = event.uid;
    markers.push(new maplibregl.Marker({ element })
      .setLngLat([event.position.lon, event.position.lat])
      .setPopup(new maplibregl.Popup({ closeButton: true, offset: 22 }).setDOMContent(popupElement(event)))
      .addTo(map));
  }
}

watch([() => props.map, () => events.records], render, { immediate: true });
onBeforeUnmount(clear);
</script>

<template>
  <span hidden data-testid="event-map-layer" />
</template>
