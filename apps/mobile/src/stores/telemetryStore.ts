import {
  createReticulumNodeClient,
  type ProjectionInvalidationEvent,
  type ReticulumNodeClient,
} from "@reticulum/node-client";
import { defineStore } from "pinia";
import { computed, reactive, ref, watch } from "vue";

import type { TelemetryPosition } from "../types/domain";
import {
  telemetryService,
  TelemetryPermissionDeniedError,
  type TelemetryPermissionState,
} from "../services/telemetry";
import { LEGACY_TELEMETRY_STORAGE_KEY } from "../utils/legacyState";
import { asNumber, asTrimmedString } from "../utils/replicationParser";
import { supportsNativeNodeRuntime } from "../utils/runtimeProfile";
import { useNodeStore } from "./nodeStore";

type TelemetryLoopStatus = "idle" | "running" | "permission_denied" | "gps_unavailable" | "error";
type ProjectionClientCache = typeof globalThis & {
  __reticulumTelemetryProjectionClient?: ReticulumNodeClient;
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

function loadLegacyPositions(): TelemetryPosition[] {
  try {
    const raw = localStorage.getItem(LEGACY_TELEMETRY_STORAGE_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as TelemetryPosition[];
    return Array.isArray(parsed)
      ? parsed.map((position) => normalizeTelemetryPosition(position))
      : [];
  } catch {
    return [];
  }
}

function clearLegacyPositionsStorage(): void {
  localStorage.removeItem(LEGACY_TELEMETRY_STORAGE_KEY);
}

function saveLegacyPositions(positions: TelemetryPosition[]): void {
  localStorage.setItem(LEGACY_TELEMETRY_STORAGE_KEY, JSON.stringify(positions));
}

function keyFor(callsign: string): string {
  return callsign.trim().toLowerCase();
}

function getProjectionClient(mode: "auto" | "capacitor"): ReticulumNodeClient {
  const cache = globalThis as ProjectionClientCache;
  if (!cache.__reticulumTelemetryProjectionClient) {
    cache.__reticulumTelemetryProjectionClient = createReticulumNodeClient({ mode });
  }
  return cache.__reticulumTelemetryProjectionClient;
}

export const useTelemetryStore = defineStore("telemetry", () => {
  const byCallsign = reactive<Record<string, TelemetryPosition>>({});
  const initialized = ref(false);
  const startupPermissionRequested = ref(false);
  const nowTimestamp = ref(Date.now());
  const loopTimer = ref<number | null>(null);
  const loopInFlight = ref(false);
  const permissionState = ref<TelemetryPermissionState>("prompt");
  const loopStatus = ref<TelemetryLoopStatus>("idle");
  const telemetryError = ref("");
  const nodeStore = useNodeStore();

  let refreshProjectionPromise: Promise<void> | null = null;
  let clockTimerId: number | null = null;
  const cleanups: Array<() => void> = [];

  const staleThresholdMs = computed(
    () => Math.max(1, nodeStore.settings.telemetry.staleAfterMinutes) * 60 * 1000,
  );
  const expireThresholdMs = computed(
    () =>
      Math.max(
        nodeStore.settings.telemetry.staleAfterMinutes,
        nodeStore.settings.telemetry.expireAfterMinutes,
      ) * 60 * 1000,
  );

  function replaceTelemetryProjection(records: TelemetryPosition[]): void {
    const nextByCallsign: Record<string, TelemetryPosition> = {};
    for (const record of records) {
      const normalized = normalizeTelemetryPosition(record);
      const key = keyFor(normalized.callsign);
      if (!key) {
        continue;
      }
      nextByCallsign[key] = normalized;
    }

    for (const key of Object.keys(byCallsign)) {
      if (!(key in nextByCallsign)) {
        delete byCallsign[key];
      }
    }
    for (const [key, value] of Object.entries(nextByCallsign)) {
      byCallsign[key] = value;
    }
  }

  async function refreshTelemetryProjection(): Promise<void> {
    if (refreshProjectionPromise) {
      return refreshProjectionPromise;
    }
    refreshProjectionPromise = (async () => {
      if (!supportsNativeNodeRuntime || !nodeStore.status.running) {
        replaceTelemetryProjection(loadLegacyPositions());
        return;
      }
      const positions = await getProjectionClient(nodeStore.settings.clientMode).getTelemetryPositions();
      replaceTelemetryProjection(positions);
    })()
      .catch((error: unknown) => {
        telemetryError.value = error instanceof Error ? error.message : String(error);
      })
      .finally(() => {
        refreshProjectionPromise = null;
      });
    return refreshProjectionPromise;
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

  async function publishOnce(): Promise<void> {
    if (loopInFlight.value) {
      return;
    }
    if (supportsNativeNodeRuntime && !nodeStore.status.running) {
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

      if (!supportsNativeNodeRuntime) {
        const nextPositions = Object.values(byCallsign)
          .filter((entry) => keyFor(entry.callsign) !== keyFor(position.callsign));
        nextPositions.push(position);
        saveLegacyPositions(nextPositions);
      } else {
        await getProjectionClient(nodeStore.settings.clientMode).recordLocalTelemetryFix(position);
      }
      await refreshTelemetryProjection();
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

    if (supportsNativeNodeRuntime && !nodeStore.status.running) {
      loopStatus.value = "idle";
      telemetryError.value = "";
      return;
    }

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

    if (supportsNativeNodeRuntime && !nodeStore.status.running) {
      stopPublishLoop();
      return;
    }

    void startPublishLoop();
  }

  async function initializeAsync(): Promise<void> {
    await refreshTelemetryProjection();

    if (clockTimerId === null) {
      clockTimerId = window.setInterval(() => {
        nowTimestamp.value = Date.now();
      }, 30_000);
    }

    if (supportsNativeNodeRuntime && cleanups.length === 0) {
      const client = getProjectionClient(nodeStore.settings.clientMode);
      cleanups.push(client.on("projectionInvalidated", (event: ProjectionInvalidationEvent) => {
        if (event.scope === "Telemetry") {
          void refreshTelemetryProjection();
        }
      }));
      cleanups.push(client.on("statusChanged", () => {
        void refreshTelemetryProjection();
      }));
    }

    watch(
      () => [
        nodeStore.settings.telemetry.enabled,
        nodeStore.settings.telemetry.publishIntervalSeconds,
        nodeStore.settings.displayName,
      ],
      () => {
        syncPublishLoopFromSettings();
      },
      { immediate: true },
    );
  }

  function init(): void {
    if (initialized.value) {
      return;
    }
    initialized.value = true;
    void initializeAsync().catch((error: unknown) => {
      telemetryError.value = error instanceof Error ? error.message : String(error);
    });
  }

  function initReplication(): void {
    init();
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
    if (!supportsNativeNodeRuntime) {
      const nextPositions = Object.values(byCallsign)
        .filter((entry) => keyFor(entry.callsign) !== keyFor(position.callsign));
      nextPositions.push(position);
      saveLegacyPositions(nextPositions);
    } else if (nodeStore.status.running) {
      await getProjectionClient(nodeStore.settings.clientMode).recordLocalTelemetryFix(position);
    }
    await refreshTelemetryProjection();
  }

  async function deleteLocal(callsign: string): Promise<void> {
    if (!supportsNativeNodeRuntime) {
      const nextPositions = Object.values(byCallsign).filter(
        (entry) => keyFor(entry.callsign) !== keyFor(callsign),
      );
      saveLegacyPositions(nextPositions);
      if (nextPositions.length === 0) {
        clearLegacyPositionsStorage();
      }
    } else if (nodeStore.status.running) {
      await getProjectionClient(nodeStore.settings.clientMode).deleteLocalTelemetry(callsign);
    }
    await refreshTelemetryProjection();
  }

  const positions = computed(() =>
    Object.values(byCallsign).sort((a, b) => b.updatedAt - a.updatedAt),
  );

  const activePositions = computed(() =>
    positions.value
      .filter((position) => nowTimestamp.value - position.updatedAt <= expireThresholdMs.value)
      .sort((a, b) => b.updatedAt - a.updatedAt),
  );

  const stalePositions = computed(() =>
    activePositions.value.filter(
      (position) => nowTimestamp.value - position.updatedAt > staleThresholdMs.value,
    ),
  );

  const expiredPositions = computed(() =>
    positions.value.filter(
      (position) => nowTimestamp.value - position.updatedAt > expireThresholdMs.value,
    ),
  );

  return {
    byCallsign,
    positions,
    activePositions,
    stalePositions,
    expiredPositions,
    staleThresholdMs,
    expireThresholdMs,
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
