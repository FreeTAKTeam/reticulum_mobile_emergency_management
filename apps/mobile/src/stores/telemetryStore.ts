import { defineStore } from "pinia";
import { computed, reactive, ref, watch } from "vue";
import type { PacketReceivedEvent } from "@reticulum/node-client";

import type { ReplicationMessage, TelemetryPosition } from "../types/domain";
import {
  telemetryService,
  TelemetryPermissionDeniedError,
  type TelemetryPermissionState,
} from "../services/telemetry";
import { asNumber, asTrimmedString, parseReplicationEnvelope } from "../utils/replicationParser";
import { useNodeStore } from "./nodeStore";

const TELEMETRY_STORAGE_KEY = "reticulum.mobile.telemetry.v1";
const STALE_THRESHOLD_MS = 5 * 60 * 1000;
const EXPIRED_THRESHOLD_MS = 10 * 60 * 1000;
const MIN_MOVEMENT_METERS = 15;
type UpsertOutcome = "inserted" | "updated" | "ignored";

type TelemetryLoopStatus = "idle" | "running" | "permission_denied" | "gps_unavailable" | "error";

const TELEMETRY_FIELD_PREFIX = "telemetry.";
const TELEMETRY_KIND_FIELD = `${TELEMETRY_FIELD_PREFIX}kind`;
const TELEMETRY_UPSERT_KIND = "upsert";
const TELEMETRY_DELETE_KIND = "delete";

type TelemetryReplicationMessage =
  | {
      kind: "telemetry_snapshot_request";
      requestedAt: number;
    }
  | {
      kind: "telemetry_snapshot_response";
      requestedAt: number;
      positions: TelemetryPosition[];
    }
  | {
      kind: "telemetry_upsert";
      position: TelemetryPosition;
    }
  | {
      kind: "telemetry_delete";
      callsign: string;
      deletedAt: number;
    };

function normalizeOptionalNumber(value: unknown): number | undefined {
  if (value === undefined || value === null || value === "") {
    return undefined;
  }
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}

function normalizeTelemetryPosition(position: TelemetryPosition): TelemetryPosition {
  return {
    callsign: asTrimmedString(position.callsign),
    lat: asNumber(position.lat, 0),
    lon: asNumber(position.lon, 0),
    alt: normalizeOptionalNumber(position.alt),
    course: normalizeOptionalNumber(position.course),
    speed: normalizeOptionalNumber(position.speed),
    accuracy: normalizeOptionalNumber(position.accuracy),
    updatedAt: asNumber(position.updatedAt, Date.now()),
  };
}

function loadPositions(): Record<string, TelemetryPosition> {
  try {
    const raw = localStorage.getItem(TELEMETRY_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw) as TelemetryPosition[];
    const out: Record<string, TelemetryPosition> = {};
    for (const position of parsed) {
      const normalized = normalizeTelemetryPosition(position);
      if (!normalized.callsign) {
        continue;
      }
      out[normalized.callsign.toLowerCase()] = normalized;
    }
    return out;
  } catch {
    return {};
  }
}

function savePositions(records: Record<string, TelemetryPosition>): void {
  localStorage.setItem(TELEMETRY_STORAGE_KEY, JSON.stringify(Object.values(records)));
}

function dedicatedTelemetryValue(fields: Record<string, string>, key: string): string | undefined {
  return fields[`${TELEMETRY_FIELD_PREFIX}${key}`];
}

function parseDedicatedTelemetryMessage(
  event: PacketReceivedEvent,
): TelemetryReplicationMessage | null {
  const fields = event.dedicatedFields;
  if (!fields) {
    return null;
  }
  const kind = dedicatedTelemetryValue(fields, "kind");
  if (kind === TELEMETRY_UPSERT_KIND) {
    const callsign = asTrimmedString(dedicatedTelemetryValue(fields, "callsign"));
    if (!callsign) {
      return null;
    }
    return {
      kind: "telemetry_upsert",
      position: normalizeTelemetryPosition({
        callsign,
        lat: asNumber(dedicatedTelemetryValue(fields, "lat"), 0),
        lon: asNumber(dedicatedTelemetryValue(fields, "lon"), 0),
        alt: normalizeOptionalNumber(dedicatedTelemetryValue(fields, "alt")),
        course: normalizeOptionalNumber(dedicatedTelemetryValue(fields, "course")),
        speed: normalizeOptionalNumber(dedicatedTelemetryValue(fields, "speed")),
        accuracy: normalizeOptionalNumber(dedicatedTelemetryValue(fields, "accuracy")),
        updatedAt: asNumber(dedicatedTelemetryValue(fields, "updatedAt"), Date.now()),
      }),
    };
  }

  if (kind === TELEMETRY_DELETE_KIND) {
    return {
      kind: "telemetry_delete",
      callsign: asTrimmedString(dedicatedTelemetryValue(fields, "callsign")),
      deletedAt: asNumber(dedicatedTelemetryValue(fields, "deletedAt"), Date.now()),
    };
  }

  return null;
}

