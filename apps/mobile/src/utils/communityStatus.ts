import type { CommunityStatusProjectionRecord, HouseholdStatus } from "@reticulum/node-client";

export const COMMUNITY_STATUS_OPTIONS: Array<{ value: HouseholdStatus; label: string }> = [
  { value: "all_home", label: "All Home" },
  { value: "one_missing", label: "Someone Missing" },
  { value: "evacuated", label: "Evacuated" },
  { value: "needs_help", label: "Needs Help" },
];

export type CommunityStatusProjection = CommunityStatusProjectionRecord;

export function statusLabel(status: HouseholdStatus): string {
  return COMMUNITY_STATUS_OPTIONS.find((option) => option.value === status)?.label ?? "All Home";
}

export function householdComposition(
  community: Pick<CommunityStatusProjection, "adults" | "children" | "pets">,
): string {
  const people = community.adults + community.children;
  return `${people} ${people === 1 ? "person" : "people"} · ${community.pets} ${community.pets === 1 ? "pet" : "pets"}`;
}

export function freshnessLabel(updatedAtMs: number, now = Date.now()): string {
  if (!Number.isFinite(updatedAtMs) || updatedAtMs <= 0) return "not published yet";
  const minutes = Math.floor(Math.max(0, now - updatedAtMs) / 60_000);
  if (minutes < 1) return "updated now";
  if (minutes < 60) return `updated ${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `updated ${hours}h ago`;
  return `stale · updated ${Math.floor(hours / 24)}d ago`;
}

/** "Shared 30m ago" style line for the dashboard; "Not shared yet" before the first beacon. */
export function sharedAgoLabel(updatedAtMs: number, now = Date.now()): string {
  if (!Number.isFinite(updatedAtMs) || updatedAtMs <= 0) return "Not shared yet";
  const minutes = Math.floor(Math.max(0, now - updatedAtMs) / 60_000);
  if (minutes < 1) return "Shared just now";
  if (minutes < 60) return `Shared ${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `Shared ${hours}h ago` : `Shared ${Math.floor(hours / 24)}d ago`;
}

export function householdIdFromDestination(destination: string): string {
  const normalized = destination.trim().toLowerCase();
  return /^[0-9a-f]{16,}$/.test(normalized) ? normalized.slice(0, 16) : "";
}

/**
 * Household ID a form sends to the runtime: the operator's own entry when present,
 * otherwise the node's app destination prefix, so a profile with only a name
 * typed is not rejected for a missing ID.
 */
export function resolveHouseholdId(typed: string, appDestinationHex: string): string {
  return typed.trim().toLowerCase() || householdIdFromDestination(appDestinationHex);
}

const COMMUNITY_EVENT_UID_PREFIX = "rem-community-status-v1:";

/** Source identity of a household status event, or "" when the event is something else. */
export function communityEventSourceIdentity(eventUid: string): string {
  return eventUid.startsWith(COMMUNITY_EVENT_UID_PREFIX)
    ? eventUid.slice(COMMUNITY_EVENT_UID_PREFIX.length).trim().toLowerCase()
    : "";
}

/** Readable lines for a household status event, built from the runtime's decoded record. */
export function communityEventDetails(
  community: Pick<CommunityStatusProjection, "householdName" | "adults" | "children" | "pets" | "roleBadges" | "saverActive">,
): string[] {
  return [
    community.householdName.trim(),
    householdComposition(community),
    ...community.roleBadges,
    community.saverActive ? "Power saver on" : "",
  ].filter((entry) => entry.trim().length > 0);
}

/** Splits the runtime's household beacons into this node's own beacon and the other households. */
export function splitOwnCommunityRecord<T extends Pick<CommunityStatusProjection, "sourceIdentity">>(
  records: readonly T[],
  ownIdentityHex: string,
): { own: T | undefined; others: T[] } {
  const own = ownIdentityHex.trim().toLowerCase();
  const isOwn = (record: T) => own.length > 0 && record.sourceIdentity.trim().toLowerCase() === own;
  return { own: records.find(isOwn), others: records.filter((record) => !isOwn(record)) };
}
