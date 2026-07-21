import {
  CANONICAL_TEAM_UIDS,
  YELLOW_TEAM_UID,
  type HubDirectoryPeerRecord,
  type HubDirectoryUpdatedEvent,
} from "./contracts";
import { normalizeHex } from "./runtime-converters";

function records(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value)
    ? value.filter((item): item is Record<string, unknown> => (
        Boolean(item) && typeof item === "object" && !Array.isArray(item)
      ))
    : [];
}

function peerRecord(item: Record<string, unknown>): HubDirectoryPeerRecord {
  return {
    identity: normalizeHex(item.identity ?? ""),
    destinationHash: normalizeHex(item.destinationHash ?? item.destination_hash ?? ""),
    displayName: typeof item.displayName === "string"
      ? item.displayName
      : typeof item.display_name === "string" ? item.display_name : undefined,
    announceCapabilities: Array.isArray(item.announceCapabilities)
      ? item.announceCapabilities.map(String)
      : Array.isArray(item.announce_capabilities) ? item.announce_capabilities.map(String) : [],
    clientType: typeof item.clientType === "string"
      ? item.clientType
      : typeof item.client_type === "string" ? item.client_type : undefined,
    registeredMode: typeof item.registeredMode === "string"
      ? item.registeredMode
      : typeof item.registered_mode === "string" ? item.registered_mode : undefined,
    lastSeen: typeof item.lastSeen === "string"
      ? item.lastSeen
      : typeof item.last_seen === "string" ? item.last_seen : undefined,
    status: typeof item.status === "string" ? item.status : undefined,
  };
}

export function toHubDirectoryUpdatedEvent(
  raw: Record<string, unknown>,
): HubDirectoryUpdatedEvent {
  const snapshot = raw.snapshot && typeof raw.snapshot === "object" && !Array.isArray(raw.snapshot)
    ? raw.snapshot as Record<string, unknown>
    : raw;
  const items = records(snapshot.items).map(peerRecord)
    .filter((item) => item.destinationHash.length > 0);
  const schemaVersion = Number(snapshot.schemaVersion ?? snapshot.schema_version ?? 0);
  const teams = records(snapshot.teams).map((team) => ({
    uid: String(team.uid ?? team.team_uid ?? "").trim().toLowerCase(),
    color: String(team.color ?? "").trim().toUpperCase(),
    teamName: String(team.teamName ?? team.team_name ?? team.color ?? "").trim(),
  })).filter((team) => CANONICAL_TEAM_UIDS.has(team.uid) && team.color.length > 0);
  if (!teams.some((team) => team.uid === YELLOW_TEAM_UID)) {
    teams.unshift({ uid: YELLOW_TEAM_UID, color: "YELLOW", teamName: "YELLOW" });
  }
  teams.sort((left, right) => Number(right.uid === YELLOW_TEAM_UID)
    - Number(left.uid === YELLOW_TEAM_UID) || left.color.localeCompare(right.color));
  const callerMemberships = records(snapshot.callerMemberships ?? snapshot.caller_memberships)
    .map((membership) => ({
      teamUid: String(membership.teamUid ?? membership.team_uid ?? "").trim().toLowerCase(),
      teamMemberUid: String(membership.teamMemberUid ?? membership.team_member_uid ?? "").trim(),
    }))
    .filter((membership) => CANONICAL_TEAM_UIDS.has(membership.teamUid)
      && membership.teamMemberUid.length > 0);
  let members = records(snapshot.members).map((member) => ({
    ...peerRecord(member),
    teamUid: String(member.teamUid ?? member.team_uid ?? "").trim().toLowerCase(),
    teamMemberUid: String(member.teamMemberUid ?? member.team_member_uid ?? "").trim(),
  })).filter((member) => CANONICAL_TEAM_UIDS.has(member.teamUid)
    && member.teamMemberUid.length > 0 && member.destinationHash.length > 0);
  if (schemaVersion < 2 && members.length === 0) {
    members = items.map((item) => ({
      ...item,
      teamUid: YELLOW_TEAM_UID,
      teamMemberUid: item.identity,
    }));
  }
  const localTeams = records(snapshot.localTeams ?? snapshot.local_teams).map((team) => {
    const destinations = team.memberDestinations ?? team.member_destinations;
    return {
      teamUid: String(team.teamUid ?? team.team_uid ?? "").trim().toLowerCase(),
      memberDestinations: Array.isArray(destinations)
        ? destinations.map((destination) => normalizeHex(destination)).filter(Boolean)
        : [],
    };
  }).filter((team) => CANONICAL_TEAM_UIDS.has(team.teamUid));
  const requestedActiveTeamUid = String(
    snapshot.activeTeamUid ?? snapshot.active_team_uid ?? YELLOW_TEAM_UID,
  ).trim().toLowerCase();
  return {
    schemaVersion,
    hubIdentityHash: normalizeHex(
      snapshot.hubIdentityHash ?? snapshot.hub_identity_hash ?? "",
    ) || undefined,
    activeTeamUid: CANONICAL_TEAM_UIDS.has(requestedActiveTeamUid)
      ? requestedActiveTeamUid
      : YELLOW_TEAM_UID,
    effectiveConnectedMode: Boolean(
      snapshot.effectiveConnectedMode ?? snapshot.effective_connected_mode,
    ),
    teams,
    callerMemberships,
    members,
    localTeams,
    items,
    receivedAtMs: Number(snapshot.receivedAtMs ?? snapshot.received_at_ms ?? Date.now()),
  };
}