function buildTelemetryDedicatedFields(
  message: Extract<TelemetryReplicationMessage, { kind: "telemetry_upsert" | "telemetry_delete" }>,
): Record<string, string> {
  if (message.kind === "telemetry_upsert") {
    const { position } = message;
    const dedicatedFields: Record<string, string> = {
      [TELEMETRY_KIND_FIELD]: TELEMETRY_UPSERT_KIND,
      [`${TELEMETRY_FIELD_PREFIX}callsign`]: position.callsign,
      [`${TELEMETRY_FIELD_PREFIX}lat`]: String(position.lat),
      [`${TELEMETRY_FIELD_PREFIX}lon`]: String(position.lon),
      [`${TELEMETRY_FIELD_PREFIX}updatedAt`]: String(position.updatedAt),
    };

    if (position.alt !== undefined) {
      dedicatedFields[`${TELEMETRY_FIELD_PREFIX}alt`] = String(position.alt);
    }
    if (position.course !== undefined) {
      dedicatedFields[`${TELEMETRY_FIELD_PREFIX}course`] = String(position.course);
    }
    if (position.speed !== undefined) {
      dedicatedFields[`${TELEMETRY_FIELD_PREFIX}speed`] = String(position.speed);
    }
    if (position.accuracy !== undefined) {
      dedicatedFields[`${TELEMETRY_FIELD_PREFIX}accuracy`] = String(position.accuracy);
    }

    return dedicatedFields;
  }

  return {
    [TELEMETRY_KIND_FIELD]: TELEMETRY_DELETE_KIND,
    [`${TELEMETRY_FIELD_PREFIX}callsign`]: message.callsign,
    [`${TELEMETRY_FIELD_PREFIX}deletedAt`]: String(message.deletedAt),
  };
}

function parseTelemetryReplicationMessage(raw: string): TelemetryReplicationMessage | null {
  const envelope = parseReplicationEnvelope(raw);
  if (!envelope) {
    return null;
  }

  const { kind, payload } = envelope;
  switch (kind) {
    case "telemetry_snapshot_request":
      return {
        kind: "telemetry_snapshot_request",
        requestedAt: asNumber(payload.requestedAt, Date.now()),
      };
    case "telemetry_snapshot_response":
      return {
        kind: "telemetry_snapshot_response",
        requestedAt: asNumber(payload.requestedAt, Date.now()),
        positions: Array.isArray(payload.positions)
          ? payload.positions.map((entry) => normalizeTelemetryPosition(entry as TelemetryPosition))
          : [],
      };
    case "telemetry_upsert":
      if (!payload.position || typeof payload.position !== "object") {
        return null;
      }
      return {
        kind: "telemetry_upsert",
        position: normalizeTelemetryPosition(payload.position as TelemetryPosition),
      };
    case "telemetry_delete":
      return {
        kind: "telemetry_delete",
        callsign: asTrimmedString(payload.callsign),
        deletedAt: asNumber(payload.deletedAt, Date.now()),
      };
    default:
      return null;
  }
}

function distanceMeters(a: { lat: number; lon: number }, b: { lat: number; lon: number }): number {
  const toRadians = (value: number): number => (value * Math.PI) / 180;
  const earthRadiusMeters = 6_371_000;
  const dLat = toRadians(b.lat - a.lat);
  const dLon = toRadians(b.lon - a.lon);
  const lat1 = toRadians(a.lat);
  const lat2 = toRadians(b.lat);

  const haversine =
    Math.sin(dLat / 2) * Math.sin(dLat / 2) +
    Math.cos(lat1) * Math.cos(lat2) * Math.sin(dLon / 2) * Math.sin(dLon / 2);
  const c = 2 * Math.atan2(Math.sqrt(haversine), Math.sqrt(1 - haversine));
  return earthRadiusMeters * c;
}

