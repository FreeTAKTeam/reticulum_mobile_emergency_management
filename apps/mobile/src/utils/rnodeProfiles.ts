import type { RnodeConnectionMode, RnodeProfileId, RnodeRegion, RnodeSettings } from "../types/domain";

export interface RnodeProfileSpec {
  id: RnodeProfileId;
  label: string;
  bandwidth: number;
  spreadingFactor: number;
  codingRate: number;
}

export const RNODE_PROFILE_SPECS: RnodeProfileSpec[] = [
  {
    id: "REM-MF-URBAN-v1",
    label: "Urban mesh",
    bandwidth: 250000,
    spreadingFactor: 9,
    codingRate: 5,
  },
  {
    id: "REM-LF-RURAL-v1",
    label: "Rural fallback",
    bandwidth: 250000,
    spreadingFactor: 11,
    codingRate: 5,
  },
  {
    id: "REM-LM-EXTREME-v1",
    label: "Extreme range",
    bandwidth: 125000,
    spreadingFactor: 11,
    codingRate: 8,
  },
];

export const DEFAULT_RNODE_SETTINGS: RnodeSettings = {
  enabled: false,
  connectionMode: "ble",
  peripheralId: "",
  displayName: "",
  region: "US915",
  profile: "REM-LF-RURAL-v1",
};

export function normalizeRnodeConnectionMode(value: unknown): RnodeConnectionMode {
  switch (String(value ?? "").trim().toLowerCase().replace(/[\s-]+/g, "_")) {
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
    default:
      return "ble";
  }
}

export function normalizeRnodeRegion(value: unknown): RnodeRegion {
  return String(value ?? "").trim().toUpperCase() === "EU868" ? "EU868" : "US915";
}

export function normalizeRnodeProfile(value: unknown): RnodeProfileId {
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

export function normalizeRnodeSettings(
  value: Partial<RnodeSettings> | Record<string, unknown> | null | undefined,
  defaults: RnodeSettings = DEFAULT_RNODE_SETTINGS,
): RnodeSettings {
  const raw = (value ?? {}) as Partial<RnodeSettings> & Record<string, unknown>;
  return {
    enabled: Boolean(raw.enabled ?? defaults.enabled),
    connectionMode: normalizeRnodeConnectionMode(
      raw.connectionMode ?? raw.connection_mode ?? raw.mode ?? defaults.connectionMode,
    ),
    peripheralId: String(raw.peripheralId ?? raw.peripheral_id ?? defaults.peripheralId ?? "").trim(),
    displayName: String(raw.displayName ?? raw.display_name ?? defaults.displayName ?? "").trim(),
    region: normalizeRnodeRegion(raw.region ?? defaults.region),
    profile: normalizeRnodeProfile(raw.profile ?? defaults.profile),
  };
}

export function rnodeProfileSummary(profile: unknown): string {
  const normalized = normalizeRnodeProfile(profile);
  const spec = RNODE_PROFILE_SPECS.find((candidate) => candidate.id === normalized) ?? RNODE_PROFILE_SPECS[1];
  return `bandwidth = ${spec.bandwidth}, spreadingfactor = ${spec.spreadingFactor}, codingrate = ${spec.codingRate}`;
}

export function inferRnodeRegionFromCoordinates(lat: number, lon: number): RnodeRegion {
  if (lat >= 34 && lat <= 72 && lon >= -25 && lon <= 45) {
    return "EU868";
  }
  return "US915";
}

export function inferRnodeRegionFromTimezone(timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone): RnodeRegion {
  return timeZone.toLowerCase().startsWith("europe/") ? "EU868" : "US915";
}
