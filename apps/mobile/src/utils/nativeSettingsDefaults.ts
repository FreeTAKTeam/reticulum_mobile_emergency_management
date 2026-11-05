import type { AppSettingsRecord } from "@reticulum/node-client";

type HouseholdFields = Pick<AppSettingsRecord["community"], "householdId" | "householdName">;
type TeamFields = Pick<AppSettingsRecord["teams"], "localTeams" | "localTeamsInitialized">;

/**
 * The native runtime owns the household defaults: a blank name becomes
 * "Household <call sign>" and a blank ID becomes the node's destination prefix once
 * the identity is known. Save verification compares the requested record against
 * the persisted one, so the blanks the runtime is allowed to fill are taken from
 * the persisted record before comparing. An empty, uninitialized team request
 * similarly accepts the runtime's first-read migration; configured teams stay strict.
 */
export function withNativeSettingsDefaults<T extends { community: HouseholdFields; teams: TeamFields }>(
  requested: T,
  persisted: { community: HouseholdFields; teams: TeamFields },
): T {
  const initializedTeams = !requested.teams.localTeamsInitialized
    && requested.teams.localTeams.length === 0
    && persisted.teams.localTeamsInitialized;
  return {
    ...requested,
    teams: initializedTeams ? {
      ...requested.teams,
      localTeams: persisted.teams.localTeams,
      localTeamsInitialized: persisted.teams.localTeamsInitialized,
    } : requested.teams,
    community: {
      ...requested.community,
      householdId: requested.community.householdId.trim() || persisted.community.householdId,
      householdName: requested.community.householdName.trim() || persisted.community.householdName,
    },
  };
}
