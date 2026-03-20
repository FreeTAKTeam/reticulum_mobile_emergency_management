import type { PacketReceivedEvent } from "@reticulum/node-client";

import {
  buildMissionCommandFieldsBase64,
  createMissionCommandEnvelope,
  parseMissionSyncFields,
  type MissionCommandEnvelope,
  type MissionResultPayload,
  type MissionResponsePayload,
} from "../utils/missionSync";
import {
  DEFAULT_R3AKT_TEAM_COLOR,
  formatR3aktTeamColorLabel,
  normalizeR3aktTeamColor,
  type R3aktTeamColor,
} from "../utils/r3akt";

const HUB_REGISTRY_LINKAGE_STORAGE_KEY = "reticulum.mobile.hubRegistryLinkage.v1";
const MESSAGE_STORAGE_KEY = "reticulum.mobile.messages.v1";
const HUB_BOOTSTRAP_TIMEOUT_MS = 35_000;

export type HubRegistrationStatus = "disabled" | "pending" | "ready" | "error";

export interface HubRegistryLinkage {
  teamUid: string;
  teamMemberUid: string;
  callsign: string;
  teamColor: R3aktTeamColor;
  localIdentityHex: string;
  hubIdentityHash: string;
  updatedAt: number;
}

export interface HubRegistryBootstrapProfile {
  callsign: string;
  teamColor: R3aktTeamColor;
  localIdentityHex: string;
  hubIdentityHash: string;
}

export interface HubRegistryCommandTransport {
  sendCommand(destinationHex: string, command: MissionCommandEnvelope): Promise<void>;
  onPacket(listener: (event: PacketReceivedEvent) => void): () => void;
}

interface JsonRecord {
  [key: string]: unknown;
}

interface StoredMessageRecord extends JsonRecord {
  updatedAt?: unknown;
  deletedAt?: unknown;
  groupName?: unknown;
}

interface TeamRecord extends JsonRecord {
  uid: string;
  team_name: string;
  color?: string;
  mission_uid?: string;
}

interface TeamMemberRecord extends JsonRecord {
  uid: string;
  team_uid?: string;
  rns_identity: string;
  display_name: string;
  callsign?: string;
}

function nowMs(): number {
  return Date.now();
}

function normalizeHex(value: unknown): string {
  return String(value ?? "").trim().toLowerCase();
}

function asRecord(value: unknown): JsonRecord | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as JsonRecord;
}

function asText(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim();
  return normalized || undefined;
}

