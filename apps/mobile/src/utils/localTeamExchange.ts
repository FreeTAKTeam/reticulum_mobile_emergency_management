import { CANONICAL_TEAM_UIDS } from "@reticulum/node-client";

export interface LocalTeamExchangeMember {
  destination: string;
  label?: string;
}

export interface LocalTeamExchangeRecord {
  teamUid: string;
  members: LocalTeamExchangeMember[];
}

export const MAX_LOCAL_TEAM_QR_MEMBERS = 40;

function objectValue(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

export function encodeLocalTeamExchange(
  teamUid: string,
  color: string,
  members: LocalTeamExchangeMember[],
): string {
  return JSON.stringify({
    schemaVersion: 1,
    type: "rem.local-team",
    team: {
      uid: teamUid,
      color,
      members: members.map(({ destination, label }) => ({
        destination: destination.toLowerCase(),
        ...(label?.trim() ? { label: label.trim().slice(0, 80) } : {}),
      })),
    },
  }, null, 2);
}

export function encodeLocalTeamQrExchange(
  teamUid: string,
  memberDestinations: string[],
): string {
  if (!CANONICAL_TEAM_UIDS.has(teamUid)) {
    throw new Error("Only canonical color teams can be exported as QR codes.");
  }
  if (memberDestinations.length > MAX_LOCAL_TEAM_QR_MEMBERS) {
    throw new Error(
      `One QR code supports at most ${MAX_LOCAL_TEAM_QR_MEMBERS} team members; use JSON export instead.`,
    );
  }
  const members = memberDestinations.map((destination) => {
    const normalized = destination.trim().toLowerCase();
    if (!/^[0-9a-f]{32}$/.test(normalized)) {
      throw new Error("The local team contains an invalid REM destination.");
    }
    return { destination: normalized };
  });
  return JSON.stringify({
    schemaVersion: 1,
    type: "rem.local-team",
    team: { uid: teamUid, members },
  });
}

export function parseLocalTeamExchange(text: string): LocalTeamExchangeRecord {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error: unknown) {
    throw new Error("Team data is not valid JSON.", { cause: error });
  }
  const envelope = objectValue(parsed);
  const team = objectValue(envelope?.team);
  const teamUid = typeof team?.uid === "string" ? team.uid.trim().toLowerCase() : "";
  if (envelope?.schemaVersion !== 1 || envelope.type !== "rem.local-team" || !team) {
    throw new Error("Team data is not a supported REM local-team export.");
  }
  if (!CANONICAL_TEAM_UIDS.has(teamUid)) {
    throw new Error("Team data uses an unsupported color team.");
  }
  if (!Array.isArray(team.members)) {
    throw new Error("Team data does not contain a member list.");
  }
  if (team.members.length > 256) {
    throw new Error("Team data contains too many members.");
  }
  const members = team.members.map((value) => {
    const member = objectValue(value);
    const destination = typeof member?.destination === "string"
      ? member.destination.trim().toLowerCase()
      : "";
    if (!/^[0-9a-f]{32}$/.test(destination)) {
      throw new Error("Team data contains an invalid REM destination.");
    }
    const label = typeof member?.label === "string" ? member.label.trim().slice(0, 80) : "";
    return { destination, ...(label ? { label } : {}) };
  });
  return {
    teamUid,
    members: members.filter((member, index, all) => (
      all.findIndex(({ destination }) => destination === member.destination) === index
    )),
  };
}
