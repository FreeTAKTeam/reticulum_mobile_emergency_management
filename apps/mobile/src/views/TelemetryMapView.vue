<script setup lang="ts">
import "maplibre-gl/dist/maplibre-gl.css";

import maplibregl, {
  Marker,
  type LngLatLike,
  type Map as MapLibreMap,
  type StyleSpecification,
} from "maplibre-gl";
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import type { ActionMessage, DiscoveredPeer, TelemetryPosition } from "../types/domain";
import { useMessagesStore } from "../stores/messagesStore";
import { useMessagingStore } from "../stores/messagingStore";
import { useNodeStore } from "../stores/nodeStore";
import { useSosStore } from "../stores/sosStore";
import { useTelemetryStore } from "../stores/telemetryStore";

const messagesStore = useMessagesStore();
const messagingStore = useMessagingStore();
const nodeStore = useNodeStore();
const route = useRoute();
const router = useRouter();
const sosStore = useSosStore();
const telemetryStore = useTelemetryStore();

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

type MapLayerId = "base" | "satellite";

interface SosRouteTarget {
  incidentId: string;
  sourceHex: string;
  messageIdHex?: string;
}

interface TelemetryRouteTarget {
  callsign?: string;
  lat: number;
  lon: number;
}

interface MarkerLabelPlacement {
  stackIndex: number;
  stackSize: number;
}

interface ProjectedPoint {
  x: number;
  y: number;
}

interface TelemetryCluster {
  key: string;
  count: number;
  lat: number;
  lon: number;
  tone: "is-live" | "is-stale" | "is-mixed";
}

const BASE_MAP_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";
const TELEMETRY_CLUSTER_MAX_ZOOM = 12;
const TELEMETRY_CLUSTER_PIXEL_RADIUS = 52;
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

