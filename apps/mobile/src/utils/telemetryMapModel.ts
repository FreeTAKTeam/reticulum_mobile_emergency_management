import type { LngLatLike, StyleSpecification } from "maplibre-gl";
import type { LocationQuery } from "vue-router";

import type { ActionMessage, DiscoveredPeer, TelemetryPosition } from "../types/domain";

export type MapLayerId = "base" | "satellite";
export interface SosRouteTarget { incidentId: string; sourceHex: string; messageIdHex?: string }
export interface TelemetryRouteTarget { callsign?: string; lat: number; lon: number }
export interface MarkerLabelPlacement { stackIndex: number; stackSize: number }
export interface TelemetryCluster {
  key: string;
  count: number;
  lat: number;
  lon: number;
  tone: "is-live" | "is-stale" | "is-mixed";
}
export interface SosPointLike {
  incidentId: string;
  sourceHex: string;
  lat: number;
  lon: number;
  recordedAtMs: number;
  batteryPercent?: number;
}
export interface SosAlertLike {
  incidentId: string;
  sourceHex: string;
  messageIdHex?: string;
  bodyUtf8?: string;
}

interface ProjectedPoint { x: number; y: number }
interface ProjectionMap {
  getZoom(): number;
  project(coordinate: LngLatLike): ProjectedPoint;
}

export const TELEMETRY_CLUSTER_MAX_ZOOM = 12;
export const TELEMETRY_CLUSTER_PIXEL_RADIUS = 52;
export const mapLayerOptions: Array<{ id: MapLayerId; label: string }> = [
  { id: "base", label: "Base" },
  { id: "satellite", label: "Satellite" },
];

const BASE_MAP_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";

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
    layers: [{ id: "esri-world-imagery", type: "raster", source: "esri-world-imagery" }],
  };
}

export function mapStyleFor(layer: MapLayerId): string | StyleSpecification {
  return layer === "satellite" ? satelliteMapStyle() : BASE_MAP_STYLE_URL;
}

