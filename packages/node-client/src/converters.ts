import {
  CANONICAL_TEAM_UIDS,
  YELLOW_TEAM_UID,
  type TeamAliasRecord,
  type AppSettingsRecord,
  type HouseholdStatus,
  type HubMode,
  type RnodeConnectionMode,
  type RnodeProfileId,
  type RnodeRegion,
  type RnodeSettingsRecord,
  type PreferredMapLayer,
  type BlockNetworkSettings,
  type BlockOnboardingInspection,
  type BlockOnboardingImportResult,
  type PowerStateRecord,
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

function householdStatus(value: unknown): HouseholdStatus {
  switch (String(value ?? "").trim().toLowerCase()) {
    case "one_missing": return "one_missing";
    case "evacuated": return "evacuated";
    case "needs_help": return "needs_help";
    default: return "all_home";
  }
}

function preferredMapLayer(value: unknown): PreferredMapLayer {
  return String(value ?? "").trim().toLowerCase() === "satellite" ? "satellite" : "base";
}

function powerThreshold(value: unknown): 10 | 20 | 30 {
  const parsed = Number(value);
  return parsed === 10 || parsed === 30 ? parsed : 20;
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

export function toPowerStateRecord(raw: Record<string, unknown>): PowerStateRecord {
  const batteryPercent = toOptionalNumber(raw.batteryPercent ?? raw.battery_percent);
  return {
    batteryPercent: batteryPercent === undefined
      ? undefined
      : Math.min(100, Math.max(0, Math.trunc(batteryPercent))),
    charging: strictBoolean(raw.charging, false),
    saverActive: strictBoolean(raw.saverActive ?? raw.saver_active, false),
    updatedAtMs: finiteInteger(raw.updatedAtMs ?? raw.updated_at_ms, 0, 0, Number.MAX_SAFE_INTEGER),
  };
}

function toBlockNetworkSettings(raw: unknown): BlockNetworkSettings {
  const record = asRecord(raw) ?? {};
  const radio = asRecord(record.radio);
  return {
    tcpClients: normalizeTcpClients(record.tcpClients ?? record.tcp_clients),
    broadcast: strictBoolean(record.broadcast, false),
    hubMode: normalizeHubMode(record.hubMode ?? record.hub_mode),
    hubIdentityHash: stringValue(record.hubIdentityHash ?? record.hub_identity_hash) || undefined,
    hubApiBaseUrl: stringValue(record.hubApiBaseUrl ?? record.hub_api_base_url) || undefined,
    hubRefreshIntervalSeconds: finiteInteger(
      record.hubRefreshIntervalSeconds ?? record.hub_refresh_interval_seconds,
      3600,
      0,
    ),
    radio: radio
      ? {
          region: stringValue(radio.region),
          profile: stringValue(radio.profile),
          frequencyHz: finiteInteger(radio.frequencyHz ?? radio.frequency_hz, 0, 0, Number.MAX_SAFE_INTEGER),
        }
      : undefined,
  };
}

export function toBlockOnboardingInspection(
  raw: Record<string, unknown>,
): BlockOnboardingInspection {
  return {
    issuerPublicIdentityHex: stringValue(raw.issuerPublicIdentityHex ?? raw.issuer_public_identity_hex),
    issuerAppDestinationHex: stringValue(raw.issuerAppDestinationHex ?? raw.issuer_app_destination_hex),
    issuerLxmfDestinationHex: stringValue(raw.issuerLxmfDestinationHex ?? raw.issuer_lxmf_destination_hex),
    signerFingerprint: stringValue(raw.signerFingerprint ?? raw.signer_fingerprint),
    issuedAtMs: finiteInteger(raw.issuedAtMs ?? raw.issued_at_ms, 0, 0, Number.MAX_SAFE_INTEGER),
    expiresAtMs: finiteInteger(raw.expiresAtMs ?? raw.expires_at_ms, 0, 0, Number.MAX_SAFE_INTEGER),
    network: toBlockNetworkSettings(raw.network),
    trustedDestinationHashes: unknownArray(
      raw.trustedDestinationHashes ?? raw.trusted_destination_hashes,
    ).filter((value): value is string => typeof value === "string"),
    preferredMapLayer: preferredMapLayer(raw.preferredMapLayer ?? raw.preferred_map_layer),
  };
}

export function toBlockOnboardingImportResult(
  raw: Record<string, unknown>,
): BlockOnboardingImportResult {
  return {
    importedPeerCount: finiteInteger(raw.importedPeerCount ?? raw.imported_peer_count, 0, 0),
    settingsUpdated: strictBoolean(raw.settingsUpdated ?? raw.settings_updated, false),
  };
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
const RNODE_FREQUENCY_MIN_HZ = 137_000_000;
const RNODE_FREQUENCY_MAX_HZ = 3_000_000_000;

export function rnodeRegionDefaultFrequencyHz(region: RnodeRegion): number {
  return RNODE_REGION_DEFAULT_FREQUENCY_HZ[region] ?? 915_000_000;
}

function normalizeRnodeRegion(value: unknown): RnodeRegion {
  const normalized = stringValue(value).trim().toUpperCase();
  switch (normalized) {
    case "":
    case "US915":
      return "US915";
    case "EU868":
    case "AU915":
    case "AS923":
    case "IN865":
    case "KR920":
    case "RU864":
      return normalized;
    default:
      throw new TypeError(`Unsupported RNode LoRa region: ${stringValue(value)}`);
  }
}

function normalizeRnodeFrequencyHz(value: unknown, region: RnodeRegion): number {
  const frequencyHz = Number(value);
  if (
    Number.isFinite(frequencyHz)
    && frequencyHz >= RNODE_FREQUENCY_MIN_HZ
    && frequencyHz <= RNODE_FREQUENCY_MAX_HZ
  ) {
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
    case "":
      return "REM-LF-RURAL-v1";
    default:
      throw new TypeError(`Unsupported RNode LoRa profile: ${stringValue(value)}`);
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
  const community = asRecord(current.community) ?? {};
  const power = asRecord(current.power) ?? {};
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
    community: {
      householdId: stringValue(community.householdId ?? community.household_id).trim().toLowerCase(),
      householdName: stringValue(community.householdName ?? community.household_name).trim().slice(0, 64),
      adults: finiteInteger(community.adults, 0, 0, 20),
      children: finiteInteger(community.children, 0, 0, 20),
      pets: finiteInteger(community.pets, 0, 0, 20),
      roleBadges: unknownArray(community.roleBadges ?? community.role_badges)
        .map((value) => stringValue(value).trim().slice(0, 24))
        .filter(Boolean)
        .filter((value, index, all) => all.indexOf(value) === index)
        .slice(0, 5),
      status: householdStatus(community.status),
      preferredMapLayer: preferredMapLayer(
        community.preferredMapLayer ?? community.preferred_map_layer,
      ),
    },
    power: {
      enabled: strictBoolean(power.enabled, true),
      thresholdPercent: powerThreshold(power.thresholdPercent ?? power.threshold_percent),
    },
  };
}
