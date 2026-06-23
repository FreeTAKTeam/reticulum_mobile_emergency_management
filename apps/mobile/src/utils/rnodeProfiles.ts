import type { RnodeProfileId, RnodeRegion, RnodeSettings } from "../types/domain";

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
  peripheralId: "",
  displayName: "",
  region: "US915",
  profile: "REM-LF-RURAL-v1",
};

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
): RnodeSettings {
  const raw = value ?? {};
  return {
    enabled: Boolean(raw.enabled),
    peripheralId: String(raw.peripheralId ?? "").trim(),
    displayName: String(raw.displayName ?? "").trim(),
    region: normalizeRnodeRegion(raw.region),
    profile: normalizeRnodeProfile(raw.profile),
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
