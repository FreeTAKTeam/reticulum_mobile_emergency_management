import type { RnodeProfileId, RnodeRegion, RnodeSettings } from "../types/domain";

export interface RnodeProfileSpec {
  id: RnodeProfileId;
  label: string;
  bandwidth: number;
  spreadingFactor: number;
  codingRate: number;
}

export interface RnodeRegionSpec {
  id: RnodeRegion;
  label: string;
  defaultFrequencyHz: number;
}

export const RNODE_REGION_SPECS: RnodeRegionSpec[] = [
  { id: "US915", label: "US 915 MHz", defaultFrequencyHz: 915_000_000 },
  { id: "EU868", label: "EU 868 MHz", defaultFrequencyHz: 868_000_000 },
  { id: "AU915", label: "AU 915 MHz", defaultFrequencyHz: 915_000_000 },
  { id: "AS923", label: "AS 923 MHz", defaultFrequencyHz: 923_000_000 },
  { id: "IN865", label: "IN 865 MHz", defaultFrequencyHz: 865_000_000 },
  { id: "KR920", label: "KR 920 MHz", defaultFrequencyHz: 920_000_000 },
  { id: "RU864", label: "RU 864 MHz", defaultFrequencyHz: 864_000_000 },
];

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
  frequencyHz: 915_000_000,
};

export function isRnodeRegion(value: unknown): value is RnodeRegion {
  const normalized = String(value ?? "").trim().toUpperCase();
  return RNODE_REGION_SPECS.some((region) => region.id === normalized);
}

export function normalizeRnodeRegion(value: unknown): RnodeRegion {
  const normalized = String(value ?? "").trim().toUpperCase();
  const match = RNODE_REGION_SPECS.find((region) => region.id === normalized);
  return match?.id ?? "US915";
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

export function rnodeRegionDefaultFrequencyHz(region: RnodeRegion): number {
  return RNODE_REGION_SPECS.find((candidate) => candidate.id === region)?.defaultFrequencyHz ?? 915_000_000;
}

export function normalizeRnodeFrequencyHz(value: unknown, region: RnodeRegion): number {
  const frequencyHz = Number(value);
  if (Number.isFinite(frequencyHz) && frequencyHz > 0) {
    return Math.round(frequencyHz);
  }
  return rnodeRegionDefaultFrequencyHz(region);
}

export function resolveRnodeFrequencyForRegionChange(
  previousRegion: RnodeRegion,
  nextRegion: RnodeRegion,
  currentFrequencyHz: number,
): number {
  return currentFrequencyHz === rnodeRegionDefaultFrequencyHz(previousRegion)
    ? rnodeRegionDefaultFrequencyHz(nextRegion)
    : currentFrequencyHz;
}

export function normalizeRnodeSettings(
  value: Partial<RnodeSettings> | Record<string, unknown> | null | undefined,
): RnodeSettings {
  const raw = (value ?? {}) as Partial<RnodeSettings> & Record<string, unknown>;
  const region = normalizeRnodeRegion(raw.region);
  return {
    enabled: Boolean(raw.enabled),
    peripheralId: String(raw.peripheralId ?? "").trim(),
    displayName: String(raw.displayName ?? "").trim(),
    region,
    profile: normalizeRnodeProfile(raw.profile),
    frequencyHz: normalizeRnodeFrequencyHz(raw.frequencyHz ?? raw.frequency_hz, region),
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
  if (lat >= -45 && lat <= -8 && lon >= 110 && lon <= 155) {
    return "AU915";
  }
  if (lat >= 6 && lat <= 36 && lon >= 68 && lon <= 98) {
    return "IN865";
  }
  if (lat >= 33 && lat <= 39 && lon >= 124 && lon <= 132) {
    return "KR920";
  }
  if (lat >= 41 && lat <= 82 && lon >= 19 && lon <= 180) {
    return "RU864";
  }
  if (lat >= -12 && lat <= 32 && lon >= 95 && lon <= 145) {
    return "AS923";
  }
  return "US915";
}

export function inferRnodeRegionFromTimezone(timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone): RnodeRegion {
  const normalized = timeZone.toLowerCase();
  if (normalized.startsWith("europe/")) {
    return "EU868";
  }
  if (normalized.startsWith("australia/")) {
    return "AU915";
  }
  if (normalized === "asia/kolkata" || normalized === "asia/calcutta") {
    return "IN865";
  }
  if (normalized === "asia/seoul") {
    return "KR920";
  }
  if (normalized.startsWith("asia/")) {
    return "AS923";
  }
  return "US915";
}
