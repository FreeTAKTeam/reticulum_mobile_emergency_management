import { pack, unpack } from "msgpackr";
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
import { TELEMETRY_CAPABILITY } from "../utils/peers";
import { useNodeStore } from "./nodeStore";

const TELEMETRY_STORAGE_KEY = "reticulum.mobile.telemetry.v1";
const STALE_THRESHOLD_MS = 5 * 60 * 1000;
const EXPIRED_THRESHOLD_MS = 10 * 60 * 1000;
const MIN_MOVEMENT_METERS = 15;
const EMPTY_BYTES = new Uint8Array(0);

type UpsertOutcome = "inserted" | "updated" | "ignored";

type TelemetryLoopStatus = "idle" | "running" | "permission_denied" | "gps_unavailable" | "error";

const TELEMETRY_FIELD_PREFIX = "telemetry.";
const TELEMETRY_KIND_FIELD = `${TELEMETRY_FIELD_PREFIX}kind`;
const TELEMETRY_UPSERT_KIND = "upsert";
const TELEMETRY_DELETE_KIND = "delete";

const LXMF_FIELD_TELEMETRY = 0x02;
const LXMF_FIELD_TELEMETRY_STREAM = 0x03;
const LXMF_FIELD_COMMANDS = 0x09;
const TELEMETRY_REQUEST_COMMAND = 1;
const SID_TIME = 0x01;
const SID_LOCATION = 0x02;
const MAX_LOCATION_ALTITUDE = 42_949_672.95;

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

function encodeBytesToBase64(value: Uint8Array): string {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: Uint8Array): { toString(encoding: string): string } };
    }
  ).Buffer;
  if (bufferCtor) {
    return bufferCtor.from(value).toString("base64");
  }

  let binary = "";
  for (const byte of value) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function decodeBase64ToBytes(value: string): Uint8Array {
  const bufferCtor = (
    globalThis as unknown as {
      Buffer?: { from(data: string, encoding: string): Uint8Array };
    }
  ).Buffer;
  if (bufferCtor) {
    return Uint8Array.from(bufferCtor.from(value, "base64"));
  }

  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

function asFiniteNumber(value: unknown): number | undefined {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : undefined;
}

function asUint8Array(value: unknown): Uint8Array | null {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (Array.isArray(value) && value.every((entry) => Number.isInteger(entry) && entry >= 0 && entry <= 255)) {
    return Uint8Array.from(value);
  }
  return null;
}

function readInt32(value: unknown): number | null {
  const bytes = asUint8Array(value);
  if (!bytes || bytes.byteLength !== 4) {
    return null;
  }
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getInt32(0);
}

function readUint32(value: unknown): number | null {
  const bytes = asUint8Array(value);
  if (!bytes || bytes.byteLength !== 4) {
    return null;
  }
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(0);
}

function readUint16(value: unknown): number | null {
  const bytes = asUint8Array(value);
  if (!bytes || bytes.byteLength !== 2) {
    return null;
  }
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint16(0);
}

function writeInt32(value: number): Uint8Array {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setInt32(0, value);
  return out;
}

function writeUint32(value: number): Uint8Array {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, value);
  return out;
}

function writeUint16(value: number): Uint8Array {
  const out = new Uint8Array(2);
  new DataView(out.buffer).setUint16(0, value);
  return out;
}

function normalizeAltitude(value: number | undefined): number {
  if (value === undefined || !Number.isFinite(value)) {
    return 0;
  }
  if (value >= MAX_LOCATION_ALTITUDE) {
    return 0;
  }
  return Math.max(0, value);
}

function toUnixSeconds(value: number): number {
  return value > 1_000_000_000_000 ? Math.floor(value / 1000) : Math.floor(value);
}

function toTimestampMs(value: unknown, fallbackMs: number): number {
  const numeric = asFiniteNumber(value);
  if (numeric === undefined) {
    return fallbackMs;
  }
  return numeric > 1_000_000_000_000 ? Math.floor(numeric) : Math.floor(numeric * 1000);
}