export function safeTrim(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function safeLower(value: unknown): string {
  return safeTrim(value).toLowerCase();
}

function routeQueryString(value: unknown): string {
  return Array.isArray(value) ? safeTrim(value[0]) : safeTrim(value);
}

function routeQueryNumber(value: unknown): number | null {
  const normalized = routeQueryString(value);
  if (!normalized) return null;
  const parsed = Number(normalized);
  return Number.isFinite(parsed) ? parsed : null;
}

export function parseSosRouteTarget(query: LocationQuery): SosRouteTarget | null {
  const incidentId = routeQueryString(query.incident);
  const sourceHex = routeQueryString(query.source);
  if (!incidentId || !sourceHex) return null;
  const messageIdHex = routeQueryString(query.message);
  return { incidentId, sourceHex, ...(messageIdHex ? { messageIdHex } : {}) };
}

export function parseTelemetryRouteTarget(query: LocationQuery): TelemetryRouteTarget | null {
  const lat = routeQueryNumber(query.lat);
  const lon = routeQueryNumber(query.lon);
  if (lat === null || lon === null) return null;
  const callsign = routeQueryString(query.callsign);
  return { ...(callsign ? { callsign } : {}), lat, lon };
}

export function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function lineBreakHtml(value: string): string {
  return escapeHtml(value).replace(/\r?\n/g, "<br>");
}

export function visibleSosBodyText(body: string): string {
  return body.split(/\r?\n/)
    .filter((line) => !safeTrim(line).toLowerCase().startsWith("gps:"))
    .join("\n")
    .trim();
}

export function sosIdentityKey(incidentId: string, sourceHex: string): string {
  return `${safeLower(incidentId)}:${safeLower(sourceHex)}`;
}

export function markerStatusClass(position: TelemetryPosition, staleThresholdMs: number): string {
  return Date.now() - position.updatedAt > staleThresholdMs ? "is-stale" : "is-live";
}

export function speedLine(position: TelemetryPosition): string {
  if (position.speed === undefined && position.course === undefined) return "";
  const parts: string[] = [];
  if (position.speed !== undefined) parts.push(`Speed ${position.speed.toFixed(1)}`);
  if (position.course !== undefined) parts.push(`Course ${position.course.toFixed(0)}&deg;`);
  return `<div class="popup-secondary">${parts.join(" &middot; ")}</div>`;
}

export function positionLabel(
  position: TelemetryPosition,
  peers: Record<string, DiscoveredPeer>,
): string {
  const peer = peers[safeLower(position.callsign)];
  return safeTrim(peer?.announcedName) || safeTrim(peer?.label) || position.callsign;
}

export function eamMessageForPosition(
  position: TelemetryPosition,
  label: string,
  messages: ActionMessage[],
): ActionMessage | undefined {
  const positionKeys = new Set([position.callsign, label].map(safeLower).filter(Boolean));
  return messages.find((message) => [
    message.callsign,
    message.reportedBy,
    message.source?.display_name,
  ].map(safeLower).some((key) => key && positionKeys.has(key)));
}

export function peerForPosition(
  position: TelemetryPosition,
  label: string,
  peers: Record<string, DiscoveredPeer>,
): DiscoveredPeer | undefined {
  const positionKeys = new Set([position.callsign, label].map(safeLower).filter(Boolean));
  return Object.values(peers).find((peer) => [
    peer.destination,
    peer.lxmfDestinationHex,
    peer.identityHex,
    peer.announcedName,
    peer.label,
  ].map(safeLower).some((key) => key && positionKeys.has(key)));
}

export function chatDestinationForPeer(peer: DiscoveredPeer | undefined): string {
  return peer ? safeTrim(peer.lxmfDestinationHex) || safeTrim(peer.destination) : "";
}

export function peerDisplayName(peer: DiscoveredPeer | undefined, fallback: string): string {
  return safeTrim(peer?.announcedName) || safeTrim(peer?.label) || fallback;
}

export function eamPieHtml(
  message: ActionMessage | undefined,
  readiness: { overallScore: number; overallRingColor: string; overallBand: string } | undefined,
): string {
  if (!message) return "";
  const score = readiness?.overallScore ?? 0;
  const color = readiness?.overallRingColor ?? "#ff3648";
  const band = readiness?.overallBand ?? "Unknown";
  return `
    <div class="popup-eam-pie" style="--popup-eam-pct: ${score}; --popup-eam-color: ${color};"
      aria-label="Overall readiness ${score}% ${band}" role="img"
      title="Overall readiness ${score}% (${band})"><span>${score}%</span></div>
  `;
}

export function telemetryMarkerKey(position: TelemetryPosition): string {
  return position.callsign.toLowerCase();
}

function telemetryCoordinateKey(position: TelemetryPosition): string {
  return `${position.lat}:${position.lon}`;
}

function distanceBetween(left: ProjectedPoint, right: ProjectedPoint): number {
  return Math.hypot(left.x - right.x, left.y - right.y);
}

function telemetryClusterFor(
  positions: TelemetryPosition[],
  staleThresholdMs: number,
): TelemetryCluster {
  const totals = positions.reduce(
    (sum, position) => ({ lat: sum.lat + position.lat, lon: sum.lon + position.lon }),
    { lat: 0, lon: 0 },
  );
  const tones = new Set(positions.map((position) => markerStatusClass(position, staleThresholdMs)));
  return {
    key: positions.map(telemetryMarkerKey).sort().join("|"),
    count: positions.length,
    lat: totals.lat / positions.length,
    lon: totals.lon / positions.length,
    tone: tones.has("is-live") && tones.has("is-stale")
      ? "is-mixed" : tones.has("is-stale") ? "is-stale" : "is-live",
  };
}

export function telemetryRenderGroups(
  positions: TelemetryPosition[],
  map: ProjectionMap | null,
  staleThresholdMs: number,
): { clusters: TelemetryCluster[]; individuals: TelemetryPosition[] } {
  if (!map || map.getZoom() > TELEMETRY_CLUSTER_MAX_ZOOM) {
    return { clusters: [], individuals: positions };
  }
  type Draft = { center: ProjectedPoint; positions: TelemetryPosition[]; cellKey: string };
  const drafts: Draft[] = [];
  const draftIndexesByCell = new Map<string, Set<number>>();
  const cellCoordinates = (point: ProjectedPoint): [number, number] => [
    Math.floor(point.x / TELEMETRY_CLUSTER_PIXEL_RADIUS),
    Math.floor(point.y / TELEMETRY_CLUSTER_PIXEL_RADIUS),
  ];
  const cellKey = (x: number, y: number): string => `${x}:${y}`;
  const addToCell = (key: string, index: number): void => {
    const indexes = draftIndexesByCell.get(key) ?? new Set<number>();
    indexes.add(index);
    draftIndexesByCell.set(key, indexes);
  };
  for (const position of positions) {
    const projected = map.project([position.lon, position.lat]);
    const [cellX, cellY] = cellCoordinates(projected);
    const candidateIndexes = new Set<number>();
    for (let x = cellX - 1; x <= cellX + 1; x += 1) {
      for (let y = cellY - 1; y <= cellY + 1; y += 1) {
        for (const index of draftIndexesByCell.get(cellKey(x, y)) ?? []) {
          candidateIndexes.add(index);
        }
      }
    }
    const draftIndex = [...candidateIndexes]
      .sort((left, right) => left - right)
      .find((index) =>
        distanceBetween(drafts[index].center, projected) <= TELEMETRY_CLUSTER_PIXEL_RADIUS,
      );
    const draft = draftIndex === undefined ? undefined : drafts[draftIndex];
    if (!draft) {
      const key = cellKey(cellX, cellY);
      drafts.push({ center: projected, positions: [position], cellKey: key });
      addToCell(key, drafts.length - 1);
      continue;
    }
    const nextCount = draft.positions.length + 1;
    draft.center = {
      x: (draft.center.x * draft.positions.length + projected.x) / nextCount,
      y: (draft.center.y * draft.positions.length + projected.y) / nextCount,
    };
    draft.positions.push(position);
    const [nextCellX, nextCellY] = cellCoordinates(draft.center);
    const nextCellKey = cellKey(nextCellX, nextCellY);
    if (nextCellKey !== draft.cellKey && draftIndex !== undefined) {
      draftIndexesByCell.get(draft.cellKey)?.delete(draftIndex);
      addToCell(nextCellKey, draftIndex);
      draft.cellKey = nextCellKey;
    }
  }
  const clusters: TelemetryCluster[] = [];
  const individuals: TelemetryPosition[] = [];
  for (const draft of drafts) {
    if (draft.positions.length > 1) {
      clusters.push(telemetryClusterFor(draft.positions, staleThresholdMs));
    } else {
      individuals.push(...draft.positions);
    }
  }
  return { clusters, individuals };
}

export function labelPlacementsFor(
  positions: TelemetryPosition[],
): Map<string, MarkerLabelPlacement> {
  const groups = new Map<string, TelemetryPosition[]>();
  for (const position of positions) {
    const key = telemetryCoordinateKey(position);
    groups.set(key, [...(groups.get(key) ?? []), position]);
  }
  const placements = new Map<string, MarkerLabelPlacement>();
  for (const group of groups.values()) {
    group.forEach((position, stackIndex) => placements.set(telemetryMarkerKey(position), {
      stackIndex,
      stackSize: group.length,
    }));
  }
  return placements;
}

export function buildTelemetryPopupHtml(
  position: TelemetryPosition,
  label: string,
  eamPie: string,
): string {
  const identityLine = label === position.callsign
    ? "" : `<div class="popup-secondary">${escapeHtml(position.callsign)}</div>`;
  return `
    <div class="popup-title">${escapeHtml(label)}</div>
    ${identityLine}
    <div class="popup-secondary">Updated ${new Date(position.updatedAt).toLocaleString()}</div>
    ${speedLine(position)}
    <div class="popup-actions">
      ${eamPie}
      <button class="popup-action-button popup-chat-button" type="button" data-chat="1">Chat</button>
      <button class="popup-details-button" type="button" data-eam-details="1">Details</button>
    </div>
  `;
}

export function createTelemetryPopupElement(
  html: string,
  onChat: () => void,
  onDetails: () => void,
): HTMLDivElement {
  const element = document.createElement("div");
  element.className = "telemetry-popup";
  element.innerHTML = html;
  element.querySelector<HTMLButtonElement>("[data-chat]")?.addEventListener("click", onChat);
  element.querySelector<HTMLButtonElement>("[data-eam-details]")?.addEventListener("click", onDetails);
  return element;
}

function findSosAlert(
  point: SosPointLike,
  activeAlerts: SosAlertLike[],
  alerts: SosAlertLike[],
): SosAlertLike | undefined {
  const key = sosIdentityKey(point.incidentId, point.sourceHex);
  return activeAlerts.find((candidate) => sosIdentityKey(candidate.incidentId, candidate.sourceHex) === key)
    ?? alerts.find((candidate) => sosIdentityKey(candidate.incidentId, candidate.sourceHex) === key);
}

export function buildSosPopupHtml(
  point: SosPointLike,
  activeAlerts: SosAlertLike[],
  alerts: SosAlertLike[],
): string {
  const alert = findSosAlert(point, activeAlerts, alerts);
  const body = visibleSosBodyText(safeTrim(alert?.bodyUtf8)) || "SOS emergency";
  const battery = point.batteryPercent !== undefined
    ? `<div class="popup-secondary">Battery ${point.batteryPercent.toFixed(0)}%</div>` : "";
  return `
    <div class="popup-title popup-title-sos">SOS EMERGENCY</div>
    <div class="popup-body">${lineBreakHtml(body)}</div>
    <div class="popup-secondary">Source ${escapeHtml(point.sourceHex)}</div>
    <div class="popup-secondary">${point.lat.toFixed(6)}, ${point.lon.toFixed(6)}</div>
    ${battery}
    <div class="popup-secondary">Updated ${new Date(point.recordedAtMs).toLocaleString()}</div>
  `;
}

export function matchesSosRouteTarget(
  point: SosPointLike,
  latestRecordedAtMs: number,
  target: SosRouteTarget | null,
  activeAlerts: SosAlertLike[],
  alerts: SosAlertLike[],
): boolean {
  if (!target || point.recordedAtMs !== latestRecordedAtMs) return false;
  if (sosIdentityKey(point.incidentId, point.sourceHex)
    !== sosIdentityKey(target.incidentId, target.sourceHex)) return false;
  if (!target.messageIdHex) return true;
  return safeLower(findSosAlert(point, activeAlerts, alerts)?.messageIdHex)
    === safeLower(target.messageIdHex);
}
