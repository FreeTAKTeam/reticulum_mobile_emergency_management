interface HouseholdFields {
  householdId: string;
  householdName: string;
}

/**
 * The native runtime owns the household defaults: a blank name becomes
 * "Household <call sign>" and a blank ID becomes the node's destination prefix once
 * the identity is known. Save verification compares the requested record against
 * the persisted one, so the blanks the runtime is allowed to fill are taken from
 * the persisted record before comparing.
 */
export function withNativeHouseholdDefaults<T extends { community: HouseholdFields }>(
  requested: T,
  persisted: { community: HouseholdFields },
): T {
  return {
    ...requested,
    community: {
      ...requested.community,
      householdId: requested.community.householdId.trim() || persisted.community.householdId,
      householdName: requested.community.householdName.trim() || persisted.community.householdName,
    },
  };
}
