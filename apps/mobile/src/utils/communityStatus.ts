import type { CommunityStatusProjectionRecord, HouseholdStatus } from "@reticulum/node-client";

export const COMMUNITY_STATUS_OPTIONS: Array<{ value: HouseholdStatus; label: string }> = [
  { value: "all_home", label: "All Home" },
  { value: "one_missing", label: "1 Missing" },
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
  const minutes = Math.floor(Math.max(0, now - updatedAtMs) / 60_000);
  if (minutes < 1) return "updated now";
  if (minutes < 60) return `updated ${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `updated ${hours}h ago`;
  return `stale · updated ${Math.floor(hours / 24)}d ago`;
}

export function householdIdFromDestination(destination: string): string {
  const normalized = destination.trim().toLowerCase();
  return /^[0-9a-f]{16,}$/.test(normalized) ? normalized.slice(0, 16) : "";
}
