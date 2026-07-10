import type {
  AppSettingsRecord,
  HubMode,
  RnodeConnectionMode,
  RnodeProfileId,
  RnodeRegion,
  RnodeSettingsRecord,
} from "./contracts";

export function toOptionalNumber(value: unknown): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function normalizeHubMode(value: unknown): HubMode {
  switch (String(value ?? "").trim()) {
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
  return String(value ?? "").trim().toUpperCase() === "EU868" ? "EU868" : "US915";
}

function normalizeRnodeProfile(value: unknown): RnodeProfileId {
  switch (String(value ?? "").trim()) {
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
  const normalized = String(value ?? "").trim().toLowerCase().replace(/[\s-]+/g, "_");
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
      throw new TypeError(`Unsupported RNode connection mode: ${String(value)}`);
  }
}

export function normalizeRnodeSettings(value: unknown): RnodeSettingsRecord {
  const raw = value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
  return {
    enabled: Boolean(raw.enabled),
    connectionMode: parseRnodeConnectionMode(raw.connectionMode ?? raw.connection_mode ?? raw.mode),
    peripheralId: String(raw.peripheralId ?? raw.peripheral_id ?? "").trim(),
    displayName: String(raw.displayName ?? raw.display_name ?? "").trim(),
    region: normalizeRnodeRegion(raw.region),
    profile: normalizeRnodeProfile(raw.profile),
  };
}

export function toAppSettingsRecord(raw: Record<string, unknown>): AppSettingsRecord | null {
  if (!raw || Object.keys(raw).length === 0) {
    return null;
  }
  if ("settings" in raw) {
    const nested = raw.settings;
    if (!nested || typeof nested !== "object" || Array.isArray(nested)) {
      return null;
    }
    return toAppSettingsRecord(nested as Record<string, unknown>);
  }
  const telemetry = (raw.telemetry ?? {}) as Record<string, unknown>;
  const hub = (raw.hub ?? {}) as Record<string, unknown>;
  const checklists = (raw.checklists ?? {}) as Record<string, unknown>;
  const defaultTaskDueStepMinutes = Math.trunc(Number(checklists.defaultTaskDueStepMinutes ?? 30));
  return {
    displayName: String(raw.displayName ?? ""),
    autoConnectSaved: Boolean(raw.autoConnectSaved),
    announceCapabilities: String(raw.announceCapabilities ?? ""),
    tcpClients: Array.isArray(raw.tcpClients) ? raw.tcpClients.map((entry) => String(entry)) : [],
    broadcast: Boolean(raw.broadcast),
    transportNodeEnabled: Boolean(raw.transportNodeEnabled ?? raw.transport_node_enabled ?? true),
    announceIntervalSeconds: Number(raw.announceIntervalSeconds ?? 1800),
    telemetry: {
      enabled: Boolean(telemetry.enabled),
      publishIntervalSeconds: Number(telemetry.publishIntervalSeconds ?? 360),
      accuracyThresholdMeters: toOptionalNumber(telemetry.accuracyThresholdMeters),
      staleAfterMinutes: Number(telemetry.staleAfterMinutes ?? 30),
      expireAfterMinutes: Number(telemetry.expireAfterMinutes ?? 180),
    },
    hub: {
      mode: normalizeHubMode(hub.mode),
      identityHash: String(hub.identityHash ?? ""),
      apiBaseUrl: String(hub.apiBaseUrl ?? ""),
      apiKey: String(hub.apiKey ?? ""),
      refreshIntervalSeconds: Number(hub.refreshIntervalSeconds ?? 3600),
    },
    checklists: {
      defaultTaskDueStepMinutes: Number.isFinite(defaultTaskDueStepMinutes)
        ? Math.max(1, defaultTaskDueStepMinutes)
        : 30,
    },
    rnode: normalizeRnodeSettings(raw.rnode),
  };
}
