import type {
  AppSettingsRecord,
  HubMode,
  RnodeConnectionMode,
  RnodeProfileId,
  RnodeRegion,
  RnodeSettingsRecord,
} from "./contracts";

export function toOptionalNumber(value: unknown): number | undefined {
  if (
    value === undefined
    || value === null
    || (typeof value === "string" && value.trim().length === 0)
    || (typeof value !== "string" && typeof value !== "number")
  ) {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

const MAX_SETTINGS_WRAPPER_DEPTH = 16;
const MAX_U32 = 0xffff_ffff;

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function finiteInteger(
  value: unknown,
  fallback: number,
  minimum: number,
  maximum = MAX_U32,
): number {
  if (
    (typeof value !== "number" && typeof value !== "string")
    || (typeof value === "string" && value.trim().length === 0)
  ) {
    return fallback;
  }
  const parsed = Math.trunc(Number(value));
  return Number.isFinite(parsed)
    ? Math.min(maximum, Math.max(minimum, parsed))
    : fallback;
}

function strictBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function normalizeTcpClients(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return [...new Set(
    value
      .filter((entry): entry is string => typeof entry === "string")
      .map((entry) => entry.trim())
      .filter((entry) => entry.length > 0),
  )];
}

function normalizeHubMode(value: unknown): HubMode {
  switch (stringValue(value).trim()) {
    case "Connected":
      return "Connected";
    case "SemiAutonomous":
    case "RchLxmf":
    case "RchHttp":
      return "SemiAutonomous";
    case "Autonomous":
    case "Disabled":
    default:
      return "Autonomous";
  }
}

function normalizeRnodeRegion(value: unknown): RnodeRegion {
  return stringValue(value).trim().toUpperCase() === "EU868" ? "EU868" : "US915";
}

function normalizeRnodeProfile(value: unknown): RnodeProfileId {
  switch (stringValue(value).trim()) {
    case "REM-MF-URBAN-v1":
      return "REM-MF-URBAN-v1";
    case "REM-LM-EXTREME-v1":
      return "REM-LM-EXTREME-v1";
    case "REM-LF-RURAL-v1":
    default:
      return "REM-LF-RURAL-v1";
  }
}

export function parseRnodeConnectionMode(value: unknown): RnodeConnectionMode {
  const raw = stringValue(value);
  const normalized = raw.trim().toLowerCase().replace(/[\s-]+/g, "_");
  if (!normalized) {
    return "ble";
  }
  switch (normalized) {
    case "bluetooth_classic":
    case "bluetoothclassic":
    case "classic":
    case "spp":
    case "rfcomm":
    case "bluetooth":
      return "bluetooth_classic";
    case "usb":
    case "serial":
      return "usb";
    case "tcp":
    case "wifi":
    case "wi_fi":
      return "tcp";
    case "ble":
    case "bluetooth_le":
    case "le":
    case "gatt":
      return "ble";
    default:
      throw new TypeError(`Unsupported RNode connection mode: ${raw}`);
  }
}

export function normalizeRnodeSettings(value: unknown): RnodeSettingsRecord {
  const raw = asRecord(value) ?? {};
  return {
    enabled: strictBoolean(raw.enabled, false),
    connectionMode: parseRnodeConnectionMode(raw.connectionMode ?? raw.connection_mode ?? raw.mode),
    peripheralId: stringValue(raw.peripheralId ?? raw.peripheral_id).trim(),
    displayName: stringValue(raw.displayName ?? raw.display_name).trim(),
    region: normalizeRnodeRegion(raw.region),
    profile: normalizeRnodeProfile(raw.profile),
  };
}

export function toAppSettingsRecord(raw: Record<string, unknown>): AppSettingsRecord | null {
  let current = asRecord(raw);
  const seen = new Set<object>();
  for (let depth = 0; depth <= MAX_SETTINGS_WRAPPER_DEPTH; depth += 1) {
    if (!current || Object.keys(current).length === 0) {
      return null;
    }
    if (!("settings" in current)) {
      break;
    }
    if (seen.has(current) || depth === MAX_SETTINGS_WRAPPER_DEPTH) {
      return null;
    }
    seen.add(current);
    current = asRecord(current.settings);
  }
  if (!current) {
    return null;
  }
  const telemetry = asRecord(current.telemetry) ?? {};
  const hub = asRecord(current.hub) ?? {};
  const checklists = asRecord(current.checklists) ?? {};
  const staleAfterMinutes = finiteInteger(telemetry.staleAfterMinutes, 30, 1);
  const expireAfterMinutes = Math.max(
    staleAfterMinutes,
    finiteInteger(telemetry.expireAfterMinutes, 180, 1),
  );
  const accuracyThresholdMeters = toOptionalNumber(telemetry.accuracyThresholdMeters);
  return {
    displayName: stringValue(current.displayName),
    autoConnectSaved: strictBoolean(current.autoConnectSaved, false),
    announceCapabilities: stringValue(current.announceCapabilities),
    tcpClients: normalizeTcpClients(current.tcpClients),
    broadcast: strictBoolean(current.broadcast, false),
    transportNodeEnabled: strictBoolean(
      current.transportNodeEnabled ?? current.transport_node_enabled,
      true,
    ),
    announceIntervalSeconds: finiteInteger(current.announceIntervalSeconds, 1800, 60),
    telemetry: {
      enabled: strictBoolean(telemetry.enabled, false),
      publishIntervalSeconds: finiteInteger(telemetry.publishIntervalSeconds, 360, 1),
      accuracyThresholdMeters: accuracyThresholdMeters === undefined
        ? undefined
        : Math.max(0, accuracyThresholdMeters),
      staleAfterMinutes,
      expireAfterMinutes,
    },
    hub: {
      mode: normalizeHubMode(hub.mode),
      identityHash: stringValue(hub.identityHash),
      apiBaseUrl: stringValue(hub.apiBaseUrl),
      apiKey: stringValue(hub.apiKey),
      refreshIntervalSeconds: finiteInteger(hub.refreshIntervalSeconds, 3600, 60),
    },
    checklists: {
      defaultTaskDueStepMinutes: finiteInteger(checklists.defaultTaskDueStepMinutes, 30, 1),
    },
    rnode: normalizeRnodeSettings(current.rnode),
  };
}
