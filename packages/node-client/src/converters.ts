import {
  CANONICAL_TEAM_UIDS,
  YELLOW_TEAM_UID,
  type TeamAliasRecord,
  type AppSettingsRecord,
  type HubMode,
  type RnodeConnectionMode,
  type RnodeProfileId,
  type RnodeRegion,
  type RnodeSettingsRecord,
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

function normalizedDestination(value: unknown): string {
  const normalized = stringValue(value).trim().toLowerCase();
  return /^[0-9a-f]{32}$/.test(normalized) ? normalized : "";
}

function unknownArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
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

const RNODE_REGION_DEFAULT_FREQUENCY_HZ: Record<RnodeRegion, number> = {
  US915: 915_000_000,
  EU868: 868_000_000,
  AU915: 915_000_000,
  AS923: 923_000_000,
  IN865: 865_000_000,
  KR920: 920_000_000,
  RU864: 864_000_000,
};

export function rnodeRegionDefaultFrequencyHz(region: RnodeRegion): number {
  return RNODE_REGION_DEFAULT_FREQUENCY_HZ[region] ?? 915_000_000;
}

function normalizeRnodeRegion(value: unknown): RnodeRegion {
  const normalized = stringValue(value).trim().toUpperCase();
  switch (normalized) {
    case "EU868":
    case "AU915":
    case "AS923":
    case "IN865":
    case "KR920":
    case "RU864":
      return normalized;
    default:
      return "US915";
  }
}

function normalizeRnodeFrequencyHz(value: unknown, region: RnodeRegion): number {
  const frequencyHz = Number(value);
  if (Number.isFinite(frequencyHz) && frequencyHz > 0) {
    return Math.round(frequencyHz);
  }
  return rnodeRegionDefaultFrequencyHz(region);
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
  const region = normalizeRnodeRegion(raw.region);
  return {
    enabled: strictBoolean(raw.enabled, false),
    connectionMode: parseRnodeConnectionMode(raw.connectionMode ?? raw.connection_mode ?? raw.mode),
    peripheralId: stringValue(raw.peripheralId ?? raw.peripheral_id).trim(),
    displayName: stringValue(raw.displayName ?? raw.display_name).trim(),
    region,
    profile: normalizeRnodeProfile(raw.profile),
    frequencyHz: normalizeRnodeFrequencyHz(raw.frequencyHz ?? raw.frequency_hz, region),
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
  const teams = asRecord(current.teams) ?? {};
  const activeTeamUid = stringValue(teams.activeTeamUid ?? teams.active_team_uid)
    .trim()
    .toLowerCase();
  const aliases: TeamAliasRecord[] = Array.isArray(teams.aliases)
    ? teams.aliases
      .map((entry) => asRecord(entry))
      .filter((entry): entry is Record<string, unknown> => entry !== null)
      .map((entry) => ({
        teamUid: stringValue(entry.teamUid ?? entry.team_uid).trim().toLowerCase(),
        alias: stringValue(entry.alias).trim().slice(0, 48),
      }))
      .filter((entry) => CANONICAL_TEAM_UIDS.has(entry.teamUid) && entry.alias.length > 0)
      .filter((entry, index, all) => all.findIndex((candidate) => candidate.teamUid === entry.teamUid) === index)
      .slice(0, 13)
    : [];
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
    teams: {
      activeTeamUid: CANONICAL_TEAM_UIDS.has(activeTeamUid) ? activeTeamUid : YELLOW_TEAM_UID,
      aliases,
      localTeams: unknownArray(teams.localTeams ?? teams.local_teams).map((team) => {
        const record = asRecord(team) ?? {};
        return {
          teamUid: stringValue(record.teamUid ?? record.team_uid).trim().toLowerCase(),
          memberDestinations: unknownArray(
            record.memberDestinations ?? record.member_destinations,
          )
            .map(normalizedDestination).filter(Boolean),
        };
      }).filter((team) => CANONICAL_TEAM_UIDS.has(team.teamUid)),
      localTeamsInitialized: strictBoolean(
        teams.localTeamsInitialized ?? teams.local_teams_initialized,
        false,
      ),
    },
    checklists: {
      defaultTaskDueStepMinutes: finiteInteger(checklists.defaultTaskDueStepMinutes, 30, 1),
    },
    rnode: normalizeRnodeSettings(current.rnode),
  };
}