function routeQueryNumber(value: unknown): number | null {
  const normalized = routeQueryString(value);
  if (!normalized) {
    return null;
  }
  const parsed = Number(normalized);
  return Number.isFinite(parsed) ? parsed : null;
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

const selectedTelemetryTarget = computed<TelemetryRouteTarget | null>(() => {
  const callsign = routeQueryString(route.query.callsign);
  const lat = routeQueryNumber(route.query.lat);
  const lon = routeQueryNumber(route.query.lon);
  if (lat !== null && lon !== null) {
    return {
      ...(callsign ? { callsign } : {}),
      lat,
      lon,
    };
  }
  const position = telemetryStore.byCallsign[safeLower(callsign)];
  return position
    ? {
        callsign: position.callsign,
        lat: position.lat,
        lon: position.lon,
      }
    : null;
});

const selectedTelemetryTargetKey = computed(() => {
  const target = selectedTelemetryTarget.value;
  if (!target) {
    return "";
  }
  return `${safeLower(target.callsign)}:${target.lat}:${target.lon}`;
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

function eamMessageForPosition(position: TelemetryPosition): ActionMessage | undefined {
  const positionKeys = new Set(
    [position.callsign, positionLabel(position)]
      .map((value) => safeLower(value))
      .filter(Boolean),
  );
  return messagesStore.messages.find((message) => {
    const messageKeys = [
      message.callsign,
      message.reportedBy,
      message.source?.display_name,
    ].map((value) => safeLower(value));
    return messageKeys.some((key) => key && positionKeys.has(key));
  });
}

function peerForPosition(position: TelemetryPosition): DiscoveredPeer | undefined {
  const positionKeys = new Set(
    [position.callsign, positionLabel(position)]
      .map((value) => safeLower(value))
      .filter(Boolean),
  );
  return Object.values(nodeStore.discoveredByDestination).find((peer) => {
    const peerKeys = [
      peer.destination,
      peer.lxmfDestinationHex,
      peer.identityHex,
      peer.announcedName,
      peer.label,
    ].map((value) => safeLower(value));
    return peerKeys.some((key) => key && positionKeys.has(key));
  });
}

function chatDestinationForPeer(peer: DiscoveredPeer | undefined): string {
  if (!peer) {
    return "";
  }
  return safeTrim(peer.lxmfDestinationHex) || safeTrim(peer.destination);
}

function peerDisplayName(peer: DiscoveredPeer | undefined, fallback: string): string {
  return safeTrim(peer?.announcedName) || safeTrim(peer?.label) || fallback;
}

function eamPieHtml(message: ActionMessage | undefined): string {
  if (!message) {
    return "";
  }
  const readiness = messagesStore.eamReadinessForCallsign(message.callsign);
  const score = readiness?.overallScore ?? 0;
  const color = readiness?.overallRingColor ?? "#ff3648";
  const band = readiness?.overallBand ?? "Unknown";
  return `
    <div
      class="popup-eam-pie"
      style="--popup-eam-pct: ${score}; --popup-eam-color: ${color};"
      aria-label="Overall readiness ${score}% ${band}"
      role="img"
      title="Overall readiness ${score}% (${band})"
    >
      <span>${score}%</span>
    </div>
  `;
}

function openEamDetails(callsign: string): void {
  const targetCallsign = safeTrim(callsign);
  void router.push({
    name: "messages",
    query: targetCallsign ? { callsign: targetCallsign } : undefined,
  });
}

async function openChatForPosition(position: TelemetryPosition): Promise<void> {
  const peer = peerForPosition(position);
  const destinationHex = chatDestinationForPeer(peer);
  if (!destinationHex) {
    return;
  }
  messagingStore.ensureConversationForDestination(
    destinationHex,
    peerDisplayName(peer, positionLabel(position)),
  );
  await router.push({
    path: "/inbox",
    query: messagingStore.selectedConversationId
      ? { conversation: messagingStore.selectedConversationId }
      : undefined,
  });
}

function telemetryMarkerKey(position: TelemetryPosition): string {
  return position.callsign.toLowerCase();
}

function telemetryCoordinateKey(position: TelemetryPosition): string {
  return `${position.lat}:${position.lon}`;
}

function distanceBetween(left: ProjectedPoint, right: ProjectedPoint): number {
  return Math.hypot(left.x - right.x, left.y - right.y);
}

function telemetryClusterKey(positions: TelemetryPosition[]): string {
  return positions.map(telemetryMarkerKey).sort().join("|");
}

function telemetryClusterTone(positions: TelemetryPosition[]): TelemetryCluster["tone"] {
  const tones = new Set(positions.map(markerStatusClass));
  if (tones.has("is-live") && tones.has("is-stale")) {
    return "is-mixed";
  }
  return tones.has("is-stale") ? "is-stale" : "is-live";
}

function telemetryClusterFor(positions: TelemetryPosition[]): TelemetryCluster {
  const totals = positions.reduce(
    (acc, position) => ({
      lat: acc.lat + position.lat,
      lon: acc.lon + position.lon,
    }),
    { lat: 0, lon: 0 },
  );
  return {
    key: telemetryClusterKey(positions),
    count: positions.length,
    lat: totals.lat / positions.length,
    lon: totals.lon / positions.length,
    tone: telemetryClusterTone(positions),
  };
}

function telemetryRenderGroups(positions: TelemetryPosition[]): {
  clusters: TelemetryCluster[];
  individuals: TelemetryPosition[];
} {
  if (!map || map.getZoom() > TELEMETRY_CLUSTER_MAX_ZOOM) {
    return { clusters: [], individuals: positions };
  }

  const drafts: Array<{ center: ProjectedPoint; positions: TelemetryPosition[] }> = [];
  for (const position of positions) {
    const projected = map.project([position.lon, position.lat] as LngLatLike);
    const draft = drafts.find((candidate) =>
      distanceBetween(candidate.center, projected) <= TELEMETRY_CLUSTER_PIXEL_RADIUS,
    );
    if (!draft) {
      drafts.push({ center: projected, positions: [position] });
      continue;
    }

    const nextCount = draft.positions.length + 1;
    draft.center = {
      x: (draft.center.x * draft.positions.length + projected.x) / nextCount,
      y: (draft.center.y * draft.positions.length + projected.y) / nextCount,
    };
    draft.positions.push(position);
  }

  const clusters: TelemetryCluster[] = [];
  const individuals: TelemetryPosition[] = [];
  for (const draft of drafts) {
    if (draft.positions.length > 1) {
      clusters.push(telemetryClusterFor(draft.positions));
    } else {
      individuals.push(...draft.positions);
    }
  }
  return { clusters, individuals };
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

function syncTelemetryClusters(clusters: TelemetryCluster[]): void {
  if (!map) {
    return;
  }

  const active = new Set<string>();
  for (const cluster of clusters) {
    active.add(cluster.key);
    let marker = telemetryClustersByKey.get(cluster.key);
    let element = telemetryClusterElementsByKey.get(cluster.key);

    if (!marker || !element) {
      element = document.createElement("div");
      element.className = "telemetry-cluster";
      marker = new maplibregl.Marker({ element })
        .setLngLat([cluster.lon, cluster.lat] as LngLatLike)
        .addTo(map);
      telemetryClustersByKey.set(cluster.key, marker);
      telemetryClusterElementsByKey.set(cluster.key, element);
    } else {
      marker.setLngLat([cluster.lon, cluster.lat] as LngLatLike);
    }

    element.classList.add("telemetry-cluster");
    element.classList.remove("is-live", "is-stale", "is-mixed");
    element.classList.add(cluster.tone);
    element.dataset.count = String(cluster.count);
    element.textContent = String(cluster.count);
    element.title = `${cluster.count} telemetry positions`;
  }

  for (const [key, marker] of telemetryClustersByKey.entries()) {
    if (active.has(key)) {
      continue;
    }
    marker.remove();
    telemetryClustersByKey.delete(key);
    telemetryClusterElementsByKey.delete(key);
  }
}

function popupHtml(position: TelemetryPosition): string {
  const label = positionLabel(position);
  const eamMessage = eamMessageForPosition(position);
  const identityLine =
    label === position.callsign
      ? ""
      : `<div class="popup-secondary">${escapeHtml(position.callsign)}</div>`;
  return `
    <div class="popup-title">${escapeHtml(label)}</div>
    ${identityLine}
    <div class="popup-secondary">Updated ${new Date(position.updatedAt).toLocaleString()}</div>
    ${speedLine(position)}
    <div class="popup-actions">
      ${eamPieHtml(eamMessage)}
      <button class="popup-action-button popup-chat-button" type="button" data-chat="1">Chat</button>
      <button class="popup-details-button" type="button" data-eam-details="1">Details</button>
    </div>
  `;
}

function telemetryPopupElement(position: TelemetryPosition): HTMLDivElement {
  const element = document.createElement("div");
  element.className = "telemetry-popup";
  element.innerHTML = popupHtml(position);
  element.querySelector<HTMLButtonElement>("[data-chat]")?.addEventListener("click", () => {
    void openChatForPosition(position);
  });
  element.querySelector<HTMLButtonElement>("[data-eam-details]")?.addEventListener("click", () => {
    openEamDetails(position.callsign);
  });
  return element;
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
  const alert = sosStore.activeAlerts.find((candidate) =>
    sosIdentityKey(candidate.incidentId, candidate.sourceHex)
      === sosIdentityKey(point.incidentId, point.sourceHex),
  ) ?? sosStore.alerts.find((candidate) =>
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
  const alert = sosStore.activeAlerts.find((candidate) =>
    sosIdentityKey(candidate.incidentId, candidate.sourceHex)
      === sosIdentityKey(point.incidentId, point.sourceHex),
  ) ?? sosStore.alerts.find((candidate) =>
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

:deep(.telemetry-cluster) {
  align-items: center;
  background: #071a36;
  border: 2px solid rgb(176 214 255 / 72%);
  border-radius: 999px;
  box-shadow:
    0 0 0 4px rgb(7 26 54 / 32%),
    0 7px 18px rgb(0 0 0 / 38%);
  color: #d9ecff;
  display: inline-flex;
  font-family: var(--font-ui);
  font-size: 0.82rem;
  font-weight: 900;
  height: 2rem;
  justify-content: center;
  line-height: 1;
  min-width: 2rem;
  padding: 0 0.48rem;
  width: max-content;
}

:deep(.telemetry-cluster.is-live) {
  background: #0b3f45;
  border-color: #2bd9b2;
  color: #bfffee;
}

:deep(.telemetry-cluster.is-stale) {
  background: #3d2d1d;
  border-color: #ffb467;
  color: #ffe1bd;
}

:deep(.telemetry-cluster.is-mixed) {
  background: #172742;
  border-color: #8fcaff;
  color: #eef7ff;
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

:deep(.maplibregl-popup) {
  filter: drop-shadow(0 18px 32px rgb(0 0 0 / 48%));
}

:deep(.maplibregl-popup-content) {
  background:
    linear-gradient(150deg, rgb(9 25 55 / 96%), rgb(7 16 37 / 98%)),
    radial-gradient(circle at 14% 0%, rgb(100 190 255 / 18%), transparent 42%);
  border: 2px solid #14f0ff;
  border-radius: 14px;
  box-shadow:
    inset 0 1px 0 rgb(224 248 255 / 16%),
    inset 0 0 0 1px rgb(0 168 255 / 24%),
    0 0 0 1px rgb(3 18 40 / 76%),
    0 0 22px rgb(20 240 255 / 34%),
    0 0 42px rgb(0 168 255 / 18%);
  color: #def1ff;
  min-width: 12.4rem;
  padding: 0.82rem 0.9rem;
}

:deep(.maplibregl-popup-anchor-top .maplibregl-popup-tip) {
  border-bottom-color: rgb(9 25 55 / 96%);
}

:deep(.maplibregl-popup-anchor-bottom .maplibregl-popup-tip) {
  border-top-color: rgb(7 16 37 / 98%);
}

:deep(.maplibregl-popup-anchor-left .maplibregl-popup-tip) {
  border-right-color: rgb(9 25 55 / 96%);
}

:deep(.maplibregl-popup-anchor-right .maplibregl-popup-tip) {
  border-left-color: rgb(9 25 55 / 96%);
}

:deep(.maplibregl-popup-close-button) {
  color: #14f0ff;
  font-family: var(--font-ui);
  font-size: 1rem;
  padding: 0.18rem 0.4rem;
}

:deep(.popup-title) {
  color: #def1ff;
  font-family: var(--font-headline);
  font-size: 1rem;
  font-weight: 700;
  line-height: 1;
  text-shadow: 0 0 12px rgb(100 190 255 / 18%);
}

:deep(.popup-title-sos) {
  color: #ff7b89;
}

:deep(.popup-body) {
  color: #c8dcf7;
  font-family: var(--font-body);
  font-size: 0.82rem;
  line-height: 1.35;
  margin: 0.25rem 0;
  max-width: 14rem;
}

:deep(.popup-secondary) {
  color: #9cb3d6;
  font-family: var(--font-body);
  font-size: 0.75rem;
  line-height: 1.35;
}

:deep(.popup-actions) {
  align-items: center;
  display: flex;
  gap: 0.6rem;
  margin-top: 0.54rem;
}

:deep(.popup-eam-pie) {
  --popup-eam-color: #8fcaff;
  --popup-eam-pct: 0;
  align-items: center;
  background:
    radial-gradient(circle at center, #071025 0 44%, transparent 45%),
    conic-gradient(
      var(--popup-eam-color) calc(var(--popup-eam-pct) * 1%),
      rgb(88 120 168 / 30%) 0
    );
  border: 1px solid rgb(100 190 255 / 36%);
  border-radius: 999px;
  box-shadow:
    inset 0 0 0 1px rgb(222 241 255 / 7%),
    0 0 18px color-mix(in srgb, var(--popup-eam-color) 24%, transparent);
  color: var(--popup-eam-color);
  display: inline-flex;
  flex: 0 0 auto;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 900;
  height: 2.72rem;
  justify-content: center;
  letter-spacing: 0;
  line-height: 1;
  width: 2.72rem;
}

:deep(.popup-eam-pie span) {
  display: inline-block;
  text-align: center;
}

:deep(.popup-action-button),
:deep(.popup-details-button) {
  --btn-bg: linear-gradient(180deg, rgb(10 35 72 / 88%), rgb(6 24 54 / 92%));
  --btn-bg-pressed: linear-gradient(180deg, rgb(196 240 255 / 96%), rgb(118 212 255 / 94%));
  --btn-border: rgb(74 133 207 / 45%);
  --btn-border-pressed: rgb(224 248 255 / 86%);
  --btn-color: #8fdbff;
  --btn-color-pressed: #042541;
  --btn-shadow: inset 0 1px 0 rgb(209 244 255 / 10%), 0 8px 18px rgb(2 14 32 / 18%);
  --btn-shadow-pressed: inset 0 1px 0 rgb(255 255 255 / 75%), 0 4px 10px rgb(3 21 47 / 24%);
  background: var(--btn-bg);
  border: 1px solid var(--btn-border);
  border-radius: 8px;
  box-shadow: var(--btn-shadow);
  color: var(--btn-color);
  cursor: pointer;
  font-family: var(--font-ui);
  font-size: 0.72rem;
  font-weight: 900;
  min-height: 2rem;
  padding: 0 0.64rem;
  text-transform: uppercase;
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