function getMapValue(source: unknown, key: number): unknown {
  if (!source || typeof source !== "object") {
    return undefined;
  }
  if (source instanceof Map) {
    return source.get(key) ?? source.get(String(key));
  }
  const record = source as Record<string, unknown>;
  return record[String(key)] ?? record[key as unknown as keyof typeof record];
}

function bytesToHex(value: unknown): string | null {
  const bytes = asUint8Array(value);
  if (!bytes || bytes.byteLength === 0) {
    return null;
  }
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function hexToBytes(hex: string): Uint8Array | null {
  const normalized = hex.trim().toLowerCase();
  if (!/^[0-9a-f]{32}$/i.test(normalized)) {
    return null;
  }
  const out = new Uint8Array(normalized.length / 2);
  for (let i = 0; i < normalized.length; i += 2) {
    out[i / 2] = Number.parseInt(normalized.slice(i, i + 2), 16);
  }
  return out;
}

function buildTelemetryPayload(position: TelemetryPosition): Uint8Array {
  const updatedAtSeconds = toUnixSeconds(position.updatedAt);
  const payload = new Map<number, unknown>([
    [SID_TIME, updatedAtSeconds],
    [
      SID_LOCATION,
      [
        writeInt32(Math.round(position.lat * 1_000_000)),
        writeInt32(Math.round(position.lon * 1_000_000)),
        writeUint32(Math.round(normalizeAltitude(position.alt) * 100)),
        writeUint32(Math.round((position.speed ?? 0) * 100)),
        writeUint32(Math.round((position.course ?? 0) * 100)),
        writeUint16(Math.round((position.accuracy ?? 0) * 100)),
        updatedAtSeconds,
      ],
    ],
  ]);
  return Uint8Array.from(pack(payload));
}

function buildTelemetryFieldsBase64(position: TelemetryPosition): string {
  const fields = new Map<number, unknown>([[LXMF_FIELD_TELEMETRY, buildTelemetryPayload(position)]]);
  return encodeBytesToBase64(Uint8Array.from(pack(fields)));
}

function buildTelemetrySnapshotRequestFieldsBase64(requestedAt: number): string {
  const fields = new Map<number, unknown>([
    [LXMF_FIELD_COMMANDS, [{ [String(TELEMETRY_REQUEST_COMMAND)]: toUnixSeconds(requestedAt) }]],
  ]);
  return encodeBytesToBase64(Uint8Array.from(pack(fields)));
}

function buildTelemetryStreamFieldsBase64(entries: Array<[Uint8Array, number, Uint8Array]>): string {
  const fields = new Map<number, unknown>([[LXMF_FIELD_TELEMETRY_STREAM, entries]]);
  return encodeBytesToBase64(Uint8Array.from(pack(fields)));
}

function parseLxmfFields(fieldsBase64: string | undefined): unknown {
  if (!fieldsBase64) {
    return null;
  }
  try {
    return unpack(decodeBase64ToBytes(fieldsBase64));
  } catch {
    return null;
  }
}

function parseTelemetryPayload(
  payload: unknown,
  callsign: string,
  fallbackUpdatedAtMs: number,
): TelemetryPosition | null {
  let decodedPayload = payload;
  const rawBytes = asUint8Array(payload);
  if (rawBytes) {
    try {
      decodedPayload = unpack(rawBytes);
    } catch {
      return null;
    }
  }

  const location = getMapValue(decodedPayload, SID_LOCATION);
  if (!Array.isArray(location) || location.length < 7) {
    return null;
  }

  const latRaw = readInt32(location[0]);
  const lonRaw = readInt32(location[1]);
  const altRaw = readUint32(location[2]);
  const speedRaw = readUint32(location[3]);
  const courseRaw = readUint32(location[4]);
  const accuracyRaw = readUint16(location[5]);

  if (latRaw === null || lonRaw === null || altRaw === null || speedRaw === null || courseRaw === null || accuracyRaw === null) {
    return null;
  }

  const timestampValue = getMapValue(decodedPayload, SID_TIME) ?? location[6];

  return normalizeTelemetryPosition({
    callsign,
    lat: latRaw / 1_000_000,
    lon: lonRaw / 1_000_000,
    alt: altRaw / 100,
    speed: speedRaw / 100,
    course: courseRaw / 100,
    accuracy: accuracyRaw / 100,
    updatedAt: toTimestampMs(timestampValue, fallbackUpdatedAtMs),
  });
}

function parseTelemetryStream(value: unknown): Array<[Uint8Array, number, Uint8Array]> {
  let entries = value;
  const rawBytes = asUint8Array(value);
  if (rawBytes) {
    try {
      entries = unpack(rawBytes);
    } catch {
      return [];
    }
  }

  if (!Array.isArray(entries)) {
    return [];
  }

  const out: Array<[Uint8Array, number, Uint8Array]> = [];
  for (const entry of entries) {
    if (!Array.isArray(entry) || entry.length < 3) {
      continue;
    }
    const peerHash = asUint8Array(entry[0]);
    const timestamp = asFiniteNumber(entry[1]);
    const payload = asUint8Array(entry[2]);
    if (!peerHash || timestamp === undefined || !payload) {
      continue;
    }
    out.push([peerHash, timestamp, payload]);
  }
  return out;
}

function parseTelemetryRequestTimestamp(value: unknown): number | null {
  if (!Array.isArray(value)) {
    return null;
  }

  for (const command of value) {
    if (!command || typeof command !== "object" || Array.isArray(command)) {
      continue;
    }
    const requestValue = getMapValue(command, TELEMETRY_REQUEST_COMMAND);
    if (requestValue === undefined) {
      continue;
    }
    if (Array.isArray(requestValue)) {
      return requestValue.length > 0 ? toTimestampMs(requestValue[0], Date.now()) : null;
    }
    return toTimestampMs(requestValue, Date.now());
  }

  return null;
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

async function settleAll(tasks: Promise<void>[]): Promise<void> {
  await Promise.allSettled(tasks);
}

export const useTelemetryStore = defineStore("telemetry", () => {
  const byCallsign = reactive<Record<string, TelemetryPosition>>({});
  const tombstones = reactive<Record<string, number>>({});
  const initialized = ref(false);
  const replicationInitialized = ref(false);
  const startupPermissionRequested = ref(false);
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

  function buildCompatibleSnapshotResponse(): string {
    const localHex = nodeStore.status.lxmfDestinationHex.trim().toLowerCase();
    const localHash = hexToBytes(localHex);
    const position = lastLocalFix.value;
    if (!localHash || !position) {
      return buildTelemetryStreamFieldsBase64([]);
    }
    return buildTelemetryStreamFieldsBase64([
      [localHash, toUnixSeconds(position.updatedAt), buildTelemetryPayload(position)],
    ]);
  }

  function parseCompatibleTelemetryMessage(
    event: PacketReceivedEvent,
  ): TelemetryReplicationMessage | null {
    const fields = parseLxmfFields(event.fieldsBase64);
    if (!fields) {
      return null;
    }

    const streamField = getMapValue(fields, LXMF_FIELD_TELEMETRY_STREAM);
    if (streamField !== undefined) {
      const positions = parseTelemetryStream(streamField)
        .map(([peerHash, timestamp, payload]) => {
          const callsign = bytesToHex(peerHash);
          if (!callsign) {
            return null;
          }
          return parseTelemetryPayload(payload, callsign, toTimestampMs(timestamp, Date.now()));
        })
        .filter((position): position is TelemetryPosition => position !== null);

      return {
        kind: "telemetry_snapshot_response",
        requestedAt: Date.now(),
        positions,
      };
    }

    const telemetryField = getMapValue(fields, LXMF_FIELD_TELEMETRY);
    if (telemetryField !== undefined && event.sourceHex) {
      const position = parseTelemetryPayload(telemetryField, event.sourceHex, Date.now());
      if (position) {
        return {
          kind: "telemetry_upsert",
          position,
        };
      }
    }

    const commandsField = getMapValue(fields, LXMF_FIELD_COMMANDS);
    if (commandsField !== undefined) {
      const requestedAt = parseTelemetryRequestTimestamp(commandsField);
      if (requestedAt !== null) {
        return {
          kind: "telemetry_snapshot_request",
          requestedAt,
        };
      }
    }

    return null;
  }

  async function sendTelemetryMessage(
    message: Extract<TelemetryReplicationMessage, { kind: "telemetry_upsert" | "telemetry_delete" }>,
  ): Promise<void> {
    const destinations = [...new Set(nodeStore.telemetryDestinations)];
    if (destinations.length === 0) {
      return;
    }

    if (message.kind === "telemetry_upsert") {
      const fieldsBase64 = buildTelemetryFieldsBase64(message.position);
      await settleAll(
        destinations.map((destination) =>
          nodeStore.sendBytes(destination, EMPTY_BYTES, { fieldsBase64 }),
        ),
      );
      return;
    }

    const dedicatedFields = buildTelemetryDedicatedFields(message);
    await settleAll(
      destinations.map((destination) =>
        nodeStore.sendJson(destination, message as ReplicationMessage, dedicatedFields),
      ),
    );
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
      await sendTelemetryMessage(message);
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

  async function requestStartupPermission(): Promise<void> {
    if (startupPermissionRequested.value) {
      return;
    }
    startupPermissionRequested.value = true;

    permissionState.value = await telemetryService.getPermissionState();
    if (permissionState.value !== "prompt") {
      return;
    }

    permissionState.value = await telemetryService.requestPermission();
    if (permissionState.value === "unavailable") {
      telemetryError.value = "Telemetry unavailable on this device.";
      return;
    }

    if (permissionState.value === "denied" && nodeStore.settings.telemetry.enabled) {
      telemetryError.value = "Telemetry disabled: location permission denied.";
    }
  }

  function syncPublishLoopFromSettings(): void {
    if (!nodeStore.settings.telemetry.enabled) {
      stopPublishLoop();
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
    await sendTelemetryMessage(message);
  }

  async function deleteLocal(callsign: string): Promise<void> {
    const deletedAt = Date.now();
    applyDelete(callsign, deletedAt);
    const message: TelemetryReplicationMessage = {
      kind: "telemetry_delete",
      callsign,
      deletedAt,
    };
    await sendTelemetryMessage(message);
  }

  function initReplication(): void {
    if (replicationInitialized.value) {
      return;
    }
    replicationInitialized.value = true;

    const decoder = new TextDecoder();
    nodeStore.onPacket((event: PacketReceivedEvent) => {
      const message =
        parseCompatibleTelemetryMessage(event) ??
        parseDedicatedTelemetryMessage(event) ??
        parseTelemetryReplicationMessage(decoder.decode(event.bytes));
      if (!message) {
        return;
      }

      if (message.kind === "telemetry_snapshot_request") {
        if (!event.sourceHex) {
          return;
        }
        nodeStore
          .sendBytes(event.sourceHex, EMPTY_BYTES, {
            fieldsBase64: buildCompatibleSnapshotResponse(),
          })
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
      () => [...nodeStore.telemetryDestinations],
      (current, previous) => {
        const previousSet = new Set(previous);
        for (const destination of current) {
          if (previousSet.has(destination)) {
            continue;
          }
          nodeStore
            .sendBytes(destination, EMPTY_BYTES, {
              fieldsBase64: buildTelemetrySnapshotRequestFieldsBase64(Date.now()),
            })
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
    requestStartupPermission,
    startPublishLoop,
    stopPublishLoop,
    upsertLocalPosition,
    deleteLocal,
  };
});
