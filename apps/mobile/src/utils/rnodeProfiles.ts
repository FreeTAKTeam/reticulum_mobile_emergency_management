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

export const RNODE_FREQUENCY_MIN_HZ = 137_000_000;
export const RNODE_FREQUENCY_MAX_HZ = 3_000_000_000;

export const DEFAULT_RNODE_SETTINGS: RnodeSettings = {
  enabled: false,
  connectionMode: "ble",
  peripheralId: "",
  displayName: "",
  region: "US915",
  profile: "REM-LF-RURAL-v1",
  frequencyHz: 915_000_000,
};

export function normalizeRnodeConnectionMode(value: unknown): RnodeConnectionMode {
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

export function isRnodeRegion(value: unknown): value is RnodeRegion {
  const normalized = String(value ?? "").trim().toUpperCase();
  return RNODE_REGION_SPECS.some((region) => region.id === normalized);
}

export function normalizeRnodeRegion(value: unknown): RnodeRegion {
  const normalized = String(value ?? "").trim().toUpperCase();
  if (!normalized) {
    return "US915";
  }
  const match = RNODE_REGION_SPECS.find((region) => region.id === normalized);
  if (!match) {
    throw new TypeError(`Unsupported RNode LoRa region: ${String(value)}`);
  }
  return match.id;
}

export function normalizeRnodeProfile(value: unknown): RnodeProfileId {
  switch (String(value ?? "").trim()) {
    case "REM-MF-URBAN-v1":
      return "REM-MF-URBAN-v1";
    case "REM-LM-EXTREME-v1":
      return "REM-LM-EXTREME-v1";
    case "REM-LF-RURAL-v1":
    case "":
      return "REM-LF-RURAL-v1";
    default:
      throw new TypeError(`Unsupported RNode LoRa profile: ${String(value)}`);
  }
}

export function rnodeRegionDefaultFrequencyHz(region: RnodeRegion): number {
  return RNODE_REGION_SPECS.find((candidate) => candidate.id === region)?.defaultFrequencyHz ?? 915_000_000;
}

export function isRnodeFrequencyHz(value: unknown): boolean {
  const frequencyHz = Number(value);
  return Number.isFinite(frequencyHz)
    && frequencyHz >= RNODE_FREQUENCY_MIN_HZ
    && frequencyHz <= RNODE_FREQUENCY_MAX_HZ;
}

export function normalizeRnodeFrequencyHz(value: unknown, region: RnodeRegion): number {
  const frequencyHz = Number(value);
  if (isRnodeFrequencyHz(value)) {
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
  defaults: RnodeSettings = DEFAULT_RNODE_SETTINGS,
): RnodeSettings {
  const raw = (value ?? {}) as Partial<RnodeSettings> & Record<string, unknown>;
  const region = normalizeRnodeRegion(raw.region ?? defaults.region);
  const frequencyValue = raw.frequencyHz ?? raw.frequency_hz ?? rnodeRegionDefaultFrequencyHz(region);
  return {
    enabled: Boolean(raw.enabled ?? defaults.enabled),
    connectionMode: normalizeRnodeConnectionMode(
      raw.connectionMode ?? raw.connection_mode ?? raw.mode ?? defaults.connectionMode,
    ),
    peripheralId: String(raw.peripheralId ?? raw.peripheral_id ?? defaults.peripheralId ?? "").trim(),
    displayName: String(raw.displayName ?? raw.display_name ?? defaults.displayName ?? "").trim(),
    region,
    profile: normalizeRnodeProfile(raw.profile ?? defaults.profile),
    frequencyHz: normalizeRnodeFrequencyHz(frequencyValue, region),
  };
}

export function rnodeProfileSummary(profile: unknown): string {
  const normalized = normalizeRnodeProfile(profile);
  const spec = RNODE_PROFILE_SPECS.find((candidate) => candidate.id === normalized) ?? RNODE_PROFILE_SPECS[1];
  return `bandwidth = ${spec.bandwidth}, spreadingfactor = ${spec.spreadingFactor}, codingrate = ${spec.codingRate}`;
}

const RNODE_RU864_TIME_ZONES = new Set([
  "asia/anadyr",
  "asia/barnaul",
  "asia/chita",
  "asia/irkutsk",
  "asia/kamchatka",
  "asia/khandyga",
  "asia/krasnoyarsk",
  "asia/magadan",
  "asia/novokuznetsk",
  "asia/novosibirsk",
  "asia/omsk",
  "asia/sakhalin",
  "asia/srednekolymsk",
  "asia/tomsk",
  "asia/ust-nera",
  "asia/vladivostok",
  "asia/yakutsk",
  "asia/yekaterinburg",
  "europe/astrakhan",
  "europe/kaliningrad",
  "europe/kirov",
  "europe/moscow",
  "europe/samara",
  "europe/saratov",
  "europe/ulyanovsk",
  "europe/volgograd",
]);
const RNODE_AS923_TIME_ZONES = new Set([
  "asia/bangkok",
  "asia/brunei",
  "asia/ho_chi_minh",
  "asia/hong_kong",
  "asia/jakarta",
  "asia/kuala_lumpur",
  "asia/kuching",
  "asia/manila",
  "asia/phnom_penh",
  "asia/singapore",
  "asia/taipei",
  "asia/tokyo",
  "asia/vientiane",
  "asia/yangon",
]);
const RNODE_US915_TIME_ZONES = new Set([
  "america/adak",
  "america/anchorage",
  "america/boise",
  "america/chicago",
  "america/denver",
  "america/detroit",
  "america/edmonton",
  "america/halifax",
  "america/iqaluit",
  "america/juneau",
  "america/los_angeles",
  "america/moncton",
  "america/new_york",
  "america/nome",
  "america/phoenix",
  "america/regina",
  "america/st_johns",
  "america/toronto",
  "america/vancouver",
  "america/whitehorse",
  "america/winnipeg",
  "america/yellowknife",
]);

export function inferRnodeRegionFromCoordinates(lat: number, lon: number): RnodeRegion | undefined {
  if (!Number.isFinite(lat) || !Number.isFinite(lon) || lat < -90 || lat > 90 || lon < -180 || lon > 180) {
    return undefined;
  }
  if (lat >= -45 && lat <= -9 && lon >= 110 && lon <= 155) {
    return "AU915";
  }
  if (lat >= 6 && lat <= 38 && lon >= 68 && lon <= 98) {
    return "IN865";
  }
  if (lat >= 33 && lat <= 39.5 && lon >= 124 && lon <= 132) {
    return "KR920";
  }
  return undefined;
}

export function inferRnodeRegionFromTimezone(
  timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone,
): RnodeRegion | undefined {
  const normalized = timeZone.trim().toLowerCase();
  if (!normalized) {
    return undefined;
  }
  if (RNODE_RU864_TIME_ZONES.has(normalized) || normalized.startsWith("russia/")) {
    return "RU864";
  }
  if (normalized.startsWith("australia/") || normalized === "antarctica/macquarie") {
    return "AU915";
  }
  if (normalized === "asia/kolkata" || normalized === "asia/calcutta") {
    return "IN865";
  }
  if (normalized === "asia/seoul") {
    return "KR920";
  }
  if (RNODE_AS923_TIME_ZONES.has(normalized)) {
    return "AS923";
  }
  if (normalized.startsWith("europe/")) {
    return "EU868";
  }
  if (
    RNODE_US915_TIME_ZONES.has(normalized)
    || normalized.startsWith("us/")
    || normalized.startsWith("canada/")
  ) {
    return "US915";
  }
  return undefined;
}