function createTrackingId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now().toString(36)}-${Math.floor(Math.random() * 1_000_000).toString(36)}`;
}

function loadJsonStorage<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) {
      return null;
    }
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

function saveJsonStorage(key: string, value: unknown): void {
  localStorage.setItem(key, JSON.stringify(value));
}

function loadPreferredTeamColor(): R3aktTeamColor {
  try {
    const parsed = loadJsonStorage<StoredMessageRecord[]>(MESSAGE_STORAGE_KEY);
    if (!Array.isArray(parsed)) {
      return DEFAULT_R3AKT_TEAM_COLOR;
    }

    let latestColor = DEFAULT_R3AKT_TEAM_COLOR;
    let latestUpdatedAt = -1;
    for (const message of parsed) {
      const deletedAt = Number(message.deletedAt ?? 0);
      if (Number.isFinite(deletedAt) && deletedAt > 0) {
        continue;
      }
      const updatedAt = Number(message.updatedAt ?? 0);
      if (!Number.isFinite(updatedAt) || updatedAt < latestUpdatedAt) {
        continue;
      }
      const candidate = normalizeR3aktTeamColor(message.groupName, latestColor);
      latestColor = candidate;
      latestUpdatedAt = updatedAt;
    }
    return latestColor;
  } catch {
    return DEFAULT_R3AKT_TEAM_COLOR;
  }
}

function normalizeLinkage(value: unknown): HubRegistryLinkage | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const teamUid = asText(record.teamUid);
  const teamMemberUid = asText(record.teamMemberUid);
  const callsign = asText(record.callsign);
  const localIdentityHex = normalizeHex(record.localIdentityHex);
  const hubIdentityHash = normalizeHex(record.hubIdentityHash);
  const updatedAt = Number(record.updatedAt ?? 0);
  const teamColor = normalizeR3aktTeamColor(record.teamColor, DEFAULT_R3AKT_TEAM_COLOR);

  if (!teamUid || !teamMemberUid || !callsign || !localIdentityHex || !hubIdentityHash) {
    return null;
  }

  return {
    teamUid,
    teamMemberUid,
    callsign,
    teamColor,
    localIdentityHex,
    hubIdentityHash,
    updatedAt: Number.isFinite(updatedAt) ? updatedAt : nowMs(),
  };
}

export function loadHubRegistryLinkage(): HubRegistryLinkage | null {
  return normalizeLinkage(loadJsonStorage<unknown>(HUB_REGISTRY_LINKAGE_STORAGE_KEY));
}

export function saveHubRegistryLinkage(linkage: HubRegistryLinkage): void {
  saveJsonStorage(HUB_REGISTRY_LINKAGE_STORAGE_KEY, linkage);
}

export function clearHubRegistryLinkage(): void {
  localStorage.removeItem(HUB_REGISTRY_LINKAGE_STORAGE_KEY);
}

export function buildHubRegistryBootstrapProfile(options: {
  callsign: string;
  localIdentityHex: string;
  hubIdentityHash: string;
}): HubRegistryBootstrapProfile | null {
  const callsign = options.callsign.trim();
  const localIdentityHex = normalizeHex(options.localIdentityHex);
  const hubIdentityHash = normalizeHex(options.hubIdentityHash);
  if (!callsign || !localIdentityHex || !hubIdentityHash) {
    return null;
  }

  return {
    callsign,
    teamColor: loadPreferredTeamColor(),
    localIdentityHex,
    hubIdentityHash,
  };
}

export function buildHubRegistryTeamName(teamColor: R3aktTeamColor): string {
  return `${formatR3aktTeamColorLabel(teamColor)} Team`;
}

export function buildHubRegistryTeamDescription(profile: HubRegistryBootstrapProfile): string {
  return `Auto-linked from ${profile.callsign}`;
}

export function matchesHubRegistryProfile(
  linkage: HubRegistryLinkage,
  profile: HubRegistryBootstrapProfile,
): boolean {
  return normalizeHex(linkage.localIdentityHex) === normalizeHex(profile.localIdentityHex)
    && normalizeHex(linkage.hubIdentityHash) === normalizeHex(profile.hubIdentityHash)
    && linkage.callsign.trim() === profile.callsign.trim()
    && linkage.teamColor === profile.teamColor;
}

function extractResultRecord(result: MissionResponsePayload | null, key: string): JsonRecord[] {
  if (!result || result.status !== "result") {
    return [];
  }
  const record = asRecord(result.result);
  const entries = Array.isArray(record?.[key]) ? record?.[key] as unknown[] : [];
  return entries.map((entry) => asRecord(entry)).filter((entry): entry is JsonRecord => Boolean(entry));
}

function normalizeTeamRecord(value: JsonRecord): TeamRecord | null {
  const uid = asText(value.uid);
  const teamName = asText(value.team_name);
  if (!uid || !teamName) {
    return null;
  }
  return {
    ...value,
    uid,
    team_name: teamName,
    color: asText(value.color),
  };
}

function normalizeTeamMemberRecord(value: JsonRecord): TeamMemberRecord | null {
  const uid = asText(value.uid);
  const teamUid = asText(value.team_uid);
  const rnsIdentity = asText(value.rns_identity);
  const displayName = asText(value.display_name);
  if (!uid || !rnsIdentity || !displayName) {
    return null;
  }
  return {
    ...value,
    uid,
    team_uid: teamUid,
    rns_identity: rnsIdentity,
    display_name: displayName,
    callsign: asText(value.callsign),
  };
}

function missionResponseMatches(
  expected: {
    commandId: string;
    correlationId: string;
    sourceHex: string;
  },
  result: MissionResponsePayload | null,
): result is MissionResponsePayload {
  if (!result) {
    return false;
  }
  const sourceMatches = true;
  const responseMatches =
    result.command_id === expected.commandId || result.correlation_id === expected.correlationId;
  return sourceMatches && responseMatches;
}

function ensureMissionResult(response: MissionResponsePayload): MissionResultPayload {
  if (response.status !== "result") {
    throw new Error(`Unexpected mission response status: ${response.status}`);
  }
  return response;
}

async function waitForMissionResponse(
  transport: HubRegistryCommandTransport,
  expected: {
    commandId: string;
    correlationId: string;
    sourceHex: string;
  },
  sendCommand: () => Promise<void>,
  timeoutMs = HUB_BOOTSTRAP_TIMEOUT_MS,
): Promise<MissionResponsePayload> {
  return await new Promise<MissionResponsePayload>((resolve, reject) => {
    let settled = false;
    let timeoutId: number | null = null;
    let unsubscribe: () => void = () => undefined;

    const finish = (action: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      if (timeoutId !== null) {
        clearTimeout(timeoutId);
      }
      unsubscribe();
      action();
    };

    unsubscribe = transport.onPacket((packet: PacketReceivedEvent) => {
      const sourceHex = normalizeHex(packet.sourceHex ?? "");
      if (sourceHex && sourceHex !== expected.sourceHex) {
        return;
      }

      const missionSync = parseMissionSyncFields(packet.fieldsBase64);
      if (!missionSync?.result || !missionResponseMatches(expected, missionSync.result)) {
        return;
      }
      const result = missionSync.result;

      if (result.status === "accepted") {
        return;
      }

      if (result.status === "rejected") {
        const rejection = result;
        finish(() => reject(new Error(rejection.reason?.trim() || rejection.reason_code)));
        return;
      }

      finish(() => resolve(result));
    });

    timeoutId = window.setTimeout(() => {
      finish(() => reject(new Error("Timed out waiting for hub registry response.")));
    }, timeoutMs);

    void sendCommand().catch((error: unknown) => {
      finish(() => reject(error instanceof Error ? error : new Error(String(error))));
    });
  });
}

async function sendCommandAndWaitForResult(
  profile: HubRegistryBootstrapProfile,
  transport: HubRegistryCommandTransport,
  commandType: string,
  args: Record<string, unknown>,
  suffix: string,
): Promise<MissionResultPayload> {
  const commandId = createTrackingId(`${suffix}-command`);
  const correlationId = createTrackingId(`${suffix}-correlation`);
  const command = createMissionCommandEnvelope({
    commandId,
    sourceIdentity: profile.localIdentityHex,
    sourceDisplayName: profile.callsign,
    commandType,
    args,
    correlationId,
    topics: ["r3akt", "registry"],
  });

  return ensureMissionResult(await waitForMissionResponse(
    transport,
    {
      commandId,
      correlationId,
      sourceHex: profile.hubIdentityHash,
    },
    async () => {
      await transport.sendCommand(profile.hubIdentityHash, command);
    },
  ));
}

function teamMatchesProfile(team: TeamRecord, profile: HubRegistryBootstrapProfile): boolean {
  const teamName = buildHubRegistryTeamName(profile.teamColor).trim().toLowerCase();
  const recordName = asText(team.team_name)?.toLowerCase() ?? "";
  const recordColor = asText(team.color)?.toLowerCase() ?? "";
  return recordName === teamName || recordColor === profile.teamColor.toLowerCase();
}

function memberMatchesProfile(member: TeamMemberRecord, profile: HubRegistryBootstrapProfile): boolean {
  const recordIdentity = asText(member.rns_identity)?.trim().toLowerCase() ?? "";
  const recordCallsign = asText(member.callsign)?.trim().toLowerCase() ?? "";
  return recordIdentity === normalizeHex(profile.localIdentityHex)
    || recordCallsign === profile.callsign.trim().toLowerCase();
}

export async function bootstrapHubRegistry(
  profile: HubRegistryBootstrapProfile,
  transport: HubRegistryCommandTransport,
): Promise<HubRegistryLinkage> {
  const teamListResponse = await sendCommandAndWaitForResult(
    profile,
    transport,
    "mission.registry.team.list",
    {},
    "hub-team-list",
  );
  const teamRecords = extractResultRecord(teamListResponse, "teams")
    .map((entry) => normalizeTeamRecord(entry))
    .filter((entry): entry is TeamRecord => Boolean(entry));

  let team = teamRecords.find((entry) => teamMatchesProfile(entry, profile));
  if (!team) {
    const teamResponse = await sendCommandAndWaitForResult(
      profile,
      transport,
      "mission.registry.team.upsert",
      {
        team_name: buildHubRegistryTeamName(profile.teamColor),
        team_description: buildHubRegistryTeamDescription(profile),
        color: profile.teamColor,
      },
      "hub-team-upsert",
    );
    team = normalizeTeamRecord(asRecord(teamResponse.result) ?? {}) ?? undefined;
    if (!team) {
      throw new Error("Hub team bootstrap did not return a valid team record.");
    }
  }

  const memberListResponse = await sendCommandAndWaitForResult(
    profile,
    transport,
    "mission.registry.team_member.list",
    {
      team_uid: team.uid,
    },
    "hub-member-list",
  );
  const memberRecords = extractResultRecord(memberListResponse, "team_members")
    .map((entry) => normalizeTeamMemberRecord(entry))
    .filter((entry): entry is TeamMemberRecord => Boolean(entry));

  let member = memberRecords.find((entry) => memberMatchesProfile(entry, profile));
  if (!member) {
    const memberResponse = await sendCommandAndWaitForResult(
      profile,
      transport,
      "mission.registry.team_member.upsert",
      {
        team_uid: team.uid,
        rns_identity: profile.localIdentityHex,
        display_name: profile.callsign,
        callsign: profile.callsign,
      },
      "hub-member-upsert",
    );
    member = normalizeTeamMemberRecord(asRecord(memberResponse.result) ?? {}) ?? undefined;
    if (!member) {
      throw new Error("Hub team-member bootstrap did not return a valid member record.");
    }
  } else if (normalizeHex(member.team_uid) !== normalizeHex(team.uid)) {
    const memberResponse = await sendCommandAndWaitForResult(
      profile,
      transport,
      "mission.registry.team_member.upsert",
      {
        uid: member.uid,
        team_uid: team.uid,
        rns_identity: profile.localIdentityHex,
        display_name: member.display_name,
        callsign: member.callsign ?? profile.callsign,
      },
      "hub-member-relocate",
    );
    member = normalizeTeamMemberRecord(asRecord(memberResponse.result) ?? member) ?? undefined;
    if (!member) {
      throw new Error("Hub team-member relocation did not return a valid member record.");
    }
  }

  await sendCommandAndWaitForResult(
    profile,
    transport,
    "mission.registry.team_member.client.link",
    {
      team_member_uid: member.uid,
      client_identity: profile.localIdentityHex,
    },
    "hub-member-link",
  );

  return {
    teamUid: team.uid,
    teamMemberUid: member.uid,
    callsign: profile.callsign,
    teamColor: profile.teamColor,
    localIdentityHex: profile.localIdentityHex,
    hubIdentityHash: profile.hubIdentityHash,
    updatedAt: nowMs(),
  };
}