export const useTelemetryStore = defineStore("telemetry", () => {
  const byCallsign = reactive<Record<string, TelemetryPosition>>({});
  const tombstones = reactive<Record<string, number>>({});
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const nowTimestamp = ref(Date.now());
  const loopTimer = ref<number | null>(null);
  const loopInFlight = ref(false);
  const permissionState = ref<TelemetryPermissionState>("prompt");
  const loopStatus = ref<TelemetryLoopStatus>("idle");
  const telemetryError = ref("");
  const lastLocalFix = ref<TelemetryPosition | null>(null);
  const nodeStore = useNodeStore();

  function persist(): void {
    savePositions(byCallsign);
  }

  function keyFor(callsign: string): string {
    return callsign.trim().toLowerCase();
  }

  function applyUpsert(position: TelemetryPosition): UpsertOutcome {
    const normalized = normalizeTelemetryPosition(position);
    const key = keyFor(normalized.callsign);
    if (!key) {
      return "ignored";
    }
    const tombstonedAt = tombstones[key];
    if (tombstonedAt && tombstonedAt >= normalized.updatedAt) {
      return "ignored";
    }
    const existing = byCallsign[key];
    if (existing && existing.updatedAt > normalized.updatedAt) {
      return "ignored";
    }
    const outcome: UpsertOutcome = existing ? "updated" : "inserted";
    byCallsign[key] = normalized;
    persist();
    return outcome;
  }

  function applyDelete(callsign: string, deletedAt: number): void {
    const key = keyFor(callsign);
    if (!key) {
      return;
    }
    tombstones[key] = Math.max(tombstones[key] ?? 0, deletedAt);

    const existing = byCallsign[key];
    if (!existing || existing.updatedAt > deletedAt) {
      return;
    }
    delete byCallsign[key];
    persist();
  }

  function snapshotPositions(): TelemetryPosition[] {
    return Object.values(byCallsign).map((entry) => ({ ...entry }));
  }

  function buildLocalPosition(): Promise<TelemetryPosition | null> {
    return telemetryService.getCurrentPosition().then((fix) => {
      const callsign = nodeStore.settings.displayName.trim();
      if (!callsign) {
        return null;
      }
      return normalizeTelemetryPosition({
        callsign,
        lat: fix.lat,
        lon: fix.lon,
        alt: fix.alt,
        course: fix.course,
        speed: fix.speed,
        accuracy: fix.accuracy,
        updatedAt: fix.timestamp || Date.now(),
      });
    });
  }

  function shouldPublishPosition(next: TelemetryPosition): boolean {
    const previous = lastLocalFix.value;
    if (!previous) {
      return true;
    }

    const moved = distanceMeters(previous, next);
    if (moved >= MIN_MOVEMENT_METERS) {
      return true;
    }

    const accuracyThreshold = nodeStore.settings.telemetry.accuracyThresholdMeters;
    if (accuracyThreshold === undefined) {
      return false;
    }

    return (next.accuracy ?? Number.POSITIVE_INFINITY) <= accuracyThreshold;
  }

  async function publishOnce(): Promise<void> {
    if (loopInFlight.value) {
      return;
    }
    loopInFlight.value = true;

    try {
      const position = await buildLocalPosition();
      if (!position) {
        loopStatus.value = "error";
        telemetryError.value = "Set a call sign before enabling telemetry.";
        return;
      }

      if (!shouldPublishPosition(position)) {
        loopStatus.value = "running";
        telemetryError.value = "";
        return;
      }

      lastLocalFix.value = position;
      applyUpsert(position);
      const message: TelemetryReplicationMessage = {
        kind: "telemetry_upsert",
        position,
      };
      await nodeStore.broadcastJson(message as ReplicationMessage, buildTelemetryDedicatedFields(message));
      loopStatus.value = "running";
      telemetryError.value = "";
    } catch (error: unknown) {
      if (error instanceof TelemetryPermissionDeniedError) {
        permissionState.value = "denied";
        loopStatus.value = "permission_denied";
        telemetryError.value = "Location permission denied.";
        stopPublishLoop();
        return;
      }

      loopStatus.value = "gps_unavailable";
      telemetryError.value = error instanceof Error ? error.message : String(error);
    } finally {
      loopInFlight.value = false;
    }
  }

  function stopPublishLoop(): void {
    if (loopTimer.value !== null) {
      window.clearInterval(loopTimer.value);
      loopTimer.value = null;
    }
    if (loopStatus.value === "running") {
      loopStatus.value = "idle";
    }
  }

  async function startPublishLoop(): Promise<void> {
    stopPublishLoop();

    permissionState.value = await telemetryService.getPermissionState();
    if (permissionState.value !== "granted") {
      permissionState.value = await telemetryService.requestPermission();
    }

    if (permissionState.value === "denied") {
      loopStatus.value = "permission_denied";
      telemetryError.value = "Telemetry disabled: location permission denied.";
      return;
    }

    if (permissionState.value === "unavailable") {
      loopStatus.value = "gps_unavailable";
      telemetryError.value = "Telemetry unavailable on this device.";
      return;
    }

    loopStatus.value = "running";
    telemetryError.value = "";

    await publishOnce();
    const intervalMs = Math.max(5, nodeStore.settings.telemetry.publishIntervalSeconds) * 1000;
    loopTimer.value = window.setInterval(() => {
      void publishOnce();
    }, intervalMs);
  }

  function syncPublishLoopFromSettings(): void {
    if (!nodeStore.settings.telemetry.enabled) {
      stopPublishLoop();
      permissionState.value = "prompt";
      telemetryError.value = "";
      loopStatus.value = "idle";
      return;
    }

    void startPublishLoop();
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;

    const loaded = loadPositions();
    for (const [key, position] of Object.entries(loaded)) {
      byCallsign[key] = position;
    }

    window.setInterval(() => {
      nowTimestamp.value = Date.now();
    }, 30_000);

    watch(
      () => [
        nodeStore.settings.telemetry.enabled,
        nodeStore.settings.telemetry.publishIntervalSeconds,
        nodeStore.settings.telemetry.accuracyThresholdMeters,
        nodeStore.settings.displayName,
      ],
      () => {
        syncPublishLoopFromSettings();
      },
      { immediate: true },
    );
  }

  async function upsertLocalPosition(
    input: Omit<TelemetryPosition, "updatedAt"> & {
      updatedAt?: number;
    },
  ): Promise<void> {
    const position = normalizeTelemetryPosition({
      ...input,
      updatedAt: asNumber(input.updatedAt, Date.now()),
    });
    applyUpsert(position);
    const message: TelemetryReplicationMessage = {
      kind: "telemetry_upsert",
      position,
    };
    await nodeStore.broadcastJson(message as ReplicationMessage, buildTelemetryDedicatedFields(message));
  }

  async function deleteLocal(callsign: string): Promise<void> {
    const deletedAt = Date.now();
    applyDelete(callsign, deletedAt);
    const message: TelemetryReplicationMessage = {
      kind: "telemetry_delete",
      callsign,
      deletedAt,
    };
    await nodeStore.broadcastJson(message as ReplicationMessage, buildTelemetryDedicatedFields(message));
  }

  function initReplication(): void {
    if (replicationInitialized.value) {
      return;
    }
    replicationInitialized.value = true;

    const decoder = new TextDecoder();
    nodeStore.onPacket((event: PacketReceivedEvent) => {
      const message = parseDedicatedTelemetryMessage(event) ?? parseTelemetryReplicationMessage(decoder.decode(event.bytes));
      if (!message) {
        return;
      }

      if (message.kind === "telemetry_snapshot_request") {
        nodeStore
          .broadcastJson({
            kind: "telemetry_snapshot_response",
            requestedAt: message.requestedAt,
            positions: snapshotPositions(),
          } as ReplicationMessage)
          .catch(() => undefined);
        return;
      }

      if (message.kind === "telemetry_snapshot_response") {
        for (const position of message.positions) {
          applyUpsert(position);
        }
        return;
      }

      if (message.kind === "telemetry_upsert") {
        applyUpsert(message.position);
        return;
      }

      applyDelete(message.callsign, message.deletedAt);
    });

    watch(
      () => [...nodeStore.connectedDestinations],
      (current, previous) => {
        const previousSet = new Set(previous);
        for (const destination of current) {
          if (previousSet.has(destination)) {
            continue;
          }
          nodeStore
            .sendJson(destination, {
              kind: "telemetry_snapshot_request",
              requestedAt: Date.now(),
            } as ReplicationMessage)
            .catch(() => undefined);
        }
      },
      { immediate: true },
    );
  }

  const positions = computed(() =>
    Object.values(byCallsign).sort((a, b) => b.updatedAt - a.updatedAt),
  );

  const activePositions = computed(() =>
    positions.value
      .filter((position) => nowTimestamp.value - position.updatedAt <= EXPIRED_THRESHOLD_MS)
      .sort((a, b) => b.updatedAt - a.updatedAt),
  );

  const stalePositions = computed(() =>
    activePositions.value.filter(
      (position) => nowTimestamp.value - position.updatedAt > STALE_THRESHOLD_MS,
    ),
  );

  const expiredPositions = computed(() =>
    positions.value.filter(
      (position) => nowTimestamp.value - position.updatedAt > EXPIRED_THRESHOLD_MS,
    ),
  );

  return {
    byCallsign,
    positions,
    activePositions,
    stalePositions,
    expiredPositions,
    permissionState,
    loopStatus,
    telemetryError,
    init,
    initReplication,
    startPublishLoop,
    stopPublishLoop,
    upsertLocalPosition,
    deleteLocal,
  };
});
